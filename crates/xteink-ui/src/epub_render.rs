//! EPUB rendering engine for Xteink X4
//!
//! Provides EPUB support using streaming architecture and built-in fonts only.
//! DISABLED: TTF font loading to prevent OOM crashes.
//! Memory target: <80KB for EPUB processing.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::Rectangle,
    text::Text,
};

use crate::epub::{
    layout::{LayoutEngine, Page, TextStyle},
    metadata::{EpubMetadata, ManifestItem},
    spine::Spine,
    streaming_zip::{StreamingZip, ZipError},
    tokenize_html,
};

use crate::portrait_dimensions;

/// Re-export Page and Line for backwards compatibility
pub use crate::epub::layout::{Line as EpubLine, Page as EpubPage, TextStyle as EpubTextStyle};

/// EPUB rendering errors
#[derive(Debug, Clone)]
pub enum EpubRenderError {
    /// File I/O error
    IoError(String),
    /// ZIP parsing error
    ZipError(ZipError),
    /// Metadata parsing error
    MetadataError(String),
    /// Spine parsing error
    SpineError(String),
    /// Tokenization error
    TokenizeError(String),
    /// Layout error
    LayoutError(String),
    /// Chapter not found
    ChapterNotFound(String),
}

impl core::fmt::Display for EpubRenderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EpubRenderError::IoError(msg) => write!(f, "I/O error: {}", msg),
            EpubRenderError::ZipError(e) => write!(f, "ZIP error: {:?}", e),
            EpubRenderError::MetadataError(msg) => write!(f, "Metadata error: {}", msg),
            EpubRenderError::SpineError(msg) => write!(f, "Spine error: {}", msg),
            EpubRenderError::TokenizeError(msg) => write!(f, "Tokenize error: {}", msg),
            EpubRenderError::LayoutError(msg) => write!(f, "Layout error: {}", msg),
            EpubRenderError::ChapterNotFound(id) => write!(f, "Chapter not found: {}", id),
        }
    }
}

impl From<ZipError> for EpubRenderError {
    fn from(e: ZipError) -> Self {
        EpubRenderError::ZipError(e)
    }
}

/// Streaming EPUB renderer - processes one chapter at a time
///
/// Architecture:
/// - StreamingZip: Memory-efficient ZIP file access
/// - EpubMetadata: Parsed metadata and manifest
/// - Spine: Reading order of chapters
/// - LayoutEngine: Converts tokens to paginated layout
pub struct StreamingEpubRenderer {
    /// Book metadata including manifest
    metadata: EpubMetadata,
    /// Spine defining reading order
    spine: Spine,
    /// Layout engine for pagination
    layout_engine: LayoutEngine,
    /// Current chapter's laid-out pages
    current_chapter: Vec<Page>,
    /// Current page index within chapter
    current_page_idx: usize,
    /// Current chapter index in spine
    current_chapter_idx: usize,
    /// File path (for reopening if needed)
    file_path: Option<String>,
}

/// Heap watermark logging for memory tracking
/// Logs checkpoint with label for memory profiling
#[cfg(feature = "std")]
pub fn log_heap_watermark(label: &str) {
    // Use eprintln which works on both desktop and ESP32 std builds
    eprintln!("[EPUB HEAP] {}: checkpoint", label);
}

#[cfg(not(feature = "std"))]
pub fn log_heap_watermark(_label: &str) {
    // No-op on no_std without allocator introspection
}

impl StreamingEpubRenderer {
    /// Display constants for Xteink X4
    const DISPLAY_WIDTH: f32 = 480.0;
    const DISPLAY_HEIGHT: f32 = 800.0;
    const TOP_MARGIN: f32 = 50.0;
    const BOTTOM_MARGIN: f32 = 100.0;
    const LEFT_MARGIN: f32 = 10.0;
    const RIGHT_MARGIN: f32 = 10.0;
    const HEADER_HEIGHT: f32 = 50.0;
    const FOOTER_HEIGHT: f32 = 40.0;

