//! Graphics support via embedded-graphics
//!
//! This module provides the [`GraphicDisplay`] struct which wraps [`Display`](crate::display::Display)
//! and implements the [`DrawTarget`](embedded_graphics_core::draw_target::DrawTarget) trait from
//! the embedded-graphics ecosystem.
//!
//! ## Features
//!
//! - 2D graphics primitives (lines, rectangles, circles, text, etc.)
//! - Image support via embedded-graphics image modules
//! - Rotation support
//! - Efficient pixel buffer management
//!
//! ## Example
//!
//! ```rust,ignore
//! use ssd1677::{GraphicDisplay, Color};
//! use embedded_graphics::{
//!     mono_font::{ascii::FONT_6X10, MonoTextStyle},
//!     pixelcolor::BinaryColor,
//!     prelude::*,
//!     primitives::{Circle, Rectangle, PrimitiveStyle},
//!     text::Text,
//! };
//!
//! // Create graphic display with buffers
//! let mut display = GraphicDisplay::new(
//!     display_driver,
//!     black_buffer,  // AsMut<[u8]>
//!     red_buffer,    // AsMut<[u8]>
//! );
//!
//! // Clear to white
//! display.clear(Color::White);
//!
//! // Draw shapes
//! Rectangle::new(Point::new(10, 10), Size::new(50, 30))
//!     .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
//!     .draw(&mut display)?;
//!
//! Circle::new(Point::new(100, 50), 40)
//!     .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
//!     .draw(&mut display)?;
//!
//! // Draw text
//! Text::new("Hello, E-Paper!", Point::new(10, 100),
//!     MonoTextStyle::new(&FONT_6X10, BinaryColor::On))
//!     .draw(&mut display)?;
//!
//! // Update physical display
//! display.update(&mut delay)?;
//! ```

use core::convert::Infallible;
use embedded_graphics_core::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    prelude::Pixel,
};
use embedded_hal::delay::DelayNs;

use crate::color::Color;
use crate::display::Display;
use crate::error::Error;
use crate::interface::DisplayInterface;
use crate::rotation::apply_rotation;

/// Display with graphics buffers
///
/// This wrapper around [`Display`](crate::display::Display) provides embedded-graphics support
/// and manages the pixel buffers for black/white and red planes.
///
/// ## Type Parameters
///
/// * `I` - Interface type implementing [`DisplayInterface`](crate::interface::DisplayInterface)
/// * `B` - Buffer type implementing `AsMut<[u8]>` for both black and red buffers
///
/// ## Example
///
/// ```rust,ignore
/// use ssd1677::GraphicDisplay;
///
/// let mut graphic_display = GraphicDisplay::new(
///     display,
///     vec![0u8; buffer_size],  // Black buffer
///     vec![0u8; buffer_size],  // Red buffer
/// );
///
/// // Use with embedded-graphics...
/// ```
pub struct GraphicDisplay<I, B>
where
    I: DisplayInterface,
    B: AsMut<[u8]>,
{
    /// The underlying display driver
    display: Display<I>,
    /// Buffer for black/white pixels
    black_buffer: B,
    /// Buffer for red pixels
    red_buffer: B,
}

