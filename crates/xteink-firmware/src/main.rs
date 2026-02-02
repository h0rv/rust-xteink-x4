extern crate alloc;

mod sdcard;

use alloc::vec::Vec;
use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{Input, PinDriver, Pull},
    peripherals::Peripherals,
    spi::{config::Config, SpiDeviceDriver, SpiDriver, SpiDriverConfig},
};
use esp_idf_svc::sys;

use xteink_ui::{
    App, BufferedDisplay, Builder, Button, Dimensions, DisplayInterface, EinkDisplay,
    EinkInterface, InputEvent, RamXAddressing, RefreshMode, Region, Rotation, UpdateRegion,
};

use sdcard::SdCardFs;

#[allow(dead_code)]
const DISPLAY_COLS: u16 = 480;
#[allow(dead_code)]
const DISPLAY_ROWS: u16 = 800;

const ADC_NO_BUTTON: i32 = 3800;
const ADC_RANGES_1: [i32; 5] = [3800, 3100, 2090, 750, i32::MIN];
const ADC_RANGES_2: [i32; 3] = [3800, 1120, i32::MIN];
const ADC_WIDTH_BIT_12: u32 = 3;
const ADC_ATTEN_DB_11: u32 = 3;
const POWER_LONG_PRESS_MS: u32 = 2000;

fn init_adc() {
    unsafe {
        sys::adc1_config_width(ADC_WIDTH_BIT_12);
        sys::adc1_config_channel_atten(sys::adc_channel_t_ADC_CHANNEL_1, ADC_ATTEN_DB_11);
        sys::adc1_config_channel_atten(sys::adc_channel_t_ADC_CHANNEL_2, ADC_ATTEN_DB_11);
    }
}

fn read_adc(channel: sys::adc_channel_t) -> i32 {
    unsafe { sys::adc1_get_raw(channel) as i32 }
}

fn get_button_from_adc(adc_value: i32, ranges: &[i32], num_buttons: usize) -> i32 {
    for i in 0..num_buttons {
        if ranges[i + 1] < adc_value && adc_value <= ranges[i] {
            return i as i32;
        }
    }
    -1
}

fn read_buttons(
    power_btn: &mut PinDriver<esp_idf_svc::hal::gpio::Gpio3, Input>,
    debug_mode: bool,
) -> (Option<Button>, bool) {
    let power_pressed = power_btn.is_low();
    if power_pressed {
        return (Some(Button::Power), true);
    }

    let adc1_value = read_adc(sys::adc_channel_t_ADC_CHANNEL_1);
    let adc2_value = read_adc(sys::adc_channel_t_ADC_CHANNEL_2);

    if debug_mode && (adc1_value < ADC_NO_BUTTON || adc2_value < ADC_NO_BUTTON) {
        log::info!("ADC1: {}, ADC2: {}", adc1_value, adc2_value);
    }

    let btn1 = get_button_from_adc(adc1_value, &ADC_RANGES_1, 4);
    if btn1 >= 0 {
        return (
            Some(match btn1 {
                0 => Button::Back,
                1 => Button::Confirm,
                2 => Button::Left,
                3 => Button::Right,
                _ => unreachable!(),
            }),
            false,
        );
    }

    let btn2 = get_button_from_adc(adc2_value, &ADC_RANGES_2, 2);
    if btn2 >= 0 {
        return (
            Some(match btn2 {
                0 => Button::VolumeUp,
                1 => Button::VolumeDown,
                _ => unreachable!(),
            }),
            false,
        );
    }

    (None, false)
}

