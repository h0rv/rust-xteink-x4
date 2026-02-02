//! EPUB rendering engine for Xteink X4
//!
//! Provides full EPUB support with TTF font rendering using fontdue.
//! Designed to work within 400KB RAM constraints by processing one chapter at a time.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*, primitives::Rectangle};

use crate::font_render::FontCache;

/// EPUB book renderer
///
/// Manages loading and rendering of EPUB files with embedded TTF fonts.
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

    /// Font cache for embedded fonts
    font_cache: FontCache,

    /// Name of the current font to use
    current_font_name: String,
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

impl EpubRenderer {
    /// Content area height after margins
    const TOP_MARGIN: i32 = 50;
    const BOTTOM_MARGIN: i32 = 40;
    const LEFT_MARGIN: i32 = 10;
    const RIGHT_MARGIN: i32 = 10;

    /// Create empty renderer
    pub fn new() -> Self {
        let mut font_cache = FontCache::new();
        font_cache.set_font_size(16.0);

        Self {
            #[cfg(feature = "std")]
            file_path: None,
            current_chapter: 0,
            total_chapters: 0,
            pages: Vec::new(),
            current_page: 0,
            title: String::from("Unknown"),
            author: String::from("Unknown"),
            font_cache,
            current_font_name: String::from("default"),
        }
    }

    /// Load EPUB file from path
    #[cfg(feature = "std")]
    pub fn load(&mut self, path: &str) -> Result<(), String> {
        use epub::doc::EpubDoc;

        // Open EPUB to extract metadata
        let mut doc = EpubDoc::new(path).map_err(|e| format!("Failed to open EPUB: {:?}", e))?;

        // Extract metadata
        if let Some(title) = doc.mdata("title") {
            self.title = title.value.clone();
        }
        if let Some(author) = doc.mdata("creator") {
            self.author = author.value.clone();
        }

        self.total_chapters = doc.spine.len();
        self.current_chapter = 0;
        self.file_path = Some(path.to_string());

        // Load embedded fonts from EPUB resources
        self.load_embedded_fonts(&mut doc)?;

        // Load first chapter
        self.load_chapter(0)?;

        Ok(())
    }

    /// Load embedded TTF/OTF fonts from EPUB resources
    #[cfg(feature = "std")]
    fn load_embedded_fonts<R: std::io::Read + std::io::Seek>(
        &mut self,
        doc: &mut epub::doc::EpubDoc<R>,
    ) -> Result<(), String> {
        // Collect font paths first to avoid borrow issues
        let font_paths: Vec<(String, String)> = doc
            .resources
            .iter()
            .filter_map(|(path, resource_item)| {
                let path_str = path.as_str();
                let mime_str = resource_item.mime.as_str();
                if mime_str.starts_with("font/")
                    || path_str.ends_with(".ttf")
                    || path_str.ends_with(".otf")
                {
                    // Extract font name from path
                    let font_name = path_str
                        .split('/')
                        .next_back()
                        .and_then(|name| {
                            name.strip_suffix(".ttf")
                                .or_else(|| name.strip_suffix(".otf"))
                        })
                        .unwrap_or("embedded")
                        .to_string();
                    Some((path_str.to_string(), font_name))
                } else {
                    None
                }
            })
            .collect();

        // Now load the fonts
        for (path_str, font_name) in font_paths {
            if let Some((font_data, _)) = doc.get_resource(&path_str) {
                if let Err(e) = self.font_cache.load_font(&font_name, &font_data) {
                    eprintln!("Warning: Failed to load font '{}': {}", font_name, e);
                } else {
                    // Set as current font if it's the first one loaded
                    if self.current_font_name == "default" {
                        self.current_font_name = font_name.clone();
                    }
                }
            }
        }

        Ok(())
    }

    /// Load a specific chapter by index
    #[cfg(feature = "std")]
    pub fn load_chapter(&mut self, chapter_idx: usize) -> Result<(), String> {
        use epub::doc::EpubDoc;

        let path = self.file_path.as_ref().ok_or("No EPUB loaded")?;

        // Reopen the EPUB file
        let mut doc = EpubDoc::new(path).map_err(|e| format!("Failed to reopen EPUB: {:?}", e))?;

        if chapter_idx >= doc.spine.len() {
            return Err("Chapter index out of bounds".to_string());
        }

        doc.set_current_chapter(chapter_idx);
        self.current_chapter = chapter_idx;

        // Get chapter content as HTML string (returns tuple of content and mime type)
        let (html_content, _) = doc
            .get_current_str()
            .ok_or("Failed to read chapter content")?;

        // Convert HTML to plain text (for now)
        // TODO: Parse HTML properly to extract structure, styles, images
        let text_content = Self::html_to_text(&html_content);

        // Layout the chapter into pages using font metrics
        let metrics = LayoutMetrics::default();
        self.pages = self.paginate_text(&text_content, &metrics);
        self.current_page = 0;

        Ok(())
    }

    /// Convert HTML content to plain text
    /// This is a simple conversion - proper HTML parsing would be better
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

    /// Paginate text into pages using font metrics from FontCache
    fn paginate_text(&self, text: &str, metrics: &LayoutMetrics) -> Vec<Page> {
        let content_height = metrics.page_height as i32 - Self::TOP_MARGIN - Self::BOTTOM_MARGIN;
        let content_width =
            metrics.page_width as f32 - Self::LEFT_MARGIN as f32 - Self::RIGHT_MARGIN as f32;

        // Get font metrics for line height calculation
        let line_height = self.font_cache.line_height(&self.current_font_name);

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

            // Use FontCache to layout text into lines with actual font metrics
            let text_lines =
                self.font_cache
                    .layout_text(trimmed, &self.current_font_name, content_width);

            for text_line in text_lines {
                // Extract the text from the laid out line
                let line_text: String = text_line
                    .words
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");

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
    #[allow(dead_code)]
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

    /// Render current page to display
    pub fn render<D: DrawTarget<Color = BinaryColor>>(
        &mut self,
        display: &mut D,
    ) -> Result<(), D::Error> {
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
        let font_name = self.current_font_name.clone();

        // Clear screen
        display.clear(BinaryColor::Off)?;

        // Draw header with title using FontCache
        let header_text = format!(
            "{} - Ch {}",
            Self::truncate(&self.title, 25),
            self.current_chapter + 1
        );
        self.font_cache
            .render_text(display, &header_text, &font_name, 10, 25)?;

        // Header line
        Rectangle::new(Point::new(0, 32), Size::new(480, 2))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        // Draw page content using FontCache
        for (text, y) in page_lines {
            if !text.is_empty() {
                self.font_cache
                    .render_text(display, &text, &font_name, 10, y)?;
            }
        }

        // Draw footer with page info using FontCache
        let footer_text = format!(
            "Pg {}/{} | Ch {}/{} | <=Prev | >=Next",
            self.current_page_number(),
            self.total_pages(),
            self.current_chapter(),
            self.total_chapters
        );
        self.font_cache
            .render_text(display, &footer_text, &font_name, 10, 790)?;

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
