use embedded_graphics::{
    mono_font::{ascii, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};

use einked::core::{Color, Rect};
use einked::input::InputEvent;
use einked::refresh::RefreshHint;
use einked::render_ir::DrawCmd;
use einked_ereader::{EreaderRuntime, FrameSink};
use ssd1677::{Display as EinkDisplay, DisplayInterface, RefreshMode};

use crate::buffered_display::BufferedDisplay;

const WIDTH: u16 = 480;
const HEIGHT: u16 = 800;

pub struct EinkedSlice {
    runtime: EreaderRuntime,
}

impl EinkedSlice {
    pub fn new() -> Self {
        Self {
            runtime: EreaderRuntime::new(Rect {
                x: 0,
                y: 0,
                width: WIDTH,
                height: HEIGHT,
            }),
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

struct FirmwareSink<'a, I: DisplayInterface, D> {
    display: &'a mut EinkDisplay<I>,
    delay: &'a mut D,
    buffered_display: &'a mut BufferedDisplay,
}

impl<I, D> FrameSink for FirmwareSink<'_, I, D>
where
    I: DisplayInterface,
    D: embedded_hal::delay::DelayNs,
{
    fn render_and_flush(&mut self, cmds: &[DrawCmd<'static>], hint: RefreshHint) -> bool {
        rasterize_commands(cmds, self.buffered_display);
        let mode = match hint {
            RefreshHint::Full => RefreshMode::Full,
            RefreshHint::Fast => RefreshMode::Fast,
            RefreshHint::Adaptive | RefreshHint::Partial => RefreshMode::Partial,
        };
        self.display
            .update_with_mode_no_lut(self.buffered_display.buffer(), &[], mode, self.delay)
            .is_ok()
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
