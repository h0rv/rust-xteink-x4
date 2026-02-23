extern crate alloc;

mod buffered_display;
mod cli;
mod cli_commands;
mod einked_slice;
mod filesystem;
mod input;
mod runtime_diagnostics;
mod sdcard;
mod web_upload;
mod wifi_manager;

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{PinDriver, Pull},
    peripherals::Peripherals,
    spi::{config::Config, Dma, SpiDeviceDriver, SpiDriver, SpiDriverConfig},
};
use esp_idf_svc::sys;

use einked::input::{Button, InputEvent};
use ssd1677::{
    Builder, Dimensions, Display as EinkDisplay, DisplayInterface, Interface as EinkInterface,
    RamXAddressing, RefreshMode, Rotation,
};

use buffered_display::BufferedDisplay;
use cli::SerialCli;
use cli_commands::handle_cli_command;
use einked_slice::{EinkedSlice, set_wifi_active, take_wifi_enable_request};
use filesystem::FileSystem;
use input::{init_adc, read_adc, read_battery_raw, read_buttons};
use runtime_diagnostics::{append_diag, log_heap};
use sdcard::SdCardFs;
use web_upload::{PollError, WebUploadServer};
use wifi_manager::WifiManager;

#[allow(dead_code)]
const DISPLAY_COLS: u16 = 480;
#[allow(dead_code)]
const DISPLAY_ROWS: u16 = 800;

const POWER_LONG_PRESS_MS: u32 = 2000;
const BATTERY_SAMPLE_INTERVAL_MS: u32 = 2000;
const BATTERY_ADC_EMPTY: i32 = 2100;
const BATTERY_ADC_FULL: i32 = 3200;
const ENABLE_WEB_UPLOAD_SERVER: bool = false;
const WEB_UPLOAD_MAX_EVENTS_PER_LOOP: usize = 8;
const AUTO_SLEEP_DURATION_MS: u32 = 10 * 60 * 1000;
const DISPLAY_WIDTH: u32 = 480;
const DISPLAY_HEIGHT: u32 = 800;
const ENABLE_BOOT_PROBE_FRAME: bool = false;

fn boot_mark(step: u8, msg: &str) {
    log::warn!("[BOOT:{:02}] {}", step, msg);
}

fn battery_percent_from_adc(raw: i32) -> u8 {
    let clamped = raw.clamp(BATTERY_ADC_EMPTY, BATTERY_ADC_FULL);
    let span = (BATTERY_ADC_FULL - BATTERY_ADC_EMPTY).max(1);
    (((clamped - BATTERY_ADC_EMPTY) * 100) / span) as u8
}

fn is_repeatable_nav_button(btn: Button) -> bool {
    matches!(
        btn,
        Button::Left | Button::Right | Button::Up | Button::Down | Button::Aux1 | Button::Aux2
    )
}

const SLEEP_IMAGES_DIR: &str = "/sd/.xteink/sleep";

struct SleepImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

fn load_custom_sleep_image(fs: &mut SdCardFs) -> Option<SleepImage> {
    let entries = fs.list_files(SLEEP_IMAGES_DIR).ok()?;
    let image_files: Vec<_> = entries
        .into_iter()
        .filter(|e| !e.is_directory)
        .filter(|e| {
            let name = e.name.to_lowercase();
            name.ends_with(".bmp")
                || name.ends_with(".png")
                || name.ends_with(".jpg")
                || name.ends_with(".jpeg")
        })
        .collect();

    if image_files.is_empty() {
        log::info!(
            "[SLEEP] No custom sleep images found in {}",
            SLEEP_IMAGES_DIR
        );
        return None;
    }

    let selected = &image_files[0];
    let path = format!("{}/{}", SLEEP_IMAGES_DIR, selected.name);
    log::info!("[SLEEP] Loading custom sleep image: {}", path);

    let bytes = fs.read_file_bytes(&path).ok()?;
    decode_image_to_binary(&bytes)
}

