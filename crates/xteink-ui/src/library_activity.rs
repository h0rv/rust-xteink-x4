//! Library Browser Activity for Xteink X4 e-reader.
//!
//! Provides a scrollable book list with cover placeholders,
//! reading progress bars, and sorting options.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use embedded_graphics::{
    mono_font::{MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};

use crate::filesystem::{basename, dirname, join_path, FileSystem};
use crate::input::{Button, InputEvent};
use crate::ui::theme::{
    layout, ui_font_body, ui_font_body_char_width, ui_font_small, ui_font_title,
};
use crate::ui::{Activity, ActivityRefreshMode, ActivityResult, Modal, Theme, ThemeMetrics};
use crate::{DISPLAY_HEIGHT, DISPLAY_WIDTH};

/// Book information structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookInfo {
    pub title: String,
    pub author: String,
    pub path: String,
    pub progress_percent: u8,
    pub last_read: Option<u64>, // timestamp
    cover_thumbnail: Option<CoverThumbnail>,
}

impl BookInfo {
    #[cfg(feature = "std")]
    const MAX_COMPACT_COVER_CHARS: usize =
        ((DISPLAY_WIDTH as usize) * (DISPLAY_HEIGHT as usize)).div_ceil(8) * 2 + 16;

    /// Create a new book info
    pub fn new(
        title: impl Into<String>,
        author: impl Into<String>,
        path: impl Into<String>,
        progress_percent: u8,
        last_read: Option<u64>,
    ) -> Self {
        Self {
            title: title.into(),
            author: author.into(),
            path: path.into(),
            progress_percent: progress_percent.min(100),
            last_read,
            cover_thumbnail: None,
        }
    }

    /// Get display title (truncated if needed)
    pub fn display_title(&self, max_chars: usize) -> &str {
        if self.title.len() <= max_chars {
            &self.title
        } else {
            &self.title[..max_chars]
        }
    }

    pub(crate) fn has_cover_thumbnail(&self) -> bool {
        self.cover_thumbnail.is_some()
    }

    pub(crate) fn draw_cover_thumbnail_scaled<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<bool, D::Error> {
        let Some(thumb) = self.cover_thumbnail.as_ref() else {
            return Ok(false);
        };
        let draw_w = width.min(thumb.width).max(1);
        let draw_h = height.min(thumb.height).max(1);
        let offset_x = ((width as i32 - draw_w as i32).max(0)) / 2;
        let offset_y = ((height as i32 - draw_h as i32).max(0)) / 2;
        for dy in 0..draw_h {
            let src_y = (dy as u64 * thumb.height as u64 / draw_h as u64) as u32;
            for dx in 0..draw_w {
                let src_x = (dx as u64 * thumb.width as u64 / draw_w as u64) as u32;
                if thumb.is_black(src_x, src_y) {
                    Pixel(
                        Point::new(x + offset_x + dx as i32, y + offset_y + dy as i32),
                        BinaryColor::On,
                    )
                    .draw(display)?;
                }
            }
        }
        Ok(true)
    }

    #[cfg(feature = "std")]
    pub(crate) fn cover_thumbnail_compact(&self) -> Option<String> {
        let thumb = self.cover_thumbnail.as_ref()?;
        let mut out = format!("{}x{}:", thumb.width, thumb.height);
        for byte in &thumb.pixels {
            out.push(Self::hex_digit((byte >> 4) & 0x0f));
            out.push(Self::hex_digit(byte & 0x0f));
        }
        Some(out)
    }

    #[cfg(feature = "std")]
    pub(crate) fn set_cover_thumbnail_from_compact(&mut self, encoded: &str) -> bool {
        if encoded.len() > Self::MAX_COMPACT_COVER_CHARS {
            return false;
        }
        let Some((dims, hex)) = encoded.split_once(':') else {
            return false;
        };
        let Some((w, h)) = dims.split_once('x') else {
            return false;
        };
        let Some(width) = w.parse::<u32>().ok() else {
            return false;
        };
        let Some(height) = h.parse::<u32>().ok() else {
            return false;
        };
        let Some(mut thumb) = CoverThumbnail::new(width, height) else {
            return false;
        };
        if hex.len() != thumb.pixels.len() * 2 {
            return false;
        }
        for (idx, chunk) in hex.as_bytes().chunks(2).enumerate() {
            let Some(hi) = Self::hex_value(chunk[0] as char) else {
                return false;
            };
            let Some(lo) = Self::hex_value(chunk[1] as char) else {
                return false;
            };
            thumb.pixels[idx] = (hi << 4) | lo;
        }
        self.cover_thumbnail = Some(thumb);
        true
    }

    #[cfg(feature = "std")]
    fn hex_digit(value: u8) -> char {
        match value & 0x0f {
            0..=9 => (b'0' + (value & 0x0f)) as char,
            v => (b'a' + (v - 10)) as char,
        }
    }

