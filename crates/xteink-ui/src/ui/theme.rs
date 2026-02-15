//! Theme system with metrics for consistent UI spacing and sizing.

use core::sync::atomic::{AtomicU8, Ordering};

use embedded_graphics::mono_font::{ascii, MonoFont};

/// FONT_7X13 character width in pixels.
/// All text width calculations should use this instead of hardcoded `* 7`.
pub const FONT_CHAR_WIDTH: i32 = 7;

/// FONT_7X13 character height in pixels.
pub const FONT_CHAR_HEIGHT: i32 = 13;

// Global device-font profile selected from Device Settings.
// 0=6x10, 1=7x13, 2=8x13, 3=9x15, 4=10x20.
// Default to larger font (9x15) for better readability
static DEVICE_FONT_PROFILE: AtomicU8 = AtomicU8::new(3);

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

/// Current primary UI font selected by device settings.
pub fn ui_font() -> &'static MonoFont<'static> {
    match DEVICE_FONT_PROFILE.load(Ordering::Relaxed) {
        0 => &ascii::FONT_6X10,
        1 => &ascii::FONT_7X13,
        2 => &ascii::FONT_8X13,
        3 => &ascii::FONT_9X15,
        4 => &ascii::FONT_10X20,
        _ => &ascii::FONT_7X13,
    }
}

/// Current bold UI font selected by device settings.
pub fn ui_font_bold() -> &'static MonoFont<'static> {
    match DEVICE_FONT_PROFILE.load(Ordering::Relaxed) {
        0 => &ascii::FONT_6X13_BOLD,
        1 => &ascii::FONT_7X13_BOLD,
        2 => &ascii::FONT_8X13_BOLD,
        3 => &ascii::FONT_9X15_BOLD,
        4 => &ascii::FONT_9X18_BOLD,
        _ => &ascii::FONT_7X13_BOLD,
    }
}

/// Runtime character width for current UI font.
pub fn ui_font_char_width() -> i32 {
    ui_font().character_size.width as i32
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

    /// Y offset to vertically center FONT_7X13 text within a box of given height.
    ///
    /// Use as: `Text::new(text, Point::new(x, y + ThemeMetrics::text_y_offset(h)), ...)`
    /// where `y` is the top edge and `h` is the box height.
    pub const fn text_y_offset(height: u32) -> i32 {
        (height as i32) / 2 + 5
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

    /// Calculate text width in pixels for FONT_7X13.
    pub fn text_width(char_count: usize) -> i32 {
        char_count as i32 * ui_font_char_width()
    }

    /// How many list items fit in the content area (below header).
    pub const fn visible_items(&self, display_height: u32) -> usize {
        let content = display_height.saturating_sub(self.header_height + self.spacing_double());
        (content / (self.list_item_height + self.spacing)) as usize
    }
}

impl Default for ThemeMetrics {
    /// Default metrics optimized for Xteink X4 at 220 PPI.
    ///
    /// Modern web design principles adapted for e-ink:
    /// - Generous spacing for breathability
    /// - Larger text for better readability
    /// - Clean, minimal visual hierarchy
    /// - Optimized for button navigation (no touch)
    fn default() -> Self {
        Self {
            header_height: 50,    // Increased for larger font
            footer_height: 40,    // Increased for larger font
            side_padding: 20,     // More breathing room
            list_item_height: 64, // Taller for comfort and readability
            button_height: 50,    // Adequate tap target
            spacing: 12,          // Generous spacing (doubled from 6)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_metrics_values() {
        let m = ThemeMetrics::default();
        assert_eq!(m.header_height, 50);
        assert_eq!(m.footer_height, 40);
        assert_eq!(m.side_padding, 20);
        assert_eq!(m.list_item_height, 60);
        assert_eq!(m.button_height, 50);
        assert_eq!(m.spacing, 8);
    }

    #[test]
    fn content_dimensions() {
        let m = ThemeMetrics::default();
        assert_eq!(m.content_width(480), 440);
        assert_eq!(m.content_height(800), 710);
    }

    #[test]
    fn spacing_helpers() {
        let m = ThemeMetrics::default();
        assert_eq!(m.spacing_double(), 16);
        assert_eq!(m.spacing_half(), 4);
    }

    #[test]
    fn text_centering() {
        let m = ThemeMetrics::default();
        // 60px item: center at 30 + 5 = 35
        assert_eq!(m.item_text_y(), 35);
        // 50px header: center at 25 + 5 = 30
        assert_eq!(m.header_text_y(), 30);
        // 50px button: center at 25 + 5 = 30
        assert_eq!(m.button_text_y(), 30);
        // Free function
        assert_eq!(ThemeMetrics::text_y_offset(40), 25);
    }

    #[test]
    fn text_width_calculation() {
        assert_eq!(ThemeMetrics::text_width(10), 10 * ui_font_char_width());
        assert_eq!(ThemeMetrics::text_width(0), 0);
    }

    #[test]
    fn visible_items_count() {
        let m = ThemeMetrics::default();
        // 800 - 50 header - 16 double_spacing = 734
        // 734 / (60 + 8) = 10.79 → 10
        assert_eq!(m.visible_items(800), 10);
    }
}
