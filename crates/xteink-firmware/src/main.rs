extern crate alloc;

mod cli;
mod sdcard;

use alloc::vec::Vec;
use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{Input, PinDriver, Pull},
    peripherals::Peripherals,
    spi::{config::Config, SpiDeviceDriver, SpiDriver, SpiDriverConfig},
};
use esp_idf_svc::sys;

use xteink_ui::ui::ActivityRefreshMode;
use xteink_ui::FileSystem;
use xteink_ui::{
    compute_diff_region, extract_region, App, BufferedDisplay, Builder, Button, Dimensions,
    DisplayInterface, EinkDisplay, EinkInterface, InputEvent, RamXAddressing, RefreshMode, Region,
    Rotation, UpdateRegion,
};

use cli::SerialCli;
use sdcard::SdCardFs;

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum UpdateStrategy {
    Full,
    PartialFull,
    FastFull,
    DiffFast,
}

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

fn format_size(size: u64) -> String {
    if size >= 1024 * 1024 {
        format!("{:.1}MB", size as f32 / (1024.0 * 1024.0))
    } else if size >= 1024 {
        format!("{:.0}KB", size as f32 / 1024.0)
    } else {
        format!("{}B", size)
    }
}
/// Log heap usage statistics
fn log_heap(label: &str) {
    let free_heap = unsafe { esp_idf_svc::sys::esp_get_free_heap_size() };
    let min_free = unsafe { esp_idf_svc::sys::esp_get_minimum_free_heap_size() };
    log::info!(
        "[HEAP] {}: free={} bytes min_free={} bytes",
        label,
        free_heap,
        min_free
    );
}

fn cli_redraw<I, D>(
    app: &mut App,
    display: &mut EinkDisplay<I>,
    delay: &mut D,
    buffered_display: &mut BufferedDisplay,
    last_buffer: &mut Vec<u8>,
    mode: RefreshMode,
) where
    I: DisplayInterface,
    D: embedded_hal::delay::DelayNs,
{
    buffered_display.clear();
    app.render(buffered_display).ok();
    display
        .update_with_mode_no_lut(buffered_display.buffer(), &[], mode, delay)
        .ok();
    last_buffer.copy_from_slice(buffered_display.buffer());
}

