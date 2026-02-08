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
    layout::{Line, Page, TextSpan, TextStyle},
    metadata::{parse_container_xml, parse_opf, EpubMetadata, ManifestItem},
    render_prep::{
        BlockRole, ComputedTextStyle, RenderPrep, RenderPrepOptions, StyledEvent, StyledEventOrRun,
    },
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
    /// Current chapter's laid-out pages
    current_chapter: Vec<Page>,
    /// Per-page line metadata used for richer rendering.
    current_line_meta: Vec<Vec<LineRenderMeta>>,
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
    /// Built-in Bookerly fallback family.
    #[cfg(feature = "fontdue")]
    builtin_bookerly_fonts: Vec<LoadedEmbeddedFont>,
}

#[cfg(feature = "fontdue")]
struct LoadedEmbeddedFont {
    weight: u16,
    style: epublet::EmbeddedFontStyle,
    font: Font,
}

#[cfg(feature = "fontdue")]
const BOOKERLY_REGULAR_BYTES: &[u8] =
    include_bytes!("../assets/fonts/bookerly/Bookerly-Regular.ttf");
#[cfg(feature = "fontdue")]
const BOOKERLY_BOLD_BYTES: &[u8] = include_bytes!("../assets/fonts/bookerly/Bookerly-Bold.ttf");
#[cfg(feature = "fontdue")]
const BOOKERLY_ITALIC_BYTES: &[u8] = include_bytes!("../assets/fonts/bookerly/Bookerly-Italic.ttf");
#[cfg(feature = "fontdue")]
const BOOKERLY_BOLD_ITALIC_BYTES: &[u8] =
    include_bytes!("../assets/fonts/bookerly/Bookerly-BoldItalic.ttf");

