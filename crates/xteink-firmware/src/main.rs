use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{Input, PinDriver, Pull},
    peripherals::Peripherals,
    spi::{config::Config, SpiDeviceDriver, SpiDriver, SpiDriverConfig},
};
use esp_idf_svc::sys; // Raw ESP-IDF C bindings

use embedded_graphics::{draw_target::DrawTarget, pixelcolor::BinaryColor, prelude::*};
use ssd1677::{RefreshMode, Ssd1677};
use xteink_ui::{App, Button, InputEvent};

/// Wrapper that rotates portrait UI coordinates to landscape hardware coordinates
struct PortraitDisplay<D> {
    pub display: D,
}

impl<D> PortraitDisplay<D> {
    fn new(display: D) -> Self {
        Self { display }
    }

    fn rotate(x: i32, y: i32) -> (i32, i32) {
        let hw_x = y;
        let hw_y = 479 - x;
        (hw_x, hw_y)
    }
}

impl<D: DrawTarget<Color = BinaryColor>> DrawTarget for PortraitDisplay<D> {
    type Color = BinaryColor;
    type Error = D::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let rotated_pixels = pixels.into_iter().map(|Pixel(point, color)| {
            let (hw_x, hw_y) = Self::rotate(point.x, point.y);
            Pixel(Point::new(hw_x, hw_y), color)
        });
        self.display.draw_iter(rotated_pixels)
    }
}

impl<D> OriginDimensions for PortraitDisplay<D> {
    fn size(&self) -> Size {
        Size::new(480, 800)
    }
}

// ADC Constants from reference implementation
const ADC_NO_BUTTON: i32 = 3800;
const ADC_RANGES_1: [i32; 5] = [3800, 3100, 2090, 750, i32::MIN];
const ADC_RANGES_2: [i32; 3] = [3800, 1120, i32::MIN];
const ADC_WIDTH_BIT_12: u32 = 3;
const ADC_ATTEN_DB_11: u32 = 3;

// Power button long press threshold (2 seconds to power off)
const POWER_LONG_PRESS_MS: u32 = 2000;

/// Initialize ADC for button reading
fn init_adc() {
    unsafe {
        sys::adc1_config_width(ADC_WIDTH_BIT_12);
        sys::adc1_config_channel_atten(sys::adc_channel_t_ADC_CHANNEL_1, ADC_ATTEN_DB_11);
        sys::adc1_config_channel_atten(sys::adc_channel_t_ADC_CHANNEL_2, ADC_ATTEN_DB_11);
    }
}

/// Read ADC value from a channel
fn read_adc(channel: sys::adc_channel_t) -> i32 {
    unsafe { sys::adc1_get_raw(channel) as i32 }
}

/// Get button from ADC value using range-based detection
fn get_button_from_adc(adc_value: i32, ranges: &[i32], num_buttons: usize) -> i32 {
    for i in 0..num_buttons {
        if ranges[i + 1] < adc_value && adc_value <= ranges[i] {
            return i as i32;
        }
    }
    -1
}

