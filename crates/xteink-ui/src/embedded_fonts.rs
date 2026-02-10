//! Embedded bitmap font types and rendering
//!
//! These types are used with compile-time generated font data from build.rs.
//! The bitmap data is embedded directly in the firmware flash, requiring
//! zero runtime allocations and zero SD card dependencies.

extern crate alloc;

use alloc::vec::Vec;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

/// Metrics for a single glyph
#[derive(Debug, Clone, Copy)]
pub struct EmbeddedGlyphMetrics {
    pub codepoint: u32,
    pub width: u8,
    pub height: u8,
    pub advance_width: u8,
    pub x_offset: i8,
    pub y_offset: i8,
    pub data_offset: u32,
    pub data_len: u32,
}

/// A bitmap font at a specific size
pub struct EmbeddedBitmapFont {
    pub size_px: u32,
    pub line_height: u8,
    pub glyph_count: usize,
    pub glyphs: &'static [EmbeddedGlyphMetrics],
    pub bitmap_data: &'static [u8],
    pub bits_per_pixel: u8,
}

/// Reference to a font at a specific size
pub struct EmbeddedFontSize {
    pub size_px: u32,
    pub font: &'static EmbeddedBitmapFont,
}

impl EmbeddedBitmapFont {
    /// Find a glyph by codepoint using binary search
    /// Returns the glyph data or None if not found
    pub fn glyph(&self, c: char) -> Option<&'static EmbeddedGlyphMetrics> {
        let codepoint = c as u32;
        self.glyphs
            .binary_search_by(|g| g.codepoint.cmp(&codepoint))
            .ok()
            .map(|idx| &self.glyphs[idx])
    }

    /// Get the bitmap data for a glyph
    pub fn glyph_bitmap(&self, glyph: &EmbeddedGlyphMetrics) -> &[u8] {
        let start = glyph.data_offset as usize;
        let end = start + glyph.data_len as usize;
        &self.bitmap_data[start..end]
    }

    /// Render a glyph to the display at the given position
    pub fn draw_glyph<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        glyph: &EmbeddedGlyphMetrics,
        x: i32,
        y: i32, // baseline y position
    ) -> Result<(), D::Error> {
        if glyph.width == 0 || glyph.height == 0 {
            return Ok(());
        }

        let bitmap = self.glyph_bitmap(glyph);
        let glyph_x = x + glyph.x_offset as i32;
        let glyph_y = y - glyph.y_offset as i32 - glyph.height as i32;

        let mut pixels = Vec::new();
        let row_bytes = (glyph.width as usize).div_ceil(8);

        for row in 0..glyph.height {
            for col in 0..glyph.width {
                let byte_idx = (row as usize) * row_bytes + (col as usize) / 8;
                let bit_idx = 7 - ((col as usize) % 8); // MSB first

                if byte_idx < bitmap.len() {
                    let byte = bitmap[byte_idx];
                    if (byte >> bit_idx) & 1 == 1 {
                        let point = Point::new(glyph_x + col as i32, glyph_y + row as i32);
                        pixels.push(Pixel(point, BinaryColor::On));
                    }
                }
            }
        }

        display.draw_iter(pixels)
    }

    /// Measure text width without rendering
    pub fn text_width(&self, text: &str) -> u32 {
        let mut width = 0u32;
        for ch in text.chars() {
            if let Some(glyph) = self.glyph(ch) {
                width += glyph.advance_width as u32;
            }
        }
        width
    }

    /// Render text to display
    pub fn draw_text<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        text: &str,
        x: i32,
        y: i32, // baseline position
    ) -> Result<i32, D::Error> {
        let mut cursor_x = x;

        for ch in text.chars() {
            if let Some(glyph) = self.glyph(ch) {
                self.draw_glyph(display, glyph, cursor_x, y)?;
                cursor_x += glyph.advance_width as i32;
            }
        }

        Ok(cursor_x - x) // Return total width
    }
}

// Include the generated font data
include!(concat!(env!("OUT_DIR"), "/embedded_fonts.rs"));

/// Font registry for looking up fonts by name
pub struct EmbeddedFontRegistry;

impl EmbeddedFontRegistry {
    /// Get a font by name and size
    pub fn get_font(name: &str, size_px: u32) -> Option<&'static EmbeddedBitmapFont> {
        let sizes = match name {
            "bookerly-regular" => BOOKERLY_REGULAR_SIZES,
            "bookerly-bold" => BOOKERLY_BOLD_SIZES,
            "bookerly-italic" => BOOKERLY_ITALIC_SIZES,
            "bookerly-bold-italic" => BOOKERLY_BOLDITALIC_SIZES,
            _ => return None,
        };

        sizes.iter().find(|s| s.size_px == size_px).map(|s| s.font)
    }

    /// Get the closest available size for a font
    pub fn get_font_nearest(name: &str, size_px: u32) -> Option<&'static EmbeddedBitmapFont> {
        let sizes = match name {
            "bookerly-regular" => BOOKERLY_REGULAR_SIZES,
            "bookerly-bold" => BOOKERLY_BOLD_SIZES,
            "bookerly-italic" => BOOKERLY_ITALIC_SIZES,
            "bookerly-bold-italic" => BOOKERLY_BOLDITALIC_SIZES,
            _ => return None,
        };

        sizes
            .iter()
            .min_by_key(|s| (s.size_px as i32 - size_px as i32).abs())
            .map(|s| s.font)
    }
}

/// A font cache that uses embedded bitmap fonts
pub struct EmbeddedFontCache {
    current_font: Option<&'static EmbeddedBitmapFont>,
    current_size: f32,
}

impl EmbeddedFontCache {
    pub fn new() -> Self {
        Self {
            current_font: None,
            current_size: 16.0,
        }
    }

    pub fn set_font(&mut self, font_name: &str) {
        if let Some(font) =
            EmbeddedFontRegistry::get_font_nearest(font_name, self.current_size as u32)
        {
            self.current_font = Some(font);
        }
    }

    pub fn set_font_size(&mut self, size: f32) {
        self.current_size = size;
        // Update current font to nearest size
        if let Some(current) = self.current_font {
            // Find the font name from the current font
            for (name, _) in EMBEDDED_FONTS {
                if let Some(font) = EmbeddedFontRegistry::get_font_nearest(name, size as u32) {
                    if core::ptr::eq(font, current) {
                        self.current_font = Some(font);
                        break;
                    }
                }
            }
        }
    }

    pub fn metrics(&self, font_name: &str, ch: char) -> Option<EmbeddedGlyphMetrics> {
        let font = EmbeddedFontRegistry::get_font_nearest(font_name, self.current_size as u32)?;
        font.glyph(ch).copied()
    }

    pub fn measure_text(&self, text: &str, font_name: &str) -> f32 {
        if let Some(font) =
            EmbeddedFontRegistry::get_font_nearest(font_name, self.current_size as u32)
        {
            font.text_width(text) as f32
        } else {
            0.0
        }
    }

    pub fn render_text<D: DrawTarget<Color = BinaryColor>>(
        &mut self,
        display: &mut D,
        text: &str,
        font_name: &str,
        x: i32,
        y: i32,
    ) -> Result<(), D::Error> {
        if let Some(font) =
            EmbeddedFontRegistry::get_font_nearest(font_name, self.current_size as u32)
        {
            font.draw_text(display, text, x, y)?;
        }
        Ok(())
    }

    pub fn load_font(&mut self, _name: &str, _data: &[u8]) {
        // Embedded fonts are pre-loaded, this is a no-op
    }
}

impl Default for EmbeddedFontCache {
    fn default() -> Self {
        Self::new()
    }
}
