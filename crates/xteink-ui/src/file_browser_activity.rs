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
#[cfg(all(feature = "std", not(target_os = "espidf")))]
use std::collections::HashMap;
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::sync::mpsc::{self, Receiver, TryRecvError};
#[cfg(feature = "std")]
use std::sync::{Arc, Mutex};
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
#[cfg(all(feature = "std", not(target_os = "espidf")))]
use mu_epub_render::{PaginationProfileId, RenderCacheStore};
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

#[cfg(feature = "std")]
mod epub;

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
        renderer: Arc<Mutex<EpubReadingState>>,
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
struct PendingEpubNavigation {
    receiver: Receiver<Result<bool, String>>,
    direction: EpubNavigationDirection,
}

#[cfg(feature = "std")]
enum EpubOpenSource {
    HostPath(String),
    Chunks(Vec<Vec<u8>>),
}

#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug)]
enum EpubNavigationDirection {
    Next,
    Prev,
}

#[cfg(feature = "std")]
impl EpubNavigationDirection {
    fn label(self) -> &'static str {
        match self {
            Self::Next => "next",
            Self::Prev => "previous",
        }
    }
}

#[cfg(all(feature = "std", not(target_os = "espidf")))]
#[derive(Debug, Default)]
struct InMemoryRenderCache {
    inner: Mutex<InMemoryRenderCacheState>,
}

#[cfg(all(feature = "std", not(target_os = "espidf")))]
#[derive(Debug, Default)]
struct InMemoryRenderCacheState {
    entries: HashMap<(PaginationProfileId, usize), Vec<RenderPage>>,
    order: Vec<(PaginationProfileId, usize)>,
}

#[cfg(all(feature = "std", not(target_os = "espidf")))]
impl InMemoryRenderCache {
    const CHAPTER_LIMIT: usize = 4;

    fn touch(order: &mut Vec<(PaginationProfileId, usize)>, key: (PaginationProfileId, usize)) {
        order.retain(|entry| *entry != key);
        order.push(key);
    }
}

#[cfg(all(feature = "std", not(target_os = "espidf")))]
impl RenderCacheStore for InMemoryRenderCache {
    fn load_chapter_pages(
        &self,
        profile: PaginationProfileId,
        chapter_index: usize,
    ) -> Option<Vec<RenderPage>> {
        let mut inner = self.inner.lock().ok()?;
        let key = (profile, chapter_index);
        let pages = inner.entries.get(&key)?.clone();
        Self::touch(&mut inner.order, key);
        Some(pages)
    }

    fn store_chapter_pages(
        &self,
        profile: PaginationProfileId,
        chapter_index: usize,
        pages: &[RenderPage],
    ) {
        if pages.is_empty() {
            return;
        }
        let mut inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        let key = (profile, chapter_index);
        inner.entries.insert(key, pages.to_vec());
        Self::touch(&mut inner.order, key);
        while inner.order.len() > Self::CHAPTER_LIMIT {
            let oldest = inner.order.remove(0);
            inner.entries.remove(&oldest);
        }
    }
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
    #[cfg(not(target_os = "espidf"))]
    render_cache: InMemoryRenderCache,
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

/// Raw filesystem browser activity.
pub struct FileBrowserActivity {
    browser: FileBrowser,
    mode: BrowserMode,
    pending_task: Option<FileBrowserTask>,
    return_to_previous_on_back: bool,
    #[cfg(feature = "std")]
    epub_open_pending: Option<PendingEpubOpen>,
    #[cfg(feature = "std")]
    epub_navigation_pending: Option<PendingEpubNavigation>,
}

impl FileBrowserActivity {
    pub const DEFAULT_ROOT: &'static str = "/";
    #[cfg(feature = "std")]
    const EPUB_READ_CHUNK_BYTES: usize = 4096;
    #[cfg(all(feature = "std", target_os = "espidf"))]
    const EPUB_WORKER_STACK_BYTES: usize = 56 * 1024;
    #[cfg(all(feature = "std", not(target_os = "espidf")))]
    const EPUB_WORKER_STACK_BYTES: usize = 64 * 1024;

    pub fn new() -> Self {
        Self {
            browser: FileBrowser::new(Self::DEFAULT_ROOT),
            mode: BrowserMode::Browsing,
            pending_task: None,
            return_to_previous_on_back: false,
            #[cfg(feature = "std")]
            epub_open_pending: None,
            #[cfg(feature = "std")]
            epub_navigation_pending: None,
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
                let renderer = match renderer.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
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
        #[cfg(feature = "std")]
        {
            updated |= self.poll_epub_navigation_result();
        }
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

    fn handle_reader_input(&mut self, event: InputEvent) -> ActivityResult {
        if matches!(event, InputEvent::Press(Button::Back)) {
            #[cfg(feature = "std")]
            {
                self.epub_open_pending = None;
                self.epub_navigation_pending = None;
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
            BrowserMode::ReadingEpub { renderer } => {
                let nav = match event {
                    InputEvent::Press(Button::Right)
                    | InputEvent::Press(Button::Down)
                    | InputEvent::Press(Button::VolumeDown)
                    | InputEvent::Press(Button::Confirm) => Some(EpubNavigationDirection::Next),
                    InputEvent::Press(Button::Left)
                    | InputEvent::Press(Button::Up)
                    | InputEvent::Press(Button::VolumeUp) => Some(EpubNavigationDirection::Prev),
                    _ => None,
                };
                if let Some(direction) = nav {
                    if self.epub_navigation_pending.is_some() {
                        return ActivityResult::Consumed;
                    }
                    let renderer = Arc::clone(renderer);
                    match Self::spawn_epub_navigation_worker(renderer, direction) {
                        Ok(receiver) => {
                            self.epub_navigation_pending = Some(PendingEpubNavigation {
                                receiver,
                                direction,
                            });
                            ActivityResult::Ignored
                        }
                        Err(error) => {
                            self.browser.set_status_message(error);
                            ActivityResult::Consumed
                        }
                    }
                } else {
                    ActivityResult::Ignored
                }
            }
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
            self.epub_navigation_pending = None;
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
            self.epub_navigation_pending = None;
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
            BrowserMode::ReadingEpub { renderer } => {
                let renderer = match renderer.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                renderer.render(display)
            }
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

    fn drain_pending_tasks(activity: &mut FileBrowserActivity, fs: &mut MockFileSystem) -> bool {
        let mut saw_update = false;
        for _ in 0..4 {
            if !activity.process_pending_task(fs) {
                break;
            }
            saw_update = true;
        }
        saw_update
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
        activity.open_path(&mut fs, "/docs/readme.txt");
        assert!(drain_pending_tasks(&mut activity, &mut fs));
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
        activity.request_open_path("/docs/image.jpg");
        let _ = drain_pending_tasks(&mut activity, &mut fs);
        assert!(!activity.is_viewing_text());
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