fn decode_image_to_binary(bytes: &[u8]) -> Option<SleepImage> {
    let img = image::load_from_memory(bytes).ok()?;

    let target_width = DISPLAY_WIDTH;
    let target_height = DISPLAY_HEIGHT;

    let resized = img.resize_to_fill(
        target_width,
        target_height,
        image::imageops::FilterType::Lanczos3,
    );

    let gray = resized.into_luma8();

    let mut pixels = vec![0u8; ((target_width as usize) * (target_height as usize) + 7) / 8];

    for (x, y, pixel) in gray.enumerate_pixels() {
        let lum = pixel.0[0];
        let is_black = lum < 128;
        if is_black {
            let idx = (y as usize) * (target_width as usize) + (x as usize);
            pixels[idx / 8] |= 1 << (7 - (idx % 8));
        }
    }

    Some(SleepImage {
        width: target_width,
        height: target_height,
        pixels,
    })
}

fn render_sleep_image_on_buffer(buffered_display: &mut BufferedDisplay, image: &SleepImage) {
    for y in 0..image.height {
        for x in 0..image.width {
            let idx = (y as usize) * (image.width as usize) + (x as usize);
            let is_black = (image.pixels[idx / 8] & (1 << (7 - (idx % 8)))) != 0;
            if is_black {
                buffered_display.set_pixel(x, y, embedded_graphics::pixelcolor::BinaryColor::On);
            }
        }
    }
}

fn draw_boot_probe_frame<I, D>(
    display: &mut EinkDisplay<I>,
    delay: &mut D,
    buffered_display: &mut BufferedDisplay,
) where
    I: DisplayInterface,
    D: embedded_hal::delay::DelayNs,
{
    buffered_display.clear();
    // Draw a simple border + center cross so boot-time panel updates are obvious.
    for x in 0..DISPLAY_WIDTH {
        buffered_display.set_pixel(x, 0, embedded_graphics::pixelcolor::BinaryColor::On);
        buffered_display.set_pixel(
            x,
            DISPLAY_HEIGHT - 1,
            embedded_graphics::pixelcolor::BinaryColor::On,
        );
    }
    for y in 0..DISPLAY_HEIGHT {
        buffered_display.set_pixel(0, y, embedded_graphics::pixelcolor::BinaryColor::On);
        buffered_display.set_pixel(
            DISPLAY_WIDTH - 1,
            y,
            embedded_graphics::pixelcolor::BinaryColor::On,
        );
    }
    let mid_x = DISPLAY_WIDTH / 2;
    let mid_y = DISPLAY_HEIGHT / 2;
    for off in 0..80 {
        buffered_display.set_pixel(
            mid_x + off,
            mid_y,
            embedded_graphics::pixelcolor::BinaryColor::On,
        );
        buffered_display.set_pixel(
            mid_x - off,
            mid_y,
            embedded_graphics::pixelcolor::BinaryColor::On,
        );
        buffered_display.set_pixel(
            mid_x,
            mid_y + off,
            embedded_graphics::pixelcolor::BinaryColor::On,
        );
        buffered_display.set_pixel(
            mid_x,
            mid_y - off,
            embedded_graphics::pixelcolor::BinaryColor::On,
        );
    }
    if display
        .update_with_mode_no_lut(buffered_display.buffer(), &[], RefreshMode::Full, delay)
        .is_err()
    {
        log::warn!("[DISPLAY] boot probe refresh failed");
    }
}

fn show_sleep_screen_with_cover<I, D>(
    display: &mut EinkDisplay<I>,
    delay: &mut D,
    buffered_display: &mut BufferedDisplay,
    fs: &mut SdCardFs,
) where
    I: DisplayInterface,
    D: embedded_hal::delay::DelayNs,
{
    buffered_display.clear();

    if let Some(image) = load_custom_sleep_image(fs) {
        log::info!("[SLEEP] Rendering custom sleep image");
        render_sleep_image_on_buffer(buffered_display, &image);
    }

    display
        .update_with_mode_no_lut(buffered_display.buffer(), &[], RefreshMode::Full, delay)
        .ok();
}

fn enter_deep_sleep(power_btn_pin: i32) {
    append_diag("deep_sleep_enter");
    log::info!("Entering deep sleep...");
    unsafe {
        sys::esp_deep_sleep_enable_gpio_wakeup(
            1u64 << power_btn_pin,
            sys::esp_deepsleep_gpio_wake_up_mode_t_ESP_GPIO_WAKEUP_GPIO_LOW,
        );
        sys::esp_deep_sleep_start();
    }
}