    /// Create empty renderer
    pub fn new() -> Self {
        log_heap_watermark("epub_new_start");

        // Calculate content area
        let content_width = Self::DISPLAY_WIDTH - Self::LEFT_MARGIN - Self::RIGHT_MARGIN;
        let content_height = Self::DISPLAY_HEIGHT
            - Self::HEADER_HEIGHT
            - Self::FOOTER_HEIGHT
            - Self::TOP_MARGIN
            - Self::BOTTOM_MARGIN;
        let line_height = 20.0; // For 10x20 font

        let renderer = Self {
            metadata: EpubMetadata::default(),
            spine: Spine::new(),
            layout_engine: LayoutEngine::new(content_width, content_height, line_height)
                .with_margins(Self::LEFT_MARGIN, Self::TOP_MARGIN),
            current_chapter: Vec::new(),
            current_page_idx: 0,
            current_chapter_idx: 0,
            file_path: None,
        };

        log_heap_watermark("epub_new_end");
        renderer
    }

    /// Load EPUB file from path
    pub fn load(&mut self, path: &str) -> Result<(), EpubRenderError> {
        use std::fs::File;
        use std::path::Path;

        log_heap_watermark("epub_load_start");

        // Resolve path
        let resolved_path = if Path::new(path).exists() {
            path.to_string()
        } else {
            let candidate = crate::filesystem::resolve_mount_path(path, "/sd");
            if Path::new(&candidate).exists() {
                candidate
            } else {
                return Err(EpubRenderError::IoError(format!(
                    "EPUB path not found: {}",
                    path
                )));
            }
        };

        // 1. Open ZIP file
        let file = File::open(&resolved_path).map_err(|e| {
            EpubRenderError::IoError(format!("Failed to open file: {}", e))
        })?;
        let mut zip = StreamingZip::new(file)?;

        log_heap_watermark("epub_zip_opened");

        // 2. Read and parse container.xml to find OPF path
        let container_entry = zip
            .get_entry("META-INF/container.xml")
            .or_else(|| zip.get_entry("meta-inf/container.xml"))
            .ok_or_else(|| {
                EpubRenderError::MetadataError("container.xml not found".to_string())
            })?;

        let container_size = container_entry.uncompressed_size as usize;
        let container_offset = container_entry.local_header_offset;
        let mut container_buf = alloc::vec![0u8; container_size];
        
        // Clone entry data to avoid borrow issues
        let _ = container_entry;
        zip.read_file_at_offset(container_offset, container_size, &mut container_buf)?;

        let opf_path = crate::epub::metadata::parse_container_xml(&container_buf)
            .map_err(EpubRenderError::MetadataError)?;

        log_heap_watermark("epub_container_parsed");

        // 3. Parse metadata from OPF
        let opf_entry = zip.get_entry(&opf_path).ok_or_else(|| {
            EpubRenderError::MetadataError(format!("OPF file not found: {}", opf_path))
        })?;

        let opf_size = opf_entry.uncompressed_size as usize;
        let opf_offset = opf_entry.local_header_offset;
        let mut opf_buf = alloc::vec![0u8; opf_size];
        
        // Clone entry data to avoid borrow issues  
        let _ = opf_entry;
        zip.read_file_at_offset(opf_offset, opf_size, &mut opf_buf)?;

        self.metadata = crate::epub::metadata::parse_opf(&opf_buf)
            .map_err(EpubRenderError::MetadataError)?;

        log_heap_watermark("epub_metadata_parsed");

        // 4. Parse spine from OPF
        self.spine = crate::epub::spine::parse_spine(&opf_buf)
            .map_err(EpubRenderError::SpineError)?;

        log_heap_watermark("epub_spine_parsed");

        // Store path for later chapter loading
        self.file_path = Some(resolved_path);
        self.current_chapter_idx = 0;

        // 5. Load first chapter
        self.load_chapter(0)?;

        log_heap_watermark("epub_load_complete");

        Ok(())
    }