    #[cfg(feature = "std")]
    fn hex_value(ch: char) -> Option<u8> {
        match ch {
            '0'..='9' => Some((ch as u8) - b'0'),
            'a'..='f' => Some((ch as u8) - b'a' + 10),
            'A'..='F' => Some((ch as u8) - b'A' + 10),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoverThumbnail {
    width: u32,
    height: u32,
    pixels: Vec<u8>, // bit-packed, 1 = black pixel
}

impl CoverThumbnail {
    fn new(width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 || width > DISPLAY_WIDTH || height > DISPLAY_HEIGHT {
            return None;
        }
        let len = (width as usize)
            .checked_mul(height as usize)?
            .checked_add(7)?
            / 8;
        Some(Self {
            width,
            height,
            pixels: vec![0u8; len],
        })
    }

    fn set_pixel(&mut self, x: u32, y: u32, is_black: bool) {
        if x >= self.width || y >= self.height || !is_black {
            return;
        }
        let idx = (y * self.width + x) as usize;
        self.pixels[idx / 8] |= 1 << (7 - (idx % 8));
    }

    fn is_black(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let idx = (y * self.width + x) as usize;
        (self.pixels[idx / 8] & (1 << (7 - (idx % 8)))) != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LumaImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// Sort order for the library
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    Title,
    Author,
    Recent,
}

impl SortOrder {
    /// All sort variants
    pub const ALL: [Self; 3] = [Self::Title, Self::Author, Self::Recent];

    /// Get display label
    pub const fn label(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Author => "Author",
            Self::Recent => "Recent",
        }
    }

    /// Get next sort order
    pub const fn next(self) -> Self {
        match self {
            Self::Title => Self::Author,
            Self::Author => Self::Recent,
            Self::Recent => Self::Title,
        }
    }
}

/// Context menu actions for books
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookAction {
    Open,
    MarkUnread,
    Delete,
    Cancel,
}

/// Library Browser Activity
#[derive(Debug, Clone)]
pub struct LibraryActivity {
    books: Vec<BookInfo>,
    filtered_books: Vec<usize>, // indices into books
    selected_index: usize,
    scroll_offset: usize,
    sort_order: SortOrder,
    theme: Theme,
    show_context_menu: bool,
    context_menu_index: usize,
    show_toast: bool,
    toast_message: String,
    toast_frames_remaining: u32,
    visible_count: usize,
    /// Tracks if this is the first render after entering (for full refresh)
    needs_full_refresh: bool,
    is_loading: bool,
    refresh_requested: bool,
    pending_open_path: Option<String>,
}

impl LibraryActivity {
    /// Default books directory for auto-detection.
    pub const DEFAULT_BOOKS_ROOT: &'static str = "/books";
    #[cfg(feature = "std")]
    const MAX_EPUB_METADATA_BYTES: u64 = 64 * 1024;
    #[cfg(feature = "std")]
    const MAX_COMPACT_COVER_CHARS: usize =
        ((DISPLAY_WIDTH as usize) * (DISPLAY_HEIGHT as usize)).div_ceil(8) * 2 + 16;

    /// Toast display duration in frames
    const TOAST_DURATION: u32 = 120; // ~2 seconds at 60fps

    /// Cover placeholder width (from layout module).
    const COVER_WIDTH: u32 = layout::COVER_W;
    const COVER_MAX_HEIGHT: u32 = layout::COVER_MAX_H;
    const MAX_BMP_PIXELS: u32 = 1_500_000;
    const MAX_BMP_BYTES: usize = 2_000_000;
    #[cfg(feature = "std")]
    const MAX_JPEG_BYTES: usize = 2_000_000;

    /// Create a new library activity with empty book list
    pub fn new() -> Self {
        let theme = Theme::default();
        let visible_count = theme.metrics.visible_items(DISPLAY_HEIGHT);

        Self {
            books: Vec::new(),
            filtered_books: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            sort_order: SortOrder::default(),
            theme,
            show_context_menu: false,
            context_menu_index: 0,
            show_toast: false,
            toast_message: String::new(),
            toast_frames_remaining: 0,
            visible_count,
            needs_full_refresh: true,
            is_loading: false,
            refresh_requested: false,
            pending_open_path: None,
        }
    }

    /// Create with book list
    pub fn with_books(books: Vec<BookInfo>) -> Self {
        let mut activity = Self::new();
        activity.set_books(books);
        activity.needs_full_refresh = true;
        activity
    }

    /// Create with mock books for testing
    pub fn with_mock_books() -> Self {
        Self::with_books(create_mock_books())
    }

    /// Scan filesystem for books and populate the library
    ///
    /// # Arguments
    /// * `fs` - Filesystem to scan
    /// * `root_path` - Root directory to scan (e.g., "/books")
    pub fn scan_books(&mut self, fs: &mut dyn FileSystem, root_path: &str) {
        let books = Self::discover_books(fs, root_path);
        self.set_books(books);
    }

    /// Discover books from filesystem.
    pub fn discover_books(fs: &mut dyn FileSystem, root_path: &str) -> Vec<BookInfo> {
        let book_paths = match fs.scan_directory(root_path) {
            Ok(paths) => paths,
            Err(_) => return Vec::new(),
        };

        let mut books = Vec::new();
        for path in book_paths {
            books.push(Self::extract_book_info_for_path(fs, &path));
        }

        books.sort_by(|a, b| a.title.cmp(&b.title));
        books
    }

    /// Extract book info for a single filesystem path.
    pub fn extract_book_info_for_path(fs: &mut dyn FileSystem, path: &str) -> BookInfo {
        Self::extract_book_info(fs, path)
    }

    /// Extract book info from file path
    fn extract_book_info(fs: &mut dyn FileSystem, path: &str) -> BookInfo {
        let filename = basename(path);
        let extension = filename
            .rsplit_once('.')
            .map(|(_, ext)| ext)
            .unwrap_or_default();
        #[cfg(feature = "std")]
        let file_size = fs.file_info(path).map(|info| info.size).unwrap_or(0);
        #[cfg(feature = "std")]
        let cached_cover = Self::load_cached_cover_thumbnail(path, file_size);
        #[cfg(feature = "std")]
        let include_epub_cover_decode = {
            #[cfg(target_os = "espidf")]
            {
                false
            }
            #[cfg(not(target_os = "espidf"))]
            {
                cached_cover.is_none()
            }
        };
        #[cfg(not(feature = "std"))]
        let include_epub_cover_decode = true;

        if extension.eq_ignore_ascii_case("epub") || extension.eq_ignore_ascii_case("epu") {
            if let Some((title, author, cover_thumbnail)) =
                Self::extract_epub_book_info(fs, path, include_epub_cover_decode)
            {
                let mut book = BookInfo::new(title, author, path, 0, None);
                #[cfg(feature = "std")]
                {
                    #[cfg(target_os = "espidf")]
                    {
                        let _ = cover_thumbnail;
                        book.cover_thumbnail = cached_cover;
                    }
                    #[cfg(not(target_os = "espidf"))]
                    {
                        book.cover_thumbnail = cached_cover
                            .or(cover_thumbnail)
                            .or_else(|| Self::load_sidecar_cover_thumbnail(fs, path));
                    }
                    if let Some(thumb) = book.cover_thumbnail.as_ref() {
                        let _ = Self::persist_cached_cover_thumbnail(path, file_size, thumb);
                    }
                }
                #[cfg(not(feature = "std"))]
                {
                    book.cover_thumbnail =
                        cover_thumbnail.or_else(|| Self::load_sidecar_cover_thumbnail(fs, path));
                }
                return book;
            }
        }

        let mut book = BookInfo::new(Self::filename_to_title(filename), "Unknown", path, 0, None);
        #[cfg(feature = "std")]
        {
            #[cfg(target_os = "espidf")]
            {
                book.cover_thumbnail = cached_cover;
            }
            #[cfg(not(target_os = "espidf"))]
            {
                book.cover_thumbnail =
                    cached_cover.or_else(|| Self::load_sidecar_cover_thumbnail(fs, path));
            }
            if let Some(thumb) = book.cover_thumbnail.as_ref() {
                let _ = Self::persist_cached_cover_thumbnail(path, file_size, thumb);
            }
        }
        #[cfg(not(feature = "std"))]
        {
            book.cover_thumbnail = Self::load_sidecar_cover_thumbnail(fs, path);
        }
        book
    }

    #[cfg(feature = "std")]
    fn extract_epub_book_info(
        fs: &mut dyn FileSystem,
        path: &str,
        include_cover: bool,
    ) -> Option<(String, String, Option<CoverThumbnail>)> {
        use mu_epub::metadata::{parse_container_xml, parse_opf};
        use mu_epub::zip::StreamingZip;
        use std::io::Cursor;

        let data = fs.read_file_bytes(path).ok()?;
        let mut zip = StreamingZip::new(Cursor::new(data)).ok()?;
        let mut input_scratch = vec![0u8; 4096];
        let mut output_scratch = vec![0u8; 4096];

        let container_entry = zip
            .get_entry("META-INF/container.xml")
            .or_else(|| zip.get_entry("meta-inf/container.xml"))?
            .clone();
        let mut container_buf = Vec::new();
        let container_read = Self::read_zip_entry_with_scratch(
            &mut zip,
            &container_entry,
            &mut container_buf,
            &mut input_scratch,
            &mut output_scratch,
            Self::MAX_EPUB_METADATA_BYTES as usize,
        )
        .ok()?;

        let opf_path = parse_container_xml(&container_buf[..container_read]).ok()?;

        let opf_entry = zip.get_entry(&opf_path)?.clone();
        let mut opf_buf = Vec::new();
        let opf_read = Self::read_zip_entry_with_scratch(
            &mut zip,
            &opf_entry,
            &mut opf_buf,
            &mut input_scratch,
            &mut output_scratch,
            Self::MAX_EPUB_METADATA_BYTES as usize,
        )
        .ok()?;

        let metadata = parse_opf(&opf_buf[..opf_read]).ok()?;
        let title = if metadata.title.trim().is_empty() {
            Self::filename_to_title(basename(path))
        } else {
            metadata.title.clone()
        };
        let author = if metadata.author.trim().is_empty() {
            "Unknown".to_string()
        } else {
            metadata.author.clone()
        };
        let cover_thumbnail = if include_cover {
            Self::extract_epub_cover_thumbnail(
                &mut zip,
                &opf_path,
                &metadata,
                &mut input_scratch,
                &mut output_scratch,
            )
        } else {
            None
        };
        Some((title, author, cover_thumbnail))
    }

    #[cfg(not(feature = "std"))]
    fn extract_epub_book_info(
        _fs: &mut dyn FileSystem,
        _path: &str,
        _include_cover: bool,
    ) -> Option<(String, String, Option<CoverThumbnail>)> {
        None
    }

    fn load_sidecar_cover_thumbnail(
        fs: &mut dyn FileSystem,
        book_path: &str,
    ) -> Option<CoverThumbnail> {
        for candidate in Self::sidecar_cover_candidates(book_path) {
            if let Ok(info) = fs.file_info(&candidate) {
                if info.size > Self::MAX_BMP_BYTES as u64 {
                    continue;
                }
            }
            let data = match fs.read_file_bytes(&candidate) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let lower = candidate.to_ascii_lowercase();
            if lower.ends_with(".bmp") {
                if let Some(thumb) =
                    Self::decode_bmp_thumbnail(&data, Self::COVER_WIDTH, Self::COVER_MAX_HEIGHT)
                {
                    return Some(thumb);
                }
                continue;
            }
            if lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".png") {
                if let Some(thumb) =
                    Self::decode_raster_thumbnail(&data, Self::COVER_WIDTH, Self::COVER_MAX_HEIGHT)
                {
                    return Some(thumb);
                }
            }
        }
        None
    }

    fn sidecar_cover_candidates(book_path: &str) -> [String; 8] {
        let stem = book_path
            .rsplit_once('.')
            .map(|(name, _)| name)
            .unwrap_or(book_path);
        let parent = dirname(book_path);
        [
            format!("{stem}.bmp"),
            format!("{stem}.jpg"),
            format!("{stem}.jpeg"),
            format!("{stem}.png"),
            join_path(parent, "cover.bmp"),
            join_path(parent, "cover.jpg"),
            join_path(parent, "cover.jpeg"),
            join_path(parent, "cover.png"),
        ]
    }

    #[cfg(feature = "std")]
    fn cover_cache_root() -> &'static str {
        if cfg!(target_os = "espidf") {
            "/sd/.xteink/covers"
        } else {
            "target/.xteink-covers"
        }
    }

    #[cfg(feature = "std")]
    fn cover_cache_key(path: &str, size: u64) -> u64 {
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;
        let mut state = FNV_OFFSET;
        for b in path.as_bytes() {
            state ^= *b as u64;
            state = state.wrapping_mul(FNV_PRIME);
        }
        for b in size.to_le_bytes() {
            state ^= b as u64;
            state = state.wrapping_mul(FNV_PRIME);
        }
        state
    }

    #[cfg(feature = "std")]
    fn cover_cache_path(path: &str, size: u64) -> String {
        format!(
            "{}/{:016x}.compact",
            Self::cover_cache_root(),
            Self::cover_cache_key(path, size)
        )
    }

    #[cfg(feature = "std")]
    fn load_cached_cover_thumbnail(path: &str, size: u64) -> Option<CoverThumbnail> {
        let cache_path = Self::cover_cache_path(path, size);
        let encoded = std::fs::read_to_string(cache_path).ok()?;
        if encoded.len() > Self::MAX_COMPACT_COVER_CHARS {
            return None;
        }
        let mut probe = BookInfo::new("", "", "", 0, None);
        if probe.set_cover_thumbnail_from_compact(encoded.trim()) {
            probe.cover_thumbnail
        } else {
            None
        }
    }

    #[cfg(feature = "std")]
    fn persist_cached_cover_thumbnail(path: &str, size: u64, thumb: &CoverThumbnail) -> bool {
        let mut probe = BookInfo::new("", "", "", 0, None);
        probe.cover_thumbnail = Some(thumb.clone());
        let Some(encoded) = probe.cover_thumbnail_compact() else {
            return false;
        };
        if encoded.len() > Self::MAX_COMPACT_COVER_CHARS {
            return false;
        }
        let cache_path = Self::cover_cache_path(path, size);
        if let Some(parent) = std::path::Path::new(&cache_path).parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return false;
            }
        }
        std::fs::write(cache_path, encoded).is_ok()
    }

    /// Convert filename to title (remove extension, replace underscores/hyphens with spaces)
    fn filename_to_title(filename: &str) -> String {
        // Remove extension
        let name = filename
            .rsplit_once('.')
            .map(|(name, _)| name)
            .unwrap_or(filename);

        // Replace underscores and hyphens with spaces
        let name = name.replace(['_', '-'], " ");

        // Capitalize first letter of each word
        name.split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[cfg(feature = "std")]
    fn extract_epub_cover_thumbnail<F: std::io::Read + std::io::Seek>(
        zip: &mut mu_epub::zip::StreamingZip<F>,
        opf_path: &str,
        metadata: &mu_epub::metadata::EpubMetadata,
        input_scratch: &mut [u8],
        output_scratch: &mut [u8],
    ) -> Option<CoverThumbnail> {
        let mut candidates: Vec<(String, Option<String>)> = Vec::new();
        if let Some(item) = metadata.get_cover_item() {
            candidates.push((item.href.clone(), Some(item.media_type.clone())));
        }
        for guide_ref in &metadata.guide {
            if guide_ref.guide_type.eq_ignore_ascii_case("cover")
                && !candidates.iter().any(|(href, _)| *href == guide_ref.href)
            {
                candidates.push((guide_ref.href.clone(), None));
            }
        }

        for (href, media_type) in candidates {
            let Some((resolved_path, bytes)) = Self::read_epub_resource_with_hints(
                zip,
                opf_path,
                &href,
                input_scratch,
                output_scratch,
                Self::MAX_BMP_BYTES,
            ) else {
                continue;
            };

            if let Some(thumb) = Self::decode_cover_thumbnail_from_resource(
                &bytes,
                media_type.as_deref(),
                &resolved_path,
            ) {
                return Some(thumb);
            }

            let lower_path = resolved_path.to_ascii_lowercase();
            let is_xhtml = lower_path.ends_with(".xhtml")
                || lower_path.ends_with(".html")
                || media_type
                    .as_deref()
                    .is_some_and(|m| m.contains("xhtml") || m.contains("html"));
            if !is_xhtml {
                continue;
            }

            let Some(image_href) = mu_epub::metadata::extract_cover_image_href_from_xhtml(&bytes)
            else {
                continue;
            };
            let Some((image_path, image_bytes)) = Self::read_epub_resource_with_hints(
                zip,
                &resolved_path,
                &image_href,
                input_scratch,
                output_scratch,
                Self::MAX_BMP_BYTES,
            ) else {
                continue;
            };

            if let Some(thumb) =
                Self::decode_cover_thumbnail_from_resource(&image_bytes, None, &image_path)
            {
                return Some(thumb);
            }
        }

        None
    }

    #[cfg(feature = "std")]
    fn read_epub_resource_with_hints<F: std::io::Read + std::io::Seek>(
        zip: &mut mu_epub::zip::StreamingZip<F>,
        base_file_path: &str,
        href: &str,
        input_scratch: &mut [u8],
        output_scratch: &mut [u8],
        max_len: usize,
    ) -> Option<(String, Vec<u8>)> {
        let resolved = Self::resolve_epub_relative_path(base_file_path, href);
        let candidates = vec![
            resolved.clone(),
            href.to_string(),
            resolved.trim_start_matches('/').to_string(),
        ];

        let mut output = Vec::new();
        for candidate in candidates {
            if candidate.is_empty() {
                continue;
            }
            let Some(entry) = zip.get_entry(&candidate).cloned() else {
                continue;
            };
            let read = Self::read_zip_entry_with_scratch(
                zip,
                &entry,
                &mut output,
                input_scratch,
                output_scratch,
                max_len,
            )
            .ok()?;
            return Some((candidate, output[..read].to_vec()));
        }
        None
    }

    #[cfg(feature = "std")]
    fn decode_cover_thumbnail_from_resource(
        data: &[u8],
        media_type: Option<&str>,
        path_hint: &str,
    ) -> Option<CoverThumbnail> {
        let lower_media = media_type
            .map(|m| m.to_ascii_lowercase())
            .unwrap_or_default();
        let lower_path = path_hint.to_ascii_lowercase();
        let is_bmp = lower_media.contains("image/bmp")
            || lower_path.ends_with(".bmp")
            || data.starts_with(b"BM");
        if is_bmp {
            return Self::decode_bmp_thumbnail(data, Self::COVER_WIDTH, Self::COVER_MAX_HEIGHT);
        }

        if lower_media.starts_with("image/")
            || lower_path.ends_with(".jpg")
            || lower_path.ends_with(".jpeg")
            || lower_path.ends_with(".png")
        {
            return Self::decode_raster_thumbnail(data, Self::COVER_WIDTH, Self::COVER_MAX_HEIGHT);
        }
        None
    }

    #[cfg(feature = "std")]
    fn read_zip_entry_with_scratch<F: std::io::Read + std::io::Seek>(
        zip: &mut mu_epub::zip::StreamingZip<F>,
        entry: &mu_epub::zip::CdEntry,
        out: &mut Vec<u8>,
        input_scratch: &mut [u8],
        output_scratch: &mut [u8],
        max_len: usize,
    ) -> Result<usize, ()> {
        let expected_len = entry.uncompressed_size as usize;
        if expected_len > max_len {
            return Err(());
        }
        out.clear();
        out.try_reserve(expected_len).map_err(|_| ())?;
        zip.read_file_to_writer_with_scratch(entry, out, input_scratch, output_scratch)
            .map_err(|_| ())
    }

    #[cfg(feature = "std")]
    fn resolve_epub_relative_path(base_file_path: &str, relative: &str) -> String {
        let mut parts: Vec<&str> = Vec::new();

        let combined = if relative.starts_with('/') {
            relative.trim_start_matches('/').to_string()
        } else {
            let base_dir = dirname(base_file_path);
            if base_dir == "." {
                relative.to_string()
            } else {
                join_path(base_dir, relative)
            }
        };

        for segment in combined.split('/') {
            match segment {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                _ => parts.push(segment),
            }
        }

        parts.join("/")
    }

    fn decode_bmp_thumbnail(
        data: &[u8],
        max_width: u32,
        max_height: u32,
    ) -> Option<CoverThumbnail> {
        let decoded = Self::decode_bmp_to_luma(data)?;
        let threshold = Self::adaptive_thumbnail_threshold(&decoded);
        Self::scale_luma_to_binary_thumbnail(&decoded, max_width, max_height, threshold)
    }

    fn decode_bmp_to_luma(data: &[u8]) -> Option<LumaImage> {
        if data.len() < 54 || !data.starts_with(b"BM") {
            return None;
        }

        let data_offset = Self::read_u32_le(data, 10)? as usize;
        let dib_header_size = Self::read_u32_le(data, 14)? as usize;
        if dib_header_size < 40 || data.len() < 14 + dib_header_size {
            return None;
        }

        let width_i32 = Self::read_i32_le(data, 18)?;
        let height_i32 = Self::read_i32_le(data, 22)?;
        if width_i32 <= 0 || height_i32 == 0 {
            return None;
        }

        let width = width_i32 as u32;
        let height = height_i32.unsigned_abs();
        if width == 0 || height == 0 || width.saturating_mul(height) > Self::MAX_BMP_PIXELS {
            return None;
        }

        let planes = Self::read_u16_le(data, 26)?;
        let bpp = Self::read_u16_le(data, 28)?;
        let compression = Self::read_u32_le(data, 30)?;
        if planes != 1 || compression != 0 {
            return None;
        }
        if !matches!(bpp, 1 | 8 | 24 | 32) {
            return None;
        }

        let row_stride = (((width as usize).checked_mul(bpp as usize)?).checked_add(31)? / 32) * 4;
        let image_bytes = row_stride.checked_mul(height as usize)?;
        if data_offset.checked_add(image_bytes)? > data.len() || data_offset > data.len() {
            return None;
        }

        let mut palette: Vec<[u8; 4]> = Vec::new();
        if bpp <= 8 {
            let palette_offset = 14 + dib_header_size;
            let colors_used = Self::read_u32_le(data, 46).unwrap_or(0);
            let palette_entries = if colors_used > 0 {
                colors_used as usize
            } else {
                1usize << bpp
            };
            let palette_bytes = palette_entries.checked_mul(4)?;
            if palette_offset.checked_add(palette_bytes)? > data.len() {
                return None;
            }
            for entry in data[palette_offset..palette_offset + palette_bytes].chunks_exact(4) {
                palette.push([entry[0], entry[1], entry[2], entry[3]]);
            }
        }

        let mut pixels = vec![0u8; (width as usize).checked_mul(height as usize)?];
        let top_down = height_i32 < 0;

        for y in 0..height {
            let src_y = if top_down { y } else { height - 1 - y };
            let row_start = data_offset + (src_y as usize) * row_stride;
            let row = &data[row_start..row_start + row_stride];
            let dst_row_start = (y * width) as usize;

            match bpp {
                1 => {
                    for x in 0..width {
                        let byte = row[(x / 8) as usize];
                        let bit = 7 - (x % 8);
                        let idx = ((byte >> bit) & 0x01) as usize;
                        let [b, g, r, _] = *palette.get(idx)?;
                        pixels[dst_row_start + x as usize] = Self::luma_from_bgr(b, g, r);
                    }
                }
                8 => {
                    for x in 0..width {
                        let idx = row[x as usize] as usize;
                        let [b, g, r, _] = *palette.get(idx)?;
                        pixels[dst_row_start + x as usize] = Self::luma_from_bgr(b, g, r);
                    }
                }
                24 => {
                    for x in 0..width {
                        let i = (x as usize) * 3;
                        let b = *row.get(i)?;
                        let g = *row.get(i + 1)?;
                        let r = *row.get(i + 2)?;
                        pixels[dst_row_start + x as usize] = Self::luma_from_bgr(b, g, r);
                    }
                }
                32 => {
                    for x in 0..width {
                        let i = (x as usize) * 4;
                        let b = *row.get(i)?;
                        let g = *row.get(i + 1)?;
                        let r = *row.get(i + 2)?;
                        pixels[dst_row_start + x as usize] = Self::luma_from_bgr(b, g, r);
                    }
                }
                _ => return None,
            }
        }

        Some(LumaImage {
            width,
            height,
            pixels,
        })
    }

    #[cfg(feature = "std")]
    fn decode_raster_thumbnail(
        data: &[u8],
        max_width: u32,
        max_height: u32,
    ) -> Option<CoverThumbnail> {
        if data.len() > Self::MAX_JPEG_BYTES {
            return None;
        }
        let decoded = image::load_from_memory(data).ok()?;
        let gray = decoded.to_luma8();
        let (width, height) = gray.dimensions();
        if width == 0
            || height == 0
            || width.saturating_mul(height) > Self::MAX_BMP_PIXELS
            || gray.len() > Self::MAX_BMP_BYTES
        {
            return None;
        }
        let decoded = LumaImage {
            width,
            height,
            pixels: gray.into_raw(),
        };
        let threshold = Self::adaptive_thumbnail_threshold(&decoded);
        Self::scale_luma_to_binary_thumbnail(&decoded, max_width, max_height, threshold)
    }

    fn adaptive_thumbnail_threshold(source: &LumaImage) -> u8 {
        if source.pixels.is_empty() {
            return 128;
        }
        let mut sum = 0u64;
        let mut dark = 0usize;
        for &px in &source.pixels {
            sum = sum.saturating_add(px as u64);
            if px < 96 {
                dark += 1;
            }
        }
        let avg = (sum / source.pixels.len() as u64) as i32;
        let dark_ratio = dark as f32 / source.pixels.len() as f32;
        let mut threshold = avg;
        if dark_ratio > 0.5 {
            threshold += 14;
        } else if dark_ratio < 0.2 {
            threshold -= 10;
        }
        threshold.clamp(78, 178) as u8
    }

    fn scale_luma_to_binary_thumbnail(
        source: &LumaImage,
        max_width: u32,
        max_height: u32,
        threshold: u8,
    ) -> Option<CoverThumbnail> {
        if source.width == 0 || source.height == 0 || max_width == 0 || max_height == 0 {
            return None;
        }

        let (dst_width, dst_height) =
            Self::fit_dimensions(source.width, source.height, max_width, max_height)?;
        let mut thumbnail = CoverThumbnail::new(dst_width, dst_height)?;

        for y in 0..dst_height {
            let src_y = (y as u64).checked_mul(source.height as u64)? / dst_height as u64;
            let src_y = src_y as u32;
            for x in 0..dst_width {
                let src_x = (x as u64).checked_mul(source.width as u64)? / dst_width as u64;
                let src_x = src_x as u32;
                let src_idx = (src_y * source.width + src_x) as usize;
                let luminance = *source.pixels.get(src_idx)?;
                thumbnail.set_pixel(x, y, luminance < threshold);
            }
        }

        Some(thumbnail)
    }

    fn fit_dimensions(
        source_width: u32,
        source_height: u32,
        max_width: u32,
        max_height: u32,
    ) -> Option<(u32, u32)> {
        if source_width == 0 || source_height == 0 || max_width == 0 || max_height == 0 {
            return None;
        }

        if (source_width as u64) * (max_height as u64) > (source_height as u64) * (max_width as u64)
        {
            let width = max_width;
            let height = ((source_height as u64 * max_width as u64) / source_width as u64)
                .max(1)
                .min(max_height as u64) as u32;
            Some((width, height))
        } else {
            let height = max_height;
            let width = ((source_width as u64 * max_height as u64) / source_height as u64)
                .max(1)
                .min(max_width as u64) as u32;
            Some((width, height))
        }
    }

    fn luma_from_bgr(b: u8, g: u8, r: u8) -> u8 {
        ((r as u16 * 77 + g as u16 * 150 + b as u16 * 29) >> 8) as u8
    }

    fn read_u16_le(data: &[u8], offset: usize) -> Option<u16> {
        let bytes = data.get(offset..offset + 2)?;
        Some(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32_le(data: &[u8], offset: usize) -> Option<u32> {
        let bytes = data.get(offset..offset + 4)?;
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_i32_le(data: &[u8], offset: usize) -> Option<i32> {
        let bytes = data.get(offset..offset + 4)?;
        Some(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Set the book list and refresh
    pub fn set_books(&mut self, books: Vec<BookInfo>) {
        self.books = books;
        self.apply_sort();
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Get book count
    pub fn book_count(&self) -> usize {
        self.books.len()
    }

    /// Check if library is empty
    pub fn is_empty(&self) -> bool {
        self.books.is_empty()
    }

    /// Get currently selected book
    pub fn selected_book(&self) -> Option<&BookInfo> {
        self.filtered_books
            .get(self.selected_index)
            .and_then(|&idx| self.books.get(idx))
    }

    /// Mark library as currently scanning.
    pub fn begin_loading_scan(&mut self) {
        self.is_loading = true;
    }

    /// Mark library scan complete.
    pub fn finish_loading_scan(&mut self) {
        self.is_loading = false;
    }

    /// Consume manual refresh request.
    pub fn take_refresh_request(&mut self) -> bool {
        let requested = self.refresh_requested;
        self.refresh_requested = false;
        requested
    }

    /// Consume pending open-book request.
    pub fn take_open_request(&mut self) -> Option<String> {
        self.pending_open_path.take()
    }

    /// Apply current sort order
    fn apply_sort(&mut self) {
        self.filtered_books = (0..self.books.len()).collect();

        match self.sort_order {
            SortOrder::Title => {
                self.filtered_books
                    .sort_by(|&a, &b| self.books[a].title.cmp(&self.books[b].title));
            }
            SortOrder::Author => {
                self.filtered_books
                    .sort_by(|&a, &b| self.books[a].author.cmp(&self.books[b].author));
            }
            SortOrder::Recent => {
                self.filtered_books.sort_by(|&a, &b| {
                    match (self.books[a].last_read, self.books[b].last_read) {
                        (Some(ta), Some(tb)) => tb.cmp(&ta), // Most recent first
                        (Some(_), None) => core::cmp::Ordering::Less,
                        (None, Some(_)) => core::cmp::Ordering::Greater,
                        (None, None) => core::cmp::Ordering::Equal,
                    }
                });
            }
        }
    }

    /// Cycle to next sort order
    fn cycle_sort(&mut self) {
        self.sort_order = self.sort_order.next();
        self.apply_sort();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.show_toast(format!("Sorted by: {}", self.sort_order.label()));
    }

    /// Move selection down (wraps around)
    fn select_next(&mut self) {
        if !self.filtered_books.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_books.len();
            self.ensure_visible();
        }
    }

    /// Move selection up (wraps around)
    fn select_prev(&mut self) {
        if !self.filtered_books.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.filtered_books.len() - 1
            } else {
                self.selected_index - 1
            };
            self.ensure_visible();
        }
    }

    /// Ensure selected item is visible
    fn ensure_visible(&mut self) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + self.visible_count {
            self.scroll_offset = self.selected_index.saturating_sub(self.visible_count - 1);
        }
    }

    /// Show a toast notification
    fn show_toast(&mut self, message: impl Into<String>) {
        self.toast_message = message.into();
        self.show_toast = true;
        self.toast_frames_remaining = Self::TOAST_DURATION;
    }

    /// Update toast state (call once per frame)
    pub fn update(&mut self) {
        if self.show_toast && self.toast_frames_remaining > 0 {
            self.toast_frames_remaining -= 1;
            if self.toast_frames_remaining == 0 {
                self.show_toast = false;
            }
        }
    }

    /// Open context menu for selected book
    #[cfg(test)]
    fn open_context_menu(&mut self) {
        if !self.filtered_books.is_empty() {
            self.show_context_menu = true;
            self.context_menu_index = 0;
        }
    }

    /// Close context menu
    fn close_context_menu(&mut self) {
        self.show_context_menu = false;
    }

    /// Handle context menu navigation (wraps)
    fn context_menu_next(&mut self) {
        self.context_menu_index = (self.context_menu_index + 1) % 4; // 4 actions
    }

    fn context_menu_prev(&mut self) {
        self.context_menu_index = if self.context_menu_index == 0 {
            3
        } else {
            self.context_menu_index - 1
        };
    }

    /// Get current context menu action
    fn current_action(&self) -> BookAction {
        match self.context_menu_index {
            0 => BookAction::Open,
            1 => BookAction::MarkUnread,
            2 => BookAction::Delete,
            _ => BookAction::Cancel,
        }
    }

    /// Execute context menu action
    fn execute_action(&mut self, action: BookAction) -> ActivityResult {
        self.close_context_menu();

        match action {
            BookAction::Open => {
                if let Some(book) = self.selected_book() {
                    let path = book.path.clone();
                    self.pending_open_path = Some(path);
                    ActivityResult::Consumed
                } else {
                    ActivityResult::Consumed
                }
            }
            BookAction::MarkUnread => {
                if let Some(&idx) = self.filtered_books.get(self.selected_index) {
                    self.books[idx].progress_percent = 0;
                    self.show_toast("Marked as unread");
                }
                ActivityResult::Consumed
            }
            BookAction::Delete => {
                // In real implementation, show confirmation modal
                if let Some(&idx) = self.filtered_books.get(self.selected_index) {
                    let title = self.books[idx].title.clone();
                    self.show_toast(format!("Deleted: {}", title));
                    // Remove from list
                    self.books.remove(idx);
                    self.apply_sort();
                    if self.selected_index >= self.filtered_books.len() && self.selected_index > 0 {
                        self.selected_index -= 1;
                    }
                }
                ActivityResult::Consumed
            }
            BookAction::Cancel => ActivityResult::Consumed,
        }
    }

    /// Handle input when context menu is shown.
    /// Left/Right and VolumeUp/Down navigate buttons, Confirm selects, Back cancels.
    fn handle_context_menu_input(&mut self, event: InputEvent) -> ActivityResult {
        match event {
            InputEvent::Press(Button::Right) | InputEvent::Press(Button::VolumeDown) => {
                self.context_menu_next();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Left) | InputEvent::Press(Button::VolumeUp) => {
                self.context_menu_prev();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Confirm) => {
                let action = self.current_action();
                self.execute_action(action)
            }
            InputEvent::Press(Button::Back) => {
                self.close_context_menu();
                ActivityResult::Consumed
            }
            _ => ActivityResult::Ignored,
        }
    }

    /// Render header bar with title and sort button
    fn render_header<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        use crate::ui::Header;

        let sort_label = format!("Sort: {}", self.sort_order.label());
        let header = Header::new("Library").with_right_text(sort_label);
        header.render(display, &self.theme)
    }

    /// Render book list or empty state
    fn render_book_list<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        if self.is_loading {
            self.render_loading_state(display)?;
        } else if self.filtered_books.is_empty() {
            self.render_empty_state(display)?;
        } else {
            self.render_books(display)?;
        }
        Ok(())
    }

    /// Render loading state while scanning the filesystem.
    fn render_loading_state<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let display_height = display.bounding_box().size.height;
        let center_y = (display_height / 2) as i32;

        let message = "Scanning library...";
        let message_width = ThemeMetrics::text_width(message.len());
        let x = (display_width as i32 - message_width) / 2;

        let style = MonoTextStyleBuilder::new()
            .font(ui_font_title())
            .text_color(BinaryColor::On)
            .build();
        Text::new(message, Point::new(x, center_y), style).draw(display)?;

        let sub_message = "Searching /books recursively";
        let sub_width = ThemeMetrics::text_width(sub_message.len());
        let sub_x = (display_width as i32 - sub_width) / 2;
        let sub_style = MonoTextStyle::new(ui_font_body(), BinaryColor::On);
        Text::new(
            sub_message,
            Point::new(sub_x, center_y + layout::GAP_LG),
            sub_style,
        )
        .draw(display)?;

        Ok(())
    }

    /// Render empty state message
    fn render_empty_state<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let display_height = display.bounding_box().size.height;
        let center_y = (display_height / 2) as i32;

        let message = "No books found";
        let message_width = ThemeMetrics::text_width(message.len());
        let x = (display_width as i32 - message_width) / 2;

        let style = MonoTextStyleBuilder::new()
            .font(ui_font_title())
            .text_color(BinaryColor::On)
            .build();

        Text::new(message, Point::new(x, center_y), style).draw(display)?;

        let sub_message = "Add EPUB/TXT/MD files to /books";
        let sub_width = ThemeMetrics::text_width(sub_message.len());
        let sub_x = (display_width as i32 - sub_width) / 2;

        let sub_style = MonoTextStyle::new(ui_font_body(), BinaryColor::On);
        Text::new(
            sub_message,
            Point::new(sub_x, center_y + layout::GAP_LG),
            sub_style,
        )
        .draw(display)?;

        Ok(())
    }

    /// Render book items
    fn render_books<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let content_width = self.theme.metrics.content_width(display_width);
        let x = self.theme.metrics.side_padding as i32;
        let start_y = self.theme.metrics.header_height as i32;
        let item_height = self.theme.metrics.list_item_height;

        for (i, &book_idx) in self
            .filtered_books
            .iter()
            .skip(self.scroll_offset)
            .take(self.visible_count)
            .enumerate()
        {
            let list_index = self.scroll_offset + i;
            let y = start_y + (i as i32) * item_height as i32;
            let book = &self.books[book_idx];
            let is_selected = list_index == self.selected_index;

            self.render_book_item(display, book, x, y, content_width, item_height, is_selected)?;
        }

        // Render scroll indicator if needed
        if self.filtered_books.len() > self.visible_count {
            self.render_scroll_indicator(display)?;
        }

        Ok(())
    }

    /// Render a single book item
    #[allow(clippy::too_many_arguments)]
    fn render_book_item<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        book: &BookInfo,
        x: i32,
        y: i32,
        width: u32,
        item_height: u32,
        is_selected: bool,
    ) -> Result<(), D::Error> {
        // Background
        let bg_color = if is_selected {
            BinaryColor::On
        } else {
            BinaryColor::Off
        };
        Rectangle::new(Point::new(x, y), Size::new(width, item_height))
            .into_styled(PrimitiveStyle::with_fill(bg_color))
            .draw(display)?;

        // Cover placeholder (rectangle)
        let cover_padding = layout::BOOK_COVER_PAD;
        let cover_x = x + cover_padding as i32;
        let cover_y = y + cover_padding as i32;
        let cover_height = item_height - cover_padding * 2;
        self.render_cover_thumbnail(
            display,
            book,
            cover_x,
            cover_y,
            Self::COVER_WIDTH,
            cover_height,
        )?;

        // Text color based on selection
        let text_color = if is_selected {
            BinaryColor::Off
        } else {
            BinaryColor::On
        };

        let title_style = MonoTextStyle::new(ui_font_title(), text_color);
        let author_style = MonoTextStyle::new(ui_font_body(), text_color);

        // Title
        let title_x = x + Self::COVER_WIDTH as i32 + (cover_padding * 2) as i32;
        let title_y = y + layout::BOOK_TITLE_Y;
        let title_max_width = (x + width as i32 - title_x).max(0);
        let max_title_chars = (title_max_width / ui_font_body_char_width()) as usize;
        let title = book.display_title(max_title_chars);
        Text::new(title, Point::new(title_x, title_y), title_style).draw(display)?;

        // Author
        let author_y = y + layout::BOOK_AUTHOR_Y;
        let author = if book.author.len() > 25 {
            format!("{}...", &book.author[..22])
        } else {
            book.author.clone()
        };
        Text::new(&author, Point::new(title_x, author_y), author_style).draw(display)?;

        // Progress bar
        self.render_progress_bar(display, book.progress_percent, x, y, width, text_color)?;

        // Bottom separator
        let sep_y = y + item_height as i32 - 1;
        Rectangle::new(Point::new(x, sep_y), Size::new(width, 1))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        Ok(())
    }

    fn render_cover_thumbnail<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        book: &BookInfo,
        cover_x: i32,
        cover_y: i32,
        cover_width: u32,
        cover_height: u32,
    ) -> Result<(), D::Error> {
        if let Some(thumb) = &book.cover_thumbnail {
            // White card behind thumbnail keeps it readable when the list item is selected.
            Rectangle::new(
                Point::new(cover_x, cover_y),
                Size::new(cover_width, cover_height),
            )
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
            .draw(display)?;
            Rectangle::new(
                Point::new(cover_x, cover_y),
                Size::new(cover_width, cover_height),
            )
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;

            let offset_x = ((cover_width as i32 - thumb.width as i32).max(0)) / 2;
            let offset_y = ((cover_height as i32 - thumb.height as i32).max(0)) / 2;
            for y in 0..thumb.height.min(cover_height) {
                for x in 0..thumb.width.min(cover_width) {
                    if thumb.is_black(x, y) {
                        Pixel(
                            Point::new(
                                cover_x + offset_x + x as i32,
                                cover_y + offset_y + y as i32,
                            ),
                            BinaryColor::On,
                        )
                        .draw(display)?;
                    }
                }
            }
            Ok(())
        } else {
            // Fallback placeholder if thumbnail decode/extraction failed.
            Rectangle::new(
                Point::new(cover_x, cover_y),
                Size::new(cover_width, cover_height),
            )
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
            .draw(display)?;
            Rectangle::new(
                Point::new(cover_x, cover_y),
                Size::new(cover_width, cover_height),
            )
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;

            let icon_w = cover_width.saturating_sub(12).clamp(12, 20);
            let icon_h = cover_height.saturating_sub(12).clamp(14, 24);
            let icon_x = cover_x + ((cover_width as i32 - icon_w as i32).max(0) / 2);
            let icon_y = cover_y + ((cover_height as i32 - icon_h as i32).max(0) / 2);
            Rectangle::new(Point::new(icon_x, icon_y), Size::new(icon_w, icon_h))
                .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                .draw(display)?;
            Rectangle::new(
                Point::new(icon_x + 2, icon_y + 2),
                Size::new(icon_w.saturating_sub(4).max(1), 2),
            )
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;
            let line_w = icon_w.saturating_sub(6).max(2);
            Rectangle::new(Point::new(icon_x + 3, icon_y + 8), Size::new(line_w, 1))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
            Rectangle::new(Point::new(icon_x + 3, icon_y + 12), Size::new(line_w, 1))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)
        }
    }

    /// Render progress bar
    fn render_progress_bar<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        progress: u8,
        x: i32,
        y: i32,
        width: u32,
        text_color: BinaryColor,
    ) -> Result<(), D::Error> {
        let bar_y = y + layout::BOOK_PROGRESS_Y;
        let bar_width = layout::BOOK_PROGRESS_W;
        let bar_height = layout::PROGRESS_BAR_H;
        let bar_x = x + width as i32 - bar_width as i32 - self.theme.metrics.side_padding as i32;

        // Background bar
        Rectangle::new(Point::new(bar_x, bar_y), Size::new(bar_width, bar_height))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;

        // Progress fill
        let fill_width = (bar_width * progress as u32 / 100).min(bar_width - 2);
        if fill_width > 0 {
            Rectangle::new(
                Point::new(bar_x + 1, bar_y + 1),
                Size::new(fill_width, bar_height - 2),
            )
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;
        }

        // Percentage text
        let percent_label = format!("{}%", progress);
        let percent_x = bar_x - 35;
        let percent_style = MonoTextStyle::new(ui_font_small(), text_color);
        Text::new(
            &percent_label,
            Point::new(percent_x, bar_y + bar_height as i32),
            percent_style,
        )
        .draw(display)?;

        Ok(())
    }

    /// Render scroll indicator
    fn render_scroll_indicator<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let display_width = display.bounding_box().size.width;
        let display_height = display.bounding_box().size.height;
        let indicator_y = display_height as i32 - layout::MARGIN;
        let indicator_width = layout::SCROLL_INDICATOR_W;
        let indicator_x = (display_width as i32 - indicator_width) / 2;

        // Draw scroll bar background
        Rectangle::new(
            Point::new(indicator_x, indicator_y),
            Size::new(indicator_width as u32, layout::SCROLL_INDICATOR_H),
        )
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display)?;

        // Calculate thumb position
        let total_items = self.filtered_books.len();
        let thumb_width = (self.visible_count * indicator_width as usize / total_items).max(10);
        let max_offset = total_items.saturating_sub(self.visible_count);
        let thumb_pos = if max_offset > 0 {
            (self.scroll_offset * (indicator_width as usize - thumb_width) / max_offset) as i32
        } else {
            0
        };

        Rectangle::new(
            Point::new(indicator_x + thumb_pos, indicator_y),
            Size::new(thumb_width as u32, layout::SCROLL_INDICATOR_H),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display)?;

        Ok(())
    }

    /// Render context menu
    fn render_context_menu<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let book = self.selected_book().cloned();

        if let Some(book) = book {
            let title = format!("Options: {}", book.title);
            let mut modal = Modal::new(&title, "Select an action")
                .with_button("Open")
                .with_button("Mark Unread")
                .with_button("Delete")
                .with_button("Cancel");
            modal.selected_button = self.context_menu_index;
            modal.render(display, &self.theme)?;
        }

        Ok(())
    }
}

