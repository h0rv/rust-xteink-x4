//! Activity wrapper for the raw filesystem browser.
//!
//! Keeps filesystem I/O out of the input/render path by scheduling
//! deferred tasks that are processed by the app loop.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
#[cfg(feature = "std")]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::collections::BTreeMap;
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::sync::mpsc::{self, Receiver, TryRecvError};
#[cfg(feature = "std")]
use std::thread;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
#[cfg(feature = "std")]
use mu_epub::book::{ChapterEventsOptions, OpenConfig};
#[cfg(all(feature = "std", not(target_os = "espidf")))]
use mu_epub::{EmbeddedFontStyle, FontLimits};
#[cfg(feature = "std")]
use mu_epub::{EpubBook, ScratchBuffers, ZipLimits};
#[cfg(all(feature = "std", not(target_os = "espidf")))]
use mu_epub_embedded_graphics::FontFaceRegistration;
#[cfg(feature = "std")]
use mu_epub_embedded_graphics::{EgRenderConfig, EgRenderer};
#[cfg(feature = "std")]
use mu_epub_render::{RenderConfig, RenderEngine, RenderEngineOptions, RenderPage};
#[cfg(feature = "std")]
use std::io::{Read, Seek, SeekFrom};

#[cfg(all(feature = "std", feature = "fontdue", not(target_os = "espidf")))]
use crate::epub_font_backend::BookerlyFontBackend;
use crate::file_browser::{FileBrowser, TextViewer};
use crate::filesystem::{basename, dirname, FileSystem};
use crate::input::{Button, InputEvent};
use crate::ui::{Activity, ActivityResult};

#[derive(Debug, Clone)]
enum FileBrowserTask {
    LoadCurrentDirectory,
    OpenPath { path: String },
    OpenTextFile { path: String },
    OpenEpubFile { path: String },
}

enum BrowserMode {
    Browsing,
    #[cfg(feature = "std")]
    OpeningEpub,
    ReadingText {
        title: String,
        viewer: TextViewer,
    },
    #[cfg(feature = "std")]
    ReadingEpub {
        renderer: Box<EpubReadingState>,
    },
}

#[cfg(feature = "std")]
trait ReadSeek: Read + Seek + Send {}

#[cfg(feature = "std")]
impl<T: Read + Seek + Send> ReadSeek for T {}

#[cfg(feature = "std")]
struct PendingEpubOpen {
    receiver: Receiver<Result<EpubReadingState, String>>,
}

#[cfg(feature = "std")]
enum EpubOpenSource {
    HostPath(String),
    Chunks(Vec<Vec<u8>>),
}

#[cfg(feature = "std")]
struct EpubReadingState {
    book: EpubBook<Box<dyn ReadSeek>>,
    engine: RenderEngine,
    eg_renderer: ReaderRenderer,
    chapter_buf: Vec<u8>,
    chapter_scratch: ScratchBuffers,
    current_page: Option<RenderPage>,
    page_cache: BTreeMap<(usize, usize), RenderPage>,
    chapter_page_counts: BTreeMap<usize, usize>,
    chapter_idx: usize,
    page_idx: usize,
}

#[cfg(feature = "std")]
#[derive(Debug, Default, Clone)]
struct ChunkedEpubReader {
    chunks: Vec<Vec<u8>>,
    chunk_offsets: Vec<usize>,
    total_len: usize,
    pos: usize,
}

#[cfg(feature = "std")]
impl ChunkedEpubReader {
    fn from_chunks(chunks: Vec<Vec<u8>>) -> Self {
        let mut chunk_offsets = Vec::with_capacity(chunks.len());
        let mut total_len = 0usize;
        for chunk in &chunks {
            chunk_offsets.push(total_len);
            total_len = total_len.saturating_add(chunk.len());
        }
        Self {
            chunks,
            chunk_offsets,
            total_len,
            pos: 0,
        }
    }

    fn locate_chunk(&self, absolute_pos: usize) -> Option<(usize, usize)> {
        if self.chunks.is_empty() || absolute_pos >= self.total_len {
            return None;
        }
        match self.chunk_offsets.binary_search(&absolute_pos) {
            Ok(idx) => Some((idx, 0)),
            Err(insert_idx) => {
                if insert_idx == 0 {
                    None
                } else {
                    let chunk_idx = insert_idx - 1;
                    let in_chunk = absolute_pos - self.chunk_offsets[chunk_idx];
                    Some((chunk_idx, in_chunk))
                }
            }
        }
    }
}

