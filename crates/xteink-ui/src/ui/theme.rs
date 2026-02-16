//! Theme system with metrics for consistent UI spacing and sizing.
//!
//! ## Semantic Font API
//!
//! All UI code should use exactly three font functions:
//! - `ui_font_title()` — headers, screen titles (largest)
//! - `ui_font_body()`  — primary content text (medium)
//! - `ui_font_small()` — secondary/meta text, captions (smallest)
//!
//! All three respect the global device font profile set via settings.

use core::sync::atomic::{AtomicU8, Ordering};

use embedded_graphics::mono_font::{ascii, MonoFont};

// Global device-font profile selected from Device Settings.
// 0..4 maps to increasingly larger font tiers.
// Default to profile 4 (largest) for optimal e-ink readability.
static DEVICE_FONT_PROFILE: AtomicU8 = AtomicU8::new(4);

/// Set global UI font profile from settings indices.
pub fn set_device_font_profile(font_size_index: usize, font_family_index: usize) {
    let profile = match (font_family_index, font_size_index) {
        // Monospace
        (0, 0) => 0,
        (0, 1) => 1,
        (0, 2) => 2,
        (0, _) => 3,
        // Serif
        (1, 0) => 1,
        (1, 1) => 2,
        (1, 2) => 3,
        (1, _) => 4,
        // Sans-serif (or unknown)
        (_, 0) => 0,
        (_, 1) => 2,
        (_, 2) => 3,
        (_, _) => 4,
    };
    DEVICE_FONT_PROFILE.store(profile, Ordering::Relaxed);
}

// ── Semantic font tiers ─────────────────────────────────────────────
//
// Profile  title               body                small
// 0        7x13 Bold           6x13 Bold           6x10
// 1        8x13 Bold           7x13 Bold           6x13 Bold
// 2        9x15 Bold           8x13 Bold           7x13 Bold
// 3        9x18 Bold           9x15 Bold           8x13 Bold
// 4        10x20               9x18 Bold           9x15 Bold

/// Title/header font — largest tier, for screen titles and section headers.
pub fn ui_font_title() -> &'static MonoFont<'static> {
    match DEVICE_FONT_PROFILE.load(Ordering::Relaxed) {
        0 => &ascii::FONT_7X13_BOLD,
        1 => &ascii::FONT_8X13_BOLD,
        2 => &ascii::FONT_9X15_BOLD,
        3 => &ascii::FONT_9X18_BOLD,
        4 => &ascii::FONT_10X20,
        _ => &ascii::FONT_9X18_BOLD,
    }
}

/// Body font — middle tier, for primary content and list items.
pub fn ui_font_body() -> &'static MonoFont<'static> {
    match DEVICE_FONT_PROFILE.load(Ordering::Relaxed) {
        0 => &ascii::FONT_6X13_BOLD,
        1 => &ascii::FONT_7X13_BOLD,
        2 => &ascii::FONT_8X13_BOLD,
        3 => &ascii::FONT_9X15_BOLD,
        4 => &ascii::FONT_9X18_BOLD,
        _ => &ascii::FONT_7X13_BOLD,
    }
}

/// Small font — smallest tier, for secondary text, captions, metadata.
pub fn ui_font_small() -> &'static MonoFont<'static> {
    match DEVICE_FONT_PROFILE.load(Ordering::Relaxed) {
        0 => &ascii::FONT_6X10,
        1 => &ascii::FONT_6X13_BOLD,
        2 => &ascii::FONT_7X13_BOLD,
        3 => &ascii::FONT_8X13_BOLD,
        4 => &ascii::FONT_9X15_BOLD,
        _ => &ascii::FONT_6X13_BOLD,
    }
}

/// Character width for the title font.
pub fn ui_font_title_char_width() -> i32 {
    ui_font_title().character_size.width as i32
}

/// Character width for the body font.
pub fn ui_font_body_char_width() -> i32 {
    ui_font_body().character_size.width as i32
}

/// Character width for the small font.
pub fn ui_font_small_char_width() -> i32 {
    ui_font_small().character_size.width as i32
}

// ── Backward-compatible aliases ─────────────────────────────────────

/// Alias for `ui_font_body()`. Prefer the semantic name in new code.
pub fn ui_font() -> &'static MonoFont<'static> {
    ui_font_body()
}

/// Alias for `ui_font_title()`. Prefer the semantic name in new code.
pub fn ui_font_bold() -> &'static MonoFont<'static> {
    ui_font_title()
}

/// Alias for `ui_font_body_char_width()`. Prefer the semantic name in new code.
pub fn ui_font_char_width() -> i32 {
    ui_font_body_char_width()
}

