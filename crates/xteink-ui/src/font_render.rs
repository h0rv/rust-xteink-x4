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

#[cfg(all(feature = "fontdue", feature = "std", test))]
use fontdue::OutlineBounds;
#[cfg(feature = "fontdue")]
use fontdue::{Font, FontSettings, Metrics};

// Standard library imports for glyph cache (requires std feature)
#[cfg(all(feature = "fontdue", feature = "std"))]
use std::collections::{HashMap, VecDeque};
#[cfg(all(feature = "fontdue", feature = "std"))]
use std::time::Instant;

/// Maximum number of glyphs to cache
#[cfg(all(feature = "fontdue", feature = "std", target_os = "espidf"))]
const GLYPH_CACHE_MAX_SIZE: usize = 24;
/// Maximum number of glyphs to cache
#[cfg(all(feature = "fontdue", feature = "std", not(target_os = "espidf")))]
const GLYPH_CACHE_MAX_SIZE: usize = 256;

/// Cache key using u32 for f32 to support Hash and Eq
#[cfg(all(feature = "fontdue", feature = "std"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphCacheKey {
    character: char,
    size_bits: u32, // f32.to_bits() for hashable representation
}

#[cfg(all(feature = "fontdue", feature = "std"))]
impl GlyphCacheKey {
    fn new(character: char, size: f32) -> Self {
        Self {
            character,
            size_bits: size.to_bits(),
        }
    }

    #[allow(dead_code)]
    fn size(&self) -> f32 {
        f32::from_bits(self.size_bits)
    }
}

/// A cached glyph with rasterized bitmap and metadata
#[cfg(all(feature = "fontdue", feature = "std"))]
#[derive(Debug, Clone)]
pub struct CachedGlyph {
    /// Font metrics for the glyph
    pub metrics: Metrics,
    /// Rasterized glyph bitmap data (grayscale)
    pub bitmap: Vec<u8>,
    /// Last access time for LRU tracking
    pub last_used: Instant,
}

/// LRU cache for rendered glyphs
#[cfg(all(feature = "fontdue", feature = "std"))]
pub struct GlyphCache {
    /// Main cache storage: (char, size) -> cached glyph
    cache: HashMap<GlyphCacheKey, CachedGlyph>,
    /// LRU order tracking: most recently used at the back
    lru_order: VecDeque<GlyphCacheKey>,
    /// Maximum cache size
    max_size: usize,
}

#[cfg(all(feature = "fontdue", feature = "std"))]
impl GlyphCache {
    /// Create a new glyph cache with default size
    pub fn new() -> Self {
        Self::with_capacity(GLYPH_CACHE_MAX_SIZE)
    }

    /// Create a new glyph cache with specified capacity
    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            cache: HashMap::with_capacity(max_size),
            lru_order: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    /// Check if a glyph is in the cache without updating LRU
    pub fn contains(&self, ch: char, size: f32) -> bool {
        self.cache.contains_key(&GlyphCacheKey::new(ch, size))
    }

    /// Get a cached glyph, updating LRU order
    /// Returns a cloned glyph (not a reference) to avoid borrow issues
    pub fn get(&mut self, ch: char, size: f32) -> Option<CachedGlyph> {
        let key = GlyphCacheKey::new(ch, size);
        if let Some(glyph) = self.cache.get(&key) {
            let glyph_clone = glyph.clone();
            // Update LRU order: move to back (most recently used)
            self.update_lru(key);
            return Some(glyph_clone);
        }
        None
    }

    /// Insert a glyph into the cache, evicting LRU if necessary
    pub fn insert(&mut self, ch: char, size: f32, metrics: Metrics, bitmap: Vec<u8>) {
        let key = GlyphCacheKey::new(ch, size);

        // Evict if cache is full and this is a new entry
        if !self.cache.contains_key(&key) && self.cache.len() >= self.max_size {
            self.evict_lru();
        }

        let glyph = CachedGlyph {
            metrics,
            bitmap,
            last_used: Instant::now(),
        };

        // If updating existing entry, remove old LRU position first
        if self.cache.contains_key(&key) {
            self.lru_order.retain(|&k| k != key);
        }

        // Insert into cache and add to back of LRU order
        self.cache.insert(key, glyph);
        self.lru_order.push_back(key);
    }

