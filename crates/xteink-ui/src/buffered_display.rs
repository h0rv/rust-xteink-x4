//! Simple buffered display for Xteink X4
//!
//! Avoids embedded-graphics iterator overhead by drawing to a buffer first,
//! then sending the entire buffer to the display in one operation.
//! This matches the approach used in crosspoint-reader C++ code.

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

/// Simple frame buffer display
///
/// The SSD1677 panel is native 800x480 landscape, but mounted in portrait orientation.
/// We render in portrait (480x800) and transpose coordinates to native (800x480).
/// The driver is configured with 800x480 + Rotate90 to display correctly.
pub struct BufferedDisplay {
    buffer: Vec<u8>,
}

impl BufferedDisplay {
    /// Native SSD1677 panel dimensions (800x480 landscape)
    #[allow(dead_code)]
    const NATIVE_WIDTH: u32 = 800;
    #[allow(dead_code)]
    const NATIVE_HEIGHT: u32 = 480;
    const NATIVE_WIDTH_BYTES: usize = 100; // 800 / 8

    /// Portrait dimensions (what the UI sees)
    const PORTRAIT_WIDTH: u32 = 480;
    const PORTRAIT_HEIGHT: u32 = 800;

    /// Total buffer size (48KB)
    const BUFFER_SIZE: usize = 100 * 480;

    /// Create new buffered display
    pub fn new() -> Self {
        Self {
            buffer: vec![0xFF; Self::BUFFER_SIZE], // White by default
        }
    }

    /// Clear buffer to white
    pub fn clear(&mut self) {
        self.buffer.fill(0xFF);
    }

    /// Set a pixel in portrait coordinates (x: 0-479, y: 0-799)
    /// Internally transposes to native 800x480 for the SSD1677 driver
    pub fn set_pixel(&mut self, x: u32, y: u32, color: BinaryColor) {
        if x >= Self::PORTRAIT_WIDTH || y >= Self::PORTRAIT_HEIGHT {
            return;
        }

        // Transpose portrait (x,y) to native coordinates for 90-degree rotation
        // Portrait (0,0) top-left -> Native (799, 0) top-right (rotated -90)
        // Actually, for 90-degree clockwise rotation:
        // x' = y
        // y' = (WIDTH - 1) - x
        let native_x = y;
        let native_y = (Self::PORTRAIT_WIDTH - 1) - x;

        let byte_index = (native_y as usize * Self::NATIVE_WIDTH_BYTES) + (native_x as usize / 8);
        let bit_index = 7 - (native_x % 8); // MSB first

        if byte_index < self.buffer.len() {
            if color == BinaryColor::On {
                // Black: clear bit
                self.buffer[byte_index] &= !(1 << bit_index);
            } else {
                // White: set bit
                self.buffer[byte_index] |= 1 << bit_index;
            }
        }
    }

    /// Get the raw buffer (in native 800x480 orientation for driver)
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Get mutable buffer
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
}

impl OriginDimensions for BufferedDisplay {
    fn size(&self) -> Size {
        // Report portrait dimensions to embedded-graphics UI
        // Internal buffer is native 800x480
        Size::new(Self::PORTRAIT_WIDTH, Self::PORTRAIT_HEIGHT)
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
    fn test_set_pixel_portrait() {
        let mut display = BufferedDisplay::new();
        // Set pixel at portrait (0, 0) - should appear at native right edge
        display.set_pixel(0, 0, BinaryColor::On);
        // After transpose: native_x = 0, native_y = 479
        // byte_index = 479 * 100 + 0 = 47900
        assert_eq!(display.buffer[47900], 0x7F); // Bit 7 cleared
    }
}
