//! Main application state with simplified 3-tab navigation.
//!
//! Single MainActivity with Library, Files, and Settings tabs.
//! Left/Right cycles tabs, no navigation stack needed.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

use crate::input::InputEvent;
use crate::library_activity::BookInfo;
use crate::main_activity::MainActivity;
use crate::system_menu_activity::DeviceStatus;
use crate::ui::{Activity, ActivityRefreshMode, ActivityResult};
use crate::FileSystem;

/// Pending library scan state
struct PendingLibraryScan {
    paths: Vec<String>,
    next_index: usize,
    books: Vec<BookInfo>,
    scan_fingerprint: u64,
}

/// AppScreen variants for compatibility during migration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppScreen {
    Main,
    // Legacy variants - these will be removed when old activities are deleted
    SystemMenu,
    Library,
    FileBrowser,
    Settings,
    ReaderSettings,
    Information,
}

/// Application state with simplified navigation.
/// Only one screen: Main with 3 tabs.
pub struct App {
    main_activity: MainActivity,
    refresh_counter: u32,
    refresh_frequency_pages: u32,
    needs_full_refresh_on_next_draw: bool,
    library_cache: Option<Vec<BookInfo>>,
    library_root: String,
    library_scan_pending: bool,
    library_cache_invalidated: bool,
    library_force_rescan: bool,
    pending_library_scan: Option<PendingLibraryScan>,
    device_status: DeviceStatus,
}

impl App {
    /// Default number of pages between full refreshes
    const DEFAULT_REFRESH_FREQUENCY: u32 = 10;
    const MAX_LIBRARY_BOOKS_PER_TICK: usize = 2;
    const MAX_FILE_BROWSER_TASKS_PER_TICK: usize = 2;

    /// Create a new App with MainActivity.
    pub fn new() -> Self {
        let mut app = Self {
            main_activity: MainActivity::new(),
            refresh_counter: 0,
            refresh_frequency_pages: Self::DEFAULT_REFRESH_FREQUENCY,
            needs_full_refresh_on_next_draw: true,
            library_cache: None,
            library_root: String::from("/books"),
            library_scan_pending: true, // Start with scan pending
            library_cache_invalidated: true,
            library_force_rescan: false,
            pending_library_scan: None,
            device_status: DeviceStatus::default(),
        };
        // Initialize library with loading state
        app.main_activity.library_tab.begin_loading_scan();
        app.main_activity.on_enter();
        app
    }

    /// Set device status (battery, etc.)
    pub fn set_device_status(&mut self, status: DeviceStatus) {
        self.device_status = status;
        self.main_activity.set_device_status(status);
    }

    /// Handle input event. Returns true if a redraw is needed.
    pub fn handle_input(&mut self, event: InputEvent) -> bool {
        let result = self.main_activity.handle_input(event);
        let mut redraw = self.process_result(result);

        // Check if library wants to open a book
        if let Some(path) = self.main_activity.library_tab.take_open_request() {
            // Forward the open request to file browser
            self.main_activity.files_tab.request_open_path(path);
            // Switch to files tab to show the book
            self.main_activity.set_tab(crate::main_activity::Tab::Files);
            redraw = true;
        }

        redraw
    }