fn update_display_diff<I, D>(
    display: &mut EinkDisplay<I>,
    delay: &mut D,
    current: &[u8],
    last: &mut Vec<u8>,
    scratch: &mut Vec<u8>,
    scratch_prev: &mut Vec<u8>,
    width_bytes: usize,
    height: usize,
) where
    I: DisplayInterface,
    D: embedded_hal::delay::DelayNs,
{
    let expected_len = width_bytes * height;
    if current.len() != expected_len {
        log::warn!(
            "Buffer size mismatch: got {}, expected {} ({}x{} bytes)",
            current.len(),
            expected_len,
            width_bytes,
            height
        );
    }
    if last.len() != current.len() {
        last.resize(current.len(), 0xFF);
    }

    let mut min_x = usize::MAX;
    let mut max_x = 0usize;
    let mut min_y = usize::MAX;
    let mut max_y = 0usize;
    let mut changed = 0usize;

    for (i, (&new_b, &old_b)) in current.iter().zip(last.iter()).enumerate() {
        if new_b != old_b {
            changed += 1;
            let y = i / width_bytes;
            let x = i % width_bytes;
            if x < min_x {
                min_x = x;
            }
            if x > max_x {
                max_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if y > max_y {
                max_y = y;
            }
        }
    }

    if changed == 0 {
        log::info!("UI: no pixel changes detected");
        return;
    }

    log::info!("UI: changed bytes: {}", changed);

    // TEMP: force full refresh to validate buffer updates
    if display
        .update_with_mode(current, &[], RefreshMode::Full, delay)
        .is_err()
    {
        log::warn!("UI: full-screen refresh failed");
    }
    last.copy_from_slice(current);
    return;

    let total_bytes = current.len();
    let region_width_bytes = (max_x - min_x + 1).max(1);
    let region_height = (max_y - min_y + 1).max(1);
    let region_bytes = region_width_bytes * region_height;

    // If more than half the screen changed, do a full partial refresh
    if region_bytes > total_bytes / 2 {
        log::info!(
            "UI: large change ({} bytes). Using partial full-screen refresh",
            region_bytes
        );
        if display
            .update_with_mode(current, &[], RefreshMode::Partial, delay)
            .is_err()
        {
            log::warn!("UI: full-screen partial refresh failed");
        }
        last.copy_from_slice(current);
        return;
    }

    // Build a compact region buffer
    scratch.clear();
    scratch.resize(region_bytes, 0xFF);
    scratch_prev.clear();
    scratch_prev.resize(region_bytes, 0xFF);

    for row in 0..region_height {
        let src = (min_y + row) * width_bytes + min_x;
        let dst = row * region_width_bytes;
        scratch[dst..dst + region_width_bytes]
            .copy_from_slice(&current[src..src + region_width_bytes]);
        scratch_prev[dst..dst + region_width_bytes]
            .copy_from_slice(&last[src..src + region_width_bytes]);
    }

    let region = Region::new(
        (min_x * 8) as u16,
        min_y as u16,
        (region_width_bytes * 8) as u16,
        region_height as u16,
    );

    // TEMP: force full partial refresh to verify rendering path
    // If this fixes UI updates, the issue is in region math/driver partials
    if display
        .update_with_mode(current, &[], RefreshMode::Partial, delay)
        .is_err()
    {
        log::warn!("UI: full-screen partial refresh failed");
    }
    last.copy_from_slice(current);
    return;

    #[allow(unreachable_code)]
    let update = UpdateRegion {
        region,
        black_buffer: scratch,
        red_buffer: scratch_prev,
        mode: RefreshMode::Fast,
    };

    log::info!(
        "UI: region update x={} y={} w={} h={} bytes={}",
        region.x,
        region.y,
        region.w,
        region.h,
        region_bytes
    );

    if display.update_region(update, delay).is_err() {
        log::warn!("UI: region update failed - falling back to partial");
        if display
            .update_with_mode(current, &[], RefreshMode::Partial, delay)
            .is_err()
        {
            log::warn!("UI: fallback partial refresh failed");
        }
    }
    last.copy_from_slice(current);
}

fn enter_deep_sleep(power_btn_pin: i32) {
    log::info!("Entering deep sleep...");
    unsafe {
        sys::esp_deep_sleep_enable_gpio_wakeup(
            1u64 << power_btn_pin,
            sys::esp_deepsleep_gpio_wake_up_mode_t_ESP_GPIO_WAKEUP_GPIO_LOW,
        );
        sys::esp_deep_sleep_start();
    }
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    // Stack size verification (runtime log)
    // ESP32-C3 has ~191KB total RAM, so we use 64KB for stack
    const REQUIRED_STACK_SIZE: u32 = 60 * 1024; // 60KB minimum
    let configured_stack = esp_idf_svc::sys::CONFIG_ESP_MAIN_TASK_STACK_SIZE;
    if configured_stack < REQUIRED_STACK_SIZE {
        log::warn!(
            "Stack size too small: {} bytes (need >= {}). Check sdkconfig.defaults",
            configured_stack,
            REQUIRED_STACK_SIZE
        );
    }

    log::info!(
        "Starting firmware with {} bytes stack",
        esp_idf_svc::sys::CONFIG_ESP_MAIN_TASK_STACK_SIZE
    );

    let peripherals = Peripherals::take().unwrap();

    let spi = SpiDriver::new(
        peripherals.spi2,
        peripherals.pins.gpio8,
        peripherals.pins.gpio10,
        Some(peripherals.pins.gpio7),
        &SpiDriverConfig::default(),
    )
    .unwrap();

    let spi_config = Config::default()
        .baudrate(esp_idf_svc::hal::units::Hertz(40_000_000))
        .data_mode(embedded_hal::spi::Mode {
            polarity: embedded_hal::spi::Polarity::IdleLow,
            phase: embedded_hal::spi::Phase::CaptureOnFirstTransition,
        });

    let spi_device =
        SpiDeviceDriver::new(&spi, Some(peripherals.pins.gpio21), &spi_config).unwrap();

    // SD card SPI device (same bus, different CS)
    let sd_spi_config = Config::default()
        .baudrate(esp_idf_svc::hal::units::Hertz(20_000_000))
        .data_mode(embedded_hal::spi::Mode {
            polarity: embedded_hal::spi::Polarity::IdleLow,
            phase: embedded_hal::spi::Phase::CaptureOnFirstTransition,
        });
    let sd_spi = SpiDeviceDriver::new(&spi, Some(peripherals.pins.gpio12), &sd_spi_config).unwrap();

    let dc = PinDriver::output(peripherals.pins.gpio4).unwrap();
    let rst = PinDriver::output(peripherals.pins.gpio5).unwrap();
    let busy = PinDriver::input(peripherals.pins.gpio6).unwrap();

    let mut power_btn = PinDriver::input(peripherals.pins.gpio3).unwrap();
    power_btn.set_pull(Pull::Up).unwrap();

    init_adc();

    // Initialize display
    let mut delay = FreeRtos;
    let interface = EinkInterface::new(spi_device, dc, rst, busy);
    // Use 480x800 dimensions (rows x cols format for SSD1677 driver)
    // Rows must be <= 680 (gates), cols must be <= 960 (sources) and multiple of 8
    // Physical display is 800x480 but driver uses rows=gates, cols=sources
    let config = Builder::new()
        .dimensions(Dimensions::new(480, 800).unwrap())
        // Rotation handled in BufferedDisplay (portrait -> native transpose)
        .rotation(Rotation::Rotate0)
        .data_entry_mode(0x01) // X_INC_Y_DEC (matches C++ reference)
        .ram_x_addressing(RamXAddressing::Pixels) // Revert: bytes caused noise on this panel
        .ram_y_inverted(true) // Match panel wiring (C++ reverses Y)
        .build()
        .unwrap();
    let mut display = EinkDisplay::new(interface, config);

    log::info!("Resetting display...");
    display.reset(&mut delay).ok();

    // Create buffered display for UI rendering (avoids stack overflow from iterator chains)
    let mut buffered_display = BufferedDisplay::new();
    let mut last_buffer: Vec<u8> = vec![0xFF; buffered_display.buffer().len()];
    let mut region_scratch: Vec<u8> = Vec::new();
    let mut region_scratch_prev: Vec<u8> = Vec::new();

    // Initialize SD card filesystem
    let mut fs = SdCardFs::new(sd_spi).expect("SD card init failed");

    // Initialize app and render initial screen
    let mut app = App::new();
    app.init(&mut fs);
    buffered_display.clear();
    app.render(&mut buffered_display).ok();
    display
        .update(buffered_display.buffer(), &[], &mut delay)
        .ok();
    last_buffer.copy_from_slice(buffered_display.buffer());

    log::info!("Starting event loop... Press a button!");
    log::info!("Hold POWER for 2 seconds to sleep...");

    let mut last_button: Option<Button> = None;
    let mut power_press_counter: u32 = 0;
    let mut is_power_pressed: bool = false;
    let mut long_press_triggered: bool = false;
    const DEBUG_ADC: bool = false;
    const POWER_LONG_PRESS_ITERATIONS: u32 = POWER_LONG_PRESS_MS / 50;

    loop {
        let (button, power_pressed) = read_buttons(&mut power_btn, DEBUG_ADC);

        if power_pressed {
            if !is_power_pressed {
                power_press_counter = 0;
                is_power_pressed = true;
                long_press_triggered = false;
                log::info!("Power button pressed...");
            } else if !long_press_triggered {
                power_press_counter += 1;
                if power_press_counter >= POWER_LONG_PRESS_ITERATIONS {
                    log::info!("Power button held for 2s - powering off!");
                    long_press_triggered = true;

                    // Show "Powering off..." message
                    buffered_display.clear();
                    // TODO: Render "Powering off..." text here when we have text rendering
                    // For now, just clear to white
                    display
                        .update_with_mode(
                            buffered_display.buffer(),
                            &[],
                            RefreshMode::Full,
                            &mut delay,
                        )
                        .ok();

                    log::info!("Display cleared for power off");

                    while power_btn.is_low() {
                        FreeRtos::delay_ms(50);
                    }

                    enter_deep_sleep(3);
                }
            }
        } else {
            if is_power_pressed && !long_press_triggered {
                if last_button != Some(Button::Power) {
                    log::info!("Power button short press");
                    last_button = Some(Button::Power);

                    if app.handle_input(InputEvent::Press(Button::Power), &mut fs) {
                        log::info!("UI: redraw after power short press");
                        buffered_display.clear();
                        app.render(&mut buffered_display).ok();
                        update_display_diff(
                            &mut display,
                            &mut delay,
                            buffered_display.buffer(),
                            &mut last_buffer,
                            &mut region_scratch,
                            &mut region_scratch_prev,
                            buffered_display.width_bytes(),
                            buffered_display.height_pixels() as usize,
                        );
                    } else {
                        log::info!("UI: no redraw after power short press");
                    }
                }
            }
            is_power_pressed = false;
            power_press_counter = 0;
        }

        if let Some(btn) = button {
            if btn != Button::Power && last_button != Some(btn) {
                log::info!("Button pressed: {:?}", btn);
                last_button = Some(btn);

                if app.handle_input(InputEvent::Press(btn), &mut fs) {
                    log::info!("UI: redraw after {:?}", btn);
                    buffered_display.clear();
                    app.render(&mut buffered_display).ok();
                    update_display_diff(
                        &mut display,
                        &mut delay,
                        buffered_display.buffer(),
                        &mut last_buffer,
                        &mut region_scratch,
                        &mut region_scratch_prev,
                        buffered_display.width_bytes(),
                        buffered_display.height_pixels() as usize,
                    );
                } else {
                    log::info!("UI: no redraw after {:?}", btn);
                }
            }
        } else if !power_pressed {
            last_button = None;
        }

        FreeRtos::delay_ms(50);
    }
}
