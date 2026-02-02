extern crate alloc;

mod sdcard;

use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{Input, PinDriver, Pull},
    peripherals::Peripherals,
    spi::{config::Config, SpiDeviceDriver, SpiDriver, SpiDriverConfig},
};
use esp_idf_svc::sys;

use xteink_ui::{
    App, BufferedDisplay, Builder, Button, Dimensions, EinkDisplay, EinkInterface, InputEvent,
    RefreshMode, Rotation,
};

use sdcard::SdCardFs;

const DISPLAY_COLS: u16 = 480;
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

    // Compile-time stack size verification
    // This will fail to compile if the stack size is less than 500KB
    const REQUIRED_STACK_SIZE: u32 = 512 * 1024;
    const _: () = assert!(
        esp_idf_svc::sys::CONFIG_ESP_MAIN_TASK_STACK_SIZE >= REQUIRED_STACK_SIZE,
        "Stack size must be at least 512KB. Check sdkconfig.defaults and run `cargo clean` before building."
    );

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
    let config = Builder::new()
        .dimensions(Dimensions::new(DISPLAY_ROWS, DISPLAY_COLS).unwrap())
        .rotation(Rotation::Rotate90)
        .build()
        .unwrap();
    let mut display = EinkDisplay::new(interface, config);

    log::info!("Resetting display...");
    display.reset(&mut delay).ok();

    // Create buffered display for UI rendering (avoids stack overflow from iterator chains)
    let mut buffered_display = BufferedDisplay::new();

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

                    if app.handle_input(InputEvent::Press(Button::Power), &mut fs) {
                        buffered_display.clear();
                        app.render(&mut buffered_display).ok();
                        display
                            .update_with_mode(
                                buffered_display.buffer(),
                                &[],
                                RefreshMode::Full,
                                &mut delay,
                            )
                            .ok();
                    }

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
                        buffered_display.clear();
                        app.render(&mut buffered_display).ok();
                        display
                            .update_with_mode(
                                buffered_display.buffer(),
                                &[],
                                RefreshMode::Fast,
                                &mut delay,
                            )
                            .ok();
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
                    buffered_display.clear();
                    app.render(&mut buffered_display).ok();
                    display
                        .update_with_mode(
                            buffered_display.buffer(),
                            &[],
                            RefreshMode::Fast,
                            &mut delay,
                        )
                        .ok();
                }
            }
        } else if !power_pressed {
            last_button = None;
        }

        FreeRtos::delay_ms(50);
    }
}
