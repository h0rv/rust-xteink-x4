extern crate alloc;

mod cli;
mod cli_commands;
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

use xteink_ui::ui::ActivityRefreshMode;
use xteink_ui::{
    App, BufferedDisplay, Builder, Button, DeviceStatus, Dimensions, DisplayInterface, EinkDisplay,
    EinkInterface, InputEvent, RamXAddressing, RefreshMode, Rotation,
};

use cli::SerialCli;
use cli_commands::handle_cli_command;
use input::{init_adc, read_adc, read_battery_raw, read_buttons};
use runtime_diagnostics::{append_diag, configure_pthread_defaults, log_heap};
use sdcard::SdCardFs;
use web_upload::{PollError, WebUploadServer};
use wifi_manager::WifiManager;

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum UpdateStrategy {
    Full,
    PartialFull,
    FastFull,
}

#[allow(dead_code)]
const DISPLAY_COLS: u16 = 480;
#[allow(dead_code)]
const DISPLAY_ROWS: u16 = 800;

const POWER_LONG_PRESS_MS: u32 = 2000;
const BATTERY_SAMPLE_INTERVAL_MS: u32 = 2000;
const BATTERY_ADC_EMPTY: i32 = 2100;
const BATTERY_ADC_FULL: i32 = 3200;
const ENABLE_WEB_UPLOAD_SERVER: bool = false;

fn battery_percent_from_adc(raw: i32) -> u8 {
    let clamped = raw.clamp(BATTERY_ADC_EMPTY, BATTERY_ADC_FULL);
    let span = (BATTERY_ADC_FULL - BATTERY_ADC_EMPTY).max(1);
    (((clamped - BATTERY_ADC_EMPTY) * 100) / span) as u8
}

fn is_repeatable_nav_button(btn: Button) -> bool {
    matches!(
        btn,
        Button::Left
            | Button::Right
            | Button::Up
            | Button::Down
            | Button::VolumeUp
            | Button::VolumeDown
    )
}

fn apply_update<I, D>(
    strategy: UpdateStrategy,
    display: &mut EinkDisplay<I>,
    delay: &mut D,
    current: &[u8],
) where
    I: DisplayInterface,
    D: embedded_hal::delay::DelayNs,
{
    match strategy {
        UpdateStrategy::Full => {
            log::info!("UI: applying full refresh");
            if display
                .update_with_mode_no_lut(current, &[], RefreshMode::Full, delay)
                .is_err()
            {
                log::warn!("UI: full refresh failed");
            }
        }
        UpdateStrategy::PartialFull => {
            log::info!("UI: applying partial full-screen refresh");
            if display
                .update_with_mode_no_lut(current, &[], RefreshMode::Partial, delay)
                .is_err()
            {
                log::warn!("UI: partial full-screen refresh failed");
            }
        }
        UpdateStrategy::FastFull => {
            log::info!("UI: applying fast full-screen refresh");
            if display
                .update_with_mode_no_lut(current, &[], RefreshMode::Fast, delay)
                .is_err()
            {
                log::warn!("UI: fast full-screen refresh failed");
            }
        }
    }
}

/// Apply display update using the new ActivityRefreshMode system.
///
/// Maps ActivityRefreshMode to the appropriate update strategy:
/// - Full: Full screen full refresh (highest quality, slowest)
/// - Partial: Full screen partial refresh (for ghost cleanup)
/// - Fast: Full screen fast refresh (single-buffer differential handled in driver)
fn apply_update_with_mode<I, D>(
    mode: ActivityRefreshMode,
    display: &mut EinkDisplay<I>,
    delay: &mut D,
    current: &[u8],
) where
    I: DisplayInterface,
    D: embedded_hal::delay::DelayNs,
{
    match mode {
        ActivityRefreshMode::Full => {
            log::info!("UI: Activity requested full refresh");
            apply_update(UpdateStrategy::Full, display, delay, current);
        }
        ActivityRefreshMode::Partial => {
            log::info!("UI: Periodic partial refresh for ghost cleanup");
            apply_update(UpdateStrategy::PartialFull, display, delay, current);
        }
        ActivityRefreshMode::Fast => {
            // Driver handles single-buffer differential fast refresh.
            apply_update(UpdateStrategy::FastFull, display, delay, current);
        }
    }
}

struct CompactCover {
    width: u32,
    height: u32,
    pixels: Vec<u8>, // bit-packed, 1 bit set = black
}