impl Activity for LibraryActivity {
    fn on_enter(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.show_context_menu = false;
        self.show_toast = false;
        self.pending_open_path = None;
    }

    fn on_exit(&mut self) {
        self.pending_open_path = None;
    }

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        if self.show_context_menu {
            return self.handle_context_menu_input(event);
        }

        match event {
            InputEvent::Press(Button::Back) => ActivityResult::NavigateBack,
            InputEvent::Press(Button::VolumeUp) => {
                self.select_prev();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::VolumeDown) => {
                self.select_next();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Left) => {
                self.cycle_sort();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Power) => {
                self.refresh_requested = true;
                self.begin_loading_scan();
                ActivityResult::Consumed
            }
            InputEvent::Press(Button::Right) | InputEvent::Press(Button::Confirm) => {
                if let Some(book) = self.selected_book() {
                    let path = book.path.clone();
                    self.pending_open_path = Some(path);
                    ActivityResult::Consumed
                } else {
                    ActivityResult::Consumed
                }
            }
            _ => ActivityResult::Ignored,
        }
    }

    fn render<D: DrawTarget<Color = BinaryColor>>(&self, display: &mut D) -> Result<(), D::Error> {
        // Clear background
        Rectangle::new(
            Point::new(0, 0),
            Size::new(
                display.bounding_box().size.width,
                display.bounding_box().size.height,
            ),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
        .draw(display)?;

        // Header
        self.render_header(display)?;

        // Book list
        self.render_book_list(display)?;

        // Toast notification
        if self.show_toast {
            let display_width = display.bounding_box().size.width;
            let display_height = display.bounding_box().size.height;
            let toast =
                crate::ui::Toast::bottom_center(&self.toast_message, display_width, display_height);
            toast.render(display)?;
        }

        // Context menu modal
        if self.show_context_menu {
            self.render_context_menu(display)?;
        }

        Ok(())
    }

    fn refresh_mode(&self) -> ActivityRefreshMode {
        if self.needs_full_refresh {
            ActivityRefreshMode::Full
        } else {
            ActivityRefreshMode::Fast
        }
    }
}

