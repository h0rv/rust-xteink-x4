//! Text layout engine for EPUB pagination
//!
//! Converts tokens into laid-out pages for display.
//! Uses greedy line breaking with embedded-graphics font metrics.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use crate::epub::tokenizer::Token;

/// Text style for layout (bold, italic, etc.)
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum TextStyle {
    /// Normal text
    #[default]
    Normal,
    /// Bold text
    Bold,
    /// Italic text
    Italic,
    /// Bold and italic text
    BoldItalic,
}

impl TextStyle {
    /// Check if style is bold
    pub fn is_bold(&self) -> bool {
        matches!(self, TextStyle::Bold | TextStyle::BoldItalic)
    }

    /// Check if style is italic
    pub fn is_italic(&self) -> bool {
        matches!(self, TextStyle::Italic | TextStyle::BoldItalic)
    }

    /// Apply bold flag to current style
    pub fn with_bold(&self, bold: bool) -> Self {
        match (bold, self.is_italic()) {
            (true, true) => TextStyle::BoldItalic,
            (true, false) => TextStyle::Bold,
            (false, true) => TextStyle::Italic,
            (false, false) => TextStyle::Normal,
        }
    }

    /// Apply italic flag to current style
    pub fn with_italic(&self, italic: bool) -> Self {
        match (self.is_bold(), italic) {
            (true, true) => TextStyle::BoldItalic,
            (true, false) => TextStyle::Bold,
            (false, true) => TextStyle::Italic,
            (false, false) => TextStyle::Normal,
        }
    }
}

/// A single laid-out line of text
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// Text content for this line
    pub text: String,
    /// Y position on the page
    pub y: i32,
    /// Text style (bold, italic, etc.)
    pub style: TextStyle,
}

impl Line {
    /// Create a new line
    pub fn new(text: String, y: i32, style: TextStyle) -> Self {
        Self { text, y, style }
    }

    /// Check if line is empty
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Get line length in characters
    pub fn len(&self) -> usize {
        self.text.len()
    }
}

/// A single page of laid-out content
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    /// Lines on this page
    pub lines: Vec<Line>,
    /// Page number (1-indexed)
    pub page_number: usize,
}

impl Page {
    /// Create a new empty page
    pub fn new(page_number: usize) -> Self {
        Self {
            lines: Vec::new(),
            page_number,
        }
    }

    /// Add a line to the page
    pub fn add_line(&mut self, line: Line) {
        self.lines.push(line);
    }

    /// Check if page has no lines
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Get number of lines on page
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

/// Font metrics for text measurement
#[derive(Clone, Debug)]
pub struct FontMetrics {
    /// Character width in pixels
    pub char_width: f32,
    /// Character height in pixels
    pub char_height: f32,
    /// Bold character width (typically same or slightly wider)
    pub bold_char_width: f32,
    /// Italic character width (typically same)
    pub italic_char_width: f32,
}

impl Default for FontMetrics {
    fn default() -> Self {
        // Use larger FONT_10X20 metrics for better readability on e-ink
        Self::font_10x20()
    }
}

impl FontMetrics {
    /// Create metrics for FONT_10X20
    pub fn font_10x20() -> Self {
        Self {
            char_width: 10.0,
            char_height: 20.0,
            bold_char_width: 10.0,
            italic_char_width: 10.0,
        }
    }

    /// Get character width for a specific style
    pub fn char_width_for_style(&self, style: TextStyle) -> f32 {
        match style {
            TextStyle::Normal | TextStyle::Italic => self.char_width,
            TextStyle::Bold | TextStyle::BoldItalic => self.bold_char_width,
        }
    }

