//! Test display that allows pixel overdraw.
//!
//! `MockDisplay` from embedded-graphics panics when a pixel is drawn twice,
//! which doesn't work for activities that clear backgrounds then draw on top.
//! This simple framebuffer display allows overdraw for render smoke-tests.

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

/// Simple framebuffer display for tests that allows overdraw.
pub struct TestDisplay {
    pixels: Vec<BinaryColor>,
    width: u32,
    height: u32,
}

impl TestDisplay {
    /// Create a new test display with the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            pixels: vec![BinaryColor::Off; (width * height) as usize],
            width,
            height,
        }
    }

    /// Create a display matching the Xteink X4 dimensions (480x800).
    pub fn default_size() -> Self {
        Self::new(crate::DISPLAY_WIDTH, crate::DISPLAY_HEIGHT)
    }
}

impl DrawTarget for TestDisplay {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels {
            if coord.x >= 0
                && coord.y >= 0
                && (coord.x as u32) < self.width
                && (coord.y as u32) < self.height
            {
                let idx = (coord.y as u32 * self.width + coord.x as u32) as usize;
                self.pixels[idx] = color;
            }
        }
        Ok(())
    }
}

impl OriginDimensions for TestDisplay {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

    #[test]
    fn test_display_allows_overdraw() {
        let mut display = TestDisplay::new(10, 10);

        // Draw white background
        Rectangle::new(Point::new(0, 0), Size::new(10, 10))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
            .draw(&mut display)
            .unwrap();

        // Draw black on top - should not panic
        Rectangle::new(Point::new(0, 0), Size::new(5, 5))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(&mut display)
            .unwrap();
    }

    #[test]
    fn test_display_default_size() {
        let display = TestDisplay::default_size();
        assert_eq!(display.size(), Size::new(480, 800));
    }
}
