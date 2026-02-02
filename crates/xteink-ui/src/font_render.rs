//! Font rendering system using fontdue
//!
//! Provides TTF/OTF font rendering for EPUB embedded fonts.
//! Optimized for embedded use with glyph caching.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

#[cfg(feature = "fontdue")]
use fontdue::{Font, FontSettings, Metrics};

/// Font cache for efficient rendering
#[cfg(feature = "fontdue")]
pub struct FontCache {
    /// Loaded fonts (name -> Font)
    fonts: BTreeMap<String, Font>,
    /// Glyph cache (font_idx, glyph_idx, size) -> bitmap
    glyph_cache: BTreeMap<(usize, u16, f32), Vec<u8>>,
    /// Default font index
    #[allow(dead_code)]
    default_font: usize,
    /// Current font size in pixels
    font_size: f32,
}

#[cfg(feature = "fontdue")]
impl FontCache {
    /// Create empty font cache
    pub fn new() -> Self {
        Self {
            fonts: BTreeMap::new(),
            glyph_cache: BTreeMap::new(),
            default_font: 0,
            font_size: 16.0,
        }
    }

    /// Load a font from bytes
    pub fn load_font(&mut self, name: &str, font_data: &[u8]) -> Result<(), String> {
        let settings = FontSettings {
            scale: self.font_size,
            ..FontSettings::default()
        };

        let font = Font::from_bytes(font_data, settings)
            .map_err(|e| format!("Failed to load font '{}': {:?}", name, e))?;

        self.fonts.insert(name.to_string(), font);
        Ok(())
    }

    /// Set current font size
    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size;
        // Clear glyph cache when size changes
        self.glyph_cache.clear();
    }

    /// Get font metrics for a specific character
    pub fn metrics(&self, font_name: &str, ch: char) -> Option<Metrics> {
        self.fonts
            .get(font_name)
            .map(|f| f.metrics(ch, self.font_size))
    }

    /// Get line height for the font
    pub fn line_height(&self, font_name: &str) -> f32 {
        // Use 'M' as a representative character for line height
        self.metrics(font_name, 'M')
            .map(|m| m.advance_height)
            .unwrap_or(self.font_size * 1.2)
    }

    /// Measure text width
    pub fn measure_text(&self, text: &str, font_name: &str) -> f32 {
        if let Some(font) = self.fonts.get(font_name) {
            let mut x = 0.0;
            for ch in text.chars() {
                let (metrics, _) = font.rasterize(ch, self.font_size);
                x += metrics.advance_width;
            }
            x
        } else {
            0.0
        }
    }

    /// Rasterize a single glyph
    pub fn rasterize_glyph(&mut self, ch: char, font_name: &str) -> Option<(Metrics, Vec<u8>)> {
        let font = self.fonts.get(font_name)?;
        let (metrics, bitmap) = font.rasterize(ch, self.font_size);
        Some((metrics, bitmap.to_vec()))
    }

    /// Layout text into lines that fit within width
    pub fn layout_text(&self, text: &str, font_name: &str, max_width: f32) -> Vec<TextLine> {
        let mut lines = Vec::new();
        let mut current_line = TextLine::new();
        let mut current_x = 0.0;

        if let Some(font) = self.fonts.get(font_name) {
            for word in text.split_whitespace() {
                let word_width = self.measure_text(word, font_name);

                if current_x + word_width > max_width && !current_line.words.is_empty() {
                    // Start new line
                    lines.push(current_line);
                    current_line = TextLine::new();
                    current_x = 0.0;
                }

                // Add word to current line
                let word_start = current_x;
                let mut glyphs = Vec::new();

                for ch in word.chars() {
                    let (metrics, _) = font.rasterize(ch, self.font_size);
                    glyphs.push(GlyphInfo {
                        character: ch,
                        x: current_x,
                        y: 0.0, // Will be set during rendering
                        metrics,
                    });
                    current_x += metrics.advance_width;
                }

                current_line.words.push(Word {
                    text: word.to_string(),
                    x: word_start,
                    width: word_width,
                    glyphs,
                });

                // Add space after word using space character metrics
                let space_metrics = font.metrics(' ', self.font_size);
                current_x += space_metrics.advance_width;
            }

            // Don't forget the last line
            if !current_line.words.is_empty() {
                lines.push(current_line);
            }
        }

        lines
    }

    /// Render text to display at position
    pub fn render_text<D: DrawTarget<Color = BinaryColor>>(
        &mut self,
        display: &mut D,
        text: &str,
        font_name: &str,
        x: i32,
        y: i32,
    ) -> Result<(), D::Error> {
        if let Some(font) = self.fonts.get(font_name) {
            let mut cursor_x = x as f32;
            let baseline_y = y as f32;

            for ch in text.chars() {
                let (metrics, bitmap) = font.rasterize(ch, self.font_size);

                // Draw glyph bitmap
                let glyph_x = (cursor_x + metrics.xmin as f32) as i32;
                let glyph_y = (baseline_y - metrics.ymin as f32 - metrics.height as f32) as i32;

                self.draw_glyph(
                    display,
                    glyph_x,
                    glyph_y,
                    &bitmap,
                    metrics.width,
                    metrics.height,
                )?;

                cursor_x += metrics.advance_width;
            }
        }

        Ok(())
    }

    /// Draw a single glyph bitmap to the display
    fn draw_glyph<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        x: i32,
        y: i32,
        bitmap: &[u8],
        width: usize,
        height: usize,
    ) -> Result<(), D::Error> {
        // Convert grayscale bitmap to binary (threshold at 128)
        let mut pixels = Vec::new();
        for row in 0..height {
            for col in 0..width {
                let pixel_idx = row * width + col;
                if pixel_idx < bitmap.len() {
                    let pixel_value = bitmap[pixel_idx];
                    // Threshold: if pixel is darker than 50%, draw it
                    if pixel_value > 128 {
                        let point = Point::new(x + col as i32, y + row as i32);
                        pixels.push(Pixel(point, BinaryColor::On));
                    }
                }
            }
        }
        display.draw_iter(pixels)
    }
}

/// Information about a single glyph
#[cfg(feature = "fontdue")]
pub struct GlyphInfo {
    pub character: char,
    pub x: f32,
    pub y: f32,
    pub metrics: Metrics,
}

/// A word in a text line
#[cfg(feature = "fontdue")]
pub struct Word {
    pub text: String,
    pub x: f32,
    pub width: f32,
    pub glyphs: Vec<GlyphInfo>,
}

/// A line of text
#[cfg(feature = "fontdue")]
pub struct TextLine {
    pub words: Vec<Word>,
    pub height: f32,
    pub baseline: f32,
}

#[cfg(feature = "fontdue")]
impl TextLine {
    fn new() -> Self {
        Self {
            words: Vec::new(),
            height: 0.0,
            baseline: 0.0,
        }
    }

    pub fn width(&self) -> f32 {
        self.words.last().map(|w| w.x + w.width).unwrap_or(0.0)
    }
}

#[cfg(feature = "fontdue")]
impl Default for FontCache {
    fn default() -> Self {
        Self::new()
    }
}

// Stub implementation when fontdue is not available
#[cfg(not(feature = "fontdue"))]
pub struct FontCache;

#[cfg(not(feature = "fontdue"))]
impl FontCache {
    pub fn new() -> Self {
        Self
    }

    pub fn set_font_size(&mut self, _size: f32) {}
}

#[cfg(not(feature = "fontdue"))]
impl Default for FontCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_cache_new() {
        let cache = FontCache::new();
        assert_eq!(cache.font_size, 16.0);
    }
}
