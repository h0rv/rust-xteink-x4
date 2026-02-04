//! EPUB rendering engine for Xteink X4
//!
//! Provides EPUB support using built-in fonts only.
//! DISABLED: TTF font loading to prevent OOM crashes.
//! Memory target: <80KB for EPUB processing.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::Rectangle,
    text::Text,
};

use crate::portrait_dimensions;

/// EPUB book renderer
///
/// Manages loading and rendering of EPUB files using built-in fonts.
/// Processes one chapter at a time to manage memory usage.
pub struct EpubRenderer {
    /// Path to the EPUB file
    #[cfg(feature = "std")]
    file_path: Option<String>,

    /// Current chapter index in spine
    current_chapter: usize,

    /// Total chapters in book
    total_chapters: usize,

    /// Current chapter content as laid out lines
    pages: Vec<Page>,

    /// Current page index
    current_page: usize,

    /// Book metadata
    title: String,
    author: String,

    /// Font size for rendering (pixels)
    font_size: f32,
    /// Line height for rendering (pixels)  
    line_height: f32,
}

/// A single page of rendered content
#[derive(Clone)]
pub struct Page {
    /// Lines of text on this page
    pub lines: Vec<Line>,
    /// Page number (1-indexed)
    pub page_number: usize,
}

/// A single line of text with positioning info
#[derive(Clone)]
pub struct Line {
    /// Text content
    pub text: String,
    /// Y position on screen
    pub y: i32,
    /// Font style (normal, bold, italic)
    pub style: FontStyle,
}

/// Font style for text
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FontStyle {
    Normal,
    Bold,
    Italic,
    BoldItalic,
}

/// Text alignment
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Alignment {
    Left,
    Center,
    Right,
    Justified,
}

/// Rendering metrics for layout
pub struct LayoutMetrics {
    /// Font size in pixels
    pub font_size: f32,
    /// Line height in pixels
    pub line_height: f32,
    /// Page width in pixels
    pub page_width: u32,
    /// Page height in pixels  
    pub page_height: u32,
    /// Margins in pixels
    pub margin: u32,
}

impl Default for LayoutMetrics {
    fn default() -> Self {
        Self {
            font_size: 16.0,
            line_height: 24.0,
            page_width: 480,
            page_height: 800,
            margin: 20,
        }
    }
}

/// Heap watermark logging for memory tracking
/// Logs checkpoint with label for memory profiling
#[cfg(feature = "std")]
pub fn log_heap_watermark(label: &str) {
    // Use eprintln which works on both desktop and ESP32 std builds
    // The actual heap stats are logged separately from main.rs
    eprintln!("[EPUB HEAP] {}: checkpoint", label);
}

#[cfg(not(feature = "std"))]
pub fn log_heap_watermark(_label: &str) {
    // No-op on no_std without allocator introspection
}

impl EpubRenderer {
    /// Content area height after margins
    const TOP_MARGIN: i32 = 50;
    const BOTTOM_MARGIN: i32 = 40;
    const LEFT_MARGIN: i32 = 10;
    const RIGHT_MARGIN: i32 = 10;
    /// Character width for built-in font (FONT_6X10 is 6px wide)
    const CHAR_WIDTH: f32 = 6.0;
    const CHAR_HEIGHT: f32 = 10.0;

    /// Create empty renderer
    pub fn new() -> Self {
        log_heap_watermark("epub_new_start");

        let renderer = Self {
            #[cfg(feature = "std")]
            file_path: None,
            current_chapter: 0,
            total_chapters: 0,
            pages: Vec::new(),
            current_page: 0,
            title: String::from("Unknown"),
            author: String::from("Unknown"),
            font_size: Self::CHAR_HEIGHT,
            line_height: Self::CHAR_HEIGHT * 1.2,
        };

        log_heap_watermark("epub_new_end");
        renderer
    }