#[cfg(feature = "std")]
impl Read for ChunkedEpubReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() || self.pos >= self.total_len {
            return Ok(0);
        }

        let mut written = 0usize;
        while written < buf.len() && self.pos < self.total_len {
            let Some((chunk_idx, in_chunk)) = self.locate_chunk(self.pos) else {
                break;
            };
            let chunk = &self.chunks[chunk_idx];
            let available = chunk.len().saturating_sub(in_chunk);
            if available == 0 {
                break;
            }
            let to_copy = available.min(buf.len() - written);
            let src = &chunk[in_chunk..in_chunk + to_copy];
            let dst = &mut buf[written..written + to_copy];
            dst.copy_from_slice(src);
            written += to_copy;
            self.pos += to_copy;
        }

        Ok(written)
    }
}

#[cfg(feature = "std")]
impl Seek for ChunkedEpubReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let base = match pos {
            SeekFrom::Start(offset) => offset as i128,
            SeekFrom::End(offset) => self.total_len as i128 + offset as i128,
            SeekFrom::Current(offset) => self.pos as i128 + offset as i128,
        };
        if base < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid seek before start",
            ));
        }
        let next = usize::try_from(base).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid seek position")
        })?;
        self.pos = next.min(self.total_len);
        Ok(self.pos as u64)
    }
}

#[cfg(all(feature = "std", feature = "fontdue", not(target_os = "espidf")))]
type ReaderRenderer = EgRenderer<BookerlyFontBackend>;

#[cfg(any(
    all(feature = "std", not(feature = "fontdue")),
    all(feature = "std", target_os = "espidf")
))]
type ReaderRenderer = EgRenderer<mu_epub_embedded_graphics::MonoFontBackend>;

#[cfg(feature = "std")]
impl EpubReadingState {
    #[cfg(not(target_os = "espidf"))]
    const MAX_FONT_FACE_BYTES: usize = 512 * 1024;
    #[cfg(not(target_os = "espidf"))]
    const MAX_FONT_TOTAL_BYTES: usize = 2 * 1024 * 1024;
    const MAX_ZIP_ENTRY_BYTES: usize = 8 * 1024 * 1024;
    const MAX_MIMETYPE_BYTES: usize = 1024;
    const MAX_NAV_BYTES: usize = 256 * 1024;
    const MAX_EOCD_SCAN_BYTES: usize = 8 * 1024;
    #[cfg(target_os = "espidf")]
    const MAX_CHAPTER_EVENTS: usize = 16_384;
    #[cfg(not(target_os = "espidf"))]
    const MAX_CHAPTER_EVENTS: usize = 65_536;
    #[cfg(target_os = "espidf")]
    const CHAPTER_BUF_CAPACITY_BYTES: usize = 16 * 1024;
    #[cfg(not(target_os = "espidf"))]
    const CHAPTER_BUF_CAPACITY_BYTES: usize = 64 * 1024;
    #[cfg(target_os = "espidf")]
    #[allow(dead_code)]
    const PAGE_CACHE_LIMIT: usize = 0;
    #[cfg(not(target_os = "espidf"))]
    const PAGE_CACHE_LIMIT: usize = 8;
    const OUT_OF_RANGE_ERR: &'static str = "Requested EPUB page is out of range";

    fn from_reader(reader: Box<dyn ReadSeek>) -> Result<Self, String> {
        log::info!("[EPUB] opening reader");
        let zip_limits = ZipLimits::new(Self::MAX_ZIP_ENTRY_BYTES, Self::MAX_MIMETYPE_BYTES)
            .with_max_eocd_scan(Self::MAX_EOCD_SCAN_BYTES);
        let open_cfg = OpenConfig {
            options: mu_epub::book::EpubBookOptions {
                zip_limits: Some(zip_limits),
                validation_mode: mu_epub::book::ValidationMode::Lenient,
                max_nav_bytes: Some(Self::MAX_NAV_BYTES),
            },
            lazy_navigation: true,
        };
        let book = EpubBook::from_reader_with_config(reader, open_cfg)
            .map_err(|e| format!("Unable to parse EPUB: {}", e))?;
        log::info!("[EPUB] open ok: chapters={}", book.chapter_count());
        let mut state = Self {
            book,
            engine: RenderEngine::new(RenderEngineOptions::for_display(
                crate::DISPLAY_WIDTH as i32,
                crate::DISPLAY_HEIGHT as i32,
            )),
            eg_renderer: Self::create_renderer(),
            chapter_buf: Vec::with_capacity(Self::CHAPTER_BUF_CAPACITY_BYTES),
            chapter_scratch: ScratchBuffers::embedded(),
            current_page: None,
            page_cache: BTreeMap::new(),
            chapter_page_counts: BTreeMap::new(),
            chapter_idx: 0,
            page_idx: 0,
        };
        state.register_embedded_fonts();
        state.load_chapter_forward(0)?;
        log::info!("[EPUB] initial chapter/page loaded");
        Ok(state)
    }

