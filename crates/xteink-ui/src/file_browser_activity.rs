//! Activity wrapper for the raw filesystem browser.
//!
//! Keeps filesystem I/O out of the input/render path by scheduling
//! deferred tasks that are processed by the app loop.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
#[cfg(feature = "std")]
use alloc::vec::Vec;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
#[cfg(feature = "std")]
use epublet::{EmbeddedFontStyle, EpubBook};
#[cfg(feature = "std")]
use epublet_embedded_graphics::{EgRenderConfig, EgRenderer, FontFaceRegistration};
#[cfg(feature = "std")]
use epublet_render::{RenderEngine, RenderEngineOptions, RenderPage};
#[cfg(feature = "std")]
use std::io::Cursor;

#[cfg(all(feature = "std", feature = "fontdue"))]
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
struct EpubReadingState {
    book: EpubBook<Cursor<Vec<u8>>>,
    engine: RenderEngine,
    eg_renderer: ReaderRenderer,
    pages: Vec<RenderPage>,
    chapter_idx: usize,
    page_idx: usize,
}

#[cfg(all(feature = "std", feature = "fontdue"))]
type ReaderRenderer = EgRenderer<BookerlyFontBackend>;

#[cfg(all(feature = "std", not(feature = "fontdue")))]
type ReaderRenderer = EgRenderer<epublet_embedded_graphics::MonoFontBackend>;

#[cfg(feature = "std")]
impl EpubReadingState {
    fn from_bytes(data: Vec<u8>) -> Result<Self, String> {
        let book = EpubBook::from_reader(Cursor::new(data))
            .map_err(|e| format!("Unable to parse EPUB: {}", e))?;
        let mut state = Self {
            book,
            engine: RenderEngine::new(RenderEngineOptions::for_display(
                crate::DISPLAY_WIDTH as i32,
                crate::DISPLAY_HEIGHT as i32,
            )),
            eg_renderer: Self::create_renderer(),
            pages: Vec::new(),
            chapter_idx: 0,
            page_idx: 0,
        };
        state.register_embedded_fonts();
        state.load_chapter(0)?;
        Ok(state)
    }

    fn load_chapter(&mut self, chapter_idx: usize) -> Result<(), String> {
        self.pages = self
            .engine
            .prepare_chapter_iter(&mut self.book, chapter_idx)
            .map_err(|e| format!("Unable to prepare EPUB chapter: {}", e))?
            .collect();
        if self.pages.is_empty() {
            self.pages.push(RenderPage::new(1));
        }
        self.chapter_idx = chapter_idx;
        self.page_idx = 0;
        Ok(())
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
        self.pages.len()
    }

    fn next_page(&mut self) -> bool {
        if self.page_idx + 1 < self.pages.len() {
            self.page_idx += 1;
            return true;
        }
        if self.chapter_idx + 1 < self.book.chapter_count()
            && self.load_chapter(self.chapter_idx + 1).is_ok()
        {
            return true;
        }
        false
    }

    fn prev_page(&mut self) -> bool {
        if self.page_idx > 0 {
            self.page_idx -= 1;
            return true;
        }
        if self.chapter_idx > 0 {
            let prev_chapter = self.chapter_idx - 1;
            if self.load_chapter(prev_chapter).is_ok() {
                self.page_idx = self.pages.len().saturating_sub(1);
                return true;
            }
        }
        false
    }

    fn render<D: DrawTarget<Color = BinaryColor>>(&self, display: &mut D) -> Result<(), D::Error> {
        if let Some(page) = self.pages.get(self.page_idx) {
            self.eg_renderer.render_page(page, display)
        } else {
            display.clear(BinaryColor::Off)
        }
    }

    fn create_renderer() -> ReaderRenderer {
        let cfg = EgRenderConfig::default();
        #[cfg(all(feature = "std", feature = "fontdue"))]
        {
            EgRenderer::with_backend(cfg, BookerlyFontBackend::default())
        }
        #[cfg(not(all(feature = "std", feature = "fontdue")))]
        {
            EgRenderer::with_backend(cfg, epublet_embedded_graphics::MonoFontBackend)
        }
    }