    /// Load EPUB file from path
    #[cfg(feature = "std")]
    pub fn load(&mut self, path: &str) -> Result<(), String> {
        use epub::doc::EpubDoc;
        use std::path::Path;

        log_heap_watermark("epub_load_start");

        let resolved_path = if Path::new(path).exists() {
            path.to_string()
        } else {
            let candidate = crate::filesystem::resolve_mount_path(path, "/sd");
            if Path::new(&candidate).exists() {
                candidate
            } else {
                return Err(format!("EPUB path not found: {}", path));
            }
        };

        // Open EPUB to extract metadata
        let mut doc =
            EpubDoc::new(&resolved_path).map_err(|e| format!("Failed to open EPUB: {:?}", e))?;

        log_heap_watermark("epub_doc_opened");

        // Extract metadata (lightweight - just strings)
        if let Some(title) = doc.mdata("title") {
            self.title = title.value.clone();
        }
        if let Some(author) = doc.mdata("creator") {
            self.author = author.value.clone();
        }

        self.total_chapters = doc.spine.len();
        self.current_chapter = 0;
        self.file_path = Some(resolved_path);

        log_heap_watermark("epub_metadata_extracted");

        // DISABLED: Loading embedded fonts causes OOM (500KB+ allocation)
        // self.load_embedded_fonts(&mut doc)?;

        // Using built-in MonoTextStyle fonts instead - zero extra memory
        eprintln!("[EPUB] Using built-in fonts (embedded font loading DISABLED to save memory)");

        // Load first chapter only (streaming approach)
        self.load_chapter(0)?;

        log_heap_watermark("epub_load_complete");

        Ok(())
    }

    /// DISABLED: Load embedded TTF/OTF fonts from EPUB resources
    ///
    /// This function is kept for reference but NOT called to prevent OOM crashes.
    /// EPUB embedded fonts can be 500KB+ which exceeds ESP32-C3 RAM constraints.
    #[cfg(feature = "std")]
    #[allow(dead_code)]
    fn load_embedded_fonts<R: std::io::Read + std::io::Seek>(
        &mut self,
        _doc: &mut epub::doc::EpubDoc<R>,
    ) -> Result<(), String> {
        // DISABLED: Font loading causes "memory allocation of 43280 bytes failed"
        // and larger allocations for typical embedded fonts (500KB+)
        eprintln!("[EPUB] Skipping embedded font loading - using built-in fonts");
        Ok(())
    }

    /// Load a specific chapter by index
    #[cfg(feature = "std")]
    pub fn load_chapter(&mut self, chapter_idx: usize) -> Result<(), String> {
        use epub::doc::EpubDoc;

        log_heap_watermark(&format!("epub_chapter_{}_start", chapter_idx));

        let path = self.file_path.as_ref().ok_or("No EPUB loaded")?;

        // Reopen the EPUB file (streaming - doesn't hold full book in memory)
        let mut doc = EpubDoc::new(path).map_err(|e| format!("Failed to reopen EPUB: {:?}", e))?;

        if chapter_idx >= doc.spine.len() {
            return Err("Chapter index out of bounds".to_string());
        }

        doc.set_current_chapter(chapter_idx);
        self.current_chapter = chapter_idx;

        // Get chapter content as HTML string
        let (html_content, _) = doc
            .get_current_str()
            .ok_or("Failed to read chapter content")?;

        log_heap_watermark("epub_html_loaded");

        // Convert HTML to plain text (lightweight processing)
        let text_content = Self::html_to_text(&html_content);

        log_heap_watermark("epub_text_extracted");

        // Layout the chapter into pages using built-in font metrics
        let metrics = LayoutMetrics {
            font_size: self.font_size,
            line_height: self.line_height,
            ..LayoutMetrics::default()
        };
        self.pages = self.paginate_text(&text_content, &metrics);
        self.current_page = 0;

        log_heap_watermark(&format!(
            "epub_chapter_{}_paginated: {} pages",
            chapter_idx,
            self.pages.len()
        ));

        Ok(())
    }

    /// Convert HTML content to plain text
    #[cfg(feature = "std")]
    fn html_to_text(html: &str) -> String {
        // Use html2text if available
        #[cfg(feature = "html2text")]
        {
            use html2text::from_read;
            from_read(html.as_bytes(), 80).unwrap_or_else(|_| {
                // Fallback to simple HTML stripping if html2text fails
                Self::strip_html_tags(html)
            })
        }

        // Simple fallback: strip tags
        #[cfg(not(feature = "html2text"))]
        {
            Self::strip_html_tags(html)
        }
    }

