use alloc::boxed::Box;
use alloc::format;
use core::convert::Infallible;

use embedded_graphics::{
    mono_font::{ascii, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};

use einked::activity_stack::{Activity, ActivityStack, Context, Transition, Ui};
use einked::core::{Color, DefaultTheme, Rect};
use einked::dsl::UiDsl;
use einked::input::{Button, InputEvent};
use einked::pipeline::FramePipeline;
use einked::refresh::{EinkDisplay as EinkedDisplay, RefreshHint};
use einked::render_ir::DrawCmd;
use einked::storage::{FileStore, FileStoreError, SettingsStore};
use einked::ui::components::Header;
use einked::ui::runtime::UiRuntime;

use xteink_ui::{BufferedDisplay, DisplayInterface, EinkDisplay, RefreshMode};

const WIDTH: u16 = 480;
const HEIGHT: u16 = 800;

pub struct EinkedSlice {
    stack: ActivityStack<DefaultTheme, 8>,
    pipeline: FramePipeline<512, 512>,
    theme: DefaultTheme,
    settings: DummySettings,
    files: DummyFiles,
}

impl EinkedSlice {
    pub fn new() -> Self {
        let mut stack = ActivityStack::new();
        let theme = DefaultTheme;
        let mut settings = DummySettings::default();
        let mut files = DummyFiles;
        let mut ctx = Context {
            theme: &theme,
            screen: Rect {
                x: 0,
                y: 0,
                width: WIDTH,
                height: HEIGHT,
            },
            settings: &mut settings,
            files: &mut files,
        };
        let _ = stack.push_root(Box::new(DemoListActivity::new()), &mut ctx);
        let mut pipeline = FramePipeline::new(8);
        pipeline.set_viewport_width(WIDTH);

        Self {
            stack,
            pipeline,
            theme,
            settings,
            files,
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
        let mut ctx = Context {
            theme: &self.theme,
            screen: Rect {
                x: 0,
                y: 0,
                width: WIDTH,
                height: HEIGHT,
            },
            settings: &mut self.settings,
            files: &mut self.files,
        };

        let hint;
        {
            let runtime = self.pipeline.begin_frame();
            let mut ui = RuntimeUi { runtime };
            let alive = self.stack.tick(input, &mut ui, &mut ctx);
            if !alive {
                return false;
            }
            Header::new("xteink")
                .with_right_text("einked")
                .render_to_runtime(&mut ui.runtime);
            hint = ui.runtime.take_refresh_hint();
        }

        rasterize_commands(self.pipeline.current_commands(), buffered_display);

        let mut display_adapter = FirmwareDisplayAdapter {
            display,
            delay,
            frame: buffered_display.buffer(),
        };

        self.pipeline.end_frame(&mut display_adapter, hint).is_ok()
    }
}

impl Default for EinkedSlice {
    fn default() -> Self {
        Self::new()
    }
}

struct RuntimeUi<'a> {
    runtime: UiRuntime<'a, 512>,
}

impl Ui<DefaultTheme> for RuntimeUi<'_> {
    fn clear(&mut self, _theme: &DefaultTheme) {}

    fn label(&mut self, text: &str) {
        self.runtime.label(text);
    }

    fn paragraph(&mut self, text: &str) {
        self.runtime.paragraph(text);
    }

    fn divider(&mut self) {
        self.runtime.draw_divider();
    }

    fn status_bar(&mut self, left: &str, right: &str) {
        self.runtime.draw_status_bar(left, right);
    }

    fn set_refresh_hint(&mut self, hint: RefreshHint) {
        self.runtime.set_refresh_hint(hint);
    }
}

#[derive(Default)]
struct DummySettings {
    slots: [u8; 32],
}

impl SettingsStore for DummySettings {
    fn load_raw(&self, key: u8, buf: &mut [u8]) -> usize {
        let idx = key as usize;
        if idx >= self.slots.len() || buf.is_empty() {
            return 0;
        }
        buf[0] = self.slots[idx];
        1
    }

    fn save_raw(&mut self, key: u8, data: &[u8]) {
        let idx = key as usize;
        if idx < self.slots.len() && !data.is_empty() {
            self.slots[idx] = data[0];
        }
    }
}

struct DummyFiles;

impl FileStore for DummyFiles {
    fn list(&self, _path: &str, _out: &mut dyn FnMut(&str)) {}

    fn read<'a>(&self, _path: &str, _buf: &'a mut [u8]) -> Result<&'a [u8], FileStoreError> {
        Err(FileStoreError::Io)
    }

    fn exists(&self, _path: &str) -> bool {
        false
    }
}