impl<I, B> GraphicDisplay<I, B>
where
    I: DisplayInterface,
    B: AsMut<[u8]>,
{
    /// Create a new GraphicDisplay
    ///
    /// # Arguments
    ///
    /// * `display` - The [`Display`](crate::display::Display) driver instance
    /// * `black_buffer` - Buffer for black/white pixels (must be at least `dimensions.buffer_size()` bytes)
    /// * `red_buffer` - Buffer for red pixels (must be at least `dimensions.buffer_size()` bytes)
    ///
    /// ## Example
    ///
    /// ```rust,ignore
    /// use ssd1677::{GraphicDisplay, Display};
    ///
    /// let graphic_display = GraphicDisplay::new(
    ///     display,
    ///     vec![0u8; buffer_size],
    ///     vec![0u8; buffer_size],
    /// );
    /// ```
    pub fn new(display: Display<I>, black_buffer: B, red_buffer: B) -> Self {
        Self {
            display,
            black_buffer,
            red_buffer,
        }
    }

    /// Clear buffers to a color
    ///
    /// Fills both buffers with the appropriate values to display the given color
    /// across the entire screen.
    ///
    /// # Arguments
    ///
    /// * `color` - The color to clear to ([`Color::Black`], [`Color::White`], or [`Color::Red`])
    ///
    /// ## Example
    ///
    /// ```rust,ignore
    /// use ssd1677::{GraphicDisplay, Color};
    ///
    /// // Clear to white
    /// display.clear(Color::White);
    ///
    /// // Clear to black
    /// display.clear(Color::Black);
    /// ```
    pub fn clear(&mut self, color: Color) {
        let (bw, red) = (color.bw_byte(), color.red_byte());

        for byte in self.black_buffer.as_mut().iter_mut() {
            *byte = bw;
        }
        for byte in self.red_buffer.as_mut().iter_mut() {
            *byte = red;
        }
    }

    /// Update the display from buffers
    ///
    /// Sends the current buffer contents to the display controller and triggers
    /// a refresh. The BUSY pin will go high during the refresh operation.
    ///
    /// # Arguments
    ///
    /// * `delay` - Delay implementation for busy-waiting
    ///
    /// # Errors
    ///
    /// Returns [`Error::Interface`](crate::error::Error::Interface) if there's a
    /// communication error.
    ///
    /// ## Example
    ///
    /// ```rust,ignore
    /// use ssd1677::GraphicDisplay;
    /// use embedded_hal::delay::DelayNs;
    ///
    /// // After drawing...
    /// graphic_display.update(&mut delay)
    ///     .expect("display update failed");
    /// ```
    pub fn update<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), Error<I>> {
        self.display
            .update(self.black_buffer.as_mut(), self.red_buffer.as_mut(), delay)
    }

    /// Access the underlying Display
    ///
    /// Returns an immutable reference to the wrapped [`Display`](crate::display::Display).
    ///
    /// ## Example
    ///
    /// ```rust,ignore
    /// use ssd1677::GraphicDisplay;
    ///
    /// let dims = graphic_display.display().dimensions();
    /// println!("Display size: {}x{}", dims.cols, dims.rows);
    /// ```
    pub fn display(&self) -> &Display<I> {
        &self.display
    }

    /// Access the underlying Display mutably
    ///
    /// Returns a mutable reference to the wrapped [`Display`](crate::display::Display).
    /// This can be used to access low-level operations directly.
    ///
    /// ## Example
    ///
    /// ```rust,ignore
    /// use ssd1677::GraphicDisplay;
    ///
    /// // Load a custom LUT
    /// graphic_display.display_mut()
    ///     .load_lut(&custom_lut)
    ///     .expect("LUT load failed");
    /// ```
    pub fn display_mut(&mut self) -> &mut Display<I> {
        &mut self.display
    }

    /// Set a single pixel to a color
    ///
    /// Internal method used by the [`DrawTarget`] implementation.
    /// Applies rotation transformation and updates both buffers appropriately.
    fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        let dims = self.display.dimensions();
        let width = dims.cols as u32;
        let height = dims.rows as u32;

        if x >= width || y >= height {
            return;
        }

        let rotation = self.display.rotation();
        let (index, bit) = apply_rotation(x, y, width, height, rotation);

        if index >= self.black_buffer.as_mut().len() {
            return;
        }

        match color {
            Color::Black => {
                self.black_buffer.as_mut()[index] &= !bit;
                self.red_buffer.as_mut()[index] &= !bit;
            }
            Color::White => {
                self.black_buffer.as_mut()[index] |= bit;
                self.red_buffer.as_mut()[index] &= !bit;
            }
            Color::Red => {
                self.black_buffer.as_mut()[index] |= bit;
                self.red_buffer.as_mut()[index] |= bit;
            }
        }
    }
}

impl<I, B> DrawTarget for GraphicDisplay<I, B>
where
    I: DisplayInterface,
    B: AsMut<[u8]>,
{
    type Color = Color;
    type Error = Infallible;

    fn draw_iter<Iter>(&mut self, pixels: Iter) -> Result<(), Self::Error>
    where
        Iter: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let sz = self.size();

        for Pixel(Point { x, y }, color) in pixels {
            if x >= 0 && y >= 0 {
                let x = x as u32;
                let y = y as u32;
                if x < sz.width && y < sz.height {
                    self.set_pixel(x, y, color);
                }
            }
        }

        Ok(())
    }
}

impl<I, B> OriginDimensions for GraphicDisplay<I, B>
where
    I: DisplayInterface,
    B: AsMut<[u8]>,
{
    fn size(&self) -> Size {
        let rotated = self.display.config().rotated_dimensions();
        Size::new(rotated.cols as u32, rotated.rows as u32)
    }
}
