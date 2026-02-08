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
    mono_font::{
        ascii::{FONT_6X13_ITALIC, FONT_7X13, FONT_7X13_BOLD, FONT_8X13, FONT_9X15_BOLD},
        MonoTextStyle,
    },
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::Rectangle,
    text::Text,
};
#[cfg(feature = "fontdue")]
use fontdue::{Font, FontSettings};

use epublet::{
    layout::{LayoutEngine, Page, TextStyle},
    metadata::{parse_container_xml, parse_opf, EpubMetadata, ManifestItem},
    render_prep::{RenderPrep, RenderPrepOptions, StyledEvent, StyledEventOrRun},
    spine::{parse_spine, Spine},
    zip::{StreamingZip, ZipError},
    EpubBook,
};

use crate::epub_prep::embedded_fonts_from_metadata;

/// Re-export Page and Line for backwards compatibility
pub use epublet::layout::{Line as EpubLine, Page as EpubPage, TextStyle as EpubTextStyle};

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
    /// In-memory EPUB bytes for non-filesystem loading (sim/web/tests).
    epub_bytes: Option<Vec<u8>>,
    /// EPUB-embedded body font (TTF/OTF) when available.
    #[cfg(feature = "fontdue")]
    embedded_fonts: Vec<LoadedEmbeddedFont>,
}

#[cfg(feature = "fontdue")]
struct LoadedEmbeddedFont {
    weight: u16,
    style: epublet::EmbeddedFontStyle,
    font: Font,
}

#[derive(Clone, Copy, Debug, Default)]
struct TokenStyleState {
    bold: bool,
    italic: bool,
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
    /// Side margins for comfortable reading (32px each side = 416px content width)
    const SIDE_MARGIN: f32 = 32.0;
    /// Top margin - compact header area
    const TOP_MARGIN: f32 = 25.0;
    /// Bottom margin - space for footer
    const BOTTOM_MARGIN: f32 = 30.0;
    /// Header height - minimal (just title)
    #[allow(dead_code)]
    const HEADER_HEIGHT: f32 = 0.0;
    /// Footer height - space for progress bar and page numbers
    #[allow(dead_code)]
    const FOOTER_HEIGHT: f32 = 30.0;