    /// Simple HTML tag stripper
    fn strip_html_tags(html: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;
        let mut in_entity = false;
        let mut entity = String::new();

        for ch in html.chars() {
            if in_entity {
                if ch == ';' {
                    // End of entity
                    match entity.as_str() {
                        "amp" => result.push('&'),
                        "lt" => result.push('<'),
                        "gt" => result.push('>'),
                        "quot" => result.push('"'),
                        "nbsp" => result.push(' '),
                        _ => {} // Unknown entity, skip
                    }
                    entity.clear();
                    in_entity = false;
                } else {
                    entity.push(ch);
                }
            } else if ch == '&' {
                in_entity = true;
            } else if ch == '<' {
                in_tag = true;
            } else if ch == '>' {
                in_tag = false;
                result.push(' '); // Add space after tags
            } else if !in_tag {
                result.push(ch);
            }
        }

        // Clean up whitespace
        result.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Paginate text into pages using built-in font metrics
    /// Uses MonoTextStyle (FONT_6X10) - no external font loading
    fn paginate_text(&self, text: &str, _metrics: &LayoutMetrics) -> Vec<Page> {
        let content_height = DISPLAY_HEIGHT as i32 - Self::TOP_MARGIN - Self::BOTTOM_MARGIN;
        let content_width =
            DISPLAY_WIDTH as f32 - Self::LEFT_MARGIN as f32 - Self::RIGHT_MARGIN as f32;

        // Built-in font metrics (FONT_6X10)
        let line_height = self.line_height;
        let max_chars_per_line = ((content_width / Self::CHAR_WIDTH).floor() as usize).max(1);
        let lines_per_page = (content_height as f32 / line_height) as usize;

        let mut pages = Vec::new();
        let mut current_page_lines = Vec::new();
        let mut page_number = 1;
        let mut line_count = 0;

        for paragraph in text.split("\n\n") {
            let trimmed = paragraph.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Simple word wrapping for built-in font
            let text_lines = Self::wrap_text(trimmed, max_chars_per_line);

            for line_text in text_lines {
                if line_count >= lines_per_page {
                    // Start new page
                    pages.push(Page {
                        lines: core::mem::take(&mut current_page_lines),
                        page_number,
                    });
                    page_number += 1;
                    line_count = 0;
                }

                let y = Self::TOP_MARGIN + (line_count as f32 * line_height) as i32;
                current_page_lines.push(Line {
                    text: line_text,
                    y,
                    style: FontStyle::Normal,
                });
                line_count += 1;
            }

            // Add paragraph break (empty line)
            if line_count < lines_per_page {
                let y = Self::TOP_MARGIN + (line_count as f32 * line_height) as i32;
                current_page_lines.push(Line {
                    text: String::new(),
                    y,
                    style: FontStyle::Normal,
                });
                line_count += 1;
            }
        }

        // Don't forget the last page
        if !current_page_lines.is_empty() {
            pages.push(Page {
                lines: current_page_lines,
                page_number,
            });
        }

        if pages.is_empty() {
            // Create at least one empty page
            pages.push(Page {
                lines: vec![Line {
                    text: String::from("[Empty Chapter]"),
                    y: Self::TOP_MARGIN,
                    style: FontStyle::Normal,
                }],
                page_number: 1,
            });
        }

        pages
    }

    /// Simple word wrapping
    fn wrap_text(text: &str, width: usize) -> Vec<String> {
        let mut result = Vec::new();
        let mut current_line = String::new();

        for word in text.split_whitespace() {
            let word_len = word.len();
            let line_len = current_line.len();

            if line_len == 0 {
                // First word on line
                if word_len <= width {
                    current_line.push_str(word);
                } else {
                    // Word is longer than width, split it
                    let mut remaining = word;
                    while !remaining.is_empty() {
                        let split_point = width.min(remaining.len());
                        let (part, rest) = remaining.split_at(split_point);
                        result.push(part.to_string());
                        remaining = rest;
                    }
                }
            } else if line_len + 1 + word_len <= width {
                // Word fits on current line
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                // Word doesn't fit, start new line
                result.push(core::mem::take(&mut current_line));
                current_line.push_str(word);
            }
        }

        // Don't forget the last line
        if !current_line.is_empty() {
            result.push(current_line);
        }

        result
    }

    /// Get current page content
    pub fn current_page(&self) -> Option<&Page> {
        self.pages.get(self.current_page)
    }

    /// Get current page number (1-indexed)
    pub fn current_page_number(&self) -> usize {
        self.current_page + 1
    }

    /// Get total pages in current chapter
    pub fn total_pages(&self) -> usize {
        self.pages.len()
    }

    /// Get total chapters
    pub fn total_chapters(&self) -> usize {
        self.total_chapters
    }

    /// Get current chapter number (1-indexed)
    pub fn current_chapter(&self) -> usize {
        self.current_chapter + 1
    }

    /// Get book title
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Get book author
    pub fn author(&self) -> &str {
        &self.author
    }

    /// Navigate to next page
    /// Returns true if moved to a new page
    pub fn next_page(&mut self) -> bool {
        if self.current_page + 1 < self.pages.len() {
            self.current_page += 1;
            true
        } else if self.current_chapter + 1 < self.total_chapters {
            // Try to load next chapter
            #[cfg(feature = "std")]
            {
                if self.load_chapter(self.current_chapter + 1).is_ok() {
                    return true;
                }
            }
            false
        } else {
            false
        }
    }

    /// Navigate to previous page
    /// Returns true if moved to a new page
    pub fn prev_page(&mut self) -> bool {
        if self.current_page > 0 {
            self.current_page -= 1;
            true
        } else if self.current_chapter > 0 {
            // Try to load previous chapter
            #[cfg(feature = "std")]
            {
                let prev_chapter = self.current_chapter - 1;
                if self.load_chapter(prev_chapter).is_ok() {
                    // Go to last page of previous chapter
                    self.current_page = self.pages.len().saturating_sub(1);
                    return true;
                }
            }
            false
        } else {
            false
        }
    }

    /// Navigate to specific page in current chapter
    pub fn go_to_page(&mut self, page: usize) -> bool {
        if page > 0 && page <= self.pages.len() {
            self.current_page = page - 1;
            true
        } else {
            false
        }
    }

    /// Render current page to display using built-in fonts only
    pub fn render<D: DrawTarget<Color = BinaryColor> + OriginDimensions>(
        &mut self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let (width, height) = portrait_dimensions(display);
        use embedded_graphics::primitives::PrimitiveStyle;

        // Collect page data first to avoid borrow issues
        let page_lines: Vec<(String, i32)> = self
            .current_page()
            .map(|page| {
                page.lines
                    .iter()
                    .map(|line| (line.text.clone(), line.y))
                    .collect()
            })
            .unwrap_or_default();

        // Always use built-in MonoTextStyle - no external fonts
        let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

        // Clear screen
        display.clear(BinaryColor::Off)?;

        // Draw header with title
        let header_text = format!(
            "{} - Ch {}",
            Self::truncate(&self.title, 25),
            self.current_chapter + 1
        );
        Text::new(&header_text, Point::new(10, 25), text_style).draw(display)?;

        // Header line
        Rectangle::new(Point::new(0, 32), Size::new(width, 2))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        // Draw page content
        for (text, y) in page_lines {
            if text.is_empty() {
                continue;
            }
            Text::new(&text, Point::new(10, y), text_style).draw(display)?;
        }

        // Progress bar
        let bar_width = width.saturating_sub(20);
        let bar_x = 10;
        let bar_y = height as i32 - 18;
        let total_pages = self.total_pages().max(1);
        let filled = ((bar_width as usize * self.current_page_number()) / total_pages) as u32;
        Rectangle::new(Point::new(bar_x, bar_y), Size::new(bar_width, 6))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;
        Rectangle::new(Point::new(bar_x, bar_y), Size::new(filled, 6))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        // Draw footer with page info
        let footer_text = format!(
            "Pg {}/{} | Ch {}/{} | <=Prev | >=Next",
            self.current_page_number(),
            self.total_pages(),
            self.current_chapter(),
            self.total_chapters
        );
        Text::new(&footer_text, Point::new(10, height as i32 - 10), text_style).draw(display)?;

        Ok(())
    }

    /// Truncate string with ellipsis
    fn truncate(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len - 3])
        }
    }
}

/// Display dimensions constant
const DISPLAY_WIDTH: u32 = 480;
const DISPLAY_HEIGHT: u32 = 800;

impl Default for EpubRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_text() {
        let lines = EpubRenderer::wrap_text("Hello world this is a test", 12);
        assert_eq!(lines, vec!["Hello world", "this is a", "test"]);
    }

    #[test]
    fn test_paginate_text() {
        let text = "Line 1.\n\nLine 2.\n\nLine 3.";
        let metrics = LayoutMetrics::default();
        let pages = EpubRenderer::new().paginate_text(text, &metrics);
        assert!(!pages.is_empty());
        assert_eq!(pages[0].page_number, 1);
    }

    #[test]
    fn test_epub_renderer_new() {
        let renderer = EpubRenderer::new();
        assert_eq!(renderer.total_pages(), 0);
        assert_eq!(renderer.current_page_number(), 1);
    }
}
