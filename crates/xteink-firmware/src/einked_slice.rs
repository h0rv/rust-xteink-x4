use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use embedded_graphics::{
    mono_font::{ascii, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};
use std::boxed::Box;

use einked::core::Color;
use einked::input::InputEvent;
use einked::refresh::RefreshHint;
use einked::render_ir::DrawCmd;
use einked::storage::{FileStore, FileStoreError, SettingsStore};
#[cfg(not(feature = "minireader-ui"))]
use einked_ereader::{
    DeviceConfig as ActiveConfig, EreaderRuntime as ActiveRuntime, FeedClient, FeedEntryData,
    FeedType, FrameSink,
};
#[cfg(feature = "minireader-ui")]
use einked_minireader::{
    FrameSink, MiniReaderConfig as ActiveConfig, MiniReaderRuntime as ActiveRuntime,
};
use ssd1677::{Display as EinkDisplay, DisplayInterface, RefreshMode};
use std::io::Read;
use std::path::PathBuf;

use crate::buffered_display::BufferedDisplay;
#[cfg(not(feature = "minireader-ui"))]
use crate::feed_service::FeedService;

pub struct EinkedSlice {
    runtime: Box<ActiveRuntime>,
}

const SETTING_KEY_WIFI_ACTIVE: u8 = 240;
const SETTING_KEY_WIFI_ENABLE_REQUEST: u8 = 241;
const SETTING_KEY_BATTERY_PERCENT: u8 = 242;
static WIFI_ACTIVE: AtomicU8 = AtomicU8::new(0);
static WIFI_ENABLE_REQUESTED: AtomicBool = AtomicBool::new(false);
static BATTERY_PERCENT: AtomicU8 = AtomicU8::new(100);

pub fn set_wifi_active(active: bool) {
    WIFI_ACTIVE.store(if active { 1 } else { 0 }, Ordering::Relaxed);
}

pub fn take_wifi_enable_request() -> bool {
    WIFI_ENABLE_REQUESTED.swap(false, Ordering::Relaxed)
}

pub fn set_battery_percent(percent: u8) {
    BATTERY_PERCENT.store(percent.min(100), Ordering::Relaxed);
}

impl EinkedSlice {
    pub fn new() -> Self {
        FIRST_NON_EMPTY_FRAME_PENDING.store(true, Ordering::Relaxed);
        #[cfg(not(feature = "minireader-ui"))]
        log::info!("[UI] runtime=einked-ereader");
        #[cfg(feature = "minireader-ui")]
        log::info!("[UI] runtime=einked-minireader");
        #[cfg(not(feature = "minireader-ui"))]
        let runtime = ActiveRuntime::with_backends_and_feed(
            ActiveConfig::xteink_x4(),
            Box::new(FirmwareSettings::default()),
            Box::new(FirmwareFiles::new("/sd".to_string())),
            Box::new(FirmwareFeedClient::default()),
        );
        #[cfg(feature = "minireader-ui")]
        let runtime = ActiveRuntime::with_backends(
            ActiveConfig::xteink_x4(),
            Box::new(FirmwareSettings::default()),
            Box::new(FirmwareFiles::new("/sd".to_string())),
        );
        Self {
            runtime: Box::new(runtime),
        }
    }

    pub fn tick_and_flush<I, D>(
        &mut self,
        input: Option<InputEvent>,
        display: &mut EinkDisplay<I>,
        delay: &mut D,
        buffered_display: &mut BufferedDisplay,
    ) -> bool
    where
        I: DisplayInterface,
        D: embedded_hal::delay::DelayNs,
    {
        let mut sink = FirmwareSink {
            display,
            delay,
            buffered_display,
        };
        self.runtime.tick(input, &mut sink)
    }
}

impl Default for EinkedSlice {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "minireader-ui"))]
#[derive(Default)]
struct FirmwareFeedClient {
    service: Option<FeedService>,
}