    fn load_chapter_exact(&mut self, chapter_idx: usize) -> Result<(), String> {
        log::info!("[EPUB] load_chapter_exact idx={}", chapter_idx);
        self.chapter_idx = chapter_idx;
        self.page_idx = 0;
        self.current_page = None;
        self.load_current_page()?;
        Ok(())
    }

    fn load_chapter_forward(&mut self, start_chapter_idx: usize) -> Result<(), String> {
        for idx in start_chapter_idx..self.book.chapter_count() {
            match self.load_chapter_exact(idx) {
                Ok(()) => return Ok(()),
                Err(err) if Self::is_out_of_range_error(&err) => continue,
                Err(err) => return Err(err),
            }
        }
        Err("No renderable pages found in remaining chapters".to_string())
    }

    fn load_chapter_backward(&mut self, start_chapter_idx: usize) -> Result<(), String> {
        let mut idx = start_chapter_idx as i32;
        while idx >= 0 {
            match self.load_chapter_exact(idx as usize) {
                Ok(()) => return Ok(()),
                Err(err) if Self::is_out_of_range_error(&err) => {
                    idx -= 1;
                }
                Err(err) => return Err(err),
            }
        }
        Err("No renderable pages found in previous chapters".to_string())
    }

    fn is_out_of_range_error(err: &str) -> bool {
        err.contains(Self::OUT_OF_RANGE_ERR)
    }

    fn current_chapter(&self) -> usize {
        self.chapter_idx + 1
    }

    fn total_chapters(&self) -> usize {
        self.book.chapter_count()
    }

    fn current_page_number(&self) -> usize {
        self.page_idx + 1
    }

    fn total_pages(&self) -> usize {
        self.chapter_page_counts
            .get(&self.chapter_idx)
            .copied()
            .unwrap_or(1)
    }

    fn next_page(&mut self) -> bool {
        let previous_chapter = self.chapter_idx;
        let previous_page = self.page_idx;
        // Free the currently rendered page before loading the next one to
        // maximize contiguous heap on constrained devices.
        self.current_page = None;
        let known_total = self.chapter_page_counts.get(&self.chapter_idx).copied();
        let can_advance = known_total.is_none() || self.page_idx + 1 < known_total.unwrap_or(0);
        if can_advance {
            let next_idx = self.page_idx + 1;
            if let Ok(page) = self.load_page(self.chapter_idx, next_idx) {
                self.page_idx = next_idx;
                self.current_page = Some(page);
                return true;
            }
        }
        if self.chapter_idx + 1 < self.book.chapter_count()
            && self.load_chapter_forward(self.chapter_idx + 1).is_ok()
        {
            return true;
        }
        if let Ok(page) = self.load_page(previous_chapter, previous_page) {
            self.chapter_idx = previous_chapter;
            self.page_idx = previous_page;
            self.current_page = Some(page);
        }
        log::warn!(
            "[EPUB] next_page failed at chapter={} page={}",
            previous_chapter,
            previous_page
        );
        false
    }

    fn prev_page(&mut self) -> bool {
        let previous_chapter = self.chapter_idx;
        let previous_page = self.page_idx;
        // Free the currently rendered page before loading the previous one to
        // maximize contiguous heap on constrained devices.
        self.current_page = None;
        if self.page_idx > 0 {
            let prev_idx = self.page_idx - 1;
            if let Ok(page) = self.load_page(self.chapter_idx, prev_idx) {
                self.page_idx = prev_idx;
                self.current_page = Some(page);
                return true;
            }
        }
        if self.chapter_idx > 0 {
            let prev_chapter = self.chapter_idx - 1;
            if self.load_chapter_backward(prev_chapter).is_ok() {
                let total_prev = self
                    .chapter_page_counts
                    .get(&prev_chapter)
                    .copied()
                    .unwrap_or(1);
                self.page_idx = total_prev.saturating_sub(1);
                if let Ok(page) = self.load_page(self.chapter_idx, self.page_idx) {
                    self.current_page = Some(page);
                    return true;
                }
            }
        }
        if let Ok(page) = self.load_page(previous_chapter, previous_page) {
            self.chapter_idx = previous_chapter;
            self.page_idx = previous_page;
            self.current_page = Some(page);
        }
        log::warn!(
            "[EPUB] prev_page failed at chapter={} page={}",
            previous_chapter,
            previous_page
        );
        false
    }

    fn render<D: DrawTarget<Color = BinaryColor>>(&self, display: &mut D) -> Result<(), D::Error> {
        if let Some(page) = self.current_page.as_ref() {
            self.eg_renderer.render_page(page, display)
        } else {
            display.clear(BinaryColor::Off)
        }
    }

    fn load_current_page(&mut self) -> Result<(), String> {
        let page = self.load_page(self.chapter_idx, self.page_idx)?;
        self.current_page = Some(page);
        Ok(())
    }