    /// Create empty renderer with improved layout
    pub fn new() -> Self {
        log_heap_watermark("epub_new_start");

        // Calculate content area: 416px wide (480 - 32*2), ~745px tall
        let content_width = Self::DISPLAY_WIDTH - (Self::SIDE_MARGIN * 2.0);
        let content_height = Self::DISPLAY_HEIGHT - Self::TOP_MARGIN - Self::BOTTOM_MARGIN;
        // Line height: 24px for 10x20 font (1.2x for readability)
        let line_height = 24.0;

        let renderer = Self {
            metadata: EpubMetadata::default(),
            spine: Spine::new(),
            layout_engine: LayoutEngine::new(content_width, content_height, line_height)
                .with_margins(Self::SIDE_MARGIN, Self::TOP_MARGIN),
            current_chapter: Vec::new(),
            current_page_idx: 0,
            current_chapter_idx: 0,
            file_path: None,
            epub_bytes: None,
            #[cfg(feature = "fontdue")]
            embedded_fonts: Vec::new(),
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

        // Parse metadata/spine from source and store location for future chapter reads.
        let file = File::open(&resolved_path)
            .map_err(|e| EpubRenderError::IoError(format!("Failed to open file: {}", e)))?;
        let mut zip = StreamingZip::new(file)?;
        self.load_book_from_zip(&mut zip)?;
        self.file_path = Some(resolved_path);
        self.epub_bytes = None;
        #[cfg(feature = "fontdue")]
        {
            self.embedded_fonts.clear();
        }
        self.current_chapter_idx = 0;

        // Load first chapter.
        self.load_chapter(0)?;

        log_heap_watermark("epub_load_complete");

        Ok(())
    }

    /// Load EPUB content from bytes (used by mock/web paths where host filesystem paths
    /// are not available).
    pub fn load_from_bytes(&mut self, data: Vec<u8>) -> Result<(), EpubRenderError> {
        use std::io::Cursor;

        log_heap_watermark("epub_load_from_bytes_start");

        let mut zip = StreamingZip::new(Cursor::new(data.as_slice()))?;
        self.load_book_from_zip(&mut zip)?;
        self.file_path = None;
        self.epub_bytes = Some(data);
        #[cfg(feature = "fontdue")]
        {
            self.embedded_fonts.clear();
        }
        self.current_chapter_idx = 0;
        self.load_chapter(0)?;

        log_heap_watermark("epub_load_from_bytes_complete");
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

        // Build styled stream via epublet render-prep APIs and translate to layout tokens.
        let tokens = if let Some(path) = self.file_path.as_ref() {
            let file = std::fs::File::open(path)
                .map_err(|e| EpubRenderError::IoError(format!("Failed to open file: {}", e)))?;
            let mut book = EpubBook::from_reader(file)
                .map_err(|e| EpubRenderError::IoError(format!("Failed to open EPUB: {}", e)))?;
            Self::chapter_tokens_from_book(&mut book, chapter_idx)?
        } else if let Some(bytes) = self.epub_bytes.as_ref() {
            use std::io::Cursor;

            let mut book = EpubBook::from_reader(Cursor::new(bytes.as_slice()))
                .map_err(|e| EpubRenderError::IoError(format!("Failed to open EPUB: {}", e)))?;
            Self::chapter_tokens_from_book(&mut book, chapter_idx)?
        } else {
            return Err(EpubRenderError::IoError(
                "No EPUB file source available".to_string(),
            ));
        };

        // Layout into pages.
        self.current_chapter = self.layout_engine.layout_tokens(&tokens);
        self.current_page_idx = 0;
        self.current_chapter_idx = chapter_idx;

        log_heap_watermark(&format!(
            "epub_chapter_{}_layout_complete: {} pages",
            chapter_idx,
            self.current_chapter.len()
        ));

        Ok(())
    }

    fn load_book_from_zip<R: std::io::Read + std::io::Seek>(
        &mut self,
        zip: &mut StreamingZip<R>,
    ) -> Result<(), EpubRenderError> {
        log_heap_watermark("epub_zip_opened");

        // Read and parse container.xml to find OPF path.
        let container_entry = zip
            .get_entry("META-INF/container.xml")
            .or_else(|| zip.get_entry("meta-inf/container.xml"))
            .ok_or_else(|| EpubRenderError::MetadataError("container.xml not found".to_string()))?;

        let container_size = container_entry.uncompressed_size as usize;
        let container_offset = container_entry.local_header_offset;
        let mut container_buf = alloc::vec![0u8; container_size];
        let _ = container_entry;
        zip.read_file_at_offset(container_offset, &mut container_buf)?;
        let opf_path = parse_container_xml(&container_buf)
            .map_err(|e| EpubRenderError::MetadataError(e.to_string()))?;

        log_heap_watermark("epub_container_parsed");

        // Parse OPF, metadata, and spine.
        let opf_entry = zip.get_entry(&opf_path).ok_or_else(|| {
            EpubRenderError::MetadataError(format!("OPF file not found: {}", opf_path))
        })?;
        let opf_size = opf_entry.uncompressed_size as usize;
        let opf_offset = opf_entry.local_header_offset;
        let mut opf_buf = alloc::vec![0u8; opf_size];
        let _ = opf_entry;
        zip.read_file_at_offset(opf_offset, &mut opf_buf)?;

        self.metadata =
            parse_opf(&opf_buf).map_err(|e| EpubRenderError::MetadataError(e.to_string()))?;
        log_heap_watermark("epub_metadata_parsed");

        self.spine =
            parse_spine(&opf_buf).map_err(|e| EpubRenderError::SpineError(e.to_string()))?;
        log_heap_watermark("epub_spine_parsed");

        #[cfg(feature = "fontdue")]
        self.try_load_embedded_fonts(zip, &opf_path);

        Ok(())
    }

    #[cfg(feature = "fontdue")]
    fn try_load_embedded_fonts<R: std::io::Read + std::io::Seek>(
        &mut self,
        zip: &mut StreamingZip<R>,
        _opf_path: &str,
    ) {
        let mut faces = embedded_fonts_from_metadata(&self.metadata);
        faces.sort_by_key(|face| {
            let family = face.family.to_ascii_lowercase();
            let serif_hint = family.contains("serif")
                || family.contains("times")
                || family.contains("georgia")
                || family.contains("garamond")
                || family.contains("baskerville");
            if serif_hint {
                0
            } else {
                1
            }
        });

        self.embedded_fonts.clear();
        for face in faces {
            let Some(bytes) = Self::read_resource_bytes(zip, &face.href) else {
                continue;
            };
            if let Ok(font) = Font::from_bytes(bytes, FontSettings::default()) {
                self.embedded_fonts.push(LoadedEmbeddedFont {
                    weight: face.weight,
                    style: face.style,
                    font,
                });
                if self.embedded_fonts.len() >= 8 {
                    break;
                }
            }
        }
    }

    fn read_resource_bytes<R: std::io::Read + std::io::Seek>(
        zip: &mut StreamingZip<R>,
        path: &str,
    ) -> Option<Vec<u8>> {
        if let Some(entry) = zip.get_entry(path) {
            let size = entry.uncompressed_size as usize;
            let offset = entry.local_header_offset;
            let mut buf = alloc::vec![0u8; size];
            let _ = entry;
            if zip.read_file_at_offset(offset, &mut buf).is_ok() {
                return Some(buf);
            }
        }
        None
    }

    fn chapter_tokens_from_book<R: std::io::Read + std::io::Seek>(
        book: &mut EpubBook<R>,
        chapter_idx: usize,
    ) -> Result<Vec<epublet::Token>, EpubRenderError> {
        let mut out = Vec::new();
        let mut style_state = TokenStyleState::default();
        let mut prep = RenderPrep::new(RenderPrepOptions::default()).with_serif_default();
        prep = prep
            .with_embedded_fonts_from_book(book)
            .map_err(|e| EpubRenderError::TokenizeError(e.to_string()))?;
        prep.prepare_chapter_with(book, chapter_idx, |item| {
            Self::push_styled_item_as_token(&mut out, &item, &mut style_state);
        })
        .map_err(|e| EpubRenderError::TokenizeError(e.to_string()))?;
        Self::apply_style_tokens(&mut out, &mut style_state, false, false);
        Ok(out)
    }

    fn push_styled_item_as_token(
        out: &mut Vec<epublet::Token>,
        item: &StyledEventOrRun,
        style_state: &mut TokenStyleState,
    ) {
        match item {
            StyledEventOrRun::Run(run) => {
                if !run.text.is_empty() {
                    let want_bold = run.style.weight >= 600;
                    let want_italic = run.style.italic;
                    Self::apply_style_tokens(out, style_state, want_bold, want_italic);
                    out.push(epublet::Token::Text(run.text.clone()));
                }
            }
            StyledEventOrRun::Event(event) => match event {
                StyledEvent::ParagraphStart => {}
                StyledEvent::ParagraphEnd => {
                    Self::apply_style_tokens(out, style_state, false, false);
                    out.push(epublet::Token::ParagraphBreak);
                }
                StyledEvent::HeadingStart(level) => out.push(epublet::Token::Heading(*level)),
                StyledEvent::HeadingEnd(_) => {
                    Self::apply_style_tokens(out, style_state, false, false);
                    out.push(epublet::Token::ParagraphBreak);
                }
                StyledEvent::ListItemStart => out.push(epublet::Token::ListItemStart),
                StyledEvent::ListItemEnd => {
                    Self::apply_style_tokens(out, style_state, false, false);
                    out.push(epublet::Token::ParagraphBreak);
                }
                StyledEvent::LineBreak => out.push(epublet::Token::LineBreak),
            },
        }
    }

    fn apply_style_tokens(
        out: &mut Vec<epublet::Token>,
        state: &mut TokenStyleState,
        want_bold: bool,
        want_italic: bool,
    ) {
        if state.bold != want_bold {
            out.push(epublet::Token::Strong(want_bold));
            state.bold = want_bold;
        }
        if state.italic != want_italic {
            out.push(epublet::Token::Emphasis(want_italic));
            state.italic = want_italic;
        }
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
    /// Uses consistent 10x20 font for better readability
    pub fn render<D>(&self, display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = BinaryColor>,
    {
        let size = display.bounding_box().size;
        let width = size.width.min(size.height);
        let height = size.width.max(size.height);
        use embedded_graphics::primitives::PrimitiveStyle;

        // Clear screen
        display.clear(BinaryColor::Off)?;

        // Margins - must match LayoutEngine::DEFAULT_MARGIN (32px)
        const MARGIN: i32 = 32;
        const HEADER_HEIGHT: i32 = 45; // Space for title + padding
        const FOOTER_HEIGHT: i32 = 40; // Space for progress bar + page numbers

        // Header with book title.
        let header_text = Self::truncate(self.title(), 52).to_string();
        let header_style = MonoTextStyle::new(&FONT_7X13_BOLD, BinaryColor::On);
        Text::new(&header_text, Point::new(MARGIN, 25), header_style).draw(display)?;

        Rectangle::new(
            Point::new(MARGIN, HEADER_HEIGHT - 9),
            Size::new(width.saturating_sub((MARGIN as u32) * 2), 1),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display)?;

        // Render styled spans line-by-line for richer typography.
        if let Some(page) = self.current_page() {
            for line in &page.lines {
                // Offset line y-position by header height so text doesn't overlap title
                let y = line.y + HEADER_HEIGHT;
                let mut x = MARGIN;
                for span in &line.spans {
                    let text = span.text.as_str();
                    if text.is_empty() {
                        continue;
                    }

                    #[cfg(feature = "fontdue")]
                    if let Some(font) = self.select_embedded_font(span.style) {
                        let drawn = Self::draw_fontdue_text(display, font, text, x, y, span.style)?;
                        x += drawn;
                        continue;
                    }

                    let style = Self::text_style_for(span.style);
                    Text::new(text, Point::new(x, y), style).draw(display)?;
                    if span.style == TextStyle::BoldItalic {
                        Text::new(text, Point::new(x + 1, y), style).draw(display)?;
                    }
                    x += Self::text_width_px(text, span.style);
                }
            }
        }

        // Progress bar at bottom
        let bar_width = width.saturating_sub(64) as i32; // MARGIN * 2
        let bar_x = MARGIN;
        let bar_y = height as i32 - FOOTER_HEIGHT + 5;
        let total_pages = self.total_pages().max(1);
        let filled = ((bar_width as usize * self.current_page_number()) / total_pages) as u32;
        Rectangle::new(Point::new(bar_x, bar_y), Size::new(bar_width as u32, 4))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;
        Rectangle::new(Point::new(bar_x, bar_y), Size::new(filled, 4))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        // Compact footer with just page numbers
        let footer_text = format!(
            "Ch {}/{} | Pg {}/{}",
            self.current_chapter(),
            self.total_chapters(),
            self.current_page_number(),
            self.total_pages()
        );
        let footer_style = MonoTextStyle::new(&FONT_7X13, BinaryColor::On);
        Text::new(
            &footer_text,
            Point::new(MARGIN, height as i32 - 12),
            footer_style,
        )
        .draw(display)?;

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

    fn text_style_for(style: TextStyle) -> MonoTextStyle<'static, BinaryColor> {
        match style {
            // Default to a serif-like reading feel by choosing a smaller, denser body face.
            TextStyle::Normal => MonoTextStyle::new(&FONT_8X13, BinaryColor::On),
            TextStyle::Bold => MonoTextStyle::new(&FONT_9X15_BOLD, BinaryColor::On),
            TextStyle::Italic => MonoTextStyle::new(&FONT_6X13_ITALIC, BinaryColor::On),
            TextStyle::BoldItalic => MonoTextStyle::new(&FONT_7X13_BOLD, BinaryColor::On),
            _ => MonoTextStyle::new(&FONT_8X13, BinaryColor::On),
        }
    }

    fn text_width_px(text: &str, style: TextStyle) -> i32 {
        let char_w = match style {
            TextStyle::Normal => FONT_8X13.character_size.width as i32,
            TextStyle::Bold => FONT_9X15_BOLD.character_size.width as i32,
            TextStyle::Italic => FONT_6X13_ITALIC.character_size.width as i32,
            TextStyle::BoldItalic => FONT_7X13_BOLD.character_size.width as i32,
            _ => FONT_8X13.character_size.width as i32,
        };
        (text.chars().count() as i32) * char_w
    }

    #[cfg(feature = "fontdue")]
    fn select_embedded_font(&self, style: TextStyle) -> Option<&Font> {
        let want_bold = matches!(style, TextStyle::Bold | TextStyle::BoldItalic);
        let want_italic = matches!(style, TextStyle::Italic | TextStyle::BoldItalic);

        let exact = self.embedded_fonts.iter().find(|f| {
            let is_bold = f.weight >= 700;
            let is_italic = matches!(
                f.style,
                epublet::EmbeddedFontStyle::Italic | epublet::EmbeddedFontStyle::Oblique
            );
            is_bold == want_bold && is_italic == want_italic
        });
        if let Some(face) = exact {
            return Some(&face.font);
        }

        let style_only = self.embedded_fonts.iter().find(|f| {
            let is_italic = matches!(
                f.style,
                epublet::EmbeddedFontStyle::Italic | epublet::EmbeddedFontStyle::Oblique
            );
            is_italic == want_italic
        });
        if let Some(face) = style_only {
            return Some(&face.font);
        }

        self.embedded_fonts.first().map(|f| &f.font)
    }

    #[cfg(feature = "fontdue")]
    fn draw_fontdue_text<D: DrawTarget<Color = BinaryColor>>(
        display: &mut D,
        font: &Font,
        text: &str,
        x: i32,
        y: i32,
        style: TextStyle,
    ) -> Result<i32, D::Error> {
        let size = match style {
            TextStyle::Bold => 18.0,
            TextStyle::Italic => 17.0,
            TextStyle::BoldItalic => 18.0,
            _ => 17.0,
        };

        let mut cursor_x = x as f32;
        for ch in text.chars() {
            let (metrics, bitmap) = font.rasterize(ch, size);
            let glyph_x = (cursor_x + metrics.xmin as f32) as i32;
            let glyph_y = (y as f32 - metrics.ymin as f32 - metrics.height as f32) as i32;

            Self::draw_fontdue_glyph(
                display,
                glyph_x,
                glyph_y,
                &bitmap,
                metrics.width,
                metrics.height,
            )?;
            if matches!(style, TextStyle::Bold | TextStyle::BoldItalic) {
                Self::draw_fontdue_glyph(
                    display,
                    glyph_x + 1,
                    glyph_y,
                    &bitmap,
                    metrics.width,
                    metrics.height,
                )?;
            }

            cursor_x += metrics.advance_width;
        }

        Ok((cursor_x as i32) - x)
    }

    #[cfg(feature = "fontdue")]
    fn draw_fontdue_glyph<D: DrawTarget<Color = BinaryColor>>(
        display: &mut D,
        x: i32,
        y: i32,
        bitmap: &[u8],
        width: usize,
        height: usize,
    ) -> Result<(), D::Error> {
        let mut pixels = Vec::new();
        for row in 0..height {
            for col in 0..width {
                let pixel_idx = row * width + col;
                if pixel_idx < bitmap.len() && bitmap[pixel_idx] > 128 {
                    pixels.push(Pixel(
                        Point::new(x + col as i32, y + row as i32),
                        BinaryColor::On,
                    ));
                }
            }
        }
        display.draw_iter(pixels)
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
pub use epublet::layout::TextStyle as FontStyle;

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