    /// Measure text width for given style
    pub fn text_width(&self, text: &str, style: TextStyle) -> f32 {
        text.len() as f32 * self.char_width_for_style(style)
    }
}

/// Layout engine for converting tokens to paginated content
pub struct LayoutEngine {
    /// Available page width (pixels)
    page_width: f32,
    /// Available page height (pixels)
    #[allow(dead_code)]
    page_height: f32,
    /// Line height in pixels
    line_height: f32,
    /// Font metrics for text measurement
    font_metrics: FontMetrics,
    /// Left margin in pixels
    left_margin: f32,
    /// Top margin in pixels
    top_margin: f32,
    /// Current line being built
    current_line: String,
    /// Current line's text style
    current_style: TextStyle,
    /// Current Y position on page
    current_y: f32,
    /// Current line width used
    current_line_width: f32,
    /// Current page being built
    current_page_lines: Vec<Line>,
    /// Completed pages
    pages: Vec<Page>,
    /// Current page number
    page_number: usize,
    /// Maximum lines per page
    max_lines_per_page: usize,
    /// Current line count on page
    current_line_count: usize,
}

impl LayoutEngine {
    /// Display constants for Xteink X4
    pub const DISPLAY_WIDTH: f32 = 480.0;
    pub const DISPLAY_HEIGHT: f32 = 800.0;
    /// Side margins for comfortable reading (larger on e-ink)
    pub const DEFAULT_MARGIN: f32 = 32.0;
    /// Top margin - minimal
    pub const DEFAULT_TOP_MARGIN: f32 = 0.0;
    /// Header area for title (must match renderer HEADER_HEIGHT)
    pub const DEFAULT_HEADER_HEIGHT: f32 = 45.0;
    /// Footer area for progress (must match renderer FOOTER_HEIGHT)
    pub const DEFAULT_FOOTER_HEIGHT: f32 = 40.0;

    /// Create a new layout engine
    ///
    /// # Arguments
    /// * `page_width` - Available width for content (excluding margins)
    /// * `page_height` - Available height for content (excluding header/footer)
    /// * `line_height` - Height of each line in pixels
    pub fn new(page_width: f32, page_height: f32, line_height: f32) -> Self {
        let font_metrics = FontMetrics::default();
        // Reserve 2 extra line heights: 1 for font descent, 1 for safety margin
        let max_lines = ((page_height - line_height * 2.0) / line_height)
            .floor()
            .max(1.0) as usize;

        Self {
            page_width,
            page_height,
            line_height,
            font_metrics,
            left_margin: Self::DEFAULT_MARGIN,
            top_margin: Self::DEFAULT_MARGIN,
            current_line: String::new(),
            current_style: TextStyle::Normal,
            current_y: Self::DEFAULT_MARGIN,
            current_line_width: 0.0,
            current_page_lines: Vec::new(),
            pages: Vec::new(),
            page_number: 1,
            max_lines_per_page: max_lines.max(1),
            current_line_count: 0,
        }
    }

    /// Create layout engine with default display dimensions
    ///
    /// Content area: 416x715 (accounting for margins, header, footer)
    /// Uses 10x20 font with 26px line height for comfortable reading
    pub fn with_defaults() -> Self {
        let content_width = Self::DISPLAY_WIDTH - (Self::DEFAULT_MARGIN * 2.0);
        let content_height =
            Self::DISPLAY_HEIGHT - Self::DEFAULT_HEADER_HEIGHT - Self::DEFAULT_FOOTER_HEIGHT;
        // Line height should be ~1.3x font height for readability on e-ink
        let line_height = 26.0; // For 10x20 font (20 * 1.3)

        let mut engine = Self::new(content_width, content_height, line_height);
        // Start at top of content area (header is rendered separately)
        engine.top_margin = 0.0;
        engine.current_y = 0.0;
        engine
    }

    /// Set font metrics
    pub fn with_font_metrics(mut self, metrics: FontMetrics) -> Self {
        self.font_metrics = metrics;
        self
    }

    /// Set margins
    pub fn with_margins(mut self, left: f32, top: f32) -> Self {
        self.left_margin = left;
        self.top_margin = top;
        self.current_y = top;
        self
    }