// ── Layout constants ────────────────────────────────────────────────
//
// Single source of truth for every pixel offset in the UI.
// Change a value here → it changes on every screen.

/// Layout constants for the Xteink X4 (480×800 @ 220 PPI).
///
/// All screens must use these instead of hardcoded magic numbers.
pub mod layout {
    /// Side margin (left/right padding from screen edge).
    pub const MARGIN: i32 = 20;

    /// Inner padding within cards, panels, overlays.
    pub const INNER_PAD: i32 = 10;

    // ── Vertical regions ────────────────────────────────────────────

    /// Header bar height (title + separator line area).
    pub const HEADER_H: i32 = 40;

    /// Y baseline for title text in the header.
    pub const HEADER_TEXT_Y: i32 = 28;

    /// Y baseline for subtitle / path text below the header title.
    pub const HEADER_SUB_Y: i32 = 38;

    /// Y position of the separator line below the header.
    pub const HEADER_SEP_Y: i32 = 42;

    /// Footer bar height (hints, progress bar area).
    pub const FOOTER_H: i32 = 50;

    /// Bottom tab-dot bar height (used by main activity).
    pub const BOTTOM_BAR_H: i32 = 50;

    // ── List / item metrics ─────────────────────────────────────────

    /// Default list entry height (file browser entries).
    pub const ENTRY_H: i32 = 44;

    /// Y-offset for primary text line within an entry (from entry top).
    pub const ENTRY_TEXT_Y: i32 = 16;

    /// Y-offset for secondary/meta text line within an entry.
    pub const ENTRY_META_Y: i32 = 34;

    // ── Spacing ─────────────────────────────────────────────────────

    /// Small gap (between tight elements).
    pub const GAP_SM: i32 = 8;

    /// Medium gap (section separators, card-to-list).
    pub const GAP_MD: i32 = 18;

    /// Large gap (major section breaks).
    pub const GAP_LG: i32 = 30;

    // ── Selection highlight ─────────────────────────────────────────

    /// Horizontal overshoot for selection highlight rectangles.
    pub const SELECT_PAD_X: i32 = 5;

    /// Separator line thickness.
    pub const SEP_THICKNESS: u32 = 1;

    /// Selection bar width (left-edge indicator in file browser).
    pub const SELECT_BAR_W: u32 = 4;

    // ── Tab dots (main activity) ────────────────────────────────────

    /// Diameter of tab indicator dots.
    pub const DOT_SIZE: u32 = 10;

    /// Horizontal spacing between tab dots.
    pub const DOT_SPACING: i32 = 24;

    // ── EPUB reader footer ──────────────────────────────────────────

    /// EPUB reading footer height.
    pub const EPUB_FOOTER_H: i32 = 36;

    /// Bottom padding below epub footer text.
    pub const EPUB_FOOTER_BOTTOM_PAD: i32 = 12;

    /// Gap between page content and epub footer.
    pub const EPUB_FOOTER_TOP_GAP: i32 = 8;

    // ── Overlay panels ──────────────────────────────────────────────

    /// Padding around overlay panel border.
    pub const OVERLAY_BORDER_PAD: i32 = 4;

    /// Row height inside overlay menus (TOC, quick menu).
    pub const OVERLAY_ROW_H: i32 = 22;

    /// Outer margin around overlay panel (from screen edge).
    pub const OVERLAY_PANEL_MARGIN: i32 = 16;

    /// Y offset from panel top to title baseline.
    pub const OVERLAY_TITLE_Y: i32 = 22;

    /// Y offset from panel top to first content row baseline.
    pub const OVERLAY_CONTENT_Y: i32 = 56;

    /// Bottom margin for hint text inside overlay (from panel bottom).
    pub const OVERLAY_HINT_BOTTOM: i32 = 14;

    /// Inset from panel edge for selection highlight.
    pub const OVERLAY_SELECT_INSET: i32 = 6;

    /// Jump-percent progress bar height.
    pub const OVERLAY_BAR_H: i32 = 16;

    // ── Library book item offsets ───────────────────────────────────

    /// Cover padding within a book list item.
    pub const BOOK_COVER_PAD: u32 = 8;

    /// Y-offset for title text within a book item (from item top).
    pub const BOOK_TITLE_Y: i32 = 20;

    /// Y-offset for author text within a book item.
    pub const BOOK_AUTHOR_Y: i32 = 40;

    /// Y-offset for progress bar within a book item.
    pub const BOOK_PROGRESS_Y: i32 = 52;

    /// Width of the progress bar in a book item.
    pub const BOOK_PROGRESS_W: u32 = 100;

    /// Scroll indicator height.
    pub const SCROLL_INDICATOR_H: u32 = 4;