    /// Clear the entire cache
    pub fn clear(&mut self) {
        self.cache.clear();
        self.lru_order.clear();
    }

    /// Get current cache size
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Update LRU order for an access
    fn update_lru(&mut self, key: GlyphCacheKey) {
        // Remove from current position
        self.lru_order.retain(|&k| k != key);
        // Add to back (most recently used)
        self.lru_order.push_back(key);
    }

    /// Evict the least recently used glyph
    fn evict_lru(&mut self) {
        if let Some(lru_key) = self.lru_order.pop_front() {
            self.cache.remove(&lru_key);
        }
    }
}

#[cfg(all(feature = "fontdue", feature = "std"))]
impl Default for GlyphCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Font cache for efficient rendering
#[cfg(feature = "fontdue")]
pub struct FontCache {
    /// Loaded fonts (name -> Font)
    fonts: BTreeMap<String, Font>,
    /// Legacy glyph cache (font_idx, glyph_idx, size) -> bitmap
    /// Kept for backward compatibility
    glyph_cache: BTreeMap<(usize, u16, f32), Vec<u8>>,
    /// LRU glyph cache for efficient repeated rendering (std only)
    #[cfg(feature = "std")]
    lru_glyph_cache: GlyphCache,
    /// Default font index
    #[allow(dead_code)]
    default_font: usize,
    /// Current font size in pixels
    font_size: f32,
    /// Current font name for glyph caching
    current_font: Option<String>,
}

#[cfg(feature = "fontdue")]
impl FontCache {
    /// Create empty font cache
    pub fn new() -> Self {
        Self {
            fonts: BTreeMap::new(),
            glyph_cache: BTreeMap::new(),
            #[cfg(feature = "std")]
            lru_glyph_cache: GlyphCache::new(),
            default_font: 0,
            font_size: 16.0,
            current_font: None,
        }
    }

    /// Create empty font cache with explicit LRU glyph-cache capacity.
    #[cfg(feature = "std")]
    pub fn new_with_glyph_cache_capacity(glyph_cache_capacity: usize) -> Self {
        Self {
            fonts: BTreeMap::new(),
            glyph_cache: BTreeMap::new(),
            lru_glyph_cache: GlyphCache::with_capacity(glyph_cache_capacity),
            default_font: 0,
            font_size: 16.0,
            current_font: None,
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
        if self.current_font.is_none() {
            self.current_font = Some(name.to_string());
        }
        Ok(())
    }

    /// Set current font size
    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size;
        // Clear glyph caches when size changes
        self.glyph_cache.clear();
        #[cfg(feature = "std")]
        self.lru_glyph_cache.clear();
    }