    /// Render the main activity to the display.
    pub fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        self.main_activity.render(display)
    }

    /// Get the refresh mode for the current activity.
    pub fn get_refresh_mode(&mut self) -> ActivityRefreshMode {
        if self.needs_full_refresh_on_next_draw {
            self.needs_full_refresh_on_next_draw = false;
            self.refresh_counter = 0;
            return ActivityRefreshMode::Full;
        }

        let activity_mode = self.main_activity.refresh_mode();

        if activity_mode == ActivityRefreshMode::Full {
            self.refresh_counter = 0;
            return ActivityRefreshMode::Full;
        }

        self.refresh_counter += 1;

        if self.refresh_counter >= self.refresh_frequency_pages {
            self.refresh_counter = 0;
            ActivityRefreshMode::Partial
        } else {
            activity_mode
        }
    }

    /// Process an ActivityResult.
    fn process_result(&mut self, result: ActivityResult) -> bool {
        match result {
            ActivityResult::Consumed => true,
            ActivityResult::NavigateBack => false,
            ActivityResult::NavigateTo(_) => false,
            ActivityResult::Ignored => false,
        }
    }

    /// Set the root directory used for library scanning.
    pub fn set_library_root(&mut self, root: impl Into<String>) {
        self.library_root = root.into();
        self.invalidate_library_cache();
    }

    /// Invalidate library cache and schedule a scan.
    pub fn invalidate_library_cache(&mut self) {
        self.library_cache_invalidated = true;
        self.library_scan_pending = true;
        self.library_force_rescan = false;
        self.pending_library_scan = None;
        self.main_activity.library_tab.begin_loading_scan();
    }

    /// Force a full metadata rescan and bypass persisted cache for one cycle.
    pub fn force_rescan_library(&mut self) {
        self.library_cache_invalidated = true;
        self.library_scan_pending = true;
        self.library_force_rescan = true;
        self.pending_library_scan = None;
        self.main_activity.library_tab.begin_loading_scan();
    }

    /// Run deferred library scan work.
    pub fn process_library_scan(&mut self, fs: &mut dyn FileSystem) -> bool {
        if !self.library_scan_pending && self.pending_library_scan.is_none() {
            return false;
        }

        if self.pending_library_scan.is_none() {
            let mut paths = fs.scan_directory(&self.library_root).unwrap_or_default();
            paths.sort_unstable();
            let scan_fingerprint = Self::compute_scan_fingerprint(fs, &paths);

            if !self.library_force_rescan {
                if let Some(cached) =
                    Self::load_library_cache_for_fingerprint(&self.library_root, scan_fingerprint)
                {
                    self.library_cache = Some(cached.clone());
                    self.library_cache_invalidated = false;
                    self.library_scan_pending = false;
                    self.pending_library_scan = None;
                    self.main_activity.library_tab.set_books(cached);
                    self.main_activity.library_tab.finish_loading_scan();
                    return true;
                }
            }

            let estimated = paths.len();
            self.pending_library_scan = Some(PendingLibraryScan {
                paths,
                next_index: 0,
                books: Vec::with_capacity(estimated),
                scan_fingerprint,
            });
        }

        let mut completed_books: Option<Vec<BookInfo>> = None;
        let mut completed_fingerprint: Option<u64> = None;
        if let Some(scan) = self.pending_library_scan.as_mut() {
            let mut processed = 0usize;
            while processed < Self::MAX_LIBRARY_BOOKS_PER_TICK && scan.next_index < scan.paths.len()
            {
                let path = scan.paths[scan.next_index].clone();
                scan.next_index += 1;
                scan.books.push(
                    crate::library_activity::LibraryActivity::extract_book_info_for_path(fs, &path),
                );
                processed += 1;
            }

            if scan.next_index >= scan.paths.len() {
                let mut books = core::mem::take(&mut scan.books);
                books.sort_by(|a, b| a.title.cmp(&b.title));
                completed_books = Some(books);
                completed_fingerprint = Some(scan.scan_fingerprint);
            }
        }

        let Some(books) = completed_books else {
            return false;
        };

        self.pending_library_scan = None;
        self.library_cache = Some(books.clone());
        self.library_cache_invalidated = false;
        self.library_scan_pending = false;
        self.library_force_rescan = false;
        if let Some(fingerprint) = completed_fingerprint {
            Self::persist_library_cache(&self.library_root, fingerprint, &books);
        }

        self.main_activity.library_tab.set_books(books);
        self.main_activity.library_tab.finish_loading_scan();
        true
    }

    /// Run deferred file browser work.
    pub fn process_file_browser_tasks(&mut self, fs: &mut dyn FileSystem) -> bool {
        // Always process file browser tasks when on Files tab
        if self.main_activity.current_tab() != crate::main_activity::Tab::Files {
            return false;
        }

        let mut updated = false;
        for _ in 0..Self::MAX_FILE_BROWSER_TASKS_PER_TICK {
            if !self.main_activity.files_tab.process_pending_task(fs) {
                break;
            }
            updated = true;
        }
        updated
    }

    /// Run all deferred app tasks.
    pub fn process_deferred_tasks(&mut self, fs: &mut dyn FileSystem) -> bool {
        if self.main_activity.library_tab.take_refresh_request() {
            self.force_rescan_library();
        }
        let library_updated = self.process_library_scan(fs);
        let file_browser_updated = self.process_file_browser_tasks(fs);
        library_updated || file_browser_updated
    }

    fn compute_scan_fingerprint(fs: &mut dyn FileSystem, paths: &[String]) -> u64 {
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;

        fn hash_bytes(mut state: u64, bytes: &[u8]) -> u64 {
            for b in bytes {
                state ^= *b as u64;
                state = state.wrapping_mul(FNV_PRIME);
            }
            state
        }

        let mut state = hash_bytes(FNV_OFFSET, paths.len().to_string().as_bytes());
        for path in paths {
            state = hash_bytes(state, path.as_bytes());
            let size = fs.file_info(path).map(|info| info.size).unwrap_or(0);
            state = hash_bytes(state, &size.to_le_bytes());
        }
        state
    }

    #[cfg(feature = "std")]
    fn cache_file_path() -> &'static str {
        if cfg!(target_os = "espidf") {
            "/sd/.xteink/library_cache.tsv"
        } else {
            "target/.xteink-library-cache.tsv"
        }
    }

    #[cfg(feature = "std")]
    fn escape_cache_field(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for ch in input.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '\t' => out.push_str("\\t"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                _ => out.push(ch),
            }
        }
        out
    }

    #[cfg(feature = "std")]
    fn unescape_cache_field(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        let mut chars = input.chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                match chars.next() {
                    Some('t') => out.push('\t'),
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('\\') => out.push('\\'),
                    Some(other) => {
                        out.push('\\');
                        out.push(other);
                    }
                    None => out.push('\\'),
                }
            } else {
                out.push(ch);
            }
        }
        out
    }

    #[cfg(feature = "std")]
    fn load_library_cache_for_fingerprint(root: &str, fingerprint: u64) -> Option<Vec<BookInfo>> {
        let raw = std::fs::read_to_string(Self::cache_file_path()).ok()?;
        let mut lines = raw.lines();
        let header = lines.next()?;
        let mut header_parts = header.split('\t');
        let version = header_parts.next()?;
        let root_raw = header_parts.next()?;
        let fp_raw = header_parts.next()?;
        if version != "v1" {
            return None;
        }
        let cached_root = Self::unescape_cache_field(root_raw);
        if cached_root != root {
            return None;
        }
        let cached_fp = u64::from_str_radix(fp_raw, 16).ok()?;
        if cached_fp != fingerprint {
            return None;
        }

        let mut books = Vec::new();
        for line in lines {
            let mut parts = line.split('\t');
            if parts.next()? != "b" {
                continue;
            }
            let title = Self::unescape_cache_field(parts.next()?);
            let author = Self::unescape_cache_field(parts.next()?);
            let path = Self::unescape_cache_field(parts.next()?);
            let progress = parts.next().and_then(|v| v.parse::<u8>().ok()).unwrap_or(0);
            let last_read = parts.next().and_then(|v| {
                if v.is_empty() {
                    None
                } else {
                    v.parse::<u64>().ok()
                }
            });
            books.push(BookInfo::new(title, author, path, progress, last_read));
        }
        Some(books)
    }

    #[cfg(not(feature = "std"))]
    fn load_library_cache_for_fingerprint(_root: &str, _fingerprint: u64) -> Option<Vec<BookInfo>> {
        None
    }

    #[cfg(feature = "std")]
    fn persist_library_cache(root: &str, fingerprint: u64, books: &[BookInfo]) {
        let path = Self::cache_file_path();
        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let mut out = String::new();
        out.push_str("v1\t");
        out.push_str(&Self::escape_cache_field(root));
        out.push('\t');
        out.push_str(&format!("{fingerprint:016x}"));
        out.push('\n');

        for book in books {
            out.push_str("b\t");
            out.push_str(&Self::escape_cache_field(&book.title));
            out.push('\t');
            out.push_str(&Self::escape_cache_field(&book.author));
            out.push('\t');
            out.push_str(&Self::escape_cache_field(&book.path));
            out.push('\t');
            out.push_str(&book.progress_percent.to_string());
            out.push('\t');
            if let Some(last_read) = book.last_read {
                out.push_str(&last_read.to_string());
            }
            out.push('\n');
        }

        let _ = std::fs::write(path, out);
    }

    #[cfg(not(feature = "std"))]
    fn persist_library_cache(_root: &str, _fingerprint: u64, _books: &[BookInfo]) {}

    /// Get current tab for testing/monitoring.
    pub fn current_tab(&self) -> crate::main_activity::Tab {
        self.main_activity.current_tab()
    }

    /// Check if file browser is currently opening an EPUB.
    pub fn file_browser_is_opening_epub(&self) -> bool {
        self.main_activity.files_tab.is_opening_epub()
    }

    /// Check if file browser is currently reading text.
    pub fn file_browser_is_reading_text(&self) -> bool {
        self.main_activity.files_tab.is_reading_text()
    }

    /// Check if file browser is currently reading an EPUB.
    pub fn file_browser_is_reading_epub(&self) -> bool {
        self.main_activity.files_tab.is_reading_epub()
    }

    /// Get current EPUB reading position.
    pub fn file_browser_epub_position(&self) -> Option<(usize, usize, usize, usize)> {
        self.main_activity.files_tab.epub_position()
    }

    /// Get auto-sleep duration in milliseconds.
    pub fn auto_sleep_duration_ms(&self) -> u32 {
        // Default: 5 minutes
        300_000
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Button;

    #[test]
    fn app_creates_main_activity() {
        let app = App::new();
        assert_eq!(app.current_tab() as usize, 0);
    }

    #[test]
    fn app_left_right_cycles_tabs() {
        let mut app = App::new();
        assert_eq!(app.current_tab() as usize, 0);
        app.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(app.current_tab() as usize, 1);
        app.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(app.current_tab() as usize, 2);
        app.handle_input(InputEvent::Press(Button::Right));
        assert_eq!(app.current_tab() as usize, 0);
        app.handle_input(InputEvent::Press(Button::Left));
        assert_eq!(app.current_tab() as usize, 2);
    }

    #[test]
    fn app_render_does_not_panic() {
        let app = App::new();
        let mut display = crate::test_display::TestDisplay::default_size();
        let result = app.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn app_forces_full_refresh_on_first_draw() {
        let mut app = App::new();
        assert_eq!(app.get_refresh_mode(), ActivityRefreshMode::Full);
        assert_eq!(app.get_refresh_mode(), ActivityRefreshMode::Fast);
    }
}