    fn load_page(&mut self, chapter_idx: usize, page_idx: usize) -> Result<RenderPage, String> {
        if let Some(page) = self.page_cache.get(&(chapter_idx, page_idx)) {
            log::info!(
                "[EPUB] page cache hit chapter={} page={}",
                chapter_idx,
                page_idx
            );
            return Ok(page.clone());
        }
        log::info!(
            "[EPUB] load_page start chapter={} page={} cache_entries={}",
            chapter_idx,
            page_idx,
            self.page_cache.len()
        );

        let mut target_page: Option<RenderPage> = None;
        let mut session = self.engine.begin(
            chapter_idx,
            RenderConfig::default().with_page_range(page_idx..page_idx + 1),
        );
        let mut layout_error: Option<String> = None;
        let chapter_opts = ChapterEventsOptions {
            max_items: Self::MAX_CHAPTER_EVENTS,
            ..ChapterEventsOptions::default()
        };

        self.book
            .chapter_events_with_scratch(
                chapter_idx,
                chapter_opts,
                &mut self.chapter_buf,
                &mut self.chapter_scratch,
                |item| {
                    if layout_error.is_some() {
                        return Ok(());
                    }
                    if target_page.is_some() {
                        return Ok(());
                    }
                    if let Err(err) = session.push(item) {
                        layout_error = Some(err.to_string());
                        return Ok(());
                    }
                    session.drain_pages(|page| {
                        if target_page.is_none() {
                            target_page = Some(page);
                        }
                    });
                    Ok(())
                },
            )
            .map_err(|e| format!("Unable to stream EPUB chapter: {}", e))?;
        log::info!("[EPUB] chapter_events streamed chapter={}", chapter_idx);

        if let Some(err) = layout_error {
            return Err(format!("Unable to layout EPUB chapter: {}", err));
        }

        // If the target page was already found, avoid finalizing this session:
        // `mu_epub_render` currently retains rendered page clones internally
        // during session finish, which can spike memory on constrained devices.
        if target_page.is_none() {
            session
                .finish()
                .map_err(|e| format!("Unable to finalize EPUB chapter layout: {}", e))?;
            session.drain_pages(|page| {
                if target_page.is_none() {
                    target_page = Some(page);
                }
            });
        }

        let page = target_page.ok_or_else(|| Self::OUT_OF_RANGE_ERR.to_string())?;
        log::info!(
            "[EPUB] load_page ok chapter={} page={} total_in_chapter={:?}",
            chapter_idx,
            page_idx,
            page.metrics.chapter_page_count
        );

        if let Some(count) = page.metrics.chapter_page_count {
            self.chapter_page_counts.insert(chapter_idx, count);
        }

        #[cfg(not(target_os = "espidf"))]
        {
            self.page_cache
                .insert((chapter_idx, page_idx), page.clone());
            self.trim_page_cache();
        }
        Ok(page)
    }

    #[allow(dead_code)]
    fn trim_page_cache(&mut self) {
        while self.page_cache.len() > Self::PAGE_CACHE_LIMIT {
            let Some((&key, _)) = self.page_cache.iter().next() else {
                break;
            };
            self.page_cache.remove(&key);
        }
    }

    fn create_renderer() -> ReaderRenderer {
        let cfg = EgRenderConfig::default();
        #[cfg(all(feature = "std", feature = "fontdue", not(target_os = "espidf")))]
        {
            EgRenderer::with_backend(cfg, BookerlyFontBackend::default())
        }
        #[cfg(any(
            all(feature = "std", not(feature = "fontdue")),
            all(feature = "std", target_os = "espidf")
        ))]
        {
            EgRenderer::with_backend(cfg, mu_epub_embedded_graphics::MonoFontBackend)
        }
    }

    fn register_embedded_fonts(&mut self) {
        #[cfg(target_os = "espidf")]
        {
            // On-device we default to bundled font families (e.g. Bookerly) and
            // avoid eager runtime TTF parsing to keep EPUB open deterministic.
        }

        #[cfg(not(target_os = "espidf"))]
        {
            let font_limits = FontLimits {
                max_faces: 16,
                max_bytes_per_font: Self::MAX_FONT_FACE_BYTES,
                max_total_font_bytes: Self::MAX_FONT_TOTAL_BYTES,
            };
            let Ok(embedded) = self.book.embedded_fonts_with_limits(font_limits) else {
                return;
            };
            for face in embedded {
                let italic = matches!(
                    face.style,
                    EmbeddedFontStyle::Italic | EmbeddedFontStyle::Oblique
                );
                let mut bytes = Vec::new();
                let Ok(_) = self.book.read_resource_into_with_limit(
                    &face.href,
                    &mut bytes,
                    Self::MAX_FONT_FACE_BYTES,
                ) else {
                    continue;
                };
                let registration = [FontFaceRegistration {
                    family: &face.family,
                    weight: face.weight,
                    italic,
                    data: &bytes,
                }];
                let _ = self.eg_renderer.register_faces(&registration);
            }
        }
    }
}