struct DemoListActivity {
    selected: usize,
}

impl DemoListActivity {
    fn new() -> Self {
        Self { selected: 0 }
    }

    const ITEMS: [&'static str; 4] = [
        "Open Detail",
        "Scheduler: Partial",
        "Scheduler: Fast",
        "Keep Reading",
    ];
}

impl Activity<DefaultTheme> for DemoListActivity {
    fn on_input(
        &mut self,
        event: InputEvent,
        _ctx: &mut Context<'_, DefaultTheme>,
    ) -> Transition<DefaultTheme> {
        match event {
            InputEvent::Press(Button::Up) | InputEvent::Press(Button::Aux1) => {
                if self.selected == 0 {
                    self.selected = Self::ITEMS.len() - 1;
                } else {
                    self.selected -= 1;
                }
                Transition::Stay
            }
            InputEvent::Press(Button::Down) | InputEvent::Press(Button::Aux2) => {
                self.selected = (self.selected + 1) % Self::ITEMS.len();
                Transition::Stay
            }
            InputEvent::Press(Button::Confirm) => {
                if self.selected == 0 {
                    Transition::Push(Box::new(DetailActivity { ticks: 0 }))
                } else {
                    Transition::Stay
                }
            }
            _ => Transition::Stay,
        }
    }

    fn render(&self, ui_ctx: &mut dyn Ui<DefaultTheme>) {
        ui_ctx.status_bar("einked v1", "firmware");
        ui_ctx.divider();
        ui_ctx.paragraph("Input -> ActivityStack -> CmdBuffer -> RefreshScheduler");
        let selected = format!("selected: {}", self.selected);
        ui_ctx.label(&selected);
        for item in Self::ITEMS {
            ui_ctx.label(item);
        }
        ui_ctx.set_refresh_hint(RefreshHint::Fast);
        ui_ctx.paragraph("Use Up/Down/Aux, Confirm, Back");
    }

    fn refresh_hint(&self) -> RefreshHint {
        match self.selected {
            1 => RefreshHint::Partial,
            2 => RefreshHint::Fast,
            _ => RefreshHint::Adaptive,
        }
    }
}

struct DetailActivity {
    ticks: u32,
}

impl Activity<DefaultTheme> for DetailActivity {
    fn on_input(
        &mut self,
        event: InputEvent,
        _ctx: &mut Context<'_, DefaultTheme>,
    ) -> Transition<DefaultTheme> {
        match event {
            InputEvent::Press(Button::Back) => Transition::Pop,
            _ => {
                self.ticks = self.ticks.saturating_add(1);
                Transition::Stay
            }
        }
    }

    fn render(&self, ui_ctx: &mut dyn Ui<DefaultTheme>) {
        ui_ctx.status_bar("detail", "Back");
        ui_ctx.divider();
        let body = format!("Detail activity ticks={}", self.ticks);
        ui_ctx.paragraph(&body);
        ui_ctx.set_refresh_hint(RefreshHint::Full);
        ui_ctx.paragraph("Back pops this activity");
    }

    fn refresh_hint(&self) -> RefreshHint {
        RefreshHint::Adaptive
    }
}

struct FirmwareDisplayAdapter<'a, I: DisplayInterface, D> {
    display: &'a mut EinkDisplay<I>,
    delay: &'a mut D,
    frame: &'a [u8],
}

impl<I, D> EinkedDisplay for FirmwareDisplayAdapter<'_, I, D>
where
    I: DisplayInterface,
    D: embedded_hal::delay::DelayNs,
{
    type Error = Infallible;

    fn full_refresh(&mut self) -> Result<(), Self::Error> {
        let _ =
            self.display
                .update_with_mode_no_lut(self.frame, &[], RefreshMode::Full, self.delay);
        Ok(())
    }

    fn partial_refresh(&mut self, _region: Rect) -> Result<(), Self::Error> {
        let _ =
            self.display
                .update_with_mode_no_lut(self.frame, &[], RefreshMode::Partial, self.delay);
        Ok(())
    }

    fn fast_refresh(&mut self, _region: Rect) -> Result<(), Self::Error> {
        let _ =
            self.display
                .update_with_mode_no_lut(self.frame, &[], RefreshMode::Fast, self.delay);
        Ok(())
    }

    fn width(&self) -> u16 {
        WIDTH
    }

    fn height(&self) -> u16 {
        HEIGHT
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
