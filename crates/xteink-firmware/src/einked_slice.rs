use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
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
use einked_ereader::{DeviceConfig, EreaderRuntime, FrameSink};
use ssd1677::{Display as EinkDisplay, DisplayInterface, RefreshMode};
use std::io::Read;
use std::path::PathBuf;

use crate::buffered_display::BufferedDisplay;

pub struct EinkedSlice {
    runtime: Box<EreaderRuntime>,
}

const SETTING_KEY_WIFI_ACTIVE: u8 = 240;
const SETTING_KEY_WIFI_ENABLE_REQUEST: u8 = 241;
static WIFI_ACTIVE: AtomicU8 = AtomicU8::new(0);
static WIFI_ENABLE_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn set_wifi_active(active: bool) {
    WIFI_ACTIVE.store(if active { 1 } else { 0 }, Ordering::Relaxed);
}

pub fn take_wifi_enable_request() -> bool {
    WIFI_ENABLE_REQUESTED.swap(false, Ordering::Relaxed)
}

impl EinkedSlice {
    pub fn new() -> Self {
        Self {
            runtime: Box::new(EreaderRuntime::with_backends(
                DeviceConfig::xteink_x4(),
                Box::new(FirmwareSettings::default()),
                Box::new(FirmwareFiles::new("/sd".to_string())),
            )),
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

    fn read<'a>(&self, path: &str, buf: &'a mut [u8]) -> Result<&'a [u8], FileStoreError> {
        let full = self.resolve(path);
        let mut file = std::fs::File::open(full).map_err(|_| FileStoreError::Io)?;
        let n = file.read(buf).map_err(|_| FileStoreError::Io)?;
        Ok(&buf[..n])
    }

    fn exists(&self, path: &str) -> bool {
        self.resolve(path).exists()
    }
}

struct FirmwareSink<'a, I: DisplayInterface, D> {
    display: &'a mut EinkDisplay<I>,
    delay: &'a mut D,
    buffered_display: &'a mut BufferedDisplay,
}

static FLUSH_SEQ: AtomicU32 = AtomicU32::new(0);

impl<I, D> FrameSink for FirmwareSink<'_, I, D>
where
    I: DisplayInterface,
    D: embedded_hal::delay::DelayNs,
{
    fn render_and_flush(&mut self, cmds: &[DrawCmd<'static>], hint: RefreshHint) -> bool {
        let seq = FLUSH_SEQ.fetch_add(1, Ordering::Relaxed);
        if seq < 24 || cmds.is_empty() {
            log::warn!(
                "[EINKED] flush seq={} cmds={} hint={:?}",
                seq,
                cmds.len(),
                hint
            );
        }

        if cmds.is_empty() {
            return true;
        }
        rasterize_commands(cmds, self.buffered_display);
        let mode = match hint {
            RefreshHint::Full => RefreshMode::Full,
            RefreshHint::Fast => RefreshMode::Fast,
            RefreshHint::Adaptive | RefreshHint::Partial => RefreshMode::Partial,
        };
        let mode = if seq == 0 { RefreshMode::Full } else { mode };
        match self.display.update_with_mode_no_lut(
            self.buffered_display.buffer(),
            &[],
            mode,
            self.delay,
        ) {
            Ok(()) => true,
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