    /// Convert tokens into laid-out pages
    pub fn layout_tokens(&mut self, tokens: Vec<Token>) -> Vec<Page> {
        self.reset();

        let mut bold_active = false;
        let mut italic_active = false;

        for token in tokens {
            match token {
                Token::Text(text) => {
                    let style = self.current_style_from_flags(bold_active, italic_active);
                    self.add_text(&text, style);
                }
                Token::ParagraphBreak => {
                    self.flush_line();
                    self.add_paragraph_space();
                }
                Token::Heading(level) => {
                    self.flush_line();
                    // Headings get extra space before (more space for higher level headings)
                    if self.current_line_count > 0 {
                        // Add 1-2 lines of space before heading based on level
                        let space_lines = if level <= 2 { 2 } else { 1 };
                        for _ in 0..space_lines {
                            self.add_paragraph_space();
                        }
                    }
                    // Headings are always bold
                    bold_active = true;
                    // Note: Currently we use same font size for all headings
                    // Future: could use larger fonts for h1-h2
                }
                Token::Emphasis(start) => {
                    self.flush_partial_word();
                    italic_active = start;
                    self.current_style = self.current_style_from_flags(bold_active, italic_active);
                }
                Token::Strong(start) => {
                    self.flush_partial_word();
                    bold_active = start;
                    self.current_style = self.current_style_from_flags(bold_active, italic_active);
                }
                Token::LineBreak => {
                    self.flush_line();
                }
            }
        }

        // Flush any remaining content
        self.flush_line();
        self.finalize_page();

        core::mem::take(&mut self.pages)
    }

    /// Reset the layout engine state
    fn reset(&mut self) {
        self.current_line.clear();
        self.current_style = TextStyle::Normal;
        self.current_y = self.top_margin;
        self.current_line_width = 0.0;
        self.current_page_lines.clear();
        self.pages.clear();
        self.page_number = 1;
        self.current_line_count = 0;
    }

    /// Get current style based on bold/italic flags
    fn current_style_from_flags(&self, bold: bool, italic: bool) -> TextStyle {
        match (bold, italic) {
            (true, true) => TextStyle::BoldItalic,
            (true, false) => TextStyle::Bold,
            (false, true) => TextStyle::Italic,
            (false, false) => TextStyle::Normal,
        }
    }

    /// Add text content, breaking into words and laying out
    fn add_text(&mut self, text: &str, style: TextStyle) {
        // Split text into words
        for word in text.split_whitespace() {
            self.add_word(word, style);
        }
    }

    /// Add a single word with greedy line breaking
    fn add_word(&mut self, word: &str, style: TextStyle) {
        let word_width = self.font_metrics.text_width(word, style);
        let space_width = if self.current_line.is_empty() {
            0.0
        } else {
            self.font_metrics.char_width_for_style(style)
        };

        let total_width = self.current_line_width + space_width + word_width;

        if total_width <= self.page_width || self.current_line.is_empty() {
            // Word fits on current line
            if !self.current_line.is_empty() {
                self.current_line.push(' ');
                self.current_line_width += space_width;
            }
            self.current_line.push_str(word);
            self.current_line_width += word_width;
            self.current_style = style; // Track style of last word
        } else {
            // Word doesn't fit, start new line
            self.flush_line();
            self.current_line.push_str(word);
            self.current_line_width = word_width;
            self.current_style = style;
        }
    }

    /// Flush current partial word state (used when style changes mid-word)
    fn flush_partial_word(&mut self) {
        // If we have content, it will continue with new style
        // No need to flush the whole line, just record style change
    }

    /// Flush current line to the page
    #[allow(dead_code)]
    fn break_line(&mut self) {
        self.flush_line();
    }

    /// Flush current line and add to page
    fn flush_line(&mut self) {
        if self.current_line.is_empty() {
            return;
        }

        // Check if we need a new page
        if self.current_line_count >= self.max_lines_per_page {
            self.finalize_page();
            self.current_y = self.top_margin;
            self.current_line_count = 0;
        }

        // Create the line
        let line = Line::new(
            core::mem::take(&mut self.current_line),
            self.current_y as i32,
            self.current_style,
        );

        self.current_page_lines.push(line);
        self.current_line_count += 1;
        self.current_y += self.line_height;
        self.current_line_width = 0.0;
    }