const MAX_COMPACT_COVER_WIDTH: u32 = xteink_ui::DISPLAY_WIDTH;
const MAX_COMPACT_COVER_HEIGHT: u32 = xteink_ui::DISPLAY_HEIGHT;
const MAX_COMPACT_COVER_PACKED_BYTES: usize =
    ((xteink_ui::DISPLAY_WIDTH as usize) * (xteink_ui::DISPLAY_HEIGHT as usize) + 7) / 8;

fn decode_compact_cover(encoded: &str) -> Option<CompactCover> {
    let (dims, hex) = encoded.split_once(':')?;
    let (w, h) = dims.split_once('x')?;
    let width = w.parse::<u32>().ok()?;
    let height = h.parse::<u32>().ok()?;
    if width == 0
        || height == 0
        || width > MAX_COMPACT_COVER_WIDTH
        || height > MAX_COMPACT_COVER_HEIGHT
    {
        return None;
    }
    let pixel_count = (width as usize).checked_mul(height as usize)?;
    let packed_len = pixel_count.checked_add(7)? / 8;
    if packed_len > MAX_COMPACT_COVER_PACKED_BYTES {
        return None;
    }
    if hex.len() != packed_len * 2 {
        return None;
    }
    let mut pixels = vec![0u8; packed_len];
    for (idx, chunk) in hex.as_bytes().chunks_exact(2).enumerate() {
        let hi = hex_value(chunk[0] as char)?;
        let lo = hex_value(chunk[1] as char)?;
        pixels[idx] = (hi << 4) | lo;
    }
    Some(CompactCover {
        width,
        height,
        pixels,
    })
}

fn hex_value(ch: char) -> Option<u8> {
    match ch {
        '0'..='9' => Some((ch as u8) - b'0'),
        'a'..='f' => Some((ch as u8) - b'a' + 10),
        'A'..='F' => Some((ch as u8) - b'A' + 10),
        _ => None,
    }
}

fn cover_pixel_is_black(cover: &CompactCover, x: u32, y: u32) -> bool {
    let idx = (y as usize)
        .saturating_mul(cover.width as usize)
        .saturating_add(x as usize);
    let byte = cover.pixels.get(idx / 8).copied().unwrap_or(0);
    (byte & (1 << (7 - (idx % 8)))) != 0
}

fn render_sleep_cover_on_buffer(buffered_display: &mut BufferedDisplay, cover: &CompactCover) {
    let display_w = xteink_ui::DISPLAY_WIDTH;
    let display_h = xteink_ui::DISPLAY_HEIGHT;

    let scale_x = display_w / cover.width;
    let scale_y = display_h / cover.height;
    let scale = scale_x.min(scale_y).max(1);
    let render_w = cover.width.saturating_mul(scale);
    let render_h = cover.height.saturating_mul(scale);
    let origin_x = ((display_w.saturating_sub(render_w)) / 2) as i32;
    let origin_y = ((display_h.saturating_sub(render_h)) / 2) as i32;

    for src_y in 0..cover.height {
        for src_x in 0..cover.width {
            if !cover_pixel_is_black(cover, src_x, src_y) {
                continue;
            }
            let dst_x = origin_x + (src_x * scale) as i32;
            let dst_y = origin_y + (src_y * scale) as i32;
            for oy in 0..scale {
                for ox in 0..scale {
                    let x = dst_x + ox as i32;
                    let y = dst_y + oy as i32;
                    if x >= 0 && y >= 0 {
                        buffered_display.set_pixel(
                            x as u32,
                            y as u32,
                            embedded_graphics::pixelcolor::BinaryColor::On,
                        );
                    }
                }
            }
        }
    }
}