    fn register_embedded_fonts(&mut self) {
        let Ok(embedded) = self.book.embedded_fonts() else {
            return;
        };

        struct LoadedFace {
            family: String,
            weight: u16,
            italic: bool,
            data: Vec<u8>,
        }

        let mut loaded = Vec::<LoadedFace>::new();
        for face in embedded {
            let italic = matches!(
                face.style,
                EmbeddedFontStyle::Italic | EmbeddedFontStyle::Oblique
            );
            let Ok(bytes) = self.book.read_resource(&face.href) else {
                continue;
            };
            loaded.push(LoadedFace {
                family: face.family,
                weight: face.weight,
                italic,
                data: bytes,
            });
        }

        let faces: Vec<FontFaceRegistration<'_>> = loaded
            .iter()
            .map(|face| FontFaceRegistration {
                family: &face.family,
                weight: face.weight,
                italic: face.italic,
                data: &face.data,
            })
            .collect();
        let _ = self.eg_renderer.register_faces(&faces);
    }
}

/// Raw filesystem browser activity.
pub struct FileBrowserActivity {
    browser: FileBrowser,
    mode: BrowserMode,
    pending_task: Option<FileBrowserTask>,
    return_to_previous_on_back: bool,
}

impl FileBrowserActivity {
    pub const DEFAULT_ROOT: &'static str = "/";

    pub fn new() -> Self {
        Self {
            browser: FileBrowser::new(Self::DEFAULT_ROOT),
            mode: BrowserMode::Browsing,
            pending_task: None,
            return_to_previous_on_back: false,
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
        let Some(task) = self.pending_task.take() else {
            return false;
        };

        match task {
            FileBrowserTask::LoadCurrentDirectory => self.process_load_current_directory_task(fs),
            FileBrowserTask::OpenPath { path } => self.process_open_path_task(fs, &path),
            FileBrowserTask::OpenTextFile { path } => self.process_open_text_file_task(fs, &path),
            FileBrowserTask::OpenEpubFile { path } => self.process_open_epub_file_task(fs, &path),
        }
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
    fn process_open_epub_file_task(&mut self, fs: &mut dyn FileSystem, path: &str) -> bool {
        #[cfg(feature = "std")]
        {
            match Self::load_epub_renderer(fs, path) {
                Ok(renderer) => {
                    self.mode = BrowserMode::ReadingEpub {
                        renderer: Box::new(renderer),
                    };
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
        path.to_lowercase().ends_with(".epub")
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
    #[inline(never)]
    fn load_epub_renderer(fs: &mut dyn FileSystem, path: &str) -> Result<EpubReadingState, String> {
        let data = fs
            .read_file_bytes(path)
            .map_err(|e| format!("Unable to read EPUB: {}", e))?;
        EpubReadingState::from_bytes(data)
    }

    fn handle_reader_input(&mut self, event: InputEvent) -> ActivityResult {
        if matches!(event, InputEvent::Press(Button::Back)) {
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
            BrowserMode::Browsing => ActivityResult::Ignored,
        }
    }
}

impl Activity for FileBrowserActivity {
    fn on_enter(&mut self) {
        self.mode = BrowserMode::Browsing;
        if self.pending_task.is_none() {
            self.queue_load_current_directory();
        }
    }

    fn on_exit(&mut self) {
        self.mode = BrowserMode::Browsing;
        self.pending_task = None;
        self.return_to_previous_on_back = false;
    }

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        if self.is_viewing_text() || self.is_viewing_epub() {
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
            BrowserMode::ReadingText { title, viewer } => viewer.render(display, title),
            #[cfg(feature = "std")]
            BrowserMode::ReadingEpub { renderer } => renderer.render(display),
        }
    }

    fn refresh_mode(&self) -> crate::ui::ActivityRefreshMode {
        // Diff-based fast updates can leave artifacts in list UIs on e-ink.
        // Use partial full-screen updates for stable visuals.
        crate::ui::ActivityRefreshMode::Partial
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