#[cfg(not(feature = "minireader-ui"))]
impl FirmwareFeedClient {
    fn service(&mut self) -> Result<&mut FeedService, String> {
        if self.service.is_none() {
            self.service =
                Some(FeedService::new().map_err(|e| format!("Feed service init failed: {:?}", e))?);
        }
        self.service
            .as_mut()
            .ok_or_else(|| "Feed service unavailable".to_string())
    }
}

#[cfg(not(feature = "minireader-ui"))]
impl FeedClient for FirmwareFeedClient {
    fn fetch_entries(
        &mut self,
        _source_name: &str,
        source_url: &str,
        source_type: FeedType,
    ) -> Result<Vec<FeedEntryData>, String> {
        let service = self.service()?;
        service
            .fetch_entries(source_url, source_type)
            .map_err(|e| format!("Feed fetch failed: {:?}", e))
    }

    fn fetch_article_lines(&mut self, url: &str) -> Result<Vec<String>, String> {
        let service = self.service()?;
        let text: String = service
            .fetch_article_text(url)
            .map_err(|e| format!("Article fetch failed: {:?}", e))?;
        let mut lines: Vec<String> = Vec::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                lines.push(String::new());
            } else {
                lines.push(trimmed.to_string());
            }
        }
        if lines.is_empty() {
            Err("Article had no readable text.".to_string())
        } else {
            Ok(lines)
        }
    }
}

struct FirmwareSettings {
    slots: [u8; 64],
}

impl Default for FirmwareSettings {
    fn default() -> Self {
        Self { slots: [0; 64] }
    }
}

impl SettingsStore for FirmwareSettings {
    fn load_raw(&self, key: u8, buf: &mut [u8]) -> usize {
        if buf.is_empty() {
            return 0;
        }
        if key == SETTING_KEY_WIFI_ACTIVE {
            buf[0] = WIFI_ACTIVE.load(Ordering::Relaxed);
            return 1;
        }
        if key == SETTING_KEY_BATTERY_PERCENT {
            buf[0] = BATTERY_PERCENT.load(Ordering::Relaxed);
            return 1;
        }
        let idx = key as usize;
        if idx >= self.slots.len() {
            return 0;
        }
        buf[0] = self.slots[idx];
        1
    }

    fn save_raw(&mut self, key: u8, data: &[u8]) {
        if key == SETTING_KEY_WIFI_ENABLE_REQUEST {
            if !data.is_empty() && data[0] != 0 {
                WIFI_ENABLE_REQUESTED.store(true, Ordering::Relaxed);
            }
            return;
        }
        let idx = key as usize;
        if idx < self.slots.len() && !data.is_empty() {
            self.slots[idx] = data[0];
        }
    }
}

struct FirmwareFiles {
    root: String,
}

impl FirmwareFiles {
    fn new(root: String) -> Self {
        Self { root }
    }

    fn resolve(&self, path: &str) -> PathBuf {
        if path == "/" || path.is_empty() {
            return PathBuf::from(&self.root);
        }
        let trimmed = path.trim_start_matches('/');
        PathBuf::from(&self.root).join(trimmed)
    }
}

impl FileStore for FirmwareFiles {
    fn list(&self, path: &str, out: &mut dyn FnMut(&str)) {
        let dir = self.resolve(path);
        if let Ok(read_dir) = std::fs::read_dir(dir) {
            for entry in read_dir.flatten() {
                let name = entry.file_name();
                if let Some(name) = name.to_str() {
                    out(name);
                }
            }
        }
    }

    fn is_dir(&self, path: &str) -> Option<bool> {
        std::fs::metadata(self.resolve(path))
            .ok()
            .map(|metadata| metadata.is_dir())
    }

    fn read<'a>(&self, path: &str, buf: &'a mut [u8]) -> Result<&'a [u8], FileStoreError> {
        let full = self.resolve(path);
        let mut file = std::fs::File::open(full).map_err(|_| FileStoreError::Io)?;
        let n = file.read(buf).map_err(|_| FileStoreError::Io)?;
        Ok(&buf[..n])
    }

    fn exists(&self, path: &str) -> bool {
        self.resolve(path).exists()
    }

    fn open_read_seek(
        &self,
        path: &str,
    ) -> Result<Box<dyn einked::storage::ReadSeek>, FileStoreError> {
        let full = self.resolve(path);
        let file = std::fs::File::open(full).map_err(|_| FileStoreError::Io)?;
        Ok(Box::new(file))
    }

    fn native_path(&self, path: &str) -> Option<String> {
        self.resolve(path).to_str().map(|value| value.to_string())
    }
}

