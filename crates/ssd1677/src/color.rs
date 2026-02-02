//! Color types for tri-color e-paper displays
//!
//! This module defines the [`Color`] enum for black, white, and red colors
//! supported by tri-color e-paper displays using the SSD1677 controller.
//!
//! ## Color Representation
//!
//! E-paper displays use a bit-packed format where each pixel is represented by:
//! - 1 bit in the black/white buffer
//! - 1 bit in the red buffer
//!
//! | Color | BW Buffer | RED Buffer |
//! |-------|-----------|------------|
//! | Black | 0         | 0          |
//! | White | 1         | 0          |
//! | Red   | 1         | 1          |
//!
//! ## Example
//!
//! ```
//! use ssd1677::Color;
//!
//! // Get byte values for buffers
//! let black_bw = Color::Black.bw_byte();   // 0x00
//! let black_red = Color::Black.red_byte(); // 0x00
//!
//! let white_bw = Color::White.bw_byte();   // 0xFF
//! let white_red = Color::White.red_byte(); // 0x00
//!
//! let red_bw = Color::Red.bw_byte();       // 0xFF
//! let red_red = Color::Red.red_byte();     // 0xFF
//! ```

use embedded_graphics_core::pixelcolor::raw::RawU8;
use embedded_graphics_core::prelude::PixelColor;

/// Colors supported by SSD1677 (tri-color displays)
///
/// Tri-color e-paper displays can show black, white, and red pixels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Color {
    /// Black pixels
    Black,
    /// White pixels
    White,
    /// Red pixels (for tri-color displays)
    Red,
}

impl PixelColor for Color {
    type Raw = RawU8;
}

impl Color {
    /// Get the byte value for black/white buffer
    ///
    /// Returns the value to write to the BW RAM buffer:
    /// - Black: 0x00 (all bits 0)
    /// - White: 0xFF (all bits 1)
    /// - Red: 0xFF (all bits 1, red requires BW=1 too)
    ///
    /// ## Example
    ///
    /// ```
    /// use ssd1677::Color;
    ///
    /// assert_eq!(Color::Black.bw_byte(), 0x00);
    /// assert_eq!(Color::White.bw_byte(), 0xFF);
    /// assert_eq!(Color::Red.bw_byte(), 0xFF);
    /// ```
    pub fn bw_byte(self) -> u8 {
        match self {
            Color::Black => 0x00,
            Color::White => 0xFF,
            Color::Red => 0xFF, // Red uses both buffers
        }
    }

    /// Get the byte value for red buffer
    ///
    /// Returns the value to write to the RED RAM buffer:
    /// - Black: 0x00 (no red)
    /// - White: 0x00 (no red)
    /// - Red: 0xFF (all bits 1)
    ///
    /// ## Example
    ///
    /// ```
    /// use ssd1677::Color;
    ///
    /// assert_eq!(Color::Black.red_byte(), 0x00);
    /// assert_eq!(Color::White.red_byte(), 0x00);
    /// assert_eq!(Color::Red.red_byte(), 0xFF);
    /// ```
    pub fn red_byte(self) -> u8 {
        match self {
            Color::Black => 0x00,
            Color::White => 0x00,
            Color::Red => 0xFF,
        }
    }
}