/// Raw filesystem browser activity.
pub struct FileBrowserActivity {
    browser: FileBrowser,
    mode: BrowserMode,
    pending_task: Option<FileBrowserTask>,
    return_to_previous_on_back: bool,
    #[cfg(feature = "std")]
    epub_open_pending: Option<PendingEpubOpen>,
}

impl FileBrowserActivity {
    pub const DEFAULT_ROOT: &'static str = "/";
    #[cfg(feature = "std")]
    const EPUB_READ_CHUNK_BYTES: usize = 4096;
    #[cfg(not(target_os = "espidf"))]
    const EPUB_OPEN_WORKER_STACK_BYTES: usize = 64 * 1024;

    pub fn new() -> Self {
        Self {
            browser: FileBrowser::new(Self::DEFAULT_ROOT),
            mode: BrowserMode::Browsing,
            pending_task: None,
            return_to_previous_on_back: false,
            #[cfg(feature = "std")]
            epub_open_pending: None,
        }
    }

    pub fn current_path(&self) -> &str {
        self.browser.current_path()
    }

    pub fn is_viewing_text(&self) -> bool {
        matches!(self.mode, BrowserMode::ReadingText { .. })
    }

    pub fn is_viewing_epub(&self) -> bool {
        #[cfg(feature = "std")]
        {
            matches!(self.mode, BrowserMode::ReadingEpub { .. })
        }

        #[cfg(not(feature = "std"))]
        {
            false
        }
    }

    fn is_opening_epub(&self) -> bool {
        #[cfg(feature = "std")]
        {
            matches!(self.mode, BrowserMode::OpeningEpub)
        }

        #[cfg(not(feature = "std"))]
        {
            false
        }
    }

    /// Returns current EPUB reading position as:
    /// `(chapter_index_1_based, chapter_total, page_index_1_based, page_total_in_chapter)`.
    pub fn epub_position(&self) -> Option<(usize, usize, usize, usize)> {
        #[cfg(feature = "std")]
        {
            if let BrowserMode::ReadingEpub { renderer } = &self.mode {
                return Some((
                    renderer.current_chapter(),
                    renderer.total_chapters(),
                    renderer.current_page_number(),
                    renderer.total_pages(),
                ));
            }
        }

        None
    }

    #[inline(never)]
    pub fn process_pending_task(&mut self, fs: &mut dyn FileSystem) -> bool {
        #[cfg(feature = "std")]
        let mut updated = self.poll_epub_open_result();
        #[cfg(not(feature = "std"))]
        let mut updated = false;

        let Some(task) = self.pending_task.take() else {
            return updated;
        };

        let task_updated = match task {
            FileBrowserTask::LoadCurrentDirectory => self.process_load_current_directory_task(fs),
            FileBrowserTask::OpenPath { path } => self.process_open_path_task(fs, &path),
            FileBrowserTask::OpenTextFile { path } => self.process_open_text_file_task(fs, &path),
            FileBrowserTask::OpenEpubFile { path } => self.process_open_epub_file_task(fs, &path),
        };
        updated |= task_updated;
        updated
    }

    #[inline(never)]
    fn process_load_current_directory_task(&mut self, fs: &mut dyn FileSystem) -> bool {
        self.mode = BrowserMode::Browsing;

        if let Err(error) = self.browser.load(fs) {
            self.browser
                .set_status_message(format!("Unable to open folder: {}", error));
        }

        true
    }

    #[inline(never)]
    fn process_open_path_task(&mut self, fs: &mut dyn FileSystem, path: &str) -> bool {
        self.open_path(fs, path);
        true
    }

    #[inline(never)]
    fn process_open_text_file_task(&mut self, fs: &mut dyn FileSystem, path: &str) -> bool {
        match fs.read_file(path) {
            Ok(content) => {
                let title = basename(path).to_string();
                self.mode = BrowserMode::ReadingText {
                    title,
                    viewer: TextViewer::new(content),
                };
            }
            Err(error) => {
                self.browser
                    .set_status_message(format!("Unable to open file: {}", error));
                self.mode = BrowserMode::Browsing;
            }
        }
        true
    }

    #[inline(never)]
    fn process_open_epub_file_task(&mut self, _fs: &mut dyn FileSystem, path: &str) -> bool {
        #[cfg(feature = "std")]
        {
            match Self::spawn_epub_open_worker(_fs, path) {
                Ok(receiver) => {
                    self.epub_open_pending = Some(PendingEpubOpen { receiver });
                    self.mode = BrowserMode::OpeningEpub;
                    self.browser
                        .set_status_message(format!("Opening EPUB: {}", basename(path)));
                }
                Err(error) => {
                    self.mode = BrowserMode::Browsing;
                    self.browser.set_status_message(error);
                }
            }
        }

        #[cfg(not(feature = "std"))]
        {
            let _ = path;
            self.mode = BrowserMode::Browsing;
            self.browser
                .set_status_message("Unsupported file type: .epub".to_string());
        }
        true
    }

