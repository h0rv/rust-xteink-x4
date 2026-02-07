//! Theme system with metrics for consistent UI spacing and sizing.

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
    /// Horizontal padding for side margins
    pub side_padding: u32,
    /// Height of each list item
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
    pub fn spacing_double(&self) -> u32 {
        self.spacing * 2
    }

    /// Half spacing for tighter layouts
    pub fn spacing_half(&self) -> u32 {
        self.spacing / 2
    }

    /// Total vertical padding (top + bottom)
    pub fn vertical_padding(&self) -> u32 {
        self.spacing * 2
    }

    /// Usable content width after side padding
    pub fn content_width(&self, display_width: u32) -> u32 {
        display_width.saturating_sub(self.side_padding * 2)
    }

    /// Usable content height after header and footer
    pub fn content_height(&self, display_height: u32) -> u32 {
        display_height.saturating_sub(self.header_height + self.footer_height)
    }
}

impl Default for ThemeMetrics {
    /// Default metrics optimized for Xteink X4
    fn default() -> Self {
        Self {
            header_height: 50,
            footer_height: 40,
            side_padding: 20,
            list_item_height: 45,
            button_height: 40,
            spacing: 8,
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
        assert_eq!(m.list_item_height, 45);
        assert_eq!(m.button_height, 40);
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
}