fn handle_cli_command<I, D>(
    line: &str,
    cli: &SerialCli,
    fs: &mut impl FsCliOps,
    app: &mut App,
    display: &mut EinkDisplay<I>,
    delay: &mut D,
    buffered_display: &mut BufferedDisplay,
    last_buffer: &mut Vec<u8>,
) where
    I: DisplayInterface,
    D: embedded_hal::delay::DelayNs,
{
    let mut parts = line.split_whitespace();
    let cmd = parts.next().unwrap_or("");

    match cmd {
        "help" => {
            cli.write_line(
                "Commands: help, ls [path], exists <path>, stat <path>, rm <path>, rmdir <path>, mkdir/md <path>, cat <path>",
            );
            cli.write_line(
                "          put <path> <size> [chunk], refresh <full|partial|fast>, sleep",
            );
            cli.write_line("OK");
        }
        "ls" => {
            let path = parts.next().unwrap_or("/");
            match fs.list_files(path) {
                Ok(files) => {
                    for file in files {
                        let kind = if file.is_directory { "D" } else { "F" };
                        let name = if file.is_directory {
                            format!("{}/", file.name)
                        } else {
                            file.name
                        };
                        cli.write_line(&format!("{} {} {}", kind, name, format_size(file.size)));
                    }
                    cli.write_line("OK");
                }
                Err(err) => cli.write_line(&format!("ERR {:?}", err)),
            }
        }
        "exists" => {
            let path = parts.next().unwrap_or("/");
            let exists = fs.exists(path);
            cli.write_line(if exists { "1" } else { "0" });
            cli.write_line("OK");
        }
        "stat" => {
            let path = match parts.next() {
                Some(path) => path,
                None => {
                    cli.write_line("ERR missing path");
                    return;
                }
            };
            match fs.file_info(path) {
                Ok(info) => {
                    let kind = if info.is_directory { "dir" } else { "file" };
                    cli.write_line(&format!("{} {}", kind, info.size));
                    cli.write_line("OK");
                }
                Err(err) => cli.write_line(&format!("ERR {:?}", err)),
            }
        }
        "rm" => {
            let path = match parts.next() {
                Some(path) => path,
                None => {
                    cli.write_line("ERR missing path");
                    return;
                }
            };
            match fs.file_info(path) {
                Ok(info) => {
                    if info.is_directory {
                        cli.write_line("ERR use rmdir for directories");
                        return;
                    }
                }
                Err(err) => {
                    cli.write_line(&format!("ERR {:?}", err));
                    return;
                }
            }
            match fs.delete_file(path) {
                Ok(()) => {
                    app.invalidate_library_cache();
                    cli.write_line("OK");
                }
                Err(err) => cli.write_line(&format!("ERR {:?}", err)),
            }
        }
        "rmdir" => {
            let path = match parts.next() {
                Some(path) => path,
                None => {
                    cli.write_line("ERR missing path");
                    return;
                }
            };
            match fs.file_info(path) {
                Ok(info) => {
                    if !info.is_directory {
                        cli.write_line("ERR not a directory");
                        return;
                    }
                }
                Err(err) => {
                    cli.write_line(&format!("ERR {:?}", err));
                    return;
                }
            }
            match fs.delete_dir(path) {
                Ok(()) => {
                    app.invalidate_library_cache();
                    cli.write_line("OK");
                }
                Err(err) => cli.write_line(&format!("ERR {:?}", err)),
            }
        }
        "mkdir" | "md" => {
            let path = match parts.next() {
                Some(path) => path,
                None => {
                    cli.write_line("ERR missing path");
                    return;
                }
            };
            match fs.make_dir(path) {
                Ok(()) => {
                    app.invalidate_library_cache();
                    cli.write_line("OK");
                }
                Err(err) => cli.write_line(&format!("ERR {:?}", err)),
            }
        }
        "cat" => {
            let path = match parts.next() {
                Some(path) => path,
                None => {
                    cli.write_line("ERR missing path");
                    return;
                }
            };
            match fs.file_info(path) {
                Ok(info) => {
                    if info.size > 16 * 1024 {
                        cli.write_line("ERR file too large");
                        return;
                    }
                }
                Err(err) => {
                    cli.write_line(&format!("ERR {:?}", err));
                    return;
                }
            }
            match fs.read_file(path) {
                Ok(content) => {
                    cli.write_str(&content);
                    cli.write_line("");
                    cli.write_line("OK");
                }
                Err(err) => cli.write_line(&format!("ERR {:?}", err)),
            }
        }
        "put" => {
            let path = match parts.next() {
                Some(path) => path,
                None => {
                    cli.write_line("ERR missing path");
                    return;
                }
            };
            let size: usize = match parts.next().and_then(|value| value.parse().ok()) {
                Some(size) => size,
                None => {
                    cli.write_line("ERR missing size");
                    return;
                }
            };
            let chunk_size: usize = parts
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(1024);

            cli.write_line(&format!("OK READY {}", chunk_size));
            let mut hasher = crc32fast::Hasher::new();
            let res = fs.write_file_streamed(
                path,
                size,
                chunk_size,
                |buf| {
                    cli.read_exact(buf, 5000)?;
                    hasher.update(buf);
                    Ok(buf.len())
                },
                |written| {
                    cli.write_line(&format!("OK {}", written));
                    Ok(())
                },
            );

            if let Err(err) = res {
                cli.write_line(&format!("ERR {:?}", err));
                return;
            }

            let crc = hasher.finalize();
            app.invalidate_library_cache();
            cli.write_line(&format!("OK DONE {:08x}", crc));
        }
        "refresh" => {
            let mode = match parts.next().unwrap_or("fast") {
                "full" => RefreshMode::Full,
                "partial" => RefreshMode::Partial,
                _ => RefreshMode::Fast,
            };
            cli_redraw(app, display, delay, buffered_display, last_buffer, mode);
            cli.write_line("OK");
        }
        "sleep" => {
            cli.write_line("OK sleeping");
            enter_deep_sleep(3);
        }
        "" => {}
        _ => cli.write_line("ERR unknown command"),
    }
}

trait FsCliOps: FileSystem {
    fn delete_file(&mut self, path: &str) -> Result<(), xteink_ui::filesystem::FileSystemError>;
    fn delete_dir(&mut self, path: &str) -> Result<(), xteink_ui::filesystem::FileSystemError>;
    fn make_dir(&mut self, path: &str) -> Result<(), xteink_ui::filesystem::FileSystemError>;
    fn write_file_streamed<F, G>(
        &mut self,
        path: &str,
        total_size: usize,
        chunk_size: usize,
        read_chunk: F,
        on_progress: G,
    ) -> Result<(), xteink_ui::filesystem::FileSystemError>
    where
        F: FnMut(&mut [u8]) -> Result<usize, xteink_ui::filesystem::FileSystemError>,
        G: FnMut(usize) -> Result<(), xteink_ui::filesystem::FileSystemError>;
}