    /// Add paragraph spacing (half line for compact e-ink layout)
    fn add_paragraph_space(&mut self) {
        // Check if we need a new page for the space
        if self.current_line_count >= self.max_lines_per_page {
            self.finalize_page();
            self.current_y = self.top_margin;
            self.current_line_count = 0;
        }

        // Add half line space between paragraphs (12px for 24px line height)
        // This saves space while maintaining visual separation
        if self.current_line_count > 0 {
            self.current_y += self.line_height * 0.5;
        }
    }

    /// Finalize current page and start new one
    fn finalize_page(&mut self) {
        if !self.current_page_lines.is_empty() {
            let mut page = Page::new(self.page_number);
            core::mem::swap(&mut page.lines, &mut self.current_page_lines);
            self.pages.push(page);
            self.page_number += 1;
        }
    }

    /// Get the completed pages
    pub fn into_pages(mut self) -> Vec<Page> {
        self.finalize_page();
        self.pages
    }

    /// Get current page number
    pub fn current_page_number(&self) -> usize {
        self.page_number
    }

    /// Get total pages created so far
    pub fn total_pages(&self) -> usize {
        self.pages.len()
    }

    /// Measure text width for given string and style
    pub fn measure_text(&self, text: &str, style: TextStyle) -> f32 {
        self.font_metrics.text_width(text, style)
    }
}

/// Layout configuration for the engine
#[derive(Clone, Debug)]
pub struct LayoutConfig {
    /// Page width in pixels
    pub page_width: f32,
    /// Page height in pixels
    pub page_height: f32,
    /// Line height in pixels
    pub line_height: f32,
    /// Left margin in pixels
    pub left_margin: f32,
    /// Right margin in pixels
    pub right_margin: f32,
    /// Top margin in pixels
    pub top_margin: f32,
    /// Bottom margin in pixels
    pub bottom_margin: f32,
    /// Font metrics
    pub font_metrics: FontMetrics,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        let display_width = 480.0;
        let display_height = 800.0;
        let margin = 10.0;
        let header_height = 50.0;
        let footer_height = 100.0;

        Self {
            page_width: display_width - (margin * 2.0),
            page_height: display_height - header_height - footer_height - (margin * 2.0),
            line_height: 20.0,
            left_margin: margin,
            right_margin: margin,
            top_margin: margin,
            bottom_margin: margin,
            font_metrics: FontMetrics::default(),
        }
    }
}

