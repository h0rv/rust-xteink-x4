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
use crate::main_activity::{MainActivity, UnifiedSettings};
#[cfg(feature = "std")]
use crate::reader_settings_activity::{
    FooterAutoHide, FooterDensity, LineSpacing, MarginSize, RefreshFrequency, TapZoneConfig,
    TextAlignment, VolumeButtonAction,
};
#[cfg(feature = "std")]
use crate::settings_activity::{AutoSleepDuration, FontFamily, FontSize};
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
    applied_settings: UnifiedSettings,
    filtered_battery_percent: Option<u8>,
    pending_file_transfer_request: Option<bool>,
    pending_wifi_mode_ap: Option<bool>,
    pending_wifi_ap_config: Option<(String, String)>,
    pending_content_open_path: Option<String>,
}

impl App {
    /// Default number of pages between full refreshes
    const DEFAULT_REFRESH_FREQUENCY: u32 = 0;
    const MAX_LIBRARY_BOOKS_PER_TICK: usize = 2;
    const MAX_FILE_BROWSER_TASKS_PER_TICK: usize = 2;
    #[cfg(feature = "std")]
    const MAX_LIBRARY_CACHE_BOOKS: usize = 2048;
    #[cfg(feature = "std")]
    const MAX_COMPACT_COVER_CHARS: usize =
        ((crate::DISPLAY_WIDTH as usize) * (crate::DISPLAY_HEIGHT as usize)).div_ceil(8) * 2 + 16;

    /// Create a new App with MainActivity.
    pub fn new() -> Self {
        Self::new_with_epub_resume(true)
    }