impl<SPI> FsCliOps for SdCardFs<SPI>
where
    SPI: embedded_hal::spi::SpiDevice,
{
    fn delete_file(&mut self, path: &str) -> Result<(), xteink_ui::filesystem::FileSystemError> {
        SdCardFs::delete_file(self, path)
    }

    fn delete_dir(&mut self, path: &str) -> Result<(), xteink_ui::filesystem::FileSystemError> {
        SdCardFs::delete_dir(self, path)
    }

    fn make_dir(&mut self, path: &str) -> Result<(), xteink_ui::filesystem::FileSystemError> {
        SdCardFs::make_dir(self, path)
    }

    fn write_file_streamed<F, G>(
        &mut self,
        path: &str,
        total_size: usize,
        chunk_size: usize,
        read_chunk: F,
        on_progress: G,
    ) -> Result<(), xteink_ui::filesystem::FileSystemError>
    where
        F: FnMut(&mut [u8]) -> Result<usize, xteink_ui::filesystem::FileSystemError>,
        G: FnMut(usize) -> Result<(), xteink_ui::filesystem::FileSystemError>,
    {
        SdCardFs::write_file_streamed(self, path, total_size, chunk_size, read_chunk, on_progress)
    }
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

    let region = match compute_diff_region(current, last, width_bytes, height) {
        Some(region) => region,
        None => {
            log::info!("UI: no pixel changes detected");
            return;
        }
    };

    let total_bytes = current.len();
    let region_bytes = region.byte_count();
    let free_heap = unsafe { esp_idf_svc::sys::esp_get_free_heap_size() as usize };

    // Diff updates need two scratch buffers (current + previous) and can fail
    // under heap pressure due to large temporary allocations.
    let estimated_scratch_need = region_bytes.saturating_mul(2);

    if region_bytes > total_bytes / 3
        || estimated_scratch_need > free_heap.saturating_sub(16 * 1024)
    {
        log::info!(
            "UI: using partial full-screen refresh (region={} free_heap={} need={})",
            region_bytes,
            free_heap,
            estimated_scratch_need
        );
        if display
            .update_with_mode_no_lut(current, &[], RefreshMode::Partial, delay)
            .is_err()
        {
            log::warn!("UI: full-screen partial refresh failed");
        }
        last.copy_from_slice(current);
        return;
    }

    extract_region(current, width_bytes, region, scratch);
    extract_region(last, width_bytes, region, scratch_prev);

    let update = UpdateRegion {
        region: Region::new(region.x_px(), region.y_px(), region.w_px(), region.h_px()),
        black_buffer: scratch,
        red_buffer: scratch_prev,
        mode: RefreshMode::Fast,
    };

    log::info!(
        "UI: region update x={} y={} w={} h={} bytes={}",
        region.x_px(),
        region.y_px(),
        region.w_px(),
        region.h_px(),
        region_bytes
    );

    if display.update_region_no_lut(update, delay).is_err() {
        log::warn!("UI: region update failed - falling back to partial");
        if display
            .update_with_mode_no_lut(current, &[], RefreshMode::Partial, delay)
            .is_err()
        {
            log::warn!("UI: fallback partial refresh failed");
        }
    }
    last.copy_from_slice(current);
}

fn apply_update<I, D>(
    strategy: UpdateStrategy,
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
    match strategy {
        UpdateStrategy::Full => {
            log::info!("UI: applying full refresh");
            if display
                .update_with_mode_no_lut(current, &[], RefreshMode::Full, delay)
                .is_err()
            {
                log::warn!("UI: full refresh failed");
            }
            last.copy_from_slice(current);
        }
        UpdateStrategy::PartialFull => {
            log::info!("UI: applying partial full-screen refresh");
            if display
                .update_with_mode_no_lut(current, &[], RefreshMode::Partial, delay)
                .is_err()
            {
                log::warn!("UI: partial full-screen refresh failed");
            }
            last.copy_from_slice(current);
        }
        UpdateStrategy::FastFull => {
            log::info!("UI: applying fast full-screen refresh");
            if display
                .update_with_mode_no_lut(current, &[], RefreshMode::Fast, delay)
                .is_err()
            {
                log::warn!("UI: fast full-screen refresh failed");
            }
            last.copy_from_slice(current);
        }
        UpdateStrategy::DiffFast => {
            update_display_diff(
                display,
                delay,
                current,
                last,
                scratch,
                scratch_prev,
                width_bytes,
                height,
            );
        }
    }
}

