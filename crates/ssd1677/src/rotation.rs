//! Coordinate rotation utilities
//!
//! This module provides functions for applying rotation transformations to pixel
//! coordinates when mapping to the display buffer.
//!
//! E-paper displays typically store pixels in a bit-packed format where each byte
//! contains 8 horizontal pixels. When the display is rotated, the byte index and
//! bit position within each byte must be calculated differently.
//!
//! ## Rotation Modes
//!
//! - **Rotate0**: Native orientation, pixels packed left-to-right
//! - **Rotate90**: 90° clockwise, width and height swapped
//! - **Rotate180**: 180° rotation, origin at bottom-right
//! - **Rotate270**: 270° clockwise (or 90° counter-clockwise)
//!
//! ## Example
//!
//! ```
//! use ssd1677::{rotation::apply_rotation, Rotation};
//!
//! // For an 8x1 display at native orientation, pixel (0,0) is at byte 0, bit 7 (MSB)
//! let (idx, bit) = apply_rotation(0, 0, 8, 1, Rotation::Rotate0);
//! assert_eq!(idx, 0);
//! assert_eq!(bit, 0x80);
//!
//! // Pixel (7,0) is at byte 0, bit 0 (LSB)
//! let (idx, bit) = apply_rotation(7, 0, 8, 1, Rotation::Rotate0);
//! assert_eq!(idx, 0);
//! assert_eq!(bit, 0x01);
//! ```

use crate::config::Rotation;

/// Apply rotation transformation to get buffer index and bit mask
///
/// Converts logical (x, y) coordinates to physical buffer location (byte_index, bit_mask)
/// based on the specified rotation.
///
/// # Arguments
///
/// * `x` - X coordinate (column), 0 to width-1
/// * `y` - Y coordinate (row), 0 to height-1
/// * `width` - Display width in pixels (must be multiple of 8)
/// * `height` - Display height in pixels
/// * `rotation` - Rotation mode
///
/// # Returns
///
/// Returns a tuple of (byte_index, bit_mask):
/// - `byte_index`: Index into the buffer array
/// - `bit_mask`: Bit mask within the byte (0x80, 0x40, 0x20, etc.)
///
/// # Example
///
/// ```
/// use ssd1677::{rotation::apply_rotation, Rotation};
///
/// // 16x16 display at 0° rotation
/// let (idx, bit) = apply_rotation(0, 0, 16, 16, Rotation::Rotate0);
/// // First pixel is at byte 0, MSB (0x80)
/// assert_eq!(idx, 0);
/// assert_eq!(bit, 0x80);
///
/// // Same display at 90° rotation
/// let (idx, bit) = apply_rotation(0, 0, 16, 16, Rotation::Rotate90);
/// // Origin moves to top-right corner
/// assert_eq!(idx, 1);  // (15 / 8 = 1)
/// assert_eq!(bit, 0x01);  // LSB
/// ```
pub fn apply_rotation(x: u32, y: u32, width: u32, height: u32, rotation: Rotation) -> (usize, u8) {
    match rotation {
        Rotation::Rotate0 => {
            let index = (x / 8 + (width / 8) * y) as usize;
            let bit = 0x80 >> (x % 8);
            (index, bit)
        }
        Rotation::Rotate90 => {
            let index = ((width - 1 - y) / 8 + (width / 8) * x) as usize;
            let bit = 0x01 << (y % 8);
            (index, bit)
        }
        Rotation::Rotate180 => {
            let index = (((width / 8) * height - 1) - (x / 8 + (width / 8) * y)) as usize;
            let bit = 0x01 << (x % 8);
            (index, bit)
        }
        Rotation::Rotate270 => {
            let index = (y / 8 + (height - 1 - x) * (width / 8)) as usize;
            let bit = 0x80 >> (y % 8);
            (index, bit)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotate0() {
        // 8x1 display, pixel at (0,0) should be byte 0, bit 7
        let (idx, bit) = apply_rotation(0, 0, 8, 1, Rotation::Rotate0);
        assert_eq!(idx, 0);
        assert_eq!(bit, 0x80);

        // pixel at (1,0) should be byte 0, bit 6
        let (idx, bit) = apply_rotation(1, 0, 8, 1, Rotation::Rotate0);
        assert_eq!(idx, 0);
        assert_eq!(bit, 0x40);

        // pixel at (7,0) should be byte 0, bit 0
        let (idx, bit) = apply_rotation(7, 0, 8, 1, Rotation::Rotate0);
        assert_eq!(idx, 0);
        assert_eq!(bit, 0x01);

        // pixel at (0,1) in 8x2 display should be byte 1
        let (idx, bit) = apply_rotation(0, 1, 8, 2, Rotation::Rotate0);
        assert_eq!(idx, 1);
        assert_eq!(bit, 0x80);
    }

    #[test]
    fn test_rotate180() {
        // 8x1 display, pixel at (7,0) should be byte 0, bit 0 (opposite of rotate0)
        let (idx, bit) = apply_rotation(7, 0, 8, 1, Rotation::Rotate180);
        assert_eq!(idx, 0);
        assert_eq!(bit, 0x80); // MSB first, so (7,0) is still bit 0 but treated as MSB

        // pixel at (0,0) in 8x1 with rotate180
        let (idx, bit) = apply_rotation(0, 0, 8, 1, Rotation::Rotate180);
        assert_eq!(idx, 0);
        assert_eq!(bit, 0x01);
    }

    #[test]
    fn test_rotate90() {
        // 16x16 display, origin (0,0) rotated 90 should go to (15,0)
        // which maps to: ((16-1-0)/8 + (16/8)*0, 0x01 << (0 % 8))
        // = (15/8 + 0, 0x01) = (1, 0x01)
        let (idx, bit) = apply_rotation(0, 0, 16, 16, Rotation::Rotate90);
        assert_eq!(idx, 1); // (15 / 8 = 1)
        assert_eq!(bit, 0x01);
    }

    #[test]
    fn test_rotate270() {
        // 16x16 display, origin (0,0) rotated 270
        // (0/8 + (16-1-0)*(16/8), 0x80 >> (0 % 8))
        // = (0 + 15*2, 0x80) = (30, 0x80)
        let (idx, bit) = apply_rotation(0, 0, 16, 16, Rotation::Rotate270);
        assert_eq!(idx, 30);
        assert_eq!(bit, 0x80);
    }
}