#[derive(Clone, Debug, Default)]
struct LineBuildState {
    spans: Vec<TextSpan>,
    text: String,
    style: TextStyle,
    width_px: f32,
    line_height_px: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenderLineRole {
    Body,
    Heading(u8),
    ListItem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenderLineAlign {
    Left,
    Justify,
}

#[derive(Clone, Copy, Debug)]
struct LineRenderMeta {
    role: RenderLineRole,
    justify: bool,
    last_in_block: bool,
    align: RenderLineAlign,
    left_inset_px: i32,
    right_inset_px: i32,
}

impl Default for LineRenderMeta {
    fn default() -> Self {
        Self {
            role: RenderLineRole::Body,
            justify: true,
            last_in_block: false,
            align: RenderLineAlign::Justify,
            left_inset_px: 0,
            right_inset_px: 0,
        }
    }
}

#[derive(Clone, Debug)]
struct PaginationState {
    pages: Vec<Page>,
    pages_meta: Vec<Vec<LineRenderMeta>>,
    page_lines: Vec<Line>,
    page_meta: Vec<LineRenderMeta>,
    line: LineBuildState,
    line_meta: LineRenderMeta,
    cursor_y: f32,
    page_number: usize,
    content_height: f32,
}

impl PaginationState {
    fn new(content_height: f32) -> Self {
        Self {
            pages: Vec::new(),
            pages_meta: Vec::new(),
            page_lines: Vec::new(),
            page_meta: Vec::new(),
            line: LineBuildState::default(),
            line_meta: LineRenderMeta::default(),
            cursor_y: 0.0,
            page_number: 1,
            content_height,
        }
    }
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
    const TOP_MARGIN: f32 = 48.0;
    /// Bottom margin - space for footer
    const BOTTOM_MARGIN: f32 = 40.0;
    /// Additional baseline offset so first content line doesn't touch header rule.
    const CONTENT_BASELINE_OFFSET: f32 = 8.0;
    /// Header height - minimal (just title)
    #[allow(dead_code)]
    const HEADER_HEIGHT: f32 = 0.0;
    /// Footer height - space for progress bar and page numbers
    #[allow(dead_code)]
    const FOOTER_HEIGHT: f32 = 30.0;

    /// Create empty renderer with improved layout
    pub fn new() -> Self {
        log_heap_watermark("epub_new_start");

        let mut metadata = EpubMetadata::default();
        if metadata.title.is_empty() {
            metadata.title = "Unknown Title".to_string();
        }
        if metadata.author.is_empty() {
            metadata.author = "Unknown Author".to_string();
        }

        let renderer = Self {
            metadata,
            spine: Spine::new(),
            current_chapter: Vec::new(),
            current_line_meta: Vec::new(),
            current_page_idx: 0,
            current_chapter_idx: 0,
            file_path: None,
            epub_bytes: None,
            #[cfg(feature = "fontdue")]
            embedded_fonts: Vec::new(),
            #[cfg(feature = "fontdue")]
            builtin_bookerly_fonts: Self::load_builtin_bookerly_fonts(),
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

        // Build styled stream and paginate with style-aware layout.
        let (pages, meta) = if let Some(path) = self.file_path.as_ref() {
            let file = std::fs::File::open(path)
                .map_err(|e| EpubRenderError::IoError(format!("Failed to open file: {}", e)))?;
            let mut book = EpubBook::from_reader(file)
                .map_err(|e| EpubRenderError::IoError(format!("Failed to open EPUB: {}", e)))?;
            Self::chapter_pages_from_book(&mut book, chapter_idx)?
        } else if let Some(bytes) = self.epub_bytes.as_ref() {
            use std::io::Cursor;

            let mut book = EpubBook::from_reader(Cursor::new(bytes.as_slice()))
                .map_err(|e| EpubRenderError::IoError(format!("Failed to open EPUB: {}", e)))?;
            Self::chapter_pages_from_book(&mut book, chapter_idx)?
        } else {
            return Err(EpubRenderError::IoError(
                "No EPUB file source available".to_string(),
            ));
        };
        self.current_chapter = pages;
        self.current_line_meta = meta;
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

    #[cfg(feature = "fontdue")]
    fn load_builtin_bookerly_fonts() -> Vec<LoadedEmbeddedFont> {
        let mut fonts = Vec::new();
        Self::push_builtin_face(
            &mut fonts,
            BOOKERLY_REGULAR_BYTES,
            400,
            epublet::EmbeddedFontStyle::Normal,
        );
        Self::push_builtin_face(
            &mut fonts,
            BOOKERLY_BOLD_BYTES,
            700,
            epublet::EmbeddedFontStyle::Normal,
        );
        Self::push_builtin_face(
            &mut fonts,
            BOOKERLY_ITALIC_BYTES,
            400,
            epublet::EmbeddedFontStyle::Italic,
        );
        Self::push_builtin_face(
            &mut fonts,
            BOOKERLY_BOLD_ITALIC_BYTES,
            700,
            epublet::EmbeddedFontStyle::Italic,
        );
        fonts
    }

    #[cfg(feature = "fontdue")]
    fn push_builtin_face(
        out: &mut Vec<LoadedEmbeddedFont>,
        bytes: &'static [u8],
        weight: u16,
        style: epublet::EmbeddedFontStyle,
    ) {
        if let Ok(font) = Font::from_bytes(bytes, FontSettings::default()) {
            out.push(LoadedEmbeddedFont {
                weight,
                style,
                font,
            });
        }
    }

    fn chapter_pages_from_book<R: std::io::Read + std::io::Seek>(
        book: &mut EpubBook<R>,
        chapter_idx: usize,
    ) -> Result<(Vec<Page>, Vec<Vec<LineRenderMeta>>), EpubRenderError> {
        let mut prep = RenderPrep::new(RenderPrepOptions::default()).with_serif_default();
        prep = prep
            .with_embedded_fonts_from_book(book)
            .map_err(|e| EpubRenderError::TokenizeError(e.to_string()))?;
        let mut in_heading: Option<u8> = None;
        let mut list_depth = 0usize;
        let mut list_index = 0usize;
        let mut pending_indent = true;
        let mut suppress_next_indent = false;
        let mut current_block_role = RenderLineRole::Body;
        let content_width = Self::DISPLAY_WIDTH - (Self::SIDE_MARGIN * 2.0);
        let content_height = Self::DISPLAY_HEIGHT
            - Self::TOP_MARGIN
            - Self::BOTTOM_MARGIN
            - Self::CONTENT_BASELINE_OFFSET;
        let mut state = PaginationState::new(content_height);

        prep.prepare_chapter_with(book, chapter_idx, |item| match item {
            StyledEventOrRun::Run(run) => {
                current_block_role = match run.style.block_role {
                    BlockRole::Heading(level) => RenderLineRole::Heading(level),
                    BlockRole::ListItem => RenderLineRole::ListItem,
                    _ => {
                        if let Some(level) = in_heading {
                            RenderLineRole::Heading(level)
                        } else {
                            RenderLineRole::Body
                        }
                    }
                };
                if pending_indent
                    && state.line.spans.is_empty()
                    && state.line.text.is_empty()
                    && in_heading.is_none()
                {
                    let indent_style = TextStyle::Normal;
                    let indent = "   ";
                    state.line.style = indent_style;
                    state.line.text.push_str(indent);
                    state.line.width_px += Self::measure_text_width(indent, indent_style);
                    state.line.line_height_px = state.line.line_height_px.max(22.0);
                }
                Self::append_run_to_pages(
                    &mut state,
                    content_width,
                    &run.style,
                    in_heading,
                    &run.text,
                );
                if !run.text.trim().is_empty() {
                    pending_indent = false;
                }
            }
            StyledEventOrRun::Event(event) => match event {
                StyledEvent::ParagraphStart => {
                    pending_indent = !suppress_next_indent;
                    suppress_next_indent = false;
                }
                StyledEvent::ParagraphEnd => {
                    Self::flush_line_to_pages(&mut state);
                    Self::mark_last_line_block_end(&mut state);
                    let gap = match current_block_role {
                        RenderLineRole::Heading(_) => 10.0,
                        RenderLineRole::ListItem => 6.0,
                        RenderLineRole::Body => 8.0,
                    };
                    Self::add_vertical_space(&mut state, gap);
                    pending_indent = true;
                    current_block_role = RenderLineRole::Body;
                }
                StyledEvent::HeadingStart(level) => {
                    Self::flush_line_to_pages(&mut state);
                    Self::add_vertical_space(&mut state, if level <= 2 { 14.0 } else { 10.0 });
                    in_heading = Some(level);
                    pending_indent = false;
                }
                StyledEvent::HeadingEnd(_) => {
                    in_heading = None;
                    Self::flush_line_to_pages(&mut state);
                    Self::mark_last_line_block_end(&mut state);
                    Self::add_vertical_space(&mut state, 10.0);
                    suppress_next_indent = true;
                    pending_indent = false;
                }
                StyledEvent::ListItemStart => {
                    Self::flush_line_to_pages(&mut state);
                    list_index += 1;
                    if list_depth == 0 {
                        list_depth = 1;
                    }
                    let marker = format!("{}• ", "  ".repeat(list_depth.saturating_sub(1)));
                    let style = TextStyle::Normal;
                    let marker_width = Self::measure_text_width(&marker, style);
                    state.line.text.push_str(&marker);
                    state.line.style = style;
                    state.line.width_px += marker_width;
                    state.line.line_height_px = state.line.line_height_px.max(22.0);
                    pending_indent = false;
                }
                StyledEvent::ListItemEnd => {
                    Self::flush_line_to_pages(&mut state);
                    Self::mark_last_line_block_end(&mut state);
                    Self::add_vertical_space(&mut state, 6.0);
                    list_index = list_index.saturating_sub(1);
                    if list_index == 0 {
                        list_depth = 0;
                    }
                    pending_indent = true;
                }
                StyledEvent::LineBreak => {
                    Self::flush_line_to_pages(&mut state);
                    pending_indent = false;
                }
            },
        })
        .map_err(|e| EpubRenderError::TokenizeError(e.to_string()))?;

        Self::flush_line_to_pages(&mut state);
        if !state.page_lines.is_empty() {
            state.pages.push(Page {
                lines: state.page_lines,
                page_number: state.page_number,
            });
            state.pages_meta.push(state.page_meta);
        }

        if state.pages.is_empty() {
            state.pages.push(Page::new(1));
            state.pages_meta.push(Vec::new());
        }
        Ok((state.pages, state.pages_meta))
    }

    #[cfg(test)]
    fn infer_heading_level(style: &ComputedTextStyle) -> Option<u8> {
        match style.block_role {
            BlockRole::Heading(level) => Some(level.clamp(1, 6)),
            _ => {
                if style.size_px >= 24.0 {
                    Some(1)
                } else if style.size_px >= 21.0 {
                    Some(2)
                } else if style.size_px >= 19.0 {
                    Some(3)
                } else {
                    None
                }
            }
        }
    }

    fn text_style_from_computed(style: &ComputedTextStyle, in_heading: Option<u8>) -> TextStyle {
        let heading_like =
            matches!(style.block_role, BlockRole::Heading(_)) || in_heading.is_some();
        let bold = style.weight >= 600 || heading_like;
        let italic = style.italic;
        match (bold, italic) {
            (true, true) => TextStyle::BoldItalic,
            (true, false) => TextStyle::Bold,
            (false, true) => TextStyle::Italic,
            (false, false) => TextStyle::Normal,
        }
    }

    fn line_height_px(style: &ComputedTextStyle, in_heading: Option<u8>) -> f32 {
        let mut px = style.size_px * style.line_height;
        if let Some(level) = in_heading {
            let boost = match level {
                1 => 1.15,
                2 => 1.10,
                3 => 1.06,
                _ => 1.03,
            };
            px *= boost;
        } else if let BlockRole::Heading(level) = style.block_role {
            let boost = match level {
                1 => 1.15,
                2 => 1.10,
                3 => 1.06,
                _ => 1.03,
            };
            px *= boost;
        }
        px.clamp(18.0, 36.0)
    }

    fn measure_text_width(text: &str, style: TextStyle) -> f32 {
        let char_w = match style {
            TextStyle::Normal => FONT_8X13.character_size.width as f32,
            TextStyle::Bold => FONT_9X15_BOLD.character_size.width as f32,
            TextStyle::Italic => FONT_6X13_ITALIC.character_size.width as f32,
            TextStyle::BoldItalic => FONT_7X13_BOLD.character_size.width as f32,
            _ => FONT_8X13.character_size.width as f32,
        };
        (text.chars().count() as f32) * char_w
    }

    fn append_run_to_pages(
        state: &mut PaginationState,
        content_width: f32,
        run_style: &ComputedTextStyle,
        in_heading: Option<u8>,
        text: &str,
    ) {
        let base_style = Self::text_style_from_computed(run_style, in_heading);
        state.line_meta.role = if let Some(level) = in_heading {
            RenderLineRole::Heading(level)
        } else {
            match run_style.block_role {
                BlockRole::Heading(level) => RenderLineRole::Heading(level),
                BlockRole::ListItem => RenderLineRole::ListItem,
                _ => RenderLineRole::Body,
            }
        };
        match state.line_meta.role {
            RenderLineRole::Body => {
                state.line_meta.align = RenderLineAlign::Justify;
                state.line_meta.left_inset_px = 0;
                state.line_meta.right_inset_px = 0;
            }
            RenderLineRole::Heading(_) => {
                state.line_meta.align = RenderLineAlign::Left;
                state.line_meta.left_inset_px = 0;
                state.line_meta.right_inset_px = 0;
            }
            RenderLineRole::ListItem => {
                state.line_meta.align = RenderLineAlign::Left;
                state.line_meta.left_inset_px = 12;
                state.line_meta.right_inset_px = 0;
            }
        }
        state.line_meta.justify = matches!(state.line_meta.align, RenderLineAlign::Justify);
        state.line_meta.last_in_block = false;
        let line_step = Self::line_height_px(run_style, in_heading);
        for raw_word in text.split_whitespace() {
            let style = Self::style_for_word(base_style, raw_word);
            let mut word = raw_word.to_string();
            let mut visible_word = Self::strip_soft_hyphens(&word);
            let mut word_w = Self::measure_text_width(&visible_word, style);
            let space_w = if state.line.spans.is_empty() && state.line.text.is_empty() {
                0.0
            } else {
                Self::measure_text_width(" ", style)
            };

            if state.line.spans.is_empty()
                && state.line.text.is_empty()
                && Self::is_punctuation_only(&visible_word)
                && !state.page_lines.is_empty()
            {
                if let Some(last_span) =
                    state.page_lines.last_mut().and_then(|l| l.spans.last_mut())
                {
                    last_span.text.push_str(&visible_word);
                }
                continue;
            }

            loop {
                let line_width_limit = (content_width
                    - state.line_meta.left_inset_px as f32
                    - state.line_meta.right_inset_px as f32)
                    .max(120.0);
                let total_w = state.line.width_px + space_w + word_w;
                if total_w <= line_width_limit
                    || (state.line.spans.is_empty() && state.line.text.is_empty())
                {
                    break;
                }
                let available = (line_width_limit - state.line.width_px - space_w).max(0.0);
                if let Some((prefix, remainder)) =
                    Self::split_word_at_soft_hyphen(&word, available, style, state.line_meta.role)
                {
                    if !state.line.spans.is_empty() || !state.line.text.is_empty() {
                        state.line.text.push(' ');
                        state.line.width_px += space_w;
                    }
                    state.line.text.push_str(&prefix);
                    state.line.style = style;
                    state.line.width_px += Self::measure_text_width(&prefix, style);
                    state.line.line_height_px = state.line.line_height_px.max(line_step);
                    Self::flush_line_to_pages(state);
                    word = remainder;
                    visible_word = Self::strip_soft_hyphens(&word);
                    word_w = Self::measure_text_width(&visible_word, style);
                    continue;
                }
                Self::flush_line_to_pages(state);
                break;
            }

            if style != state.line.style && !state.line.text.is_empty() {
                state.line.spans.push(TextSpan::new(
                    core::mem::take(&mut state.line.text),
                    state.line.style,
                ));
            }
            if !state.line.spans.is_empty() || !state.line.text.is_empty() {
                state.line.text.push(' ');
                state.line.width_px += space_w;
            }
            state.line.text.push_str(&visible_word);
            state.line.style = style;
            state.line.width_px += word_w;
            state.line.line_height_px = state.line.line_height_px.max(line_step);
        }
    }

    fn style_for_word(base: TextStyle, word: &str) -> TextStyle {
        if Self::looks_like_link(word) {
            match base {
                TextStyle::Bold => TextStyle::BoldItalic,
                TextStyle::Normal => TextStyle::Italic,
                _ => base,
            }
        } else {
            base
        }
    }

    fn looks_like_link(word: &str) -> bool {
        word.starts_with("http://") || word.starts_with("https://") || word.starts_with("www.")
    }

    fn is_punctuation_only(word: &str) -> bool {
        !word.is_empty() && word.chars().all(|ch| !ch.is_alphanumeric())
    }

    fn split_word_at_soft_hyphen(
        word: &str,
        available_width: f32,
        style: TextStyle,
        role: RenderLineRole,
    ) -> Option<(String, String)> {
        let mut best: Option<(usize, f32)> = None;
        for (idx, ch) in word.char_indices() {
            if ch != '\u{00AD}' {
                continue;
            }
            if idx == 0 || idx + ch.len_utf8() >= word.len() {
                continue;
            }
            let mut prefix = word[..idx].to_string();
            prefix.push('-');
            let width = Self::text_width_px(&prefix, style, role) as f32;
            if width <= available_width {
                match best {
                    Some((_, w)) if width <= w => {}
                    _ => best = Some((idx, width)),
                }
            }
        }
        let (idx, _) = best?;
        let mut prefix = word[..idx].to_string();
        let remainder = word[idx + '\u{00AD}'.len_utf8()..].to_string();
        if prefix.is_empty() || remainder.is_empty() {
            return None;
        }
        prefix.push('-');
        Some((prefix, remainder))
    }

    fn strip_soft_hyphens(text: &str) -> String {
        text.chars().filter(|ch| *ch != '\u{00AD}').collect()
    }

    fn flush_line_to_pages(state: &mut PaginationState) {
        if !state.line.text.is_empty() {
            state.line.spans.push(TextSpan::new(
                core::mem::take(&mut state.line.text),
                state.line.style,
            ));
        }
        if state.line.spans.is_empty() {
            return;
        }

        let step = state.line.line_height_px.max(22.0);
        if state.cursor_y + step > state.content_height {
            state.pages.push(Page {
                lines: core::mem::take(&mut state.page_lines),
                page_number: state.page_number,
            });
            state.pages_meta.push(core::mem::take(&mut state.page_meta));
            state.page_number += 1;
            state.cursor_y = 0.0;
        }

        state.page_lines.push(Line {
            spans: core::mem::take(&mut state.line.spans),
            y: state.cursor_y as i32,
        });
        state.page_meta.push(state.line_meta);
        state.cursor_y += step;
        state.line.width_px = 0.0;
        state.line.line_height_px = 0.0;
        state.line_meta = LineRenderMeta::default();
    }

    fn add_vertical_space(state: &mut PaginationState, space_px: f32) {
        if state.page_lines.is_empty() {
            return;
        }
        if state.cursor_y + space_px > state.content_height {
            state.pages.push(Page {
                lines: core::mem::take(&mut state.page_lines),
                page_number: state.page_number,
            });
            state.pages_meta.push(core::mem::take(&mut state.page_meta));
            state.page_number += 1;
            state.cursor_y = 0.0;
            return;
        }
        state.cursor_y += space_px;
    }

    fn mark_last_line_block_end(state: &mut PaginationState) {
        if let Some(last) = state.page_meta.last_mut() {
            last.last_in_block = true;
            last.justify = false;
            return;
        }
        if let Some(last_page) = state.pages_meta.last_mut() {
            if let Some(last) = last_page.last_mut() {
                last.last_in_block = true;
                last.justify = false;
            }
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
        let margin = Self::SIDE_MARGIN as i32;
        let header_height = Self::TOP_MARGIN as i32;
        let footer_height = Self::BOTTOM_MARGIN as i32;

        // Header with book title.
        let header_text = Self::truncate(self.title(), 52).to_string();
        let header_style = MonoTextStyle::new(&FONT_7X13_BOLD, BinaryColor::On);
        Text::new(&header_text, Point::new(margin, 25), header_style).draw(display)?;

        Rectangle::new(
            Point::new(margin, header_height - 9),
            Size::new(width.saturating_sub((margin as u32) * 2), 1),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display)?;

        // Render styled spans line-by-line for richer typography.
        if let Some(page) = self.current_page() {
            for (idx, line) in page.lines.iter().enumerate() {
                let meta = self
                    .current_line_meta
                    .get(self.current_page_idx)
                    .and_then(|m| m.get(idx))
                    .copied()
                    .unwrap_or_default();
                // Offset line y-position by header height so text doesn't overlap title
                let y = line.y + header_height + Self::CONTENT_BASELINE_OFFSET as i32;

                if self.should_justify_line(line, meta) {
                    self.render_justified_line(display, line, y, meta)?;
                    continue;
                }

                let mut x = margin + meta.left_inset_px;
                for span in &line.spans {
                    let text = span.text.as_str();
                    if text.is_empty() {
                        continue;
                    }
                    x += self.draw_span_text(display, text, x, y, span.style, meta.role)?;
                }
            }
        }

        // Progress bar at bottom
        let bar_width = width.saturating_sub(64) as i32; // MARGIN * 2
        let bar_x = margin;
        let bar_y = height as i32 - footer_height + 5;
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
            Point::new(margin, height as i32 - 12),
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

    fn text_style_for(
        style: TextStyle,
        role: RenderLineRole,
    ) -> MonoTextStyle<'static, BinaryColor> {
        match role {
            RenderLineRole::Heading(level) if level <= 2 => {
                MonoTextStyle::new(&FONT_9X15_BOLD, BinaryColor::On)
            }
            RenderLineRole::Heading(_) => MonoTextStyle::new(&FONT_7X13_BOLD, BinaryColor::On),
            _ => match style {
                // Default to a serif-like reading feel by choosing a smaller, denser body face.
                TextStyle::Normal => MonoTextStyle::new(&FONT_8X13, BinaryColor::On),
                TextStyle::Bold => MonoTextStyle::new(&FONT_9X15_BOLD, BinaryColor::On),
                TextStyle::Italic => MonoTextStyle::new(&FONT_6X13_ITALIC, BinaryColor::On),
                TextStyle::BoldItalic => MonoTextStyle::new(&FONT_7X13_BOLD, BinaryColor::On),
                _ => MonoTextStyle::new(&FONT_8X13, BinaryColor::On),
            },
        }
    }

    fn text_width_px(text: &str, style: TextStyle, role: RenderLineRole) -> i32 {
        let char_w = match role {
            RenderLineRole::Heading(level) if level <= 2 => {
                FONT_9X15_BOLD.character_size.width as i32
            }
            RenderLineRole::Heading(_) => FONT_7X13_BOLD.character_size.width as i32,
            _ => match style {
                TextStyle::Normal => FONT_8X13.character_size.width as i32,
                TextStyle::Bold => FONT_9X15_BOLD.character_size.width as i32,
                TextStyle::Italic => FONT_6X13_ITALIC.character_size.width as i32,
                TextStyle::BoldItalic => FONT_7X13_BOLD.character_size.width as i32,
                _ => FONT_8X13.character_size.width as i32,
            },
        };
        (text.chars().count() as i32) * char_w
    }

    fn should_justify_line(&self, line: &Line, meta: LineRenderMeta) -> bool {
        if !meta.justify
            || !matches!(meta.align, RenderLineAlign::Justify)
            || meta.last_in_block
            || !matches!(meta.role, RenderLineRole::Body)
        {
            return false;
        }
        let words = line.text().split_whitespace().count();
        let spaces = line.text().chars().filter(|c| *c == ' ').count();
        if words < 7 || spaces < 3 {
            return false;
        }
        let width: i32 = line
            .spans
            .iter()
            .map(|s| Self::text_width_px(&s.text, s.style, meta.role))
            .sum();
        let target_width = (Self::DISPLAY_WIDTH as i32)
            - ((Self::SIDE_MARGIN as i32) * 2)
            - meta.left_inset_px
            - meta.right_inset_px;
        width > (target_width * 3 / 4)
    }

    fn render_justified_line<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        line: &Line,
        y: i32,
        meta: LineRenderMeta,
    ) -> Result<(), D::Error> {
        let total_width: i32 = line
            .spans
            .iter()
            .map(|s| Self::text_width_px(&s.text, s.style, meta.role))
            .sum();
        let target_width = (Self::DISPLAY_WIDTH as i32)
            - ((Self::SIDE_MARGIN as i32) * 2)
            - meta.left_inset_px
            - meta.right_inset_px;
        let extra = (target_width - total_width).max(0);
        let spaces = line.text().chars().filter(|c| *c == ' ').count() as i32;
        if spaces <= 0 || extra <= 0 {
            let mut x = Self::SIDE_MARGIN as i32 + meta.left_inset_px;
            for span in &line.spans {
                if span.text.is_empty() {
                    continue;
                }
                x += self.draw_span_text(display, &span.text, x, y, span.style, meta.role)?;
            }
            return Ok(());
        }

        let per_space = extra / spaces;
        let mut remainder = extra % spaces;
        let mut x = Self::SIDE_MARGIN as i32 + meta.left_inset_px;
        for span in &line.spans {
            for ch in span.text.chars() {
                let mut buf = [0u8; 4];
                let ch_str = ch.encode_utf8(&mut buf);
                x += self.draw_span_text(display, ch_str, x, y, span.style, meta.role)?;
                if ch == ' ' {
                    x += per_space;
                    if remainder > 0 {
                        x += 1;
                        remainder -= 1;
                    }
                }
            }
        }
        Ok(())
    }

    fn draw_span_text<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        text: &str,
        x: i32,
        y: i32,
        style: TextStyle,
        role: RenderLineRole,
    ) -> Result<i32, D::Error> {
        #[cfg(feature = "fontdue")]
        if let Some(font) = self.select_embedded_font(style, role) {
            return Self::draw_fontdue_text(display, font, text, x, y, style, role);
        }

        let mono_style = Self::text_style_for(style, role);
        Text::new(text, Point::new(x, y), mono_style).draw(display)?;
        if style == TextStyle::BoldItalic {
            Text::new(text, Point::new(x + 1, y), mono_style).draw(display)?;
        }
        Ok(Self::text_width_px(text, style, role))
    }

    #[cfg(feature = "fontdue")]
    fn select_embedded_font(&self, style: TextStyle, role: RenderLineRole) -> Option<&Font> {
        if let Some(face) = Self::pick_font_face(&self.embedded_fonts, style, role) {
            return Some(face);
        }
        Self::pick_font_face(&self.builtin_bookerly_fonts, style, role)
    }

    #[cfg(feature = "fontdue")]
    fn pick_font_face(
        faces: &[LoadedEmbeddedFont],
        style: TextStyle,
        role: RenderLineRole,
    ) -> Option<&Font> {
        if faces.is_empty() {
            return None;
        }
        if matches!(role, RenderLineRole::Heading(_)) {
            if let Some(face) = faces.iter().find(|f| {
                f.weight >= 700
                    && !matches!(
                        f.style,
                        epublet::EmbeddedFontStyle::Italic | epublet::EmbeddedFontStyle::Oblique
                    )
            }) {
                return Some(&face.font);
            }
        }
        let want_bold = matches!(style, TextStyle::Bold | TextStyle::BoldItalic);
        let want_italic = matches!(style, TextStyle::Italic | TextStyle::BoldItalic);

        let exact = faces.iter().find(|f| {
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

        let style_only = faces.iter().find(|f| {
            let is_italic = matches!(
                f.style,
                epublet::EmbeddedFontStyle::Italic | epublet::EmbeddedFontStyle::Oblique
            );
            is_italic == want_italic
        });
        if let Some(face) = style_only {
            return Some(&face.font);
        }

        faces.first().map(|f| &f.font)
    }

    #[cfg(feature = "fontdue")]
    fn draw_fontdue_text<D: DrawTarget<Color = BinaryColor>>(
        display: &mut D,
        font: &Font,
        text: &str,
        x: i32,
        y: i32,
        style: TextStyle,
        role: RenderLineRole,
    ) -> Result<i32, D::Error> {
        let mut size = match style {
            TextStyle::Bold => 18.0,
            TextStyle::Italic => 17.0,
            TextStyle::BoldItalic => 18.0,
            _ => 17.0,
        };
        if let RenderLineRole::Heading(level) = role {
            size = match level {
                1 => 18.0,
                2 => 17.0,
                3 => 16.0,
                _ => 17.0,
            };
        }

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
        let on_pixels = bitmap.iter().enumerate().filter_map(|(idx, alpha)| {
            if idx >= width * height || *alpha <= 128 {
                return None;
            }
            let row = idx / width;
            let col = idx % width;
            Some(Pixel(
                Point::new(x + col as i32, y + row as i32),
                BinaryColor::On,
            ))
        });
        display.draw_iter(on_pixels)
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
    use epublet::render_prep::{BlockRole, ComputedTextStyle};

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

    #[test]
    fn infer_heading_level_uses_block_role() {
        let style = ComputedTextStyle {
            family_stack: vec!["serif".to_string()],
            weight: 400,
            italic: false,
            size_px: 16.0,
            line_height: 1.4,
            letter_spacing: 0.0,
            block_role: BlockRole::Heading(3),
        };
        assert_eq!(StreamingEpubRenderer::infer_heading_level(&style), Some(3));
    }

    #[test]
    fn infer_heading_level_uses_size_heuristic() {
        let style = ComputedTextStyle {
            family_stack: vec!["serif".to_string()],
            weight: 400,
            italic: false,
            size_px: 24.0,
            line_height: 1.4,
            letter_spacing: 0.0,
            block_role: BlockRole::Paragraph,
        };
        assert_eq!(StreamingEpubRenderer::infer_heading_level(&style), Some(1));
    }

    #[test]
    fn line_height_increases_for_heading() {
        let heading = ComputedTextStyle {
            family_stack: vec!["serif".to_string()],
            weight: 700,
            italic: false,
            size_px: 24.0,
            line_height: 1.4,
            letter_spacing: 0.0,
            block_role: BlockRole::Heading(1),
        };
        let body = ComputedTextStyle {
            block_role: BlockRole::Paragraph,
            size_px: 16.0,
            ..heading.clone()
        };
        let heading_px = StreamingEpubRenderer::line_height_px(&heading, Some(1));
        let body_px = StreamingEpubRenderer::line_height_px(&body, None);
        assert!(heading_px > body_px);
    }
}