    /// Create a new App with optional EPUB auto-resume from last session.
    pub fn new_with_epub_resume(auto_resume_epub: bool) -> Self {
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
            applied_settings: UnifiedSettings::default(),
            filtered_battery_percent: None,
            pending_file_transfer_request: None,
            pending_wifi_mode_ap: None,
            pending_wifi_ap_config: None,
            pending_content_open_path: None,
        };
        // Initialize library with loading state
        app.main_activity.library_tab.begin_loading_scan();
        if let Some(settings) = Self::load_persisted_settings() {
            app.main_activity.apply_settings(settings);
            app.refresh_frequency_pages = settings.refresh_frequency.pages() as u32;
            app.applied_settings = settings;
        } else {
            let settings = app.main_activity.settings();
            app.main_activity.apply_settings(settings);
            app.refresh_frequency_pages = settings.refresh_frequency.pages() as u32;
            app.applied_settings = settings;
        }
        if let Some(snapshot) = Self::load_latest_library_snapshot(&app.library_root) {
            app.library_cache = Some(snapshot.clone());
            app.main_activity.library_tab.set_books(snapshot);
            app.main_activity.library_tab.finish_loading_scan();
        }
        #[cfg(feature = "std")]
        {
            if auto_resume_epub {
                if let Some(path) =
                    crate::file_browser_activity::FileBrowserActivity::load_last_active_epub_path()
                {
                    app.main_activity.queue_open_content_path(path);
                    app.main_activity
                        .switch_to_tab(crate::main_activity::Tab::Files);
                }
            } else {
                crate::file_browser_activity::FileBrowserActivity::clear_last_active_epub_path();
            }
        }
        app.main_activity.on_enter();
        app
    }

    /// Set device status (battery, etc.)
    pub fn set_device_status(&mut self, status: DeviceStatus) {
        let raw = status.battery_percent.min(100);
        let filtered = match self.filtered_battery_percent {
            Some(prev) => {
                // Simple low-pass filter: 80% previous, 20% new.
                (((prev as u16) * 4 + (raw as u16)) / 5) as u8
            }
            None => raw,
        };
        self.filtered_battery_percent = Some(filtered);
        let mut filtered_status = status;
        filtered_status.battery_percent = filtered;
        self.device_status = filtered_status;
        self.main_activity.set_device_status(filtered_status);
    }

    /// Handle input event. Returns true if a redraw is needed.
    pub fn handle_input(&mut self, event: InputEvent) -> bool {
        let result = self.main_activity.handle_input(event);
        let mut redraw = self.process_result(result);
        redraw |= self.sync_runtime_settings();

        redraw |= self.pull_library_requests();

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

        if self.refresh_frequency_pages == 0 {
            return activity_mode;
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
        let on_files_tab = self.main_activity.current_tab() == crate::main_activity::Tab::Files;
        let needs_background_work = self.main_activity.files_tab.has_pending_task()
            || self.main_activity.files_tab.is_opening_epub();
        if !on_files_tab && !needs_background_work {
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
        let settings_updated = self.sync_runtime_settings();
        let library_request_updated = self.pull_library_requests();
        let library_updated = self.process_library_scan(fs);
        let content_open_updated = self.process_pending_content_open_request();
        let file_browser_updated = self.process_file_browser_tasks(fs);
        let mut switched_to_reader = false;
        if self.main_activity.current_tab() != crate::main_activity::Tab::Files
            && self.main_activity.files_tab.is_reading()
        {
            self.main_activity.set_tab(crate::main_activity::Tab::Files);
            switched_to_reader = true;
        }
        let library_progress_updated = self.sync_active_book_progress();
        settings_updated
            || library_request_updated
            || library_updated
            || content_open_updated
            || file_browser_updated
            || switched_to_reader
            || library_progress_updated
    }

    fn pull_library_requests(&mut self) -> bool {
        let mut updated = false;
        if self.main_activity.library_tab.take_refresh_request() {
            self.force_rescan_library();
            updated = true;
        }
        if let Some(path) = self.main_activity.library_tab.take_open_request() {
            if !self.main_activity.library_tab.is_transfer_screen_open() {
                self.pending_content_open_path = Some(path);
                updated = true;
            }
        }
        if let Some(request) = self.main_activity.library_tab.take_file_transfer_request() {
            self.pending_file_transfer_request = Some(request);
            updated = true;
        }
        if let Some(mode_ap) = self.main_activity.library_tab.take_wifi_mode_request() {
            self.pending_wifi_mode_ap = Some(mode_ap);
            updated = true;
        }
        if let Some(config) = self.main_activity.library_tab.take_wifi_ap_config_request() {
            self.pending_wifi_ap_config = Some(config);
            updated = true;
        }
        updated
    }

    fn process_pending_content_open_request(&mut self) -> bool {
        let Some(path) = self.pending_content_open_path.take() else {
            return false;
        };
        self.main_activity.queue_open_content_path(path);
        true
    }

    #[cfg(feature = "std")]
    fn sync_active_book_progress(&mut self) -> bool {
        let Some(path) = self.main_activity.files_tab.active_epub_path() else {
            return false;
        };
        let Some(progress) = self.main_activity.files_tab.epub_book_progress_percent() else {
            return false;
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut ui_progress_changed = false;
        if let Some(cache) = self.library_cache.as_mut() {
            if let Some(book) = cache.iter_mut().find(|book| book.path == path) {
                let next = progress.min(100);
                if book.progress_percent != next {
                    book.progress_percent = next;
                    book.last_read = Some(now);
                    ui_progress_changed = true;
                } else if book.last_read.is_none() {
                    // Initialize missing metadata without forcing a redraw.
                    book.last_read = Some(now);
                }
            }
        }
        if ui_progress_changed {
            self.main_activity
                .library_tab
                .update_book_progress(path, progress.min(100), now);
        }
        // Progress bookkeeping should not force display refreshes while reading.
        false
    }

    #[cfg(not(feature = "std"))]
    fn sync_active_book_progress(&mut self) -> bool {
        false
    }

    fn sync_runtime_settings(&mut self) -> bool {
        let settings = self.main_activity.settings();
        if settings == self.applied_settings {
            return false;
        }
        self.main_activity.apply_settings(settings);
        self.refresh_frequency_pages = settings.refresh_frequency.pages() as u32;
        self.applied_settings = settings;
        Self::persist_settings(settings);
        true
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
            if books.len() >= Self::MAX_LIBRARY_CACHE_BOOKS {
                break;
            }
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
            let mut book = BookInfo::new(title, author, path, progress, last_read);
            if let Some(cover_raw) = parts.next() {
                if cover_raw.len() <= Self::MAX_COMPACT_COVER_CHARS {
                    let cover = Self::unescape_cache_field(cover_raw);
                    if cover.len() <= Self::MAX_COMPACT_COVER_CHARS {
                        let _ = book.set_cover_thumbnail_from_compact(&cover);
                    }
                }
            }
            books.push(book);
        }
        Some(books)
    }

    #[cfg(feature = "std")]
    fn load_latest_library_snapshot(root: &str) -> Option<Vec<BookInfo>> {
        let raw = std::fs::read_to_string(Self::cache_file_path()).ok()?;
        let mut lines = raw.lines();
        let header = lines.next()?;
        let mut header_parts = header.split('\t');
        let version = header_parts.next()?;
        let root_raw = header_parts.next()?;
        if version != "v1" {
            return None;
        }
        let cached_root = Self::unescape_cache_field(root_raw);
        if cached_root != root {
            return None;
        }

        let mut books = Vec::new();
        for line in lines {
            if books.len() >= Self::MAX_LIBRARY_CACHE_BOOKS {
                break;
            }
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
            let mut book = BookInfo::new(title, author, path, progress, last_read);
            if let Some(cover_raw) = parts.next() {
                if cover_raw.len() <= Self::MAX_COMPACT_COVER_CHARS {
                    let cover = Self::unescape_cache_field(cover_raw);
                    if cover.len() <= Self::MAX_COMPACT_COVER_CHARS {
                        let _ = book.set_cover_thumbnail_from_compact(&cover);
                    }
                }
            }
            books.push(book);
        }
        if books.is_empty() {
            None
        } else {
            Some(books)
        }
    }

    #[cfg(not(feature = "std"))]
    fn load_latest_library_snapshot(_root: &str) -> Option<Vec<BookInfo>> {
        None
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

        for book in books.iter().take(Self::MAX_LIBRARY_CACHE_BOOKS) {
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
            out.push('\t');
            if let Some(cover) = book.cover_thumbnail_compact() {
                out.push_str(&Self::escape_cache_field(&cover));
            }
            out.push('\n');
        }

        let _ = std::fs::write(path, out);
    }

    #[cfg(not(feature = "std"))]
    fn persist_library_cache(_root: &str, _fingerprint: u64, _books: &[BookInfo]) {}

    #[cfg(feature = "std")]
    fn settings_file_path() -> &'static str {
        if cfg!(target_os = "espidf") {
            "/sd/.xteink/app_settings.tsv"
        } else {
            "target/.xteink-app-settings.tsv"
        }
    }

    #[cfg(feature = "std")]
    fn load_persisted_settings() -> Option<UnifiedSettings> {
        let raw = std::fs::read_to_string(Self::settings_file_path()).ok()?;
        let mut lines = raw.lines();
        let header = lines.next()?;
        if header != "v1" {
            return None;
        }
        let line = lines.next()?;
        let mut fields = line.split('\t');
        let font_size = FontSize::from_index(fields.next()?.parse::<usize>().ok()?)?;
        let font_family = FontFamily::from_index(fields.next()?.parse::<usize>().ok()?)?;
        let auto_sleep_duration =
            AutoSleepDuration::from_index(fields.next()?.parse::<usize>().ok()?)?;
        let line_spacing = LineSpacing::from_index(fields.next()?.parse::<usize>().ok()?)?;
        let margin_size = MarginSize::from_index(fields.next()?.parse::<usize>().ok()?)?;
        let text_alignment = TextAlignment::from_index(fields.next()?.parse::<usize>().ok()?)?;
        let show_page_numbers = fields.next()? == "1";
        let footer_density = FooterDensity::from_index(fields.next()?.parse::<usize>().ok()?)?;
        let footer_auto_hide = FooterAutoHide::from_index(fields.next()?.parse::<usize>().ok()?)?;
        let refresh_frequency =
            RefreshFrequency::from_index(fields.next()?.parse::<usize>().ok()?)?;
        let invert_colors = fields.next()? == "1";
        let volume_button_action =
            VolumeButtonAction::from_index(fields.next()?.parse::<usize>().ok()?)?;
        let tap_zone_config = TapZoneConfig::from_index(fields.next()?.parse::<usize>().ok()?)?;
        Some(UnifiedSettings {
            font_size,
            font_family,
            auto_sleep_duration,
            line_spacing,
            margin_size,
            text_alignment,
            show_page_numbers,
            footer_density,
            footer_auto_hide,
            refresh_frequency,
            invert_colors,
            volume_button_action,
            tap_zone_config,
        })
    }

    #[cfg(not(feature = "std"))]
    fn load_persisted_settings() -> Option<UnifiedSettings> {
        None
    }

    #[cfg(feature = "std")]
    fn persist_settings(settings: UnifiedSettings) {
        let path = Self::settings_file_path();
        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let line = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            settings.font_size.index(),
            settings.font_family.index(),
            settings.auto_sleep_duration.index(),
            settings.line_spacing.index(),
            settings.margin_size.index(),
            settings.text_alignment.index(),
            if settings.show_page_numbers { 1 } else { 0 },
            settings.footer_density.index(),
            settings.footer_auto_hide.index(),
            settings.refresh_frequency.index(),
            if settings.invert_colors { 1 } else { 0 },
            settings.volume_button_action.index(),
            settings.tap_zone_config.index(),
        );
        let mut out = String::from("v1\n");
        out.push_str(&line);
        out.push('\n');
        let _ = std::fs::write(path, out);
    }

    #[cfg(not(feature = "std"))]
    fn persist_settings(_settings: UnifiedSettings) {}

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

    /// Set whether file transfer web server is currently active.
    pub fn set_file_transfer_active(&mut self, active: bool) {
        self.main_activity
            .library_tab
            .set_file_transfer_active(active);
    }

    /// Set file transfer network details for transfer UI.
    pub fn set_file_transfer_network_details(
        &mut self,
        mode: String,
        ssid: String,
        password_hint: String,
        url: String,
        message: String,
    ) {
        self.main_activity
            .library_tab
            .set_file_transfer_network_details(mode, ssid, password_hint, url, message);
    }

    /// Take pending file transfer request (true=start, false=stop).
    pub fn take_file_transfer_request(&mut self) -> Option<bool> {
        self.pending_file_transfer_request.take()
    }

    /// Take pending wifi mode request from transfer UI (true=AP, false=STA).
    pub fn take_wifi_mode_request(&mut self) -> Option<bool> {
        self.pending_wifi_mode_ap.take()
    }

    /// Take pending AP config request from transfer UI.
    pub fn take_wifi_ap_config_request(&mut self) -> Option<(String, String)> {
        self.pending_wifi_ap_config.take()
    }

    /// Get compact-encoded cover thumbnail for the currently open EPUB, if known.
    #[cfg(feature = "std")]
    pub fn active_book_cover_thumbnail_compact(&self) -> Option<String> {
        let path = self.main_activity.files_tab.active_epub_path()?;
        self.find_cover_thumbnail_compact_for_path(path)
    }

    #[cfg(feature = "std")]
    fn find_cover_thumbnail_compact_for_path(&self, path: &str) -> Option<String> {
        if let Some(books) = self.library_cache.as_ref() {
            if let Some(cover) = books
                .iter()
                .find(|book| book.path == path)
                .and_then(|book| book.cover_thumbnail_compact())
            {
                return Some(cover);
            }
        }

        Self::load_latest_library_snapshot(&self.library_root).and_then(|books| {
            books
                .iter()
                .find(|book| book.path == path)
                .and_then(|book| book.cover_thumbnail_compact())
        })
    }

    #[cfg(not(feature = "std"))]
    pub fn active_book_cover_thumbnail_compact(&self) -> Option<String> {
        None
    }

    /// Get auto-sleep duration in milliseconds.
    pub fn auto_sleep_duration_ms(&self) -> u32 {
        self.main_activity.auto_sleep_duration_ms()
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
        let app = App::new_with_epub_resume(false);
        assert_eq!(app.current_tab() as usize, 0);
    }

    #[test]
    fn app_left_right_cycles_tabs() {
        let mut app = App::new_with_epub_resume(false);
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
        let app = App::new_with_epub_resume(false);
        let mut display = crate::test_display::TestDisplay::default_size();
        let result = app.render(&mut display);
        assert!(result.is_ok());
    }

    #[test]
    fn app_forces_full_refresh_on_first_draw() {
        let mut app = App::new_with_epub_resume(false);
        assert_eq!(app.get_refresh_mode(), ActivityRefreshMode::Full);
        assert_eq!(app.get_refresh_mode(), ActivityRefreshMode::Fast);
    }
}