    /// Scroll indicator width.
    pub const SCROLL_INDICATOR_W: i32 = 60;

    // ── Progress bar ────────────────────────────────────────────────

    /// Height of thin progress bars.
    pub const PROGRESS_BAR_H: u32 = 6;

    // ── Hero card (library tab) ─────────────────────────────────────

    /// Hero card height in library tab.
    pub const HERO_H: i32 = 140;

    // ── Cover thumbnails (library activity) ─────────────────────────

    /// Cover thumbnail width.
    pub const COVER_W: u32 = 50;

    /// Cover thumbnail max height.
    pub const COVER_MAX_H: u32 = 44;

    // ── Derived helpers ─────────────────────────────────────────────

    /// Y where content starts (below header + separator + gap).
    pub const fn content_start_y() -> i32 {
        HEADER_SEP_Y + GAP_SM
    }

    /// Calculate how many items of `item_h` fit between `start_y` and
    /// `display_height - BOTTOM_BAR_H`.
    pub const fn max_items(start_y: i32, item_h: i32, display_height: i32) -> i32 {
        let available = display_height - BOTTOM_BAR_H - start_y;
        if item_h <= 0 {
            0
        } else {
            available / item_h
        }
    }
}

/// UI spacing and sizing metrics (in pixels)
///
/// All values are optimized for the Xteink X4's 480x800 display
/// at 220 PPI, ensuring comfortable touch targets and readable text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeMetrics {
    /// Header height for navigation bars
    pub header_height: u32,
    /// Footer height for action bars
    pub footer_height: u32,
    /// Horizontal padding for side margins and item internal padding
    pub side_padding: u32,
    /// Height of each list item (sized for comfortable interaction at 220 PPI)
    pub list_item_height: u32,
    /// Standard button height
    pub button_height: u32,
    /// Base spacing unit for margins and gaps
    pub spacing: u32,
}

impl ThemeMetrics {
    /// Create metrics with custom values
    pub const fn new(
        header_height: u32,
        footer_height: u32,
        side_padding: u32,
        list_item_height: u32,
        button_height: u32,
        spacing: u32,
    ) -> Self {
        Self {
            header_height,
            footer_height,
            side_padding,
            list_item_height,
            button_height,
            spacing,
        }
    }

    /// Double spacing for larger gaps
    pub const fn spacing_double(&self) -> u32 {
        self.spacing * 2
    }

    /// Half spacing for tighter layouts
    pub const fn spacing_half(&self) -> u32 {
        self.spacing / 2
    }

    /// Total vertical padding (top + bottom)
    pub const fn vertical_padding(&self) -> u32 {
        self.spacing * 2
    }

    /// Usable content width after side padding
    pub const fn content_width(&self, display_width: u32) -> u32 {
        display_width.saturating_sub(self.side_padding * 2)
    }

    /// Usable content height after header and footer
    pub const fn content_height(&self, display_height: u32) -> u32 {
        display_height.saturating_sub(self.header_height + self.footer_height)
    }

    /// Y offset to vertically center text within a box of given height.
    pub const fn text_y_offset(height: u32) -> i32 {
        (height as i32) / 2 + 10
    }

    /// Shorthand: Y offset for centering text within a list item.
    pub const fn item_text_y(&self) -> i32 {
        Self::text_y_offset(self.list_item_height)
    }

    /// Shorthand: Y offset for centering text within the header bar.
    pub const fn header_text_y(&self) -> i32 {
        Self::text_y_offset(self.header_height)
    }

    /// Shorthand: Y offset for centering text within a button.
    pub const fn button_text_y(&self) -> i32 {
        Self::text_y_offset(self.button_height)
    }

    /// Calculate text width in pixels using the current body font.
    pub fn text_width(char_count: usize) -> i32 {
        char_count as i32 * ui_font_body_char_width()
    }

    /// How many list items fit in the content area (below header).
    pub const fn visible_items(&self, display_height: u32) -> usize {
        let content = display_height.saturating_sub(self.header_height + self.spacing_double());
        (content / (self.list_item_height + self.spacing)) as usize
    }
}

impl Default for ThemeMetrics {
    fn default() -> Self {
        Self {
            header_height: 72,
            footer_height: 56,
            side_padding: layout::MARGIN as u32,
            list_item_height: 96,
            button_height: 74,
            spacing: layout::GAP_MD as u32,
        }
    }
}

/// Complete theme definition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub metrics: ThemeMetrics,
}

impl Theme {
    /// Create a new theme with the given metrics
    pub const fn new(metrics: ThemeMetrics) -> Self {
        Self { metrics }
    }

