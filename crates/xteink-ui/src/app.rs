//! Main application state.

use embedded_graphics::{
    image::{Image, ImageRaw, ImageRawBE},
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};

use crate::input::{Button, InputEvent};

/// Application state
pub struct App {
    label: &'static str,
}

impl App {
    pub fn new() -> Self {
        Self {
            label: "No button pressed.",
        }
    }

    /// Handle input. Returns true if redraw needed.
    pub fn handle_input(&mut self, event: InputEvent) -> bool {
        let InputEvent::Press(btn) = event;

        self.label = match btn {
            Button::Left => "Left button pressed!",
            Button::Right => "Right button pressed!",
            Button::Confirm => "Confirm button pressed!",
            Button::Back => "Back button pressed!",
            Button::VolumeUp => "Volume up button pressed!",
            Button::VolumeDown => "Volume down button pressed!",
            Button::Power => "Power button pressed!",
        };

        true
    }

    /// Render to any display.
    pub fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        display.clear(BinaryColor::Off)?;

        let style = PrimitiveStyle::with_fill(BinaryColor::On);

        Rectangle::new(Point::new(100, 100), Size::new(50, 100))
            .into_styled(style)
            .draw(display)?;

        let data: &[u8] = include_bytes!("ferris_small.raw");

        // Create a raw image instance. Other image formats will require different code to load them.
        // All code after loading is the same for any image format.
        let raw: ImageRawBE<BinaryColor> = ImageRaw::new(data, 115);

        // Create an `Image` object to position the image at `Point::zero()`.
        let image = Image::new(&raw, Point::new(200, 200));

        // Draw the image to the display.
        image.draw(display)?;

        let text_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        Text::new(self.label, Point::new(50, 50), text_style).draw(display)?;

        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