fn stop_web_upload_server(web_upload_server: &mut Option<WebUploadServer>) {
    if let Some(server) = web_upload_server.take() {
        server.stop();
    }
}

fn firmware_main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    boot_mark(1, "logger init done");
    log::warn!("[BOOT] rust main entered");
    let reset_reason = unsafe { sys::esp_reset_reason() };
    let wake_cause = unsafe { sys::esp_sleep_get_wakeup_cause() };
    log::info!(
        "Boot reason: reset={:?} wake_cause={:?}",
        reset_reason,
        wake_cause
    );
    // Avoid touching /sd diagnostics before the SD stack is initialized.
    // Defer optional pthread tuning during boot isolation.
    // configure_pthread_defaults();
    log_heap("startup");

    // Stack size verification (runtime log).
    // Keep this reasonably high, but avoid consuming most heap at boot.
    const REQUIRED_STACK_SIZE: u32 = 112 * 1024;
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
    boot_mark(2, "about to take peripherals");

    let peripherals = Peripherals::take().unwrap();
    boot_mark(3, "peripherals acquired");

    let sys_loop = EspSystemEventLoop::take().unwrap();
    let mut wifi_manager = WifiManager::new(peripherals.modem, sys_loop);
    boot_mark(4, "wifi manager initialized");
    let spi = SpiDriver::new(
        peripherals.spi2,
        peripherals.pins.gpio8,
        peripherals.pins.gpio10,
        Some(peripherals.pins.gpio7),
        &SpiDriverConfig::default().dma(Dma::Auto(4096)),
    )
    .unwrap();
    boot_mark(5, "spi driver created");

    let spi_config = Config::default()
        .baudrate(esp_idf_svc::hal::units::Hertz(40_000_000))
        .data_mode(embedded_hal::spi::Mode {
            polarity: embedded_hal::spi::Polarity::IdleLow,
            phase: embedded_hal::spi::Phase::CaptureOnFirstTransition,
        });

    let spi_device =
        SpiDeviceDriver::new(&spi, Some(peripherals.pins.gpio21), &spi_config).unwrap();
    boot_mark(6, "spi device created");
    let dc = PinDriver::output(peripherals.pins.gpio4).unwrap();
    let rst = PinDriver::output(peripherals.pins.gpio5).unwrap();
    let busy = PinDriver::input(peripherals.pins.gpio6).unwrap();
    boot_mark(7, "display pins ready");

    let mut power_btn = PinDriver::input(peripherals.pins.gpio3).unwrap();
    power_btn.set_pull(Pull::Up).unwrap();
    boot_mark(8, "power button pin ready");

    init_adc();
    boot_mark(9, "adc init done");

    // Initialize display
    let mut delay = FreeRtos;
    let mut interface = EinkInterface::new(spi_device, dc, rst, busy);
    // Keep boot responsive even when BUSY is noisy; timeouts fail-open with warning.
    interface.set_busy_timeout(2_000);
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
        // Match crosspoint refresh control values (OTP LUT based)
        .display_update_ctrl2_full(0x34)
        .display_update_ctrl2_partial(0xD4)
        .display_update_ctrl2_fast(0x1C)
        .build()
        .unwrap();
    boot_mark(10, "display config built");
    let mut display = EinkDisplay::new(interface, config);
    boot_mark(11, "display object created");

    log::info!("Resetting display...");
    boot_mark(12, "before display.reset");
    if display.reset(&mut delay).is_err() {
        log::warn!("[DISPLAY] reset/init failed");
    }
    boot_mark(13, "after display.reset");

    // Create buffered display for UI rendering (avoids stack overflow from iterator chains)
    let mut buffered_display = BufferedDisplay::new();
    boot_mark(14, "buffered display allocated");
    log_heap("after_buffered_display");
    if ENABLE_BOOT_PROBE_FRAME {
        boot_mark(15, "before boot probe frame");
        draw_boot_probe_frame(&mut display, &mut delay, &mut buffered_display);
        boot_mark(16, "after boot probe frame");
    }
    // Initialize SD card filesystem.
    // Boot must remain usable even when SD card is absent or mount fails.
    let mut fs = match SdCardFs::new(spi.host() as i32, 12) {
        Ok(fs) => {
            append_diag("sd_mount_ok");
            fs
        }
        Err(err) => {
            log::warn!("SD card mount failed: {}", err);
            append_diag(&format!("sd_mount_failed: {}", err));
            SdCardFs::unavailable(err.to_string())
        }
    };
    boot_mark(17, "sd init attempted");
    log_heap("before_einked_runtime");

    let mut einked_slice = EinkedSlice::new();
    boot_mark(18, "einked runtime created");
    log_heap("after_einked_runtime");
    append_diag(&format!(
        "boot reset={:?} wake={:?}",
        reset_reason, wake_cause
    ));

    // Initialize runtime and render initial screen
    if let Some(initial_battery_raw) = read_battery_raw() {
        append_diag(&format!(
            "battery_init raw={} pct={}",
            initial_battery_raw,
            battery_percent_from_adc(initial_battery_raw)
        ));
    } else {
        append_diag("battery_init skipped (adc-disabled)");
    }

    log::warn!("[BOOT] starting first einked render");
    log_heap("before_app_init");
    buffered_display.clear();
    boot_mark(19, "before first einked tick_and_flush");
    let first_ok =
        einked_slice.tick_and_flush(None, &mut display, &mut delay, &mut buffered_display);
    boot_mark(20, "after first einked tick_and_flush");
    if !first_ok {
        log::warn!("[EINKED] initial render/flush failed");
    } else {
        log::warn!("[BOOT] first einked render complete");
    }
    log_heap("after_first_render");
    boot_mark(21, "after first render bookkeeping");
    let mut web_upload_server = if ENABLE_WEB_UPLOAD_SERVER {
        let _ = wifi_manager.start_transfer_network();
        match WebUploadServer::start() {
            Ok(server) => Some(server),
            Err(err) => {
                log::warn!("[WEB] upload server start failed: {}", err);
                None
            }
        }
    } else {
        None
    };

    log::info!("Starting event loop with adaptive refresh strategy");

    log::info!("Starting event loop... Press a button!");
    log::info!("Hold POWER for 2 seconds to sleep...");
    log::info!("CLI: connect via USB-Serial/JTAG @ 115200 (type 'help')");
    boot_mark(22, "entering main event loop");

    let mut power_press_counter: u32 = 0;
    let mut is_power_pressed: bool = false;
    let mut long_press_triggered: bool = false;
    let mut held_button: Option<Button> = None;
    let mut held_button_ticks: u32 = 0;
    let mut next_repeat_tick: u32 = 0;
    const DEBUG_ADC: bool = false;
    const DEBUG_INPUT: bool = false;
    const LOOP_DELAY_MS: u32 = 20;
    const POWER_LONG_PRESS_ITERATIONS: u32 = POWER_LONG_PRESS_MS / LOOP_DELAY_MS;
    const BUTTON_REPEAT_INITIAL_MS: u32 = 220;
    const BUTTON_REPEAT_INTERVAL_MS: u32 = 120;
    const BUTTON_REPEAT_INITIAL_TICKS: u32 =
        (BUTTON_REPEAT_INITIAL_MS + LOOP_DELAY_MS - 1) / LOOP_DELAY_MS;
    const BUTTON_REPEAT_INTERVAL_TICKS: u32 =
        (BUTTON_REPEAT_INTERVAL_MS + LOOP_DELAY_MS - 1) / LOOP_DELAY_MS;
    const ENABLE_CLI: bool = true;
    let mut cli = if ENABLE_CLI {
        Some(SerialCli::new())
    } else {
        None
    };
    let mut input_debug_ticks: u32 = 0;
    let mut battery_sample_elapsed_ms: u32 = 0;
    let mut sleep_requested = false;

    // Auto-sleep tracking
    let mut inactivity_ms: u32 = 0;
    let mut sleep_warning_shown: bool = false;
    let mut power_line_high_stable_ms: u32 = 0;
    const SLEEP_WARNING_MS: u32 = 10_000; // Show warning 10 seconds before sleep
    const POWER_LINE_STABLE_BEFORE_SLEEP_MS: u32 = 2_000;

    loop {
        set_wifi_active(wifi_manager.is_network_active());

        if take_wifi_enable_request() {
            match wifi_manager.start_transfer_network() {
                Ok(()) => log::info!("[WIFI] started from einked feed request"),
                Err(err) => log::warn!("[WIFI] feed request start failed: {}", err),
            }
            set_wifi_active(wifi_manager.is_network_active());
        }

        if let Some(cli) = cli.as_mut() {
            if let Some(line) = cli.poll_line() {
                handle_cli_command(
                    &line,
                    cli,
                    &mut fs,
                    &mut display,
                    &mut delay,
                    &mut buffered_display,
                    &mut sleep_requested,
                    &mut wifi_manager,
                );
            }
        }

        if let Some(server) = web_upload_server.as_mut() {
            let mut processed_events = 0usize;
            loop {
                match server.poll() {
                    Ok(Some(event)) => {
                        log::info!(
                            "[WEB] upload completed path={} bytes={}",
                            event.path,
                            event.received_bytes
                        );
                        processed_events = processed_events.saturating_add(1);
                        if processed_events >= WEB_UPLOAD_MAX_EVENTS_PER_LOOP {
                            log::warn!(
                                "[WEB] poll capped at {} events this loop",
                                WEB_UPLOAD_MAX_EVENTS_PER_LOOP
                            );
                            append_diag("web_poll_capped");
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(PollError::QueueDisconnected) => {
                        log::warn!("[WEB] upload event queue disconnected, stopping server");
                        stop_web_upload_server(&mut web_upload_server);
                        wifi_manager.stop_transfer_network();
                        break;
                    }
                }
            }
        }

        if sleep_requested {
            sleep_requested = false;
            stop_web_upload_server(&mut web_upload_server);
            wifi_manager.stop_transfer_network();
            show_sleep_screen_with_cover(&mut display, &mut delay, &mut buffered_display, &mut fs);
            enter_deep_sleep(3);
        }

        let (button, power_pressed) = read_buttons(&mut power_btn, DEBUG_ADC);
        if power_pressed {
            power_line_high_stable_ms = 0;
        } else {
            power_line_high_stable_ms = power_line_high_stable_ms.saturating_add(LOOP_DELAY_MS);
        }

        // Reset inactivity timer on any button press
        if button.is_some() || power_pressed {
            inactivity_ms = 0;
            sleep_warning_shown = false;
        }

        if DEBUG_INPUT {
            input_debug_ticks = input_debug_ticks.saturating_add(1);
            if input_debug_ticks >= 10 {
                input_debug_ticks = 0;
                let adc1_value = read_adc(sys::adc_channel_t_ADC_CHANNEL_1);
                let adc2_value = read_adc(sys::adc_channel_t_ADC_CHANNEL_2);
                log::debug!(
                    "INPUT: power={} adc1={} adc2={} decoded={:?} held={:?}",
                    power_pressed,
                    adc1_value,
                    adc2_value,
                    button,
                    held_button
                );
            }
        }

        battery_sample_elapsed_ms = battery_sample_elapsed_ms.saturating_add(LOOP_DELAY_MS);
        if battery_sample_elapsed_ms >= BATTERY_SAMPLE_INTERVAL_MS {
            battery_sample_elapsed_ms = 0;
            if let Some(battery_raw) = read_battery_raw() {
                append_diag(&format!(
                    "battery_sample raw={} pct={}",
                    battery_raw,
                    battery_percent_from_adc(battery_raw)
                ));
            }
        }

        if power_pressed {
            if !is_power_pressed {
                power_press_counter = 0;
                is_power_pressed = true;
                long_press_triggered = false;
                log::info!("Power button pressed...");
                append_diag("power_press");
            } else if !long_press_triggered {
                power_press_counter += 1;
                if power_press_counter >= POWER_LONG_PRESS_ITERATIONS {
                    log::info!("Power button held for 2s - powering off!");
                    append_diag("power_long_press");
                    long_press_triggered = true;

                    show_sleep_screen_with_cover(
                        &mut display,
                        &mut delay,
                        &mut buffered_display,
                        &mut fs,
                    );
                    log::info!("Displayed centered cover for power off");
                    stop_web_upload_server(&mut web_upload_server);
                    wifi_manager.stop_transfer_network();

                    while power_btn.is_low() {
                        FreeRtos::delay_ms(50);
                    }

                    enter_deep_sleep(3);
                }
            }
        } else {
            if is_power_pressed && !long_press_triggered {
                log::info!("Power button short press");
                append_diag("power_short_press");

                if !einked_slice.tick_and_flush(
                    Some(InputEvent::Press(Button::Aux3)),
                    &mut display,
                    &mut delay,
                    &mut buffered_display,
                ) {
                    log::warn!("[EINKED] power short press flush failed");
                }
            }
            is_power_pressed = false;
            power_press_counter = 0;
        }

        if let Some(btn) = button {
            if btn != Button::Aux3 {
                let mut emit_press = false;
                if is_repeatable_nav_button(btn) {
                    if held_button == Some(btn) {
                        held_button_ticks = held_button_ticks.saturating_add(1);
                        if held_button_ticks >= next_repeat_tick {
                            emit_press = true;
                            next_repeat_tick = next_repeat_tick
                                .saturating_add(BUTTON_REPEAT_INTERVAL_TICKS.max(1));
                        }
                    } else {
                        held_button = Some(btn);
                        held_button_ticks = 0;
                        next_repeat_tick = BUTTON_REPEAT_INITIAL_TICKS.max(1);
                        emit_press = true;
                    }
                } else {
                    emit_press = held_button != Some(btn);
                    if emit_press {
                        held_button = Some(btn);
                    }
                }

                if !emit_press {
                    FreeRtos::delay_ms(LOOP_DELAY_MS);
                    continue;
                }

                log::info!("Button pressed: {:?}", btn);
                if !einked_slice.tick_and_flush(
                    Some(InputEvent::Press(btn)),
                    &mut display,
                    &mut delay,
                    &mut buffered_display,
                ) {
                    log::warn!("[EINKED] button press flush failed: {:?}", btn);
                }
            }
        } else if !power_pressed {
            held_button = None;
            held_button_ticks = 0;
            next_repeat_tick = 0;
        }

        // Auto-sleep handling
        if AUTO_SLEEP_DURATION_MS > 0 {
            // Increment inactivity timer
            inactivity_ms = inactivity_ms.saturating_add(LOOP_DELAY_MS);

            // Check if we should show the warning (10 seconds before sleep)
            if !sleep_warning_shown
                && inactivity_ms >= AUTO_SLEEP_DURATION_MS.saturating_sub(SLEEP_WARNING_MS)
                && inactivity_ms < AUTO_SLEEP_DURATION_MS
            {
                sleep_warning_shown = true;
                log::info!("Auto-sleep: showing warning (sleeping in 10s)");

                // Show warning toast - we need to render it
                // For now, just log it. In a full implementation, we'd use a toast overlay
                // that persists across renders until dismissed or sleep occurs.
                // Since we don't have a global toast system, we'll just log for now.
            }

            // Check if we should enter sleep
            if inactivity_ms >= AUTO_SLEEP_DURATION_MS {
                log::info!(
                    "Auto-sleep: entering deep sleep after {}ms of inactivity",
                    inactivity_ms
                );
                append_diag(&format!("auto_sleep_enter inactivity_ms={}", inactivity_ms));

                // Require the wake line to be stably high before sleeping. This avoids
                // immediate wake/reboot loops on noisy or floating button lines.
                if power_line_high_stable_ms < POWER_LINE_STABLE_BEFORE_SLEEP_MS {
                    log::warn!(
                        "Auto-sleep postponed: power line not stable-high long enough ({}ms)",
                        power_line_high_stable_ms
                    );
                    inactivity_ms = AUTO_SLEEP_DURATION_MS.saturating_sub(SLEEP_WARNING_MS);
                    FreeRtos::delay_ms(100);
                    continue;
                }

                // If wake button line is already low, entering deep sleep can cause
                // immediate wake/reboot cycles that look like crash loops.
                if power_btn.is_low() {
                    log::warn!(
                        "Auto-sleep postponed: power button line is low (preventing wake loop)"
                    );
                    inactivity_ms = AUTO_SLEEP_DURATION_MS.saturating_sub(SLEEP_WARNING_MS);
                    FreeRtos::delay_ms(100);
                    continue;
                }

                show_sleep_screen_with_cover(
                    &mut display,
                    &mut delay,
                    &mut buffered_display,
                    &mut fs,
                );
                stop_web_upload_server(&mut web_upload_server);
                wifi_manager.stop_transfer_network();

                enter_deep_sleep(3);
            }
        }

        FreeRtos::delay_ms(LOOP_DELAY_MS);
    }
}

fn main() {
    firmware_main();
}