impl LibraryActivity {
    /// Mark that the initial full refresh has been performed
    pub fn mark_refresh_complete(&mut self) {
        self.needs_full_refresh = false;
    }
}

impl Default for LibraryActivity {
    fn default() -> Self {
        Self::new()
    }
}

/// Create mock books for testing
pub fn create_mock_books() -> Vec<BookInfo> {
    vec![
        BookInfo::new(
            "The Great Gatsby",
            "F. Scott Fitzgerald",
            "/books/gatsby.epub",
            75,
            Some(1704067200), // 2024-01-01
        ),
        BookInfo::new(
            "1984",
            "George Orwell",
            "/books/1984.epub",
            30,
            Some(1703980800), // 2023-12-31
        ),
        BookInfo::new(
            "Pride and Prejudice",
            "Jane Austen",
            "/books/pride.epub",
            100,
            Some(1703894400), // 2023-12-30
        ),
        BookInfo::new(
            "To Kill a Mockingbird",
            "Harper Lee",
            "/books/mockingbird.epub",
            0,
            None,
        ),
        BookInfo::new(
            "The Catcher in the Rye",
            "J.D. Salinger",
            "/books/catcher.epub",
            45,
            Some(1703808000), // 2023-12-29
        ),
        BookInfo::new(
            "Moby Dick",
            "Herman Melville",
            "/books/moby.epub",
            12,
            Some(1703721600), // 2023-12-28
        ),
        BookInfo::new(
            "War and Peace",
            "Leo Tolstoy",
            "/books/war_and_peace.epub",
            8,
            Some(1703635200), // 2023-12-27
        ),
        BookInfo::new(
            "The Hobbit",
            "J.R.R. Tolkien",
            "/books/hobbit.epub",
            100,
            Some(1703548800), // 2023-12-26
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "std")]
    use crate::mock_filesystem::MockFileSystem;

    fn build_test_bmp_24(width: u32, height: u32, pixels_top_down: &[[u8; 3]]) -> Vec<u8> {
        let row_bytes = (width as usize) * 3;
        let row_stride = (row_bytes + 3) & !3;
        let pixel_data_size = row_stride * height as usize;
        let file_size = 14 + 40 + pixel_data_size;
        let mut out = vec![0u8; file_size];

        out[0..2].copy_from_slice(b"BM");
        out[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
        out[10..14].copy_from_slice(&(54u32).to_le_bytes());
        out[14..18].copy_from_slice(&(40u32).to_le_bytes());
        out[18..22].copy_from_slice(&(width as i32).to_le_bytes());
        out[22..26].copy_from_slice(&(height as i32).to_le_bytes());
        out[26..28].copy_from_slice(&(1u16).to_le_bytes());
        out[28..30].copy_from_slice(&(24u16).to_le_bytes());
        out[34..38].copy_from_slice(&(pixel_data_size as u32).to_le_bytes());

        for y in 0..height {
            let src_y = (height - 1 - y) as usize; // BMP rows are bottom-up for positive height.
            let row_start = 54 + (y as usize) * row_stride;
            for x in 0..width {
                let rgb = pixels_top_down[src_y * width as usize + x as usize];
                let px = row_start + (x as usize) * 3;
                out[px] = rgb[2]; // B
                out[px + 1] = rgb[1]; // G
                out[px + 2] = rgb[0]; // R
            }
        }
        out
    }

    #[test]
    fn book_info_creation() {
        let book = BookInfo::new(
            "Test Title",
            "Test Author",
            "/path/to/book.epub",
            50,
            Some(1234567890),
        );

        assert_eq!(book.title, "Test Title");
        assert_eq!(book.author, "Test Author");
        assert_eq!(book.path, "/path/to/book.epub");
        assert_eq!(book.progress_percent, 50);
        assert_eq!(book.last_read, Some(1234567890));
    }

    #[test]
    fn book_info_progress_clamped() {
        let book = BookInfo::new("Test", "Author", "/path", 150, None);
        assert_eq!(book.progress_percent, 100);
    }

    #[test]
    fn sort_order_cycling() {
        let mut order = SortOrder::Title;
        assert_eq!(order.label(), "Title");

        order = order.next();
        assert_eq!(order, SortOrder::Author);

        order = order.next();
        assert_eq!(order, SortOrder::Recent);

        order = order.next();
        assert_eq!(order, SortOrder::Title);
    }

    #[test]
    fn library_activity_new() {
        let activity = LibraryActivity::new();
        assert!(activity.is_empty());
        assert_eq!(activity.book_count(), 0);
    }

    #[test]
    fn library_activity_with_books() {
        let books = create_mock_books();
        let activity = LibraryActivity::with_books(books.clone());

        assert_eq!(activity.book_count(), 8);
        assert!(!activity.is_empty());
    }

    #[test]
    fn library_activity_with_mock_books() {
        let activity = LibraryActivity::with_mock_books();
        assert_eq!(activity.book_count(), 8);
    }

    #[test]
    fn library_activity_selection() {
        let activity = LibraryActivity::with_mock_books();

        // First book should be selected by default
        let selected = activity.selected_book();
        assert!(selected.is_some());
    }

    #[test]
    fn library_activity_navigation() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        assert_eq!(activity.selected_index, 0);

        // Navigate down
        activity.select_next();
        assert_eq!(activity.selected_index, 1);

        activity.select_next();
        assert_eq!(activity.selected_index, 2);

        // Navigate up
        activity.select_prev();
        assert_eq!(activity.selected_index, 1);
    }

    #[test]
    fn library_activity_navigation_wraps() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        // Wrap backward from 0
        activity.select_prev();
        assert_eq!(activity.selected_index, 7); // Last of 8 books

        // Wrap forward from last
        activity.select_next();
        assert_eq!(activity.selected_index, 0);
    }

    #[test]
    fn library_activity_sort_by_title() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.sort_order = SortOrder::Title;
        activity.apply_sort();

        let first = activity.selected_book().unwrap();
        assert_eq!(first.title, "1984"); // Alphabetically first
    }

    #[test]
    fn library_activity_sort_by_author() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.sort_order = SortOrder::Author;
        activity.apply_sort();

        let first = activity.selected_book().unwrap();
        assert_eq!(first.author, "F. Scott Fitzgerald"); // Alphabetically first
    }

    #[test]
    fn library_activity_sort_by_recent() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.sort_order = SortOrder::Recent;
        activity.apply_sort();

        let first = activity.selected_book().unwrap();
        assert_eq!(first.title, "The Great Gatsby"); // Most recent
    }

    #[test]
    fn library_activity_cycle_sort() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        assert_eq!(activity.sort_order, SortOrder::Title);

        activity.cycle_sort();
        assert_eq!(activity.sort_order, SortOrder::Author);

        activity.cycle_sort();
        assert_eq!(activity.sort_order, SortOrder::Recent);

        activity.cycle_sort();
        assert_eq!(activity.sort_order, SortOrder::Title);
    }

    #[test]
    fn library_activity_input_back() {
        let mut activity = LibraryActivity::with_mock_books();
        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(result, ActivityResult::NavigateBack);
    }

    #[test]
    fn library_activity_input_navigation() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        let result = activity.handle_input(InputEvent::Press(Button::VolumeDown));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 1);

        let result = activity.handle_input(InputEvent::Press(Button::VolumeUp));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 0);
    }

    #[test]
    fn library_activity_input_volume_buttons() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        let result = activity.handle_input(InputEvent::Press(Button::VolumeDown));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 1);

        let result = activity.handle_input(InputEvent::Press(Button::VolumeUp));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.selected_index, 0);
    }

    #[test]
    fn library_activity_input_sort() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        assert_eq!(activity.sort_order, SortOrder::Title);

        let result = activity.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.sort_order, SortOrder::Author);
        assert!(activity.show_toast);
    }

    #[test]
    fn library_activity_context_menu() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        // Open context menu
        activity.open_context_menu();
        assert!(activity.show_context_menu);
        assert_eq!(activity.context_menu_index, 0);

        // Navigate within menu
        activity.context_menu_next();
        assert_eq!(activity.context_menu_index, 1);

        activity.context_menu_prev();
        assert_eq!(activity.context_menu_index, 0);

        // Close menu
        activity.close_context_menu();
        assert!(!activity.show_context_menu);
    }

    #[test]
    fn library_activity_context_menu_actions() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        activity.open_context_menu();

        assert_eq!(activity.current_action(), BookAction::Open);

        activity.context_menu_next();
        assert_eq!(activity.current_action(), BookAction::MarkUnread);

        activity.context_menu_next();
        assert_eq!(activity.current_action(), BookAction::Delete);

        activity.context_menu_next();
        assert_eq!(activity.current_action(), BookAction::Cancel);
    }

    #[test]
    fn library_activity_mark_unread() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        // Find a book with progress
        let first = activity.selected_book().unwrap();
        assert!(first.progress_percent > 0);

        // Mark as unread
        activity.execute_action(BookAction::MarkUnread);

        let first = activity.selected_book().unwrap();
        assert_eq!(first.progress_percent, 0);
    }

    #[test]
    fn library_activity_delete_book() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        let initial_count = activity.book_count();

        // Delete first book
        activity.execute_action(BookAction::Delete);

        assert_eq!(activity.book_count(), initial_count - 1);
    }

    #[test]
    fn library_activity_toast() {
        let mut activity = LibraryActivity::new();

        activity.show_toast("Test message");
        assert!(activity.show_toast);
        assert_eq!(activity.toast_message, "Test message");
        assert_eq!(
            activity.toast_frames_remaining,
            LibraryActivity::TOAST_DURATION
        );

        // Update toast
        activity.update();
        assert_eq!(
            activity.toast_frames_remaining,
            LibraryActivity::TOAST_DURATION - 1
        );

        // Simulate full duration
        for _ in 0..LibraryActivity::TOAST_DURATION {
            activity.update();
        }

        assert!(!activity.show_toast);
    }

    #[test]
    fn library_activity_render() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();

        let mut display = crate::test_display::TestDisplay::default_size();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn library_activity_render_empty() {
        let mut activity = LibraryActivity::new();
        activity.on_enter();

        let mut display = crate::test_display::TestDisplay::default_size();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn library_activity_render_with_context_menu() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();
        activity.open_context_menu();

        let mut display = crate::test_display::TestDisplay::default_size();
        let result = activity.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn library_activity_scroll_visibility() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.visible_count = 3; // Small for testing
        activity.on_enter();

        // Select beyond visible area
        activity.selected_index = 5;
        activity.ensure_visible();

        // Scroll offset should have adjusted
        assert!(activity.scroll_offset > 0);
    }

    #[test]
    fn mock_books_created() {
        let books = create_mock_books();
        assert_eq!(books.len(), 8);

        // Verify variety of progress values
        let progresses: Vec<u8> = books.iter().map(|b| b.progress_percent).collect();
        assert!(progresses.contains(&0));
        assert!(progresses.contains(&100));
        assert!(progresses.contains(&50) || progresses.contains(&45) || progresses.contains(&75));
    }

    #[test]
    fn book_info_display_title() {
        let book = BookInfo::new(
            "A Very Long Title That Needs Truncating",
            "Author",
            "/path",
            0,
            None,
        );

        let short = book.display_title(10);
        assert_eq!(short.len(), 10);

        let exact = book.display_title(5);
        assert_eq!(exact, "A Ver");
    }

    #[test]
    fn context_menu_input_handling() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();
        activity.open_context_menu();

        // Navigate with Right
        let result = activity.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.context_menu_index, 1);

        // Navigate with VolumeDown
        let result = activity.handle_input(InputEvent::Press(Button::VolumeDown));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.context_menu_index, 2);

        // Navigate back with VolumeUp
        let result = activity.handle_input(InputEvent::Press(Button::VolumeUp));
        assert_eq!(result, ActivityResult::Consumed);
        assert_eq!(activity.context_menu_index, 1);

        // Cancel with Back
        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(!activity.show_context_menu);
    }

    #[test]
    fn context_menu_confirm_action() {
        let mut activity = LibraryActivity::with_mock_books();
        activity.on_enter();
        activity.open_context_menu();

        // Confirm should open the book
        let result = activity.handle_input(InputEvent::Press(Button::Confirm));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(!activity.show_context_menu);
        assert!(activity.pending_open_path.is_some());
    }

    #[cfg(feature = "std")]
    #[test]
    fn discover_books_reads_epub_metadata_and_recurses() {
        let mut fs = MockFileSystem::new();
        let books = LibraryActivity::discover_books(&mut fs, "/books");

        assert!(books.len() >= 4);
        assert!(!books.iter().any(|book| book.path.contains("/.hidden/")));

        let epub = books
            .iter()
            .find(|book| book.path.ends_with("sample.epub"))
            .expect("sample EPUB should be discovered");
        assert_eq!(epub.title, "Sample EPUB Book");
        assert_eq!(epub.author, "Sample Author");

        let markdown = books
            .iter()
            .find(|book| book.path.ends_with("notes.md"))
            .expect("nested markdown should be discovered");
        assert_eq!(markdown.author, "Unknown");
        assert_eq!(markdown.title, "Notes");
    }

    #[cfg(feature = "std")]
    #[test]
    fn discover_books_extracts_cover_for_large_epub_file() {
        let mut fs = MockFileSystem::empty();
        fs.add_directory("/");
        fs.add_directory("/books");
        fs.add_file(
            "/books/Fundamental-Accessibility-Tests-Basic-Functionality-v2.0.0.epub",
            include_bytes!("../../../sample_books/Fundamental-Accessibility-Tests-Basic-Functionality-v2.0.0.epub"),
        );

        let books = LibraryActivity::discover_books(&mut fs, "/books");
        let book = books
            .iter()
            .find(|book| {
                book.path
                    .ends_with("Fundamental-Accessibility-Tests-Basic-Functionality-v2.0.0.epub")
            })
            .expect("sample EPUB should be discovered");
        assert!(book.cover_thumbnail.is_some());
    }

    #[cfg(feature = "std")]
    #[test]
    fn extract_epub_cover_thumbnail_supports_cover_xhtml_img_reference() {
        use mu_epub::metadata::{parse_container_xml, parse_opf};
        use mu_epub::zip::StreamingZip;
        use std::io::Cursor;

        let data = include_bytes!(
            "../../../sample_books/Fundamental-Accessibility-Tests-Basic-Functionality-v2.0.0.epub"
        )
        .to_vec();
        let mut zip = StreamingZip::new(Cursor::new(data)).expect("zip should open");
        let mut input_scratch = vec![0u8; 4096];
        let mut output_scratch = vec![0u8; 4096];

        let container_entry = zip
            .get_entry("META-INF/container.xml")
            .or_else(|| zip.get_entry("meta-inf/container.xml"))
            .expect("container.xml must exist")
            .clone();
        let mut container_buf = Vec::new();
        let container_read = LibraryActivity::read_zip_entry_with_scratch(
            &mut zip,
            &container_entry,
            &mut container_buf,
            &mut input_scratch,
            &mut output_scratch,
            LibraryActivity::MAX_EPUB_METADATA_BYTES as usize,
        )
        .expect("container should read");
        let opf_path =
            parse_container_xml(&container_buf[..container_read]).expect("opf path should parse");

        let opf_entry = zip
            .get_entry(&opf_path)
            .expect("opf entry should exist")
            .clone();
        let mut opf_buf = Vec::new();
        let opf_read = LibraryActivity::read_zip_entry_with_scratch(
            &mut zip,
            &opf_entry,
            &mut opf_buf,
            &mut input_scratch,
            &mut output_scratch,
            LibraryActivity::MAX_EPUB_METADATA_BYTES as usize,
        )
        .expect("opf should read");
        let metadata = parse_opf(&opf_buf[..opf_read]).expect("opf should parse");

        let thumb = LibraryActivity::extract_epub_cover_thumbnail(
            &mut zip,
            &opf_path,
            &metadata,
            &mut input_scratch,
            &mut output_scratch,
        );
        assert!(
            thumb.is_some(),
            "cover thumbnail should resolve via cover.xhtml"
        );
    }

    #[test]
    fn power_button_requests_manual_refresh() {
        let mut activity = LibraryActivity::new();
        let result = activity.handle_input(InputEvent::Press(Button::Power));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(activity.take_refresh_request());
        assert!(!activity.take_refresh_request());
    }

    #[test]
    fn bmp_decoder_handles_simple_24bit_image() {
        let bmp = build_test_bmp_24(
            2,
            2,
            &[[0, 0, 0], [255, 255, 255], [255, 255, 255], [0, 0, 0]],
        );
        let decoded = LibraryActivity::decode_bmp_to_luma(&bmp).expect("valid BMP should decode");
        assert_eq!(decoded.width, 2);
        assert_eq!(decoded.height, 2);
        assert_eq!(decoded.pixels.len(), 4);
        assert!(decoded.pixels[0] < 16);
        assert!(decoded.pixels[1] > 240);
    }

    #[test]
    fn scale_luma_to_binary_thumbnail_preserves_aspect_fit() {
        let source = LumaImage {
            width: 200,
            height: 100,
            pixels: vec![0u8; 200 * 100],
        };
        let thumb = LibraryActivity::scale_luma_to_binary_thumbnail(&source, 50, 44, 128).unwrap();
        assert_eq!(thumb.width, 50);
        assert_eq!(thumb.height, 25);
        assert!(thumb.is_black(0, 0));
    }

    #[test]
    fn adaptive_thumbnail_threshold_tracks_dark_images() {
        let source = LumaImage {
            width: 4,
            height: 4,
            pixels: vec![30u8; 16],
        };
        let threshold = LibraryActivity::adaptive_thumbnail_threshold(&source);
        assert!(threshold > 30);
    }

    #[test]
    fn adaptive_thumbnail_threshold_tracks_bright_images() {
        let source = LumaImage {
            width: 4,
            height: 4,
            pixels: vec![220u8; 16],
        };
        let threshold = LibraryActivity::adaptive_thumbnail_threshold(&source);
        assert!(threshold < 220);
    }

    #[test]
    fn decode_bmp_thumbnail_rejects_invalid_data() {
        assert!(LibraryActivity::decode_bmp_thumbnail(b"not-a-bmp", 50, 44).is_none());
    }

    #[cfg(feature = "std")]
    #[test]
    fn discover_books_loads_sidecar_bmp_for_non_epub() {
        let mut fs = MockFileSystem::empty();
        fs.add_directory("/");
        fs.add_directory("/books");
        fs.add_file("/books/notes.txt", b"example");
        let sidecar = build_test_bmp_24(
            2,
            2,
            &[[0, 0, 0], [255, 255, 255], [255, 255, 255], [0, 0, 0]],
        );
        fs.add_file("/books/notes.bmp", &sidecar);

        let books = LibraryActivity::discover_books(&mut fs, "/books");
        let notes = books
            .iter()
            .find(|book| book.path.ends_with("notes.txt"))
            .expect("sidecar test book should exist");
        assert!(notes.cover_thumbnail.is_some());
    }

    #[cfg(feature = "std")]
    #[test]
    fn sidecar_cover_candidates_include_calibre_cover_jpg() {
        let candidates = LibraryActivity::sidecar_cover_candidates(
            "/books/Author/Book (123)/Book - Author.epub",
        );
        assert!(candidates
            .iter()
            .any(|path| path == "/books/Author/Book (123)/cover.jpg"));
        assert!(candidates
            .iter()
            .any(|path| path == "/books/Author/Book (123)/cover.jpeg"));
    }

    #[cfg(all(feature = "std", not(target_os = "espidf")))]
    #[test]
    fn discover_books_loads_calibre_cover_jpg_sidecar() {
        use image::codecs::jpeg::JpegEncoder;

        let mut fs = MockFileSystem::empty();
        fs.add_directory("/");
        fs.add_directory("/books");
        fs.add_directory("/books/Author");
        fs.add_directory("/books/Author/Book (123)");
        fs.add_file(
            "/books/Author/Book (123)/Book - Author.epub",
            b"not-an-epub",
        );

        let rgb: [u8; 12] = [0, 0, 0, 255, 255, 255, 255, 255, 255, 0, 0, 0];
        let mut jpeg = Vec::new();
        let mut encoder = JpegEncoder::new_with_quality(&mut jpeg, 80);
        encoder
            .encode(&rgb, 2, 2, image::ExtendedColorType::Rgb8)
            .expect("test jpeg encode should succeed");
        fs.add_file("/books/Author/Book (123)/cover.jpg", &jpeg);

        let books = LibraryActivity::discover_books(&mut fs, "/books");
        let book = books
            .iter()
            .find(|book| book.path.ends_with("Book - Author.epub"))
            .expect("calibre-style book should exist");
        assert!(book.cover_thumbnail.is_some());
    }

    #[cfg(feature = "std")]
    #[test]
    fn cover_cache_roundtrip_persists_thumbnail_artifact() {
        let bmp = build_test_bmp_24(
            2,
            2,
            &[[0, 0, 0], [255, 255, 255], [255, 255, 255], [0, 0, 0]],
        );
        let thumb = LibraryActivity::decode_bmp_thumbnail(&bmp, 40, 40)
            .expect("bmp decode should produce a thumbnail");
        let book_path = "/books/test-cache-roundtrip.epub";
        let size = 4242u64;
        let cache_path = LibraryActivity::cover_cache_path(book_path, size);
        let _ = std::fs::remove_file(&cache_path);

        assert!(LibraryActivity::persist_cached_cover_thumbnail(
            book_path, size, &thumb
        ));
        let loaded = LibraryActivity::load_cached_cover_thumbnail(book_path, size);
        assert!(loaded.is_some(), "cached artifact should round-trip");

        let _ = std::fs::remove_file(&cache_path);
    }
}