    #[cfg(feature = "std")]
    fn poll_epub_open_result(&mut self) -> bool {
        let recv_result = match self.epub_open_pending.as_mut() {
            Some(pending) => pending.receiver.try_recv(),
            None => return false,
        };

        match recv_result {
            Ok(Ok(renderer)) => {
                self.epub_open_pending = None;
                self.mode = BrowserMode::ReadingEpub {
                    renderer: Box::new(renderer),
                };
                true
            }
            Ok(Err(error)) => {
                self.epub_open_pending = None;
                self.mode = BrowserMode::Browsing;
                self.browser.set_status_message(error);
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.epub_open_pending = None;
                self.mode = BrowserMode::Browsing;
                self.browser
                    .set_status_message("Unable to open EPUB: worker disconnected".to_string());
                true
            }
        }
    }

    fn queue_task(&mut self, task: FileBrowserTask) {
        self.pending_task = Some(task);
    }

    fn queue_load_current_directory(&mut self) {
        self.queue_task(FileBrowserTask::LoadCurrentDirectory);
    }

    pub fn request_open_path(&mut self, path: impl Into<String>) {
        self.mode = BrowserMode::Browsing;
        self.return_to_previous_on_back = true;
        let path = path.into();
        if Self::is_text_file(&path) || Self::is_epub_file(&path) {
            // Open directly without first loading parent directory, so
            // library-open doesn't flash filesystem browser UI.
            self.queue_open_file(path);
        } else {
            self.queue_task(FileBrowserTask::OpenPath { path });
        }
    }

    fn queue_open_file(&mut self, path: String) {
        if Self::is_text_file(&path) {
            self.queue_task(FileBrowserTask::OpenTextFile { path });
        } else if cfg!(feature = "std") && Self::is_epub_file(&path) {
            self.queue_task(FileBrowserTask::OpenEpubFile { path });
        } else {
            let filename = basename(&path);
            self.browser
                .set_status_message(format!("Unsupported file type: {}", filename));
        }
    }

    fn is_text_file(path: &str) -> bool {
        let lower = path.to_lowercase();
        lower.ends_with(".txt") || lower.ends_with(".md")
    }

    fn is_epub_file(path: &str) -> bool {
        let lower = path.to_lowercase();
        // FAT 8.3 backends can expose EPUB as `.epu`.
        lower.ends_with(".epub") || lower.ends_with(".epu")
    }

    fn open_path(&mut self, fs: &mut dyn FileSystem, path: &str) {
        let info = match fs.file_info(path) {
            Ok(info) => info,
            Err(error) => {
                self.mode = BrowserMode::Browsing;
                self.browser
                    .set_status_message(format!("Unable to open path: {}", error));
                return;
            }
        };

        if info.is_directory {
            self.mode = BrowserMode::Browsing;
            self.browser.set_path(path);
            if let Err(error) = self.browser.load(fs) {
                self.browser
                    .set_status_message(format!("Unable to open folder: {}", error));
            }
            return;
        }

        let parent = dirname(path);
        self.browser.set_path(parent);
        if let Err(error) = self.browser.load(fs) {
            self.browser
                .set_status_message(format!("Unable to open folder: {}", error));
        }

        self.queue_open_file(path.to_string());
    }

    #[cfg(feature = "std")]
    fn prepare_epub_open_source(
        fs: &mut dyn FileSystem,
        path: &str,
    ) -> Result<EpubOpenSource, String> {
        if let Some(host_path) = Self::resolve_host_backed_epub_path(path) {
            return Ok(EpubOpenSource::HostPath(host_path));
        }

        let mut chunks: Vec<Vec<u8>> = Vec::new();
        let mut on_chunk = |chunk: &[u8]| -> Result<(), crate::filesystem::FileSystemError> {
            chunks.push(chunk.to_vec());
            Ok(())
        };
        fs.read_file_chunks(path, Self::EPUB_READ_CHUNK_BYTES, &mut on_chunk)
            .map_err(|e| format!("Unable to read EPUB: {}", e))?;

        if chunks.is_empty() {
            return Err("Unable to read EPUB: empty file".to_string());
        }

        Ok(EpubOpenSource::Chunks(chunks))
    }