/// Read button state from ADC and digital pins
/// Returns (button, is_power_pressed)
fn read_buttons(
    power_btn: &mut PinDriver<esp_idf_svc::hal::gpio::Gpio3, Input>,
    debug_mode: bool,
) -> (Option<Button>, bool) {
    // Check power button first (digital, active LOW with pull-up)
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

/// Enter deep sleep mode
fn enter_deep_sleep(power_btn_pin: i32) {
    log::info!("Entering deep sleep...");
    unsafe {
        // Configure wake on power button (GPIO 3, active LOW)
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

    log::info!("Starting firmware...");

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

    let dc = PinDriver::output(peripherals.pins.gpio4).unwrap();
    let rst = PinDriver::output(peripherals.pins.gpio5).unwrap();
    let busy = PinDriver::input(peripherals.pins.gpio6).unwrap();

    let mut power_btn = PinDriver::input(peripherals.pins.gpio3).unwrap();
    power_btn.set_pull(Pull::Up).unwrap();

    init_adc();

    let mut delay = FreeRtos;
    let display = Ssd1677::new(spi_device, dc, rst, busy);
    let mut portrait_display = PortraitDisplay::new(display);

    log::info!("Resetting display...");
    portrait_display.display.reset_display(&mut delay);

    log::info!("Initializing display...");
    portrait_display.display.init(&mut delay);

    // Initial draw with Full refresh
    let mut app = App::new();
    app.render(&mut portrait_display).ok();
    portrait_display.display.write_buffer_full();
    portrait_display
        .display
        .refresh_display(RefreshMode::Full, false, &mut delay);
    portrait_display.display.swap_buffers();

    log::info!("Starting event loop... Press a button!");
    log::info!("Hold POWER button for 2 seconds to sleep...");

    // Event loop
    let mut last_button: Option<Button> = None;
    let mut power_press_counter: u32 = 0;
    let mut is_power_pressed: bool = false;
    let mut long_press_triggered: bool = false;
    let mut refresh_count: u32 = 0;
    const DEBUG_ADC: bool = false;
    const FULL_REFRESH_EVERY_N: u32 = 15;
    // Power long press: 2 seconds / 50ms poll = 40 iterations
    const POWER_LONG_PRESS_ITERATIONS: u32 = POWER_LONG_PRESS_MS / 50;

    loop {
        let (button, power_pressed) = read_buttons(&mut power_btn, DEBUG_ADC);

        // Handle power button long press
        if power_pressed {
            if !is_power_pressed {
                // Power button just pressed
                power_press_counter = 0;
                is_power_pressed = true;
                long_press_triggered = false;
                log::info!("Power button pressed...");
            } else if !long_press_triggered {
                // Increment counter while holding
                power_press_counter += 1;
                if power_press_counter >= POWER_LONG_PRESS_ITERATIONS {
                    log::info!("Power button held for 2s - powering off!");
                    long_press_triggered = true;
                    
                    // Show "Sleeping..." message
                    app.handle_input(InputEvent::Press(Button::Power));
                    app.render(&mut portrait_display).ok();
                    portrait_display.display.write_buffer_full();
                    portrait_display
                        .display
                        .refresh_display(RefreshMode::Full, false, &mut delay);
                    
                    // Wait for button release before sleeping
                    while power_btn.is_low() {
                        FreeRtos::delay_ms(50);
                    }
                    
                    // Enter deep sleep
                    enter_deep_sleep(3); // GPIO 3
                }
            }
        } else {
            // Power button released
            if is_power_pressed && !long_press_triggered {
                // Short press - treat as normal button
                if last_button != Some(Button::Power) {
                    log::info!("Power button short press");
                    last_button = Some(Button::Power);
                    
                    let event = InputEvent::Press(Button::Power);
                    if app.handle_input(event) {
                        app.render(&mut portrait_display).ok();
                        refresh_count += 1;
                        if refresh_count % FULL_REFRESH_EVERY_N == 0 {
                            portrait_display.display.write_buffer_full();
                            portrait_display
                                .display
                                .refresh_display(RefreshMode::Half, false, &mut delay);
                            portrait_display.display.swap_buffers();
                        } else {
                            portrait_display.display.write_buffer_fast();
                            portrait_display
                                .display
                                .refresh_display(RefreshMode::Fast, false, &mut delay);
                            portrait_display.display.swap_buffers();
                        }
                    }
                }
            }
            is_power_pressed = false;
            power_press_counter = 0;
        }

                    // Enter deep sleep
                    enter_deep_sleep(3); // GPIO 3
                }
            }
        } else {
            // Power button released
            if is_power_pressed && !long_press_triggered {
                // Short press - treat as normal button
                if last_button != Some(Button::Power) {
                    log::info!("Power button short press");
                    last_button = Some(Button::Power);

                    let event = InputEvent::Press(Button::Power);
                    if app.handle_input(event) {
                        app.render(&mut portrait_display).ok();
                        refresh_count += 1;
                        if refresh_count % FULL_REFRESH_EVERY_N == 0 {
                            portrait_display.display.write_buffer_full();
                            portrait_display.display.refresh_display(
                                RefreshMode::Half,
                                false,
                                &mut delay,
                            );
                            portrait_display.display.swap_buffers();
                        } else {
                            portrait_display.display.write_buffer_fast();
                            portrait_display.display.refresh_display(
                                RefreshMode::Fast,
                                false,
                                &mut delay,
                            );
                            portrait_display.display.swap_buffers();
                        }
                    }
                }
            }
            is_power_pressed = false;
        }

        // Handle other buttons
        if let Some(btn) = button {
            if btn != Button::Power && last_button != Some(btn) {
                log::info!("Button pressed: {:?}", btn);
                last_button = Some(btn);

                let event = InputEvent::Press(btn);
                if app.handle_input(event) {
                    app.render(&mut portrait_display).ok();

                    refresh_count += 1;
                    if refresh_count % FULL_REFRESH_EVERY_N == 0 {
                        log::info!("Half refresh...");
                        portrait_display.display.write_buffer_full();
                        portrait_display.display.refresh_display(
                            RefreshMode::Half,
                            false,
                            &mut delay,
                        );
                        portrait_display.display.swap_buffers();
                    } else {
                        portrait_display.display.write_buffer_fast();
                        portrait_display.display.refresh_display(
                            RefreshMode::Fast,
                            false,
                            &mut delay,
                        );
                        portrait_display.display.swap_buffers();
                    }
                }
            }
        } else if !power_pressed {
            last_button = None;
        }

        FreeRtos::delay_ms(50);
    }
}
