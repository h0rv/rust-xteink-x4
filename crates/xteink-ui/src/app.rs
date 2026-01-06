//! Main application state.

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};

use crate::input::{Button, InputEvent};
use crate::{DISPLAY_HEIGHT, DISPLAY_WIDTH};

/// Application state
pub struct App {}

impl App {
    pub fn new() -> Self {
        Self {}
    }

    /// Handle input. Returns true if redraw needed.
    pub fn handle_input(&mut self, event: InputEvent) -> bool {
        let InputEvent::Press(btn) = event;

        match btn {
            Button::Left => (),
            Button::Right => (),
            Button::VolumeUp => (),
            Button::VolumeDown => (),
            _ => return false,
        }

        true
    }

    /// Render to any display.
    pub fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        display.clear(BinaryColor::Off)?;

        let style = PrimitiveStyle::with_fill(BinaryColor::On);

        Rectangle::new(Point::new(100, 100), Size::new(50, 50))
            .into_styled(style)
            .draw(display)?;

        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