    /// Get the default theme for Xteink X4
    pub fn default_theme() -> Self {
        Self {
            metrics: ThemeMetrics::default(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_theme()
    }
}

/// UI text rendering system - change FONT_NAME to switch fonts globally
pub mod ui_text {
    use crate::embedded_fonts::EmbeddedFontRegistry;
    use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

    /// **CHANGE THIS to switch fonts everywhere**
    /// Options: "bookerly-bold", "bookerly-regular", "bookerly-italic", "bookerly-bold-italic"
    pub const FONT_NAME: &str = "bookerly-regular";

    /// Default UI font size
    pub const DEFAULT_SIZE: u32 = 30;

    /// Header/title font size
    pub const HEADER_SIZE: u32 = 35;

    /// Small UI font size
    pub const SMALL_SIZE: u32 = 25;

    /// Render text using the configured UI font (see FONT_NAME)
    /// Returns the width of the rendered text
    ///
    /// # Arguments
    /// * `display` - Target display
    /// * `text` - Text to render
    /// * `x` - X position
    /// * `y` - Y position (baseline)
    /// * `size` - Font size in pixels (defaults to DEFAULT_SIZE if None)
    pub fn draw<D: DrawTarget<Color = BinaryColor>>(
        display: &mut D,
        text: &str,
        x: i32,
        y: i32,
        size: Option<u32>,
    ) -> Result<i32, D::Error> {
        draw_colored(display, text, x, y, size, BinaryColor::On)
    }

    /// Render text with explicit color using the configured UI font.
    pub fn draw_colored<D: DrawTarget<Color = BinaryColor>>(
        display: &mut D,
        text: &str,
        x: i32,
        y: i32,
        size: Option<u32>,
        color: BinaryColor,
    ) -> Result<i32, D::Error> {
        let size = size.unwrap_or(DEFAULT_SIZE);
        if let Some(font) = EmbeddedFontRegistry::get_font_nearest(FONT_NAME, size) {
            font.draw_text_colored(display, text, x, y, color)
        } else {
            Ok(0)
        }
    }

    /// Measure text width using the configured UI font
    pub fn width(text: &str, size: Option<u32>) -> u32 {
        let size = size.unwrap_or(DEFAULT_SIZE);
        if let Some(font) = EmbeddedFontRegistry::get_font_nearest(FONT_NAME, size) {
            font.text_width(text)
        } else {
            0
        }
    }

    /// Get line height for the configured UI font
    pub fn line_height(size: Option<u32>) -> u8 {
        let size = size.unwrap_or(DEFAULT_SIZE);
        if let Some(font) = EmbeddedFontRegistry::get_font_nearest(FONT_NAME, size) {
            font.line_height
        } else {
            size as u8
        }
    }

    /// Calculate Y position to center text vertically in a box
    pub fn center_y(box_height: u32, size: Option<u32>) -> i32 {
        let size = size.unwrap_or(DEFAULT_SIZE);
        let lh = line_height(Some(size)) as i32;
        (box_height as i32 / 2) + (lh / 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_metrics_values() {
        let m = ThemeMetrics::default();
        assert_eq!(m.header_height, 72);
        assert_eq!(m.footer_height, 56);
        assert_eq!(m.side_padding, layout::MARGIN as u32);
        assert_eq!(m.list_item_height, 96);
        assert_eq!(m.button_height, 74);
        assert_eq!(m.spacing, layout::GAP_MD as u32);
    }

    #[test]
    fn content_dimensions() {
        let m = ThemeMetrics::default();
        let expected_width = 480 - 2 * layout::MARGIN as u32;
        assert_eq!(m.content_width(480), expected_width);
        assert_eq!(
            m.content_height(800),
            800 - m.header_height - m.footer_height
        );
    }

    #[test]
    fn spacing_helpers() {
        let m = ThemeMetrics::default();
        assert_eq!(m.spacing_double(), 36);
        assert_eq!(m.spacing_half(), 9);
    }

    #[test]
    fn text_centering() {
        let m = ThemeMetrics::default();
        assert_eq!(m.item_text_y(), 58);
        assert_eq!(m.header_text_y(), 46);
        assert_eq!(m.button_text_y(), 47);
        // Free function
        assert_eq!(ThemeMetrics::text_y_offset(40), 30); // 40/2 + 10
    }

    #[test]
    fn text_width_calculation() {
        assert_eq!(ThemeMetrics::text_width(10), 10 * ui_font_char_width());
        assert_eq!(ThemeMetrics::text_width(0), 0);
    }

    #[test]
    fn visible_items_count() {
        let m = ThemeMetrics::default();
        // 800 - 72 header - 36 spacing = 692
        // 692 / (96 + 18) = 6.07 -> 6
        assert_eq!(m.visible_items(800), 6);
    }
}