struct FirmwareSink<'a, I: DisplayInterface, D> {
    display: &'a mut EinkDisplay<I>,
    delay: &'a mut D,
    buffered_display: &'a mut BufferedDisplay,
}

static FIRST_NON_EMPTY_FRAME_PENDING: AtomicBool = AtomicBool::new(true);

impl<I, D> FrameSink for FirmwareSink<'_, I, D>
where
    I: DisplayInterface,
    D: embedded_hal::delay::DelayNs,
{
    fn render_and_flush(&mut self, cmds: &[DrawCmd<'static>], hint: RefreshHint) -> bool {
        if cmds.is_empty() {
            return true;
        }
        rasterize_commands(cmds, self.buffered_display);
        let hint_mode = match hint {
            RefreshHint::Full => RefreshMode::Full,
            RefreshHint::Fast => RefreshMode::Fast,
            RefreshHint::Adaptive | RefreshHint::Partial => RefreshMode::Partial,
        };
        let force_full = FIRST_NON_EMPTY_FRAME_PENDING.load(Ordering::Relaxed);
        let mode = if force_full {
            RefreshMode::Full
        } else {
            hint_mode
        };
        match self.display.update_with_mode_no_lut(
            self.buffered_display.buffer(),
            &[],
            mode,
            self.delay,
        ) {
            Ok(()) => {
                if force_full {
                    FIRST_NON_EMPTY_FRAME_PENDING.store(false, Ordering::Relaxed);
                }
                true
            }
            Err(_) => {
                log::warn!(
                    "[EINKED] display update_with_mode_no_lut failed mode={:?}",
                    mode
                );
                false
            }
        }
    }
}

fn rasterize_commands(cmds: &[DrawCmd<'static>], buffered_display: &mut BufferedDisplay) {
    buffered_display.clear();

    for cmd in cmds {
        match cmd {
            DrawCmd::FillRect { rect, color } => {
                let draw_color = to_binary(*color);
                let _ = Rectangle::new(
                    Point::new(rect.x as i32, rect.y as i32),
                    Size::new(rect.width as u32, rect.height as u32),
                )
                .into_styled(PrimitiveStyle::with_fill(draw_color))
                .draw(buffered_display);
            }
            DrawCmd::DrawText { pos, text, .. } => {
                let style = MonoTextStyleBuilder::new()
                    .font(&ascii::FONT_8X13_BOLD)
                    .text_color(BinaryColor::On)
                    .build();
                let _ = Text::new(text.as_str(), Point::new(pos.x as i32, pos.y as i32), style)
                    .draw(buffered_display);
            }
            DrawCmd::DrawLine {
                start, end, color, ..
            } => {
                let min_x = start.x.min(end.x);
                let max_x = start.x.max(end.x);
                let min_y = start.y.min(end.y);
                let max_y = start.y.max(end.y);
                let _ = Rectangle::new(
                    Point::new(min_x as i32, min_y as i32),
                    Size::new((max_x - min_x + 1) as u32, (max_y - min_y + 1) as u32),
                )
                .into_styled(PrimitiveStyle::with_fill(to_binary(*color)))
                .draw(buffered_display);
            }
            DrawCmd::DrawImage { .. } | DrawCmd::Clip { .. } | DrawCmd::Unclip => {}
        }
    }
}

fn to_binary(color: Color) -> BinaryColor {
    match color {
        Color::Black => BinaryColor::On,
        Color::White => BinaryColor::Off,
        Color::Gray(v) => {
            if v < 128 {
                BinaryColor::On
            } else {
                BinaryColor::Off
            }
        }
        Color::Red | Color::Custom(_) => BinaryColor::On,
    }
}
