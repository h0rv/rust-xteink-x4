//! Activity wrapper for the raw filesystem browser.
//!
//! Keeps filesystem I/O out of the input/render path by scheduling
//! deferred tasks that are processed by the app loop.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

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
    ReadingText { title: String, viewer: TextViewer },
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
            match Self::load_epub_as_text(fs, path) {
                Ok((title, content)) => {
                    self.mode = BrowserMode::ReadingText {
                        title,
                        viewer: TextViewer::new(content),
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
    fn load_epub_as_text(fs: &mut dyn FileSystem, path: &str) -> Result<(String, String), String> {
        let fallback_title = basename(path).to_string();
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Prefer bytes via FileSystem so scenario tests can use MockFileSystem.
            if let Ok(data) = fs.read_file_bytes(path) {
                return Self::parse_epub_from_bytes(data, fallback_title);
            }
            Self::parse_epub_from_path(path, fallback_title)
        }

        #[cfg(target_arch = "wasm32")]
        {
            let data = fs
                .read_file_bytes(path)
                .map_err(|e| format!("Unable to read EPUB: {}", e))?;
            Self::parse_epub_from_bytes(data, fallback_title)
        }
    }

    #[cfg(feature = "std")]
    #[cfg(not(target_arch = "wasm32"))]
    #[inline(never)]
    fn parse_epub_from_path(
        path: &str,
        fallback_title: String,
    ) -> Result<(String, String), String> {
        use std::path::Path;

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

        let mut book = epublet::EpubBook::open(&resolved_path)
            .map_err(|e| format!("Unable to parse EPUB: {}", e))?;
        if book.chapter_count() == 0 {
            return Err("EPUB has no spine items".to_string());
        }

        let tokens = book
            .tokenize_spine_item(0)
            .map_err(|e| format!("Unable to load first chapter: {}", e))?;
        let content = Self::tokens_to_text(&tokens);
        let title = if book.metadata().title.trim().is_empty() {
            fallback_title
        } else {
            book.metadata().title.clone()
        };

        Ok((title, content))
    }

    #[cfg(feature = "std")]
    #[inline(never)]
    fn parse_epub_from_bytes(
        data: alloc::vec::Vec<u8>,
        fallback_title: String,
    ) -> Result<(String, String), String> {
        use std::io::Cursor;

        let mut book = epublet::EpubBook::from_reader(Cursor::new(data))
            .map_err(|e| format!("Unable to parse EPUB: {}", e))?;
        if book.chapter_count() == 0 {
            return Err("EPUB has no spine items".to_string());
        }

        let tokens = book
            .tokenize_spine_item(0)
            .map_err(|e| format!("Unable to load first chapter: {}", e))?;
        let content = Self::tokens_to_text(&tokens);
        let title = if book.metadata().title.trim().is_empty() {
            fallback_title
        } else {
            book.metadata().title.clone()
        };

        Ok((title, content))
    }

    #[cfg(feature = "std")]
    #[inline(never)]
    fn tokens_to_text(tokens: &[epublet::tokenizer::Token]) -> String {
        use epublet::tokenizer::Token;

        let mut out = String::new();
        for token in tokens {
            match token {
                Token::Text(text) => {
                    if text.is_empty() {
                        continue;
                    }
                    if !out.is_empty() && !out.ends_with('\n') && !out.ends_with(' ') {
                        out.push(' ');
                    }
                    out.push_str(text);
                }
                Token::ParagraphBreak
                | Token::Heading(_)
                | Token::LineBreak
                | Token::ListItemStart
                | Token::ListEnd => {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
                _ => {}
            }
        }

        if out.trim().is_empty() {
            "No readable text extracted from first chapter.".to_string()
        } else {
            out
        }
    }

    fn handle_text_input(&mut self, event: InputEvent) -> ActivityResult {
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
        if self.is_viewing_text() {
            return self.handle_text_input(event);
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