fn show_sleep_screen_with_cover<I, D>(
    app: &App,
    display: &mut EinkDisplay<I>,
    delay: &mut D,
    buffered_display: &mut BufferedDisplay,
) where
    I: DisplayInterface,
    D: embedded_hal::delay::DelayNs,
{
    buffered_display.clear();
    if let Some(compact) = app.active_book_cover_thumbnail_compact() {
        if let Some(cover) = decode_compact_cover(&compact) {
            render_sleep_cover_on_buffer(buffered_display, &cover);
        }
    }
    display
        .update_with_mode(buffered_display.buffer(), &[], RefreshMode::Full, delay)
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

fn reconcile_web_upload_server(
    app: &mut App,
    wifi_manager: &mut WifiManager,
    web_upload_server: &mut Option<WebUploadServer>,
) -> bool {
    let was_active = web_upload_server.is_some();
    if let Some(request_start) = app.take_file_transfer_request() {
        if request_start {
            match wifi_manager.start_transfer_network() {
                Ok(()) => {
                    if web_upload_server.is_none() {
                        match WebUploadServer::start() {
                            Ok(server) => {
                                *web_upload_server = Some(server);
                            }
                            Err(err) => {
                                log::warn!("[WEB] upload server start failed: {}", err);
                            }
                        }
                    }
                }
                Err(err) => {
                    log::warn!("[WIFI] unable to start transfer network: {}", err);
                }
            }
        } else {
            stop_web_upload_server(web_upload_server);
            wifi_manager.stop_transfer_network();
        }
    }
    let is_active = web_upload_server.is_some();
    app.set_file_transfer_active(is_active);
    let transfer_info = wifi_manager.transfer_info();
    app.set_file_transfer_network_details(
        transfer_info.mode,
        transfer_info.ssid,
        transfer_info.password_hint,
        transfer_info.url,
        transfer_info.message,
    );
    was_active != is_active
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let reset_reason = unsafe { sys::esp_reset_reason() };
    let wake_cause = unsafe { sys::esp_sleep_get_wakeup_cause() };
    log::info!(
        "Boot reason: reset={:?} wake_cause={:?}",
        reset_reason,
        wake_cause
    );
    append_diag(&format!(
        "boot reset={:?} wake={:?}",
        reset_reason, wake_cause
    ));
    configure_pthread_defaults();
    log_heap("startup");

    // Stack size verification (runtime log)
    // std-enabled UI task handling needs additional headroom on ESP32-C3.
    const REQUIRED_STACK_SIZE: u32 = 120 * 1024; // 120KB minimum for EPUB open/render on main task
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
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let mut wifi_manager = WifiManager::new(peripherals.modem, sys_loop);

    let spi = SpiDriver::new(
        peripherals.spi2,
        peripherals.pins.gpio8,
        peripherals.pins.gpio10,
        Some(peripherals.pins.gpio7),
        &SpiDriverConfig::default().dma(Dma::Auto(4096)),
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
        // Match crosspoint refresh control values (OTP LUT based)
        .display_update_ctrl2_full(0x34)
        .display_update_ctrl2_partial(0xD4)
        .display_update_ctrl2_fast(0x1C)
        .build()
        .unwrap();
    let mut display = EinkDisplay::new(interface, config);

    log::info!("Resetting display...");
    display.reset(&mut delay).ok();

    // Create buffered display for UI rendering (avoids stack overflow from iterator chains)
    let mut buffered_display = BufferedDisplay::new();

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

    // Initialize app and render initial screen
    let mut app = App::new();
    let mut last_epub_pos: Option<(usize, usize, usize, usize)> = None;
    let mut device_status = DeviceStatus::default();
    if let Some(initial_battery_raw) = read_battery_raw() {
        device_status.battery_percent = battery_percent_from_adc(initial_battery_raw);
        app.set_device_status(device_status);
        append_diag(&format!(
            "battery_init raw={} pct={}",
            initial_battery_raw, device_status.battery_percent
        ));
    } else {
        app.set_device_status(device_status);
        append_diag("battery_init skipped (adc-disabled)");
    }
    app.set_file_transfer_active(web_upload_server.is_some());
    let transfer_info = wifi_manager.transfer_info();
    app.set_file_transfer_network_details(
        transfer_info.mode,
        transfer_info.ssid,
        transfer_info.password_hint,
        transfer_info.url,
        transfer_info.message,
    );
    log_heap("before_app_init");
    // Prime deferred work (library cache/load scan) before the first paint to
    // avoid a guaranteed second full-screen refresh immediately after boot.
    const STARTUP_DEFERRED_MAX_TICKS: usize = 1024;
    let mut startup_deferred_ticks = 0usize;
    while startup_deferred_ticks < STARTUP_DEFERRED_MAX_TICKS && app.process_deferred_tasks(&mut fs)
    {
        startup_deferred_ticks += 1;
        if startup_deferred_ticks % 32 == 0 {
            FreeRtos::delay_ms(1);
        }
    }
    if startup_deferred_ticks >= STARTUP_DEFERRED_MAX_TICKS {
        log::warn!(
            "Startup deferred pre-pass reached tick cap ({}), continuing boot",
            STARTUP_DEFERRED_MAX_TICKS
        );
    }
    buffered_display.clear();
    app.render(&mut buffered_display).ok();
    log_heap("before_first_render");
    display
        .update(buffered_display.buffer(), &[], &mut delay)
        .ok();
    log_heap("after_first_render");

    log::info!("Starting event loop with adaptive refresh strategy");

    log::info!("Starting event loop... Press a button!");
    log::info!("Hold POWER for 2 seconds to sleep...");
    log::info!("CLI: connect via USB-Serial/JTAG @ 115200 (type 'help')");

    let mut power_press_counter: u32 = 0;
    let mut is_power_pressed: bool = false;
    let mut long_press_triggered: bool = false;
    let mut held_button: Option<Button> = None;
    let mut held_button_ticks: u32 = 0;
    let mut next_repeat_tick: u32 = 0;
    const DEBUG_ADC: bool = false;
    const DEBUG_INPUT: bool = true;
    const LOOP_DELAY_MS: u32 = 20;
    const POWER_LONG_PRESS_ITERATIONS: u32 = POWER_LONG_PRESS_MS / LOOP_DELAY_MS;
    const BUTTON_REPEAT_INITIAL_MS: u32 = 220;
    const BUTTON_REPEAT_INTERVAL_MS: u32 = 120;
    const BUTTON_REPEAT_INITIAL_TICKS: u32 =
        (BUTTON_REPEAT_INITIAL_MS + LOOP_DELAY_MS - 1) / LOOP_DELAY_MS;
    const BUTTON_REPEAT_INTERVAL_TICKS: u32 =
        (BUTTON_REPEAT_INTERVAL_MS + LOOP_DELAY_MS - 1) / LOOP_DELAY_MS;
    const ENABLE_CLI: bool = false;
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
        if let Some(cli) = cli.as_mut() {
            if let Some(line) = cli.poll_line() {
                handle_cli_command(
                    &line,
                    cli,
                    &mut fs,
                    &mut app,
                    &mut display,
                    &mut delay,
                    &mut buffered_display,
                    &mut sleep_requested,
                );
            }
        }

        if let Some(server) = web_upload_server.as_mut() {
            loop {
                match server.poll() {
                    Ok(Some(event)) => {
                        log::info!(
                            "[WEB] upload completed path={} bytes={}",
                            event.path,
                            event.received_bytes
                        );
                        app.invalidate_library_cache();
                    }
                    Ok(None) => break,
                    Err(PollError::QueueDisconnected) => {
                        log::warn!("[WEB] upload event queue disconnected, stopping server");
                        stop_web_upload_server(&mut web_upload_server);
                        wifi_manager.stop_transfer_network();
                        app.set_file_transfer_active(false);
                        break;
                    }
                }
            }
        }

        if sleep_requested {
            sleep_requested = false;
            stop_web_upload_server(&mut web_upload_server);
            wifi_manager.stop_transfer_network();
            show_sleep_screen_with_cover(&app, &mut display, &mut delay, &mut buffered_display);
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
                log::info!(
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
                let battery_percent = battery_percent_from_adc(battery_raw);
                device_status.battery_percent = battery_percent;
                app.set_device_status(device_status);
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
                        &app,
                        &mut display,
                        &mut delay,
                        &mut buffered_display,
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

                if app.handle_input(InputEvent::Press(Button::Power)) {
                    let _ = reconcile_web_upload_server(
                        &mut app,
                        &mut wifi_manager,
                        &mut web_upload_server,
                    );
                    log::info!("UI: redraw after power short press");
                    buffered_display.clear();
                    app.render(&mut buffered_display).ok();

                    let refresh_mode = app.get_refresh_mode();
                    log::info!("UI: using refresh mode {:?}", refresh_mode);
                    apply_update_with_mode(
                        refresh_mode,
                        &mut display,
                        &mut delay,
                        buffered_display.buffer(),
                    );
                } else {
                    log::info!("UI: no redraw after power short press");
                }
            }
            is_power_pressed = false;
            power_press_counter = 0;
        }

        if let Some(btn) = button {
            if btn != Button::Power {
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
                log_heap("before_handle_input");

                if app.handle_input(InputEvent::Press(btn)) {
                    let _ = reconcile_web_upload_server(
                        &mut app,
                        &mut wifi_manager,
                        &mut web_upload_server,
                    );
                    log::info!("UI: redraw after {:?}", btn);
                    log_heap("before_render");
                    buffered_display.clear();
                    app.render(&mut buffered_display).ok();
                    let refresh_mode = app.get_refresh_mode();
                    log::info!("UI: using refresh mode {:?}", refresh_mode);
                    apply_update_with_mode(
                        refresh_mode,
                        &mut display,
                        &mut delay,
                        buffered_display.buffer(),
                    );
                    log_heap("after_render");
                    if app.file_browser_is_reading_epub() {
                        let pos = app.file_browser_epub_position();
                        if pos != last_epub_pos {
                            if let Some((ch, ch_total, pg, pg_total)) = pos {
                                log::info!(
                                    "[EPUB] position changed: ch {}/{} pg {}/{}",
                                    ch,
                                    ch_total,
                                    pg,
                                    pg_total
                                );
                            }
                            log_heap("after_epub_page_change");
                            last_epub_pos = pos;
                        }
                    }
                } else {
                    let _ = reconcile_web_upload_server(
                        &mut app,
                        &mut wifi_manager,
                        &mut web_upload_server,
                    );
                    log::info!("UI: no redraw after {:?}", btn);
                    log_heap("after_handle_input_no_redraw");
                }
            }
        } else if !power_pressed {
            held_button = None;
            held_button_ticks = 0;
            next_repeat_tick = 0;
        }

        if app.process_deferred_tasks(&mut fs) {
            let _ =
                reconcile_web_upload_server(&mut app, &mut wifi_manager, &mut web_upload_server);
            log_heap("before_deferred_render");
            buffered_display.clear();
            app.render(&mut buffered_display).ok();
            let refresh_mode = app.get_refresh_mode();
            apply_update_with_mode(
                refresh_mode,
                &mut display,
                &mut delay,
                buffered_display.buffer(),
            );
            log_heap("after_deferred_render");
            if app.file_browser_is_reading_epub() {
                let pos = app.file_browser_epub_position();
                if pos != last_epub_pos {
                    if let Some((ch, ch_total, pg, pg_total)) = pos {
                        log::info!(
                            "[EPUB] deferred position changed: ch {}/{} pg {}/{}",
                            ch,
                            ch_total,
                            pg,
                            pg_total
                        );
                    }
                    log_heap("after_epub_deferred_change");
                    last_epub_pos = pos;
                }
            }
        }

        // Auto-sleep handling
        let auto_sleep_duration_ms = app.auto_sleep_duration_ms();
        if auto_sleep_duration_ms > 0 {
            // Increment inactivity timer
            inactivity_ms = inactivity_ms.saturating_add(LOOP_DELAY_MS);

            // Check if we should show the warning (10 seconds before sleep)
            if !sleep_warning_shown
                && inactivity_ms >= auto_sleep_duration_ms.saturating_sub(SLEEP_WARNING_MS)
                && inactivity_ms < auto_sleep_duration_ms
            {
                sleep_warning_shown = true;
                log::info!("Auto-sleep: showing warning (sleeping in 10s)");

                // Show warning toast - we need to render it
                // For now, just log it. In a full implementation, we'd use a toast overlay
                // that persists across renders until dismissed or sleep occurs.
                // Since we don't have a global toast system, we'll just log for now.
            }

            // Check if we should enter sleep
            if inactivity_ms >= auto_sleep_duration_ms {
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
                    inactivity_ms = auto_sleep_duration_ms.saturating_sub(SLEEP_WARNING_MS);
                    FreeRtos::delay_ms(100);
                    continue;
                }

                // If wake button line is already low, entering deep sleep can cause
                // immediate wake/reboot cycles that look like crash loops.
                if power_btn.is_low() {
                    log::warn!(
                        "Auto-sleep postponed: power button line is low (preventing wake loop)"
                    );
                    inactivity_ms = auto_sleep_duration_ms.saturating_sub(SLEEP_WARNING_MS);
                    FreeRtos::delay_ms(100);
                    continue;
                }

                show_sleep_screen_with_cover(&app, &mut display, &mut delay, &mut buffered_display);
                stop_web_upload_server(&mut web_upload_server);
                wifi_manager.stop_transfer_network();
                app.set_file_transfer_active(false);

                enter_deep_sleep(3);
            }
        }

        FreeRtos::delay_ms(LOOP_DELAY_MS);
    }
}