    #[inline(never)]
    fn spawn_epub_open_worker(
        fs: &mut dyn FileSystem,
        path: &str,
    ) -> Result<Receiver<Result<EpubReadingState, String>>, String> {
        let source = Self::prepare_epub_open_source(fs, path)?;
        let (tx, rx) = mpsc::channel();
        #[cfg(target_os = "espidf")]
        let builder = thread::Builder::new().name("epub-open-worker".to_string());
        #[cfg(not(target_os = "espidf"))]
        let builder = thread::Builder::new()
            .name("epub-open-worker".to_string())
            .stack_size(Self::EPUB_OPEN_WORKER_STACK_BYTES);
        builder
            .spawn(move || {
                let result = match source {
                    EpubOpenSource::HostPath(path) => match File::open(&path) {
                        Ok(file) => EpubReadingState::from_reader(Box::new(file)),
                        Err(err) => Err(format!("Unable to read EPUB: {}", err)),
                    },
                    EpubOpenSource::Chunks(chunks) => EpubReadingState::from_reader(Box::new(
                        ChunkedEpubReader::from_chunks(chunks),
                    )),
                };
                let _ = tx.send(result);
            })
            .map_err(|e| format!("Unable to start EPUB worker: {}", e))?;
        Ok(rx)
    }

    #[cfg(feature = "std")]
    #[inline(never)]
    fn resolve_host_backed_epub_path(path: &str) -> Option<String> {
        let mut candidates: Vec<String> = Vec::new();
        candidates.push(path.to_string());

        if path.starts_with('/') {
            candidates.push(format!("/sd{}", path));
        } else {
            candidates.push(format!("/sd/{}", path));
        }

        for candidate in candidates {
            if File::open(&candidate).is_ok() {
                return Some(candidate);
            }
        }
        None
    }

    fn handle_reader_input(&mut self, event: InputEvent) -> ActivityResult {
        if matches!(event, InputEvent::Press(Button::Back)) {
            #[cfg(feature = "std")]
            {
                self.epub_open_pending = None;
            }
            self.mode = BrowserMode::Browsing;
            if self.return_to_previous_on_back {
                self.return_to_previous_on_back = false;
                return ActivityResult::NavigateBack;
            }
            return ActivityResult::Consumed;
        }

        match &mut self.mode {
            BrowserMode::ReadingText { viewer, .. } => {
                if viewer.handle_input(event) {
                    ActivityResult::Consumed
                } else {
                    ActivityResult::Ignored
                }
            }
            #[cfg(feature = "std")]
            BrowserMode::ReadingEpub { renderer } => match event {
                InputEvent::Press(Button::Right)
                | InputEvent::Press(Button::Down)
                | InputEvent::Press(Button::VolumeDown)
                | InputEvent::Press(Button::Confirm) => {
                    renderer.next_page();
                    ActivityResult::Consumed
                }
                InputEvent::Press(Button::Left)
                | InputEvent::Press(Button::Up)
                | InputEvent::Press(Button::VolumeUp) => {
                    renderer.prev_page();
                    ActivityResult::Consumed
                }
                _ => ActivityResult::Ignored,
            },
            #[cfg(feature = "std")]
            BrowserMode::OpeningEpub => ActivityResult::Consumed,
            BrowserMode::Browsing => ActivityResult::Ignored,
        }
    }
}

impl Activity for FileBrowserActivity {
    fn on_enter(&mut self) {
        self.mode = BrowserMode::Browsing;
        #[cfg(feature = "std")]
        {
            self.epub_open_pending = None;
        }
        if self.pending_task.is_none() {
            self.queue_load_current_directory();
        }
    }

    fn on_exit(&mut self) {
        self.mode = BrowserMode::Browsing;
        self.pending_task = None;
        self.return_to_previous_on_back = false;
        #[cfg(feature = "std")]
        {
            self.epub_open_pending = None;
        }
    }

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        if self.is_viewing_text() || self.is_viewing_epub() || self.is_opening_epub() {
            return self.handle_reader_input(event);
        }

        if matches!(event, InputEvent::Press(Button::Back)) && self.return_to_previous_on_back {
            self.return_to_previous_on_back = false;
            return ActivityResult::NavigateBack;
        }

        if matches!(event, InputEvent::Press(Button::Back))
            && self.browser.current_path() == Self::DEFAULT_ROOT
        {
            return ActivityResult::NavigateBack;
        }

        let (needs_redraw, action) = self.browser.handle_input(event);

        if let Some(path) = action {
            if path.is_empty() {
                self.queue_load_current_directory();
                return ActivityResult::Consumed;
            }

            if Self::is_text_file(&path) || Self::is_epub_file(&path) {
                self.queue_open_file(path);
                return ActivityResult::Consumed;
            }

            let filename = basename(&path);
            self.browser
                .set_status_message(format!("Unsupported file type: {}", filename));
            return ActivityResult::Consumed;
        }