    /// Load a specific chapter by index
    pub fn load_chapter(&mut self, chapter_idx: usize) -> Result<(), EpubRenderError> {
        // Validate chapter index
        if chapter_idx >= self.spine.len() {
            return Err(EpubRenderError::ChapterNotFound(format!(
                "Chapter index {} out of bounds (total: {})",
                chapter_idx,
                self.spine.len()
            )));
        }

        log_heap_watermark(&format!("epub_chapter_{}_start", chapter_idx));

        // Get file path
        let path = self.file_path.as_ref().ok_or_else(|| {
            EpubRenderError::IoError("No EPUB file loaded".to_string())
        })?;

        // Open ZIP file
        use std::fs::File;
        let file = File::open(path).map_err(|e| {
            EpubRenderError::IoError(format!("Failed to open file: {}", e))
        })?;
        let mut zip = StreamingZip::new(file)?;

        // 1. Get chapter ID from spine
        let chapter_id = self
            .spine
            .get_id(chapter_idx)
            .ok_or_else(|| EpubRenderError::ChapterNotFound("Empty spine".to_string()))?;

        // 2. Look up manifest item
        let manifest_item = self
            .metadata
            .get_item(chapter_id)
            .ok_or_else(|| EpubRenderError::ChapterNotFound(chapter_id.to_string()))?;

        // 3. Read chapter HTML from ZIP
        let html = self.read_chapter_html(&mut zip, manifest_item)?;

        log_heap_watermark("epub_html_loaded");

        // 4. Tokenize HTML
        let tokens = tokenize_html(&html).map_err(|e| EpubRenderError::TokenizeError(e.to_string()))?;

        log_heap_watermark("epub_html_tokenized");

        // 5. Layout into pages
        self.current_chapter = self.layout_engine.layout_tokens(tokens);
        self.current_page_idx = 0;
        self.current_chapter_idx = chapter_idx;

        log_heap_watermark(&format!(
            "epub_chapter_{}_layout_complete: {} pages",
            chapter_idx,
            self.current_chapter.len()
        ));

        Ok(())
    }

    /// Read chapter HTML content from ZIP
    fn read_chapter_html(
        &self,
        zip: &mut StreamingZip<std::fs::File>,
        manifest_item: &ManifestItem,
    ) -> Result<String, EpubRenderError> {
        // Try direct href first
        if let Some(entry) = zip.get_entry(&manifest_item.href) {
            let size = entry.uncompressed_size as usize;
            let offset = entry.local_header_offset;
            let mut buf = alloc::vec![0u8; size];
            zip.read_file_at_offset(offset, size, &mut buf)?;
            return String::from_utf8(buf).map_err(|e| {
                EpubRenderError::IoError(format!("Invalid UTF-8 in chapter: {}", e))
            });
        }

        // Try with common prefixes (some EPUBs have OEBPS/ or OPS/ prefixes)
        for prefix in &["OEBPS/", "OPS/", "EPUB/", "Content/"] {
            let prefixed_path = format!("{}{}", prefix, manifest_item.href);
            if let Some(entry) = zip.get_entry(&prefixed_path) {
                let size = entry.uncompressed_size as usize;
                let offset = entry.local_header_offset;
                let mut buf = alloc::vec![0u8; size];
                zip.read_file_at_offset(offset, size, &mut buf)?;
                return String::from_utf8(buf).map_err(|e| {
                    EpubRenderError::IoError(format!("Invalid UTF-8 in chapter: {}", e))
                });
            }
        }

        Err(EpubRenderError::ChapterNotFound(format!(
            "Chapter file not found in ZIP: {}",
            manifest_item.href
        )))
    }

    /// Get manifest item by ID (helper for TOC navigation)
    pub fn get_manifest_item(&self, id: &str) -> Option<&ManifestItem> {
        self.metadata.get_item(id)
    }

    /// Get current page content
    pub fn current_page(&self) -> Option<&Page> {
        self.current_chapter.get(self.current_page_idx)
    }

    /// Get current page number (1-indexed within chapter)
    pub fn current_page_number(&self) -> usize {
        self.current_page_idx + 1
    }

    /// Get total pages in current chapter
    pub fn total_pages(&self) -> usize {
        self.current_chapter.len()
    }

    /// Get total chapters in book
    pub fn total_chapters(&self) -> usize {
        self.spine.len()
    }

    /// Get current chapter number (1-indexed)
    pub fn current_chapter(&self) -> usize {
        self.current_chapter_idx + 1
    }

    /// Get book title
    pub fn title(&self) -> &str {
        &self.metadata.title
    }

    /// Get book author
    pub fn author(&self) -> &str {
        &self.metadata.author
    }

    /// Navigate to next page
    /// Returns true if moved to a new page
    pub fn next_page(&mut self) -> bool {
        if self.current_page_idx + 1 < self.current_chapter.len() {
            self.current_page_idx += 1;
            true
        } else if self.current_chapter_idx + 1 < self.spine.len() {
            // Try to load next chapter
            if self.load_chapter(self.current_chapter_idx + 1).is_ok() {
                return true;
            }
            false
        } else {
            false
        }
    }