impl LayoutConfig {
    /// Create layout engine from this configuration
    pub fn create_engine(&self) -> LayoutEngine {
        LayoutEngine::new(self.page_width, self.page_height, self.line_height)
            .with_font_metrics(self.font_metrics.clone())
            .with_margins(self.left_margin, self.top_margin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tokens() -> Vec<Token> {
        vec![
            Token::Text("This is ".to_string()),
            Token::Emphasis(true),
            Token::Text("italic".to_string()),
            Token::Emphasis(false),
            Token::Text(" and ".to_string()),
            Token::Strong(true),
            Token::Text("bold".to_string()),
            Token::Strong(false),
            Token::Text(" text.".to_string()),
            Token::ParagraphBreak,
            Token::Heading(1),
            Token::Text("Chapter Title".to_string()),
            Token::ParagraphBreak,
            Token::Text("Another paragraph with more content here.".to_string()),
            Token::ParagraphBreak,
        ]
    }

    #[test]
    fn test_layout_engine_new() {
        let engine = LayoutEngine::new(460.0, 650.0, 20.0);
        assert_eq!(engine.current_page_number(), 1);
        assert_eq!(engine.total_pages(), 0);
    }

    #[test]
    fn test_text_style() {
        let mut style = TextStyle::Normal;
        assert!(!style.is_bold());
        assert!(!style.is_italic());

        style = style.with_bold(true);
        assert!(style.is_bold());
        assert!(!style.is_italic());

        style = style.with_italic(true);
        assert!(style.is_bold());
        assert!(style.is_italic());

        style = style.with_bold(false);
        assert!(!style.is_bold());
        assert!(style.is_italic());
    }

    #[test]
    fn test_layout_tokens_basic() {
        let tokens = create_test_tokens();
        let mut engine = LayoutEngine::new(460.0, 650.0, 20.0);
        let pages = engine.layout_tokens(tokens);

        assert!(!pages.is_empty());
        assert_eq!(pages[0].page_number, 1);

        // Check that we have lines
        let total_lines: usize = pages.iter().map(|p| p.line_count()).sum();
        assert!(total_lines > 0);
    }

    #[test]
    fn test_pagination() {
        // Create a lot of text to force pagination
        let mut tokens = Vec::new();
        for i in 0..50 {
            tokens.push(Token::Text(format!(
                "This is paragraph number {} with some content. ",
                i
            )));
            tokens.push(Token::Text(
                "Here is more text to fill the line. ".to_string(),
            ));
            tokens.push(Token::Text(
                "And even more words here to make it long enough.".to_string(),
            ));
            tokens.push(Token::ParagraphBreak);
        }

        let mut engine = LayoutEngine::new(460.0, 200.0, 20.0); // Small page height
        let pages = engine.layout_tokens(tokens);

        // Should have multiple pages
        assert!(pages.len() > 1);

        // Page numbers should be sequential
        for (i, page) in pages.iter().enumerate() {
            assert_eq!(page.page_number, i + 1);
        }
    }

    #[test]
    fn test_line_breaking() {
        // Create text that should wrap
        let tokens = vec![
            Token::Text("This is a very long line of text that should definitely wrap to multiple lines because it is longer than the available width".to_string()),
            Token::ParagraphBreak,
        ];

        let mut engine = LayoutEngine::new(100.0, 200.0, 20.0); // Narrow page
        let pages = engine.layout_tokens(tokens);

        assert!(!pages.is_empty());
        // Should have multiple lines on the page
        assert!(pages[0].line_count() > 1);
    }

    #[test]
    fn test_empty_input() {
        let tokens: Vec<Token> = vec![];
        let mut engine = LayoutEngine::new(460.0, 650.0, 20.0);
        let pages = engine.layout_tokens(tokens);

        // Should have no pages for empty input
        assert!(pages.is_empty());
    }

    #[test]
    fn test_font_metrics() {
        let metrics = FontMetrics::default();
        assert_eq!(metrics.text_width("hello", TextStyle::Normal), 30.0); // 5 * 6.0
        assert_eq!(metrics.text_width("hello", TextStyle::Bold), 30.0); // 5 * 6.0

        let metrics_10x20 = FontMetrics::font_10x20();
        assert_eq!(metrics_10x20.text_width("hello", TextStyle::Normal), 50.0); // 5 * 10.0
    }

    #[test]
    fn test_page_struct() {
        let mut page = Page::new(1);
        assert!(page.is_empty());
        assert_eq!(page.line_count(), 0);

        page.add_line(Line::new("Test".to_string(), 10, TextStyle::Normal));
        assert!(!page.is_empty());
        assert_eq!(page.line_count(), 1);
    }

    #[test]
    fn test_line_struct() {
        let line = Line::new("Hello".to_string(), 50, TextStyle::Bold);
        assert_eq!(line.text, "Hello");
        assert_eq!(line.y, 50);
        assert_eq!(line.style, TextStyle::Bold);
        assert!(!line.is_empty());
        assert_eq!(line.len(), 5);
    }

    #[test]
    fn test_layout_config() {
        let config = LayoutConfig::default();
        assert_eq!(config.page_width, 460.0); // 480 - 2*10
        assert_eq!(config.line_height, 20.0);

        let engine = config.create_engine();
        assert_eq!(engine.current_page_number(), 1);
    }

    #[test]
    fn test_with_defaults() {
        let engine = LayoutEngine::with_defaults();
        assert_eq!(engine.current_page_number(), 1);
        // Default content area: 480 - 2*10 = 460 width
        // 800 - 50 - 100 - 2*10 = 630 height
    }
}