        if needs_redraw {
            ActivityResult::Consumed
        } else {
            ActivityResult::Ignored
        }
    }

    fn render<D: DrawTarget<Color = BinaryColor>>(&self, display: &mut D) -> Result<(), D::Error> {
        match &self.mode {
            BrowserMode::Browsing => self.browser.render(display),
            #[cfg(feature = "std")]
            BrowserMode::OpeningEpub => self.browser.render(display),
            BrowserMode::ReadingText { title, viewer } => viewer.render(display, title),
            #[cfg(feature = "std")]
            BrowserMode::ReadingEpub { renderer } => renderer.render(display),
        }
    }

    fn refresh_mode(&self) -> crate::ui::ActivityRefreshMode {
        #[cfg(feature = "std")]
        if self.is_viewing_epub() {
            // Avoid diff-based updates for EPUB pages on-device: diff mode keeps
            // large scratch buffers alive and can starve chapter parsing of
            // contiguous heap during page turns.
            return crate::ui::ActivityRefreshMode::Partial;
        }

        crate::ui::ActivityRefreshMode::Fast
    }
}

impl Default for FileBrowserActivity {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::MockFileSystem;

    fn create_fs() -> MockFileSystem {
        let mut fs = MockFileSystem::empty();
        fs.add_directory("/");
        fs.add_directory("/docs");
        fs.add_file("/docs/readme.txt", b"hello");
        fs.add_file("/docs/image.jpg", b"binary");
        fs
    }

    #[test]
    fn back_at_root_returns_to_system_menu() {
        let mut activity = FileBrowserActivity::new();
        let mut fs = create_fs();
        activity.on_enter();
        assert!(activity.process_pending_task(&mut fs));

        assert_eq!(
            activity.handle_input(InputEvent::Press(Button::Back)),
            ActivityResult::NavigateBack
        );
    }

    #[test]
    fn back_in_text_viewer_returns_to_browser() {
        let mut activity = FileBrowserActivity::new();
        let mut fs = create_fs();
        activity.on_enter();

        assert!(activity.process_pending_task(&mut fs));

        // Enter /docs.
        assert_eq!(
            activity.handle_input(InputEvent::Press(Button::Confirm)),
            ActivityResult::Consumed
        );
        assert!(activity.process_pending_task(&mut fs));

        // Open /docs/readme.txt.
        assert_eq!(
            activity.handle_input(InputEvent::Press(Button::VolumeDown)),
            ActivityResult::Consumed
        );
        assert_eq!(
            activity.handle_input(InputEvent::Press(Button::Confirm)),
            ActivityResult::Consumed
        );
        assert!(activity.process_pending_task(&mut fs));
        assert!(activity.is_viewing_text());

        assert_eq!(
            activity.handle_input(InputEvent::Press(Button::Back)),
            ActivityResult::Consumed
        );
        assert!(!activity.is_viewing_text());
    }

    #[test]
    fn unsupported_file_shows_clean_message() {
        let mut activity = FileBrowserActivity::new();
        let mut fs = create_fs();
        activity.on_enter();

        assert!(activity.process_pending_task(&mut fs));

        activity.handle_input(InputEvent::Press(Button::Confirm)); // open docs directory
        assert!(activity.process_pending_task(&mut fs));

        // Move to image.jpg and attempt open.
        activity.handle_input(InputEvent::Press(Button::VolumeDown)); // readme.txt
        activity.handle_input(InputEvent::Press(Button::VolumeDown)); // image.jpg
        assert_eq!(
            activity.handle_input(InputEvent::Press(Button::Confirm)),
            ActivityResult::Consumed
        );

        assert!(!activity.is_viewing_text());
        assert!(!activity.process_pending_task(&mut fs));
    }

    #[test]
    fn back_returns_to_previous_screen_after_library_open_fallback() {
        let mut activity = FileBrowserActivity::new();
        let mut fs = create_fs();
        activity.on_enter();
        assert!(activity.process_pending_task(&mut fs));

        // Simulate library-initiated open of unsupported file type.
        activity.request_open_path("/docs/image.jpg");
        assert!(activity.process_pending_task(&mut fs));
        assert!(!activity.is_viewing_text());

        assert_eq!(
            activity.handle_input(InputEvent::Press(Button::Back)),
            ActivityResult::NavigateBack
        );
    }

    #[test]
    fn back_inside_directory_goes_up_one_level() {
        let mut activity = FileBrowserActivity::new();
        let mut fs = create_fs();
        activity.on_enter();

        assert!(activity.process_pending_task(&mut fs));
        assert_eq!(activity.current_path(), "/");

        activity.handle_input(InputEvent::Press(Button::Confirm)); // /docs
        assert!(activity.process_pending_task(&mut fs));
        assert_eq!(activity.current_path(), "/docs");

        assert_eq!(
            activity.handle_input(InputEvent::Press(Button::Back)),
            ActivityResult::Consumed
        );
        assert!(activity.process_pending_task(&mut fs));
        assert_eq!(activity.current_path(), "/");
    }
}
