//! Main application state.

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};

use crate::input::{Button, InputEvent};
use crate::{DISPLAY_HEIGHT, DISPLAY_WIDTH};

/// Application state
pub struct App {
    cursor_x: i32,
    cursor_y: i32,
}

impl App {
    pub fn new() -> Self {
        Self {
            cursor_x: (DISPLAY_WIDTH / 2) as i32,
            cursor_y: (DISPLAY_HEIGHT / 2) as i32,
        }
    }

    /// Handle input. Returns true if redraw needed.
    pub fn handle_input(&mut self, event: InputEvent) -> bool {
        let InputEvent::Press(btn) = event;
        let step = 20;

        match btn {
            Button::Left => self.cursor_x -= step,
            Button::Right => self.cursor_x += step,
            Button::Up => self.cursor_y -= step,
            Button::Down => self.cursor_y += step,
            _ => return false,
        }

        self.cursor_x = self.cursor_x.clamp(15, (DISPLAY_WIDTH - 15) as i32);
        self.cursor_y = self.cursor_y.clamp(15, (DISPLAY_HEIGHT - 15) as i32);
        true
    }

    /// Render to any display.
    pub fn render<D: DrawTarget<Color = BinaryColor>>(&self, display: &mut D) -> Result<(), D::Error> {
        display.clear(BinaryColor::Off)?;

        let style = PrimitiveStyle::with_fill(BinaryColor::On);

        // Crosshair
        Rectangle::new(Point::new(self.cursor_x - 15, self.cursor_y - 1), Size::new(30, 3))
            .into_styled(style)
            .draw(display)?;
        Rectangle::new(Point::new(self.cursor_x - 1, self.cursor_y - 15), Size::new(3, 30))
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
