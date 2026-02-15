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
#[cfg(feature = "std")]
use mu_epub::{EpubBook, ScratchBuffers, ZipLimits};
#[cfg(feature = "std")]
use mu_epub_embedded_graphics::{EgRenderConfig, EgRenderer};
#[cfg(all(feature = "std", not(target_os = "espidf")))]
use mu_epub_render::{PaginationProfileId, RenderCacheStore};
#[cfg(feature = "std")]
use mu_epub_render::{RenderConfig, RenderEngine, RenderEngineOptions, RenderPage};
#[cfg(all(feature = "std", not(target_os = "espidf")))]
use std::io::Cursor;
#[cfg(all(feature = "std", not(target_os = "espidf")))]
use std::io::{Read, Seek};

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

#[cfg(all(feature = "std", not(target_os = "espidf")))]
trait ReadSeek: Read + Seek + Send {}

#[cfg(all(feature = "std", not(target_os = "espidf")))]
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
    #[cfg(not(target_os = "espidf"))]
    Bytes(Vec<u8>),
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
#[cfg(all(feature = "std", target_os = "espidf"))]
type EpubReader = std::fs::File;

#[cfg(all(feature = "std", not(target_os = "espidf")))]
type EpubReader = Box<dyn ReadSeek>;

#[cfg(feature = "std")]
struct EpubReadingState {
    book: EpubBook<EpubReader>,
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
    #[cfg(all(feature = "std", target_os = "espidf"))]
    const EPUB_OPEN_WORKER_STACK_BYTES: usize = 96 * 1024;
    #[cfg(all(feature = "std", target_os = "espidf"))]
    const EPUB_NAV_WORKER_STACK_BYTES: usize = 64 * 1024;
    #[cfg(all(feature = "std", not(target_os = "espidf")))]
    const EPUB_OPEN_WORKER_STACK_BYTES: usize = 2 * 1024 * 1024;
    #[cfg(all(feature = "std", not(target_os = "espidf")))]
    const EPUB_NAV_WORKER_STACK_BYTES: usize = 512 * 1024;

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

    pub(crate) fn is_opening_epub(&self) -> bool {
        #[cfg(feature = "std")]
        {
            matches!(self.mode, BrowserMode::OpeningEpub)
        }

        #[cfg(not(feature = "std"))]
        {
            false
        }
    }

    pub(crate) fn status_message(&self) -> Option<&str> {
        self.browser.status_message()
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

    #[cfg(not(feature = "std"))]
    #[inline(never)]
    fn process_open_epub_file_task(&mut self, _fs: &mut dyn FileSystem, path: &str) -> bool {
        self.mode = BrowserMode::Browsing;
        self.browser
            .set_status_message(format!("Unsupported file type: {}", basename(path)));
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
                            ActivityResult::Consumed
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
    use std::thread;
    use std::time::Duration;
    use std::time::Instant;

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

    #[test]
    fn epub_open_worker_completes_for_sample_epub() {
        let mut activity = FileBrowserActivity::new();
        let mut fs = MockFileSystem::empty();
        fs.add_directory("/");
        fs.add_directory("/books");
        fs.add_file(
            "/books/sample.epub",
            include_bytes!("../../../sample_books/sample.epub"),
        );

        activity.on_enter();
        assert!(activity.process_pending_task(&mut fs));

        activity.request_open_path("/books/sample.epub");
        assert!(
            activity.process_pending_task(&mut fs),
            "epub open task should start"
        );
        assert!(activity.is_opening_epub());

        let start = Instant::now();
        while activity.is_opening_epub() && start.elapsed() < Duration::from_secs(20) {
            let _ = activity.process_pending_task(&mut fs);
            thread::sleep(Duration::from_millis(1));
        }

        assert!(
            activity.is_viewing_epub(),
            "epub open did not complete: opening={} status={:?}",
            activity.is_opening_epub(),
            activity.status_message()
        );
    }

    #[test]
    fn sample_epub_parses_with_reasonable_spine_count() {
        let bytes = include_bytes!("../../../sample_books/sample.epub").to_vec();
        let reader = Cursor::new(bytes);
        let zip_limits = ZipLimits::new(8 * 1024 * 1024, 1024).with_max_eocd_scan(8 * 1024);
        let open_cfg = OpenConfig {
            options: mu_epub::book::EpubBookOptions {
                zip_limits: Some(zip_limits),
                validation_mode: mu_epub::book::ValidationMode::Lenient,
                max_nav_bytes: Some(256 * 1024),
            },
            lazy_navigation: true,
        };
        let book =
            EpubBook::from_reader_with_config(reader, open_cfg).expect("sample epub should parse");
        assert!(
            book.chapter_count() > 0 && book.chapter_count() < 4096,
            "unexpected chapter count: {}",
            book.chapter_count()
        );
    }

    #[test]
    fn epub_reading_state_from_reader_completes_for_sample_epub() {
        let bytes = include_bytes!("../../../sample_books/sample.epub").to_vec();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let result = EpubReadingState::from_reader(Box::new(Cursor::new(bytes)));
            let _ = tx.send(result.map(|_| ()));
        });

        let result = rx
            .recv_timeout(Duration::from_secs(20))
            .expect("epub reading-state build timed out");
        assert!(
            result.is_ok(),
            "epub reading-state build failed: {:?}",
            result
        );
    }
}