    /// Navigate to previous page
    /// Returns true if moved to a new page
    pub fn prev_page(&mut self) -> bool {
        if self.current_page_idx > 0 {
            self.current_page_idx -= 1;
            true
        } else if self.current_chapter_idx > 0 {
            // Try to load previous chapter
            let prev_chapter = self.current_chapter_idx - 1;
            if self.load_chapter(prev_chapter).is_ok() {
                // Go to last page of previous chapter
                self.current_page_idx = self.current_chapter.len().saturating_sub(1);
                return true;
            }
            false
        } else {
            false
        }
    }

    /// Navigate to specific page in current chapter
    pub fn go_to_page(&mut self, page: usize) -> bool {
        if page > 0 && page <= self.current_chapter.len() {
            self.current_page_idx = page - 1;
            true
        } else {
            false
        }
    }

    /// Navigate to specific chapter
    pub fn go_to_chapter(&mut self, chapter: usize) -> bool {
        if chapter > 0 && chapter <= self.spine.len() {
            self.load_chapter(chapter - 1).is_ok()
        } else {
            false
        }
    }

    /// Render current page to display using built-in fonts only
    pub fn render<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        let (width, height) = portrait_dimensions(display);
        use embedded_graphics::primitives::PrimitiveStyle;

        // Clear screen
        display.clear(BinaryColor::Off)?;

        // Draw header with title
        let header_text = format!(
            "{} - Ch {}/{}",
            Self::truncate(self.title(), 20),
            self.current_chapter(),
            self.total_chapters()
        );
        let header_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        Text::new(&header_text, Point::new(10, 25), header_style).draw(display)?;

        // Header line
        Rectangle::new(Point::new(0, 32), Size::new(width, 2))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        // Get current page and render lines
        if let Some(page) = self.current_page() {
            for line in &page.lines {
                let style = match line.style {
                    TextStyle::Normal => MonoTextStyle::new(&FONT_6X10, BinaryColor::On),
                    TextStyle::Bold => MonoTextStyle::new(&FONT_10X20, BinaryColor::On),
                    TextStyle::Italic => MonoTextStyle::new(&FONT_6X10, BinaryColor::On),
                    TextStyle::BoldItalic => MonoTextStyle::new(&FONT_10X20, BinaryColor::On),
                };

                // Add header offset to y position
                let y = line.y + Self::HEADER_HEIGHT as i32;
                Text::new(&line.text, Point::new(10, y), style).draw(display)?;
            }
        }

        // Progress bar
        let bar_width = width.saturating_sub(20);
        let bar_x = 10;
        let bar_y = height as i32 - 28;
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
            "Pg {}/{} | <=Prev | >=Next",
            self.current_page_number(),
            self.total_pages()
        );
        Text::new(&footer_text, Point::new(10, height as i32 - 10), header_style).draw(display)?;

        Ok(())
    }

    /// Truncate string with ellipsis
    fn truncate(s: &str, max_len: usize) -> &str {
        if s.len() <= max_len {
            s
        } else {
            // Find a safe UTF-8 boundary
            let mut end = max_len - 3;
            while end > 0 && !s.is_char_boundary(end) {
                end -= 1;
            }
            &s[..end]
        }
    }
}

impl Default for StreamingEpubRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// BACKWARDS COMPATIBILITY: Keep old EpubRenderer as alias
// =============================================================================

/// Backwards compatibility alias - use StreamingEpubRenderer
pub type EpubRenderer = StreamingEpubRenderer;

// Re-export old types for compatibility
pub use crate::epub::layout::TextStyle as FontStyle;

/// Layout metrics (legacy, for compatibility)
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

/// Alignment options (legacy)
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Alignment {
    Left,
    Center,
    Right,
    Justified,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_epub_renderer_new() {
        let renderer = StreamingEpubRenderer::new();
        assert_eq!(renderer.total_pages(), 0);
        assert_eq!(renderer.current_page_number(), 1);
        assert_eq!(renderer.title(), "Unknown Title");
    }

    #[test]
    fn test_text_style_mapping() {
        // Verify text styles map correctly
        let normal = TextStyle::Normal;
        let bold = TextStyle::Bold;
        let italic = TextStyle::Italic;
        let bold_italic = TextStyle::BoldItalic;

        assert!(!normal.is_bold());
        assert!(bold.is_bold());
        assert!(bold_italic.is_bold());
        assert!(bold_italic.is_italic());
    }

    #[test]
    fn test_truncate() {
        assert_eq!(StreamingEpubRenderer::truncate("hello", 10), "hello");
        let truncated = StreamingEpubRenderer::truncate("hello world this is long", 10);
        assert!(truncated.len() <= 10);
    }
}