/// Apply display update using the new ActivityRefreshMode system.
///
/// Maps ActivityRefreshMode to the appropriate update strategy:
/// - Full: Full screen full refresh (highest quality, slowest)
/// - Partial: Full screen partial refresh (for ghost cleanup)
/// - Fast: Diff-based fast refresh (most efficient for small changes)
fn apply_update_with_mode<I, D>(
    mode: ActivityRefreshMode,
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
    match mode {
        ActivityRefreshMode::Full => {
            log::info!("UI: Activity requested full refresh");
            apply_update(
                UpdateStrategy::Full,
                display,
                delay,
                current,
                last,
                scratch,
                scratch_prev,
                width_bytes,
                height,
            );
        }
        ActivityRefreshMode::Partial => {
            log::info!("UI: Periodic partial refresh for ghost cleanup");
            apply_update(
                UpdateStrategy::PartialFull,
                display,
                delay,
                current,
                last,
                scratch,
                scratch_prev,
                width_bytes,
                height,
            );
        }
        ActivityRefreshMode::Fast => {
            // Use diff-based fast refresh for most efficient updates
            apply_update(
                UpdateStrategy::DiffFast,
                display,
                delay,
                current,
                last,
                scratch,
                scratch_prev,
                width_bytes,
                height,
            );
        }
    }
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
    log_heap("startup");

    // Stack size verification (runtime log)
    // std-enabled UI task handling needs additional headroom on ESP32-C3.
    const REQUIRED_STACK_SIZE: u32 = 80 * 1024; // 80KB minimum
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
    let mut last_buffer: Vec<u8> = vec![0xFF; buffered_display.buffer().len()];
    let mut region_scratch: Vec<u8> = Vec::new();
    let mut region_scratch_prev: Vec<u8> = Vec::new();

    // Initialize SD card filesystem.
    // Boot must remain usable even when SD card is absent or mount fails.
    let mut fs = match SdCardFs::new(sd_spi) {
        Ok(fs) => fs,
        Err(err) => {
            log::warn!("SD card mount failed: {}", err);
            SdCardFs::unavailable(err.to_string())
        }
    };

    // Initialize app and render initial screen
    let mut app = App::new();
    log_heap("before_app_init");
    buffered_display.clear();
    app.render(&mut buffered_display).ok();
    log_heap("before_first_render");
    display
        .update(buffered_display.buffer(), &[], &mut delay)
        .ok();
    log_heap("after_first_render");
    last_buffer.copy_from_slice(buffered_display.buffer());

    log::info!("Starting event loop with adaptive refresh strategy");

    log::info!("Starting event loop... Press a button!");
    log::info!("Hold POWER for 2 seconds to sleep...");
    log::info!("CLI: connect via USB-Serial/JTAG @ 115200 (type 'help')");

    let mut last_button: Option<Button> = None;
    let mut power_press_counter: u32 = 0;
    let mut is_power_pressed: bool = false;
    let mut long_press_triggered: bool = false;
    const DEBUG_ADC: bool = false;
    const DEBUG_INPUT: bool = true;
    const POWER_LONG_PRESS_ITERATIONS: u32 = POWER_LONG_PRESS_MS / 50;
    const ENABLE_CLI: bool = false;
    let mut cli = if ENABLE_CLI {
        Some(SerialCli::new())
    } else {
        None
    };
    let mut input_debug_ticks: u32 = 0;

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
                    &mut last_buffer,
                );
            }
        }

        let (button, power_pressed) = read_buttons(&mut power_btn, DEBUG_ADC);
        if DEBUG_INPUT {
            input_debug_ticks = input_debug_ticks.saturating_add(1);
            if input_debug_ticks >= 10 {
                input_debug_ticks = 0;
                let adc1_value = read_adc(sys::adc_channel_t_ADC_CHANNEL_1);
                let adc2_value = read_adc(sys::adc_channel_t_ADC_CHANNEL_2);
                log::info!(
                    "INPUT: power={} adc1={} adc2={} decoded={:?} last={:?}",
                    power_pressed,
                    adc1_value,
                    adc2_value,
                    button,
                    last_button
                );
            }
        }

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

                    if app.handle_input(InputEvent::Press(Button::Power)) {
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

                if app.handle_input(InputEvent::Press(btn)) {
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
                        &mut last_buffer,
                        &mut region_scratch,
                        &mut region_scratch_prev,
                        buffered_display.width_bytes(),
                        buffered_display.height_pixels() as usize,
                    );
                    log_heap("after_render");
                } else {
                    log::info!("UI: no redraw after {:?}", btn);
                }
            }
        } else if !power_pressed {
            last_button = None;
        }

        if app.process_deferred_tasks(&mut fs) {
            buffered_display.clear();
            app.render(&mut buffered_display).ok();
            let refresh_mode = app.get_refresh_mode();
            apply_update_with_mode(
                refresh_mode,
                &mut display,
                &mut delay,
                buffered_display.buffer(),
                &mut last_buffer,
                &mut region_scratch,
                &mut region_scratch_prev,
                buffered_display.width_bytes(),
                buffered_display.height_pixels() as usize,
            );
        }

        FreeRtos::delay_ms(50);
    }
}
