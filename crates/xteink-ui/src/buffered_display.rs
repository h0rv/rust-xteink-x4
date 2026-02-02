//! Simple buffered display for Xteink X4
//!
//! Buffer for SSD1677 driver with 480x800 dimensions (rows x cols).
//! The driver validation requires: rows <= 680, cols <= 960, cols % 8 == 0
//! For our 800x480 panel mounted in portrait: rows=480, cols=800
//!
//! Buffer format: Each byte represents 8 horizontal pixels (MSB = leftmost).
//! Row stride: 100 bytes (800 pixels / 8 bits per byte).
//! Total size: 100 * 480 = 48,000 bytes.

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

/// Simple frame buffer display for SSD1677
///
/// Buffer dimensions (matching driver requirements):
/// - 480 rows (gates, must be <= 680 for SSD1677)
/// - 800 columns (sources, must be <= 960 and multiple of 8)
/// - 100 bytes per row (800 pixels / 8 bits per byte)
///
/// The SSD1677 driver is configured with Rotate90 to handle the physical
/// portrait mounting of the panel.
pub struct BufferedDisplay {
    buffer: Vec<u8>,
}

impl BufferedDisplay {
    /// Buffer dimensions (480 rows x 800 cols for driver compatibility)
    const WIDTH: u32 = 800; // columns (sources)
    const HEIGHT: u32 = 480; // rows (gates)
    const WIDTH_BYTES: usize = 100; // 800 / 8
    const BUFFER_SIZE: usize = 100 * 480; // 48KB

    /// Create new buffered display initialized to white
    pub fn new() -> Self {
        Self {
            buffer: vec![0xFF; Self::BUFFER_SIZE],
        }
    }

    /// Clear buffer to white (all bits set)
    pub fn clear(&mut self) {
        self.buffer.fill(0xFF);
    }

    /// Set a pixel in native coordinates (x: 0-799, y: 0-479)
    ///
    /// Bit format: MSB first (bit 7 = x=0, bit 0 = x=7)
    /// Color: On = black (clear bit), Off = white (set bit)
    pub fn set_pixel(&mut self, x: u32, y: u32, color: BinaryColor) {
        if x >= Self::WIDTH || y >= Self::HEIGHT {
            return;
        }

        let byte_index = (y as usize * Self::WIDTH_BYTES) + (x as usize / 8);
        let bit_index = 7 - (x % 8); // MSB first

        if color == BinaryColor::On {
            self.buffer[byte_index] &= !(1 << bit_index); // Black: clear bit
        } else {
            self.buffer[byte_index] |= 1 << bit_index; // White: set bit
        }
    }

    /// Get the raw buffer for the driver
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Get mutable buffer (for direct manipulation)
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buffer
    }
}

impl DrawTarget for BufferedDisplay {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            self.set_pixel(point.x as u32, point.y as u32, color);
        }
        Ok(())
    }

    fn clear(&mut self, color: BinaryColor) -> Result<(), Self::Error> {
        let fill_byte = if color == BinaryColor::On { 0x00 } else { 0xFF };
        self.buffer.fill(fill_byte);
        Ok(())
    }
}

impl OriginDimensions for BufferedDisplay {
    fn size(&self) -> Size {
        // Report native dimensions (800x480)
        // The SSD1677 driver with Rotate90 handles physical rotation
        Size::new(Self::WIDTH, Self::HEIGHT)
    }
}

impl Default for BufferedDisplay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_size() {
        let display = BufferedDisplay::new();
        assert_eq!(display.buffer.len(), 48000); // 800 * 480 / 8
    }

    #[test]
    fn test_set_pixel_native() {
        let mut display = BufferedDisplay::new();
        // Set pixel at native (0, 0) - top-left
        display.set_pixel(0, 0, BinaryColor::On);
        // byte_index = 0 * 100 + 0 = 0, bit 7 cleared
        assert_eq!(display.buffer[0], 0x7F);
    }

    #[test]
    fn test_set_pixel_coordinates() {
        let mut display = BufferedDisplay::new();
        // Set pixel at (7, 0) - should be bit 0 of first byte
        display.set_pixel(7, 0, BinaryColor::On);
        assert_eq!(display.buffer[0], 0xFE); // Bit 0 cleared (LSB)

        // Set pixel at (8, 0) - should be bit 7 of second byte
        display.clear();
        display.set_pixel(8, 0, BinaryColor::On);
        assert_eq!(display.buffer[1], 0x7F); // Bit 7 cleared (MSB of byte 1)
    }

    #[test]
    fn test_dimensions() {
        let display = BufferedDisplay::new();
        let size = display.size();
        assert_eq!(size.width, 800);
        assert_eq!(size.height, 480);
    }

    #[test]
    fn test_clear_trait_method() {
        use embedded_graphics::prelude::DrawTarget;

        let mut display = BufferedDisplay::new();
        // Test DrawTarget::clear with black
        DrawTarget::clear(&mut display, BinaryColor::On).unwrap();
        assert!(display.buffer.iter().all(|&b| b == 0x00));

        // Test DrawTarget::clear with white
        DrawTarget::clear(&mut display, BinaryColor::Off).unwrap();
        assert!(display.buffer.iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn test_inherent_clear_white() {
        let mut display = BufferedDisplay::new();
        display.set_pixel(0, 0, BinaryColor::On);
        display.clear(); // Inherent method clears to white
        assert!(display.buffer.iter().all(|&b| b == 0xFF));
    }
}