    /// Set current font by name
    pub fn set_font(&mut self, font_name: &str) {
        if self.fonts.contains_key(font_name) {
            self.current_font = Some(font_name.to_string());
        }
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

    /// Baseline offset (px) when the caller provides a top-aligned Y origin.
    pub fn baseline_offset(&self, font_name: &str) -> i32 {
        if let Some(font) = self.fonts.get(font_name) {
            if let Some(lines) = font.horizontal_line_metrics(self.font_size) {
                let ascent = lines.ascent;
                let rounded = if ascent >= 0.0 {
                    (ascent + 0.5) as i32
                } else {
                    (ascent - 0.5) as i32
                };
                return rounded.max(1);
            }
            if let Some(metrics) = self.metrics(font_name, 'M') {
                return (metrics.height as i32 + metrics.ymin).max(1);
            }
        }
        let fallback = self.font_size * 0.8;
        let rounded = if fallback >= 0.0 {
            (fallback + 0.5) as i32
        } else {
            (fallback - 0.5) as i32
        };
        rounded.max(1)
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

    /// Rasterize a single glyph (uncached)
    pub fn rasterize_glyph(&self, ch: char, font_name: &str) -> Option<(Metrics, Vec<u8>)> {
        let font = self.fonts.get(font_name)?;
        let (metrics, bitmap) = font.rasterize(ch, self.font_size);
        Some((metrics, bitmap.to_vec()))
    }

    /// Get a glyph from the LRU cache, rasterizing if necessary
    ///
    /// This method provides fast access to cached glyphs with automatic
    /// rasterization on cache miss. Uses LRU eviction when cache is full.
    ///
    /// # Performance
    /// - Cache hit: < 0.1ms (bitmap lookup)
    /// - Cache miss: ~10ms (rasterization + cache insertion)
    ///
    /// # Returns
    /// A cloned CachedGlyph to avoid lifetime issues. For repeated rendering
    /// of the same text, this provides significant speedup.
    #[cfg(feature = "std")]
    pub fn get_glyph(&mut self, ch: char) -> Option<CachedGlyph> {
        // Try to get from cache first
        if let Some(glyph) = self.lru_glyph_cache.get(ch, self.font_size) {
            return Some(glyph);
        }

        // Cache miss: need to rasterize
        let font_name = self.current_font.as_ref()?;
        let font = self.fonts.get(font_name)?;

        // Rasterize the glyph
        let (metrics, bitmap) = font.rasterize(ch, self.font_size);

        // Insert into cache
        self.lru_glyph_cache
            .insert(ch, self.font_size, metrics, bitmap.clone());

        // Return the rasterized glyph
        Some(CachedGlyph {
            metrics,
            bitmap,
            last_used: Instant::now(),
        })
    }

    /// Get a glyph without caching (for no_std or when cache is disabled)
    #[cfg(not(feature = "std"))]
    pub fn get_glyph(&self, ch: char) -> Option<(Metrics, Vec<u8>)> {
        let font_name = self.current_font.as_ref()?;
        self.rasterize_glyph(ch, font_name)
    }

    /// Get cache statistics (std builds only)
    #[cfg(feature = "std")]
    pub fn cache_stats(&self) -> (usize, usize) {
        (self.lru_glyph_cache.len(), GLYPH_CACHE_MAX_SIZE)
    }

    /// Clear the LRU glyph cache (std builds only)
    #[cfg(feature = "std")]
    pub fn clear_glyph_cache(&mut self) {
        self.lru_glyph_cache.clear();
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

    /// Render text to display at position using glyph caching
    ///
    /// Uses the LRU glyph cache for efficient repeated rendering.
    /// First render of a character takes ~10ms, subsequent renders <0.1ms.
    pub fn render_text<D: DrawTarget<Color = BinaryColor>>(
        &mut self,
        display: &mut D,
        text: &str,
        font_name: &str,
        x: i32,
        y: i32,
    ) -> Result<(), D::Error> {
        #[cfg(feature = "std")]
        {
            self.render_text_with_cache(display, text, font_name, x, y)
        }
        #[cfg(not(feature = "std"))]
        {
            self.render_text_uncached(display, text, font_name, x, y)
        }
    }

    /// Render text using the LRU glyph cache (std builds only)
    #[cfg(feature = "std")]
    fn render_text_with_cache<D: DrawTarget<Color = BinaryColor>>(
        &mut self,
        display: &mut D,
        text: &str,
        font_name: &str,
        x: i32,
        y: i32,
    ) -> Result<(), D::Error> {
        // Ensure the requested font is set
        if self.current_font.as_ref() != Some(&font_name.to_string()) {
            self.set_font(font_name);
        }

        let mut cursor_x = x as f32;
        let baseline_y = y as f32;

        for ch in text.chars() {
            // Get glyph from cache or rasterize
            let glyph = if self.lru_glyph_cache.contains(ch, self.font_size) {
                let Some(glyph) = self.lru_glyph_cache.get(ch, self.font_size) else {
                    continue;
                };
                glyph
            } else {
                // Cache miss - get font and rasterize
                let font = match self.fonts.get(font_name) {
                    Some(f) => f,
                    None => continue,
                };
                let (metrics, bitmap) = font.rasterize(ch, self.font_size);
                self.lru_glyph_cache
                    .insert(ch, self.font_size, metrics, bitmap);
                let Some(glyph) = self.lru_glyph_cache.get(ch, self.font_size) else {
                    continue;
                };
                glyph
            };

            // Draw glyph bitmap
            let glyph_x = (cursor_x + glyph.metrics.xmin as f32) as i32;
            let glyph_y =
                (baseline_y - glyph.metrics.ymin as f32 - glyph.metrics.height as f32) as i32;

            self.draw_glyph(
                display,
                glyph_x,
                glyph_y,
                &glyph.bitmap,
                glyph.metrics.width,
                glyph.metrics.height,
            )?;

            cursor_x += glyph.metrics.advance_width;
        }

        Ok(())
    }

    /// Render text without caching (for no_std builds)
    #[cfg(not(feature = "std"))]
    fn render_text_uncached<D: DrawTarget<Color = BinaryColor>>(
        &self,
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
        let threshold = adaptive_glyph_threshold(bitmap, width, height);
        let mut pixels = Vec::new();
        for row in 0..height {
            for col in 0..width {
                let pixel_idx = row * width + col;
                if pixel_idx < bitmap.len() {
                    let pixel_value = bitmap[pixel_idx];
                    if pixel_value >= threshold {
                        let point = Point::new(x + col as i32, y + row as i32);
                        pixels.push(Pixel(point, BinaryColor::On));
                    }
                }
            }
        }
        display.draw_iter(pixels)
    }
}

fn adaptive_glyph_threshold(bitmap: &[u8], width: usize, height: usize) -> u8 {
    if bitmap.is_empty() || width == 0 || height == 0 {
        return 128;
    }
    let mut sum = 0u32;
    let mut ink = 0u32;
    for &px in bitmap {
        sum = sum.saturating_add(px as u32);
        if px >= 128 {
            ink = ink.saturating_add(1);
        }
    }
    let avg = (sum / bitmap.len() as u32) as i32;
    let mut threshold = avg + 18;
    let area = width.saturating_mul(height);
    if area <= 64 {
        threshold -= 10;
    } else if area >= 240 {
        threshold += 6;
    }
    let ink_ratio = (ink as f32 / bitmap.len() as f32).clamp(0.0, 1.0);
    if ink_ratio < 0.18 {
        threshold -= 8;
    } else if ink_ratio > 0.55 {
        threshold += 6;
    }
    threshold.clamp(92, 178) as u8
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

    pub fn baseline_offset(&self, _font_name: &str) -> i32 {
        12
    }
}

#[cfg(not(feature = "fontdue"))]
impl Default for FontCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    #[test]
    fn test_font_cache_new() {
        let cache = FontCache::new();
        assert_eq!(cache.font_size, 16.0);
    }

    #[test]
    fn adaptive_threshold_darkens_sparse_small_glyphs() {
        let bitmap = vec![0, 0, 0, 110, 120, 140, 0, 0, 0];
        let t = adaptive_glyph_threshold(&bitmap, 3, 3);
        assert!(t < 128);
    }

    #[test]
    fn adaptive_threshold_controls_dense_large_glyphs() {
        let bitmap = vec![200u8; 20 * 20];
        let t = adaptive_glyph_threshold(&bitmap, 20, 20);
        assert!(t >= 120);
    }

    #[cfg(all(feature = "fontdue", feature = "std"))]
    #[test]
    fn test_glyph_cache_key() {
        let key1 = GlyphCacheKey::new('A', 16.0);
        let key2 = GlyphCacheKey::new('A', 16.0);
        let key3 = GlyphCacheKey::new('B', 16.0);
        let key4 = GlyphCacheKey::new('A', 12.0);

        assert_eq!(key1, key2); // Same char and size
        assert_ne!(key1, key3); // Different char
        assert_ne!(key1, key4); // Different size
        assert_eq!(key1.size(), 16.0);
        assert_eq!(key4.size(), 12.0);
    }

    #[cfg(all(feature = "fontdue", feature = "std"))]
    #[test]
    fn test_glyph_cache_basic() {
        let mut cache = GlyphCache::new();

        // Test empty cache
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        // Insert a glyph
        let metrics = Metrics {
            xmin: 0,
            ymin: -2,
            width: 10,
            height: 12,
            advance_width: 8.0,
            advance_height: 14.0,
            bounds: OutlineBounds {
                xmin: 0.0,
                ymin: -2.0,
                width: 8.0,
                height: 12.0,
            },
        };
        let bitmap = vec![128u8; 120]; // 10x12 grayscale

        cache.insert('A', 16.0, metrics, bitmap.clone());

        // Verify insertion
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());

        // Verify retrieval
        let cached = cache.get('A', 16.0);
        assert!(cached.is_some());
        let glyph = cached.unwrap();
        assert_eq!(glyph.metrics.width, 10);
        assert_eq!(glyph.bitmap.len(), 120);

        // Verify miss
        assert!(cache.get('B', 16.0).is_none());
    }

    #[cfg(all(feature = "fontdue", feature = "std"))]
    #[test]
    fn test_glyph_cache_lru_eviction() {
        let mut cache = GlyphCache::with_capacity(3);

        // Insert 3 glyphs
        for i in 0..3 {
            let metrics = Metrics {
                xmin: 0,
                ymin: -2,
                width: 8,
                height: 10,
                advance_width: 8.0,
                advance_height: 12.0,
                bounds: OutlineBounds {
                    xmin: 0.0,
                    ymin: -2.0,
                    width: 8.0,
                    height: 10.0,
                },
            };
            cache.insert((b'A' + i as u8) as char, 16.0, metrics, vec![255u8; 80]);
        }

        assert_eq!(cache.len(), 3);

        // Access 'A' to make it recently used
        cache.get('A', 16.0);

        // Insert 4th glyph, should evict 'B' (the LRU)
        let metrics = Metrics {
            xmin: 0,
            ymin: -2,
            width: 8,
            height: 10,
            advance_width: 8.0,
            advance_height: 12.0,
            bounds: OutlineBounds {
                xmin: 0.0,
                ymin: -2.0,
                width: 8.0,
                height: 10.0,
            },
        };
        cache.insert('D', 16.0, metrics, vec![255u8; 80]);

        assert_eq!(cache.len(), 3);
        assert!(cache.contains('A', 16.0)); // Still there
        assert!(!cache.contains('B', 16.0)); // Evicted
        assert!(cache.contains('C', 16.0)); // Still there
        assert!(cache.contains('D', 16.0)); // New
    }

    #[cfg(all(feature = "fontdue", feature = "std"))]
    #[test]
    fn test_glyph_cache_clear() {
        let mut cache = GlyphCache::new();

        let metrics = Metrics {
            xmin: 0,
            ymin: -2,
            width: 8,
            height: 10,
            advance_width: 8.0,
            advance_height: 12.0,
            bounds: OutlineBounds {
                xmin: 0.0,
                ymin: -2.0,
                width: 8.0,
                height: 10.0,
            },
        };
        cache.insert('A', 16.0, metrics, vec![255u8; 80]);

        assert_eq!(cache.len(), 1);

        cache.clear();

        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert!(!cache.contains('A', 16.0));
    }

    #[cfg(all(feature = "fontdue", feature = "std"))]
    #[test]
    fn test_glyph_cache_update_existing() {
        let mut cache = GlyphCache::new();

        let metrics1 = Metrics {
            xmin: 0,
            ymin: -2,
            width: 8,
            height: 10,
            advance_width: 8.0,
            advance_height: 12.0,
            bounds: OutlineBounds {
                xmin: 0.0,
                ymin: -2.0,
                width: 8.0,
                height: 10.0,
            },
        };
        cache.insert('A', 16.0, metrics1, vec![100u8; 80]);

        // Update with new data
        let metrics2 = Metrics {
            xmin: 0,
            ymin: -2,
            width: 10,
            height: 12,
            advance_width: 10.0,
            advance_height: 14.0,
            bounds: OutlineBounds {
                xmin: 0.0,
                ymin: -2.0,
                width: 10.0,
                height: 12.0,
            },
        };
        cache.insert('A', 16.0, metrics2, vec![200u8; 120]);

        // Should still be only 1 entry
        assert_eq!(cache.len(), 1);

        let cached = cache.get('A', 16.0).unwrap();
        assert_eq!(cached.metrics.width, 10);
        assert_eq!(cached.bitmap.len(), 120);
    }
}
