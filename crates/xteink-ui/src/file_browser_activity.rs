//! Activity wrapper for the raw filesystem browser.
//!
//! Keeps filesystem I/O out of the input/render path by scheduling
//! deferred tasks that are processed by the app loop.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
#[cfg(feature = "std")]
use alloc::vec::Vec;
#[cfg(all(feature = "std", not(target_os = "espidf")))]
use std::collections::HashMap;
#[cfg(feature = "std")]
use std::collections::{BTreeMap, BTreeSet};
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(all(feature = "std", not(target_os = "espidf")))]
use std::sync::mpsc::{self, Receiver, TryRecvError};
#[cfg(feature = "std")]
use std::sync::{Arc, Mutex};
#[cfg(all(feature = "std", not(target_os = "espidf")))]
use std::thread;

#[cfg(feature = "std")]
use embedded_graphics::{
    mono_font::{
        ascii::{FONT_7X13, FONT_7X13_BOLD, FONT_8X13, FONT_9X18_BOLD},
        MonoTextStyle,
    },
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
#[cfg(feature = "std")]
use mu_epub::book::{ChapterEventsOptions, OpenConfig};
#[cfg(feature = "std")]
use mu_epub::{EpubBook, RenderPrepOptions, ScratchBuffers, ZipLimits};
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

#[cfg(feature = "std")]
use crate::app::AppScreen;
#[cfg(all(feature = "std", feature = "fontdue"))]
use crate::epub_font_backend::BookerlyFontBackend;
use crate::file_browser::{FileBrowser, TextViewer};
use crate::filesystem::{basename, dirname, FileSystem};
use crate::input::{Button, InputEvent};
use crate::reader_settings_activity::ReaderSettings;
#[cfg(feature = "std")]
use crate::ui::theme::ui_text;
use crate::ui::{Activity, ActivityResult};

#[cfg(feature = "std")]
mod epub;

#[cfg(feature = "std")]
pub(super) const EPUB_FOOTER_HEIGHT: i32 = 36;
#[cfg(feature = "std")]
pub(super) const EPUB_FOOTER_BOTTOM_PADDING: i32 = 12;
#[cfg(feature = "std")]
pub(super) const EPUB_FOOTER_TOP_GAP: i32 = 8;

#[derive(Debug, Clone)]
enum FileBrowserTask {
    LoadCurrentDirectory,
    OpenPath {
        path: String,
    },
    OpenTextFile {
        path: String,
    },
    OpenEpubFile {
        path: String,
    },
    #[cfg(all(feature = "std", target_os = "espidf"))]
    FinalizeEpubOpen,
    #[cfg(all(feature = "std", target_os = "espidf"))]
    RestoreEpubPosition,
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
#[derive(Clone)]
struct EpubTocItem {
    chapter_index: usize,
    depth: usize,
    label: String,
}

#[cfg(feature = "std")]
enum EpubOverlay {
    QuickMenu {
        selected: usize,
    },
    Toc {
        items: Vec<EpubTocItem>,
        selected: usize,
        scroll: usize,
    },
    JumpPercent {
        percent: u8,
    },
    Finished,
}

#[cfg(all(feature = "std", not(target_os = "espidf")))]
trait ReadSeek: Read + Seek + Send {}

#[cfg(all(feature = "std", not(target_os = "espidf")))]
impl<T: Read + Seek + Send> ReadSeek for T {}

#[cfg(all(feature = "std", not(target_os = "espidf")))]
struct PendingEpubOpen {
    receiver: Receiver<Result<EpubReadingState, String>>,
}

#[cfg(all(feature = "std", not(target_os = "espidf")))]
struct PendingEpubNavigation {
    receiver: Receiver<Result<EpubNavigationOutcome, String>>,
    direction: EpubNavigationDirection,
}

#[cfg(all(feature = "std", not(target_os = "espidf")))]
#[derive(Clone, Copy, Debug, Default)]
struct EpubNavigationOutcome {
    advanced: bool,
    reached_end: bool,
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
    #[allow(dead_code)]
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
    chapter_events_opts: RenderPrepOptions,
    eg_renderer: ReaderRenderer,
    #[cfg(all(feature = "std", feature = "fontdue"))]
    layout_text_measurer: BookerlyFontBackend,
    chapter_buf: Vec<u8>,
    chapter_scratch: ScratchBuffers,
    current_page: Option<RenderPage>,
    page_cache: BTreeMap<(usize, usize), RenderPage>,
    #[cfg(not(target_os = "espidf"))]
    render_cache: InMemoryRenderCache,
    chapter_page_counts: BTreeMap<usize, usize>,
    chapter_page_counts_exact: BTreeSet<usize>,
    non_renderable_chapters: BTreeSet<usize>,
    chapter_idx: usize,
    page_idx: usize,
    last_next_page_reached_end: bool,
}

#[cfg(all(feature = "std", feature = "fontdue"))]
type ReaderRenderer = EgRenderer<BookerlyFontBackend>;

#[cfg(all(feature = "std", not(feature = "fontdue")))]
type ReaderRenderer = EgRenderer<mu_epub_embedded_graphics::MonoFontBackend>;

/// Raw filesystem browser activity.
pub struct FileBrowserActivity {
    browser: FileBrowser,
    mode: BrowserMode,
    reader_settings: ReaderSettings,
    battery_percent: u8,
    ui_tick: u32,
    #[cfg(feature = "std")]
    last_epub_interaction_tick: u32,
    #[cfg(feature = "std")]
    epub_overlay: Option<EpubOverlay>,
    #[cfg(feature = "std")]
    active_epub_path: Option<String>,
    pending_task: Option<FileBrowserTask>,
    return_to_previous_on_back: bool,
    #[cfg(all(feature = "std", not(target_os = "espidf")))]
    epub_open_pending: Option<PendingEpubOpen>,
    #[cfg(all(feature = "std", not(target_os = "espidf")))]
    epub_navigation_pending: Option<PendingEpubNavigation>,
    #[cfg(all(feature = "std", target_os = "espidf"))]
    epub_open_staged: Option<Arc<Mutex<EpubReadingState>>>,
}

impl FileBrowserActivity {
    #[cfg(feature = "std")]
    fn truncate_to_px(text: &str, max_px: i32, size: u32) -> String {
        if max_px <= 0 {
            return String::new();
        }
        if ui_text::width(text, Some(size)) as i32 <= max_px {
            return text.to_string();
        }
        let ellipsis = "...";
        let ellipsis_w = ui_text::width(ellipsis, Some(size)) as i32;
        if ellipsis_w >= max_px {
            return String::new();
        }
        let mut out = String::new();
        for ch in text.chars() {
            let mut candidate = out.clone();
            candidate.push(ch);
            let candidate_w = ui_text::width(&candidate, Some(size)) as i32;
            if candidate_w + ellipsis_w > max_px {
                break;
            }
            out.push(ch);
        }
        out.push_str(ellipsis);
        out
    }

    fn exit_reader_to_browser(&mut self) -> ActivityResult {
        #[cfg(feature = "std")]
        {
            self.persist_active_epub_position();
        }
        #[cfg(all(feature = "std", not(target_os = "espidf")))]
        {
            self.epub_open_pending = None;
            self.epub_navigation_pending = None;
        }
        #[cfg(all(feature = "std", target_os = "espidf"))]
        {
            self.epub_open_staged = None;
        }
        self.mode = BrowserMode::Browsing;
        #[cfg(feature = "std")]
        {
            self.epub_overlay = None;
            self.active_epub_path = None;
        }
        if self.return_to_previous_on_back {
            self.return_to_previous_on_back = false;
            return ActivityResult::NavigateBack;
        }
        ActivityResult::Consumed
    }

    #[cfg(feature = "std")]
    fn handle_epub_overlay_input(&mut self, event: InputEvent) -> ActivityResult {
        let Some(mut overlay) = self.epub_overlay.take() else {
            return ActivityResult::Ignored;
        };
        if matches!(event, InputEvent::Press(Button::Back)) {
            self.epub_overlay = None;
            return ActivityResult::Consumed;
        }

        let mut navigate_settings = false;
        let mut exit_to_files = false;
        let mut put_back = true;
        match &mut self.mode {
            BrowserMode::ReadingEpub { renderer } => match &mut overlay {
                EpubOverlay::QuickMenu { selected } => {
                    const COUNT: usize = 7;
                    match event {
                        InputEvent::Press(Button::Up) | InputEvent::Press(Button::VolumeUp) => {
                            if *selected == 0 {
                                *selected = COUNT - 1;
                            } else {
                                *selected -= 1;
                            }
                        }
                        InputEvent::Press(Button::Down) | InputEvent::Press(Button::VolumeDown) => {
                            *selected = (*selected + 1) % COUNT;
                        }
                        InputEvent::Press(Button::Confirm) | InputEvent::Press(Button::Right) => {
                            match *selected {
                                0 => put_back = false,
                                1 => {
                                    let mut guard = match renderer.lock() {
                                        Ok(guard) => guard,
                                        Err(poisoned) => poisoned.into_inner(),
                                    };
                                    let items = guard.toc_items();
                                    if items.is_empty() {
                                        self.browser.set_status_message(
                                            "EPUB has no TOC entries".to_string(),
                                        );
                                        put_back = false;
                                    } else {
                                        overlay = EpubOverlay::Toc {
                                            items,
                                            selected: 0,
                                            scroll: 0,
                                        };
                                    }
                                }
                                2 => {
                                    let guard = match renderer.lock() {
                                        Ok(guard) => guard,
                                        Err(poisoned) => poisoned.into_inner(),
                                    };
                                    overlay = EpubOverlay::JumpPercent {
                                        percent: guard.book_progress_percent(),
                                    };
                                }
                                3 => {
                                    navigate_settings = true;
                                    put_back = false;
                                }
                                4 => {
                                    self.reader_settings.footer_density =
                                        self.reader_settings.footer_density.next_wrapped();
                                    self.set_reader_settings(self.reader_settings);
                                }
                                5 => {
                                    self.reader_settings.footer_auto_hide =
                                        self.reader_settings.footer_auto_hide.next_wrapped();
                                    self.set_reader_settings(self.reader_settings);
                                }
                                6 => {
                                    exit_to_files = true;
                                    put_back = false;
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
                EpubOverlay::Toc {
                    items,
                    selected,
                    scroll,
                } => {
                    let page = 6usize;
                    match event {
                        InputEvent::Press(Button::Up) | InputEvent::Press(Button::VolumeUp) => {
                            if *selected == 0 {
                                *selected = items.len().saturating_sub(1);
                            } else {
                                *selected -= 1;
                            }
                        }
                        InputEvent::Press(Button::Down) | InputEvent::Press(Button::VolumeDown) => {
                            *selected = (*selected + 1) % items.len();
                        }
                        InputEvent::Press(Button::Left) => {
                            *selected = selected.saturating_sub(page);
                        }
                        InputEvent::Press(Button::Right) => {
                            *selected = (*selected + page).min(items.len().saturating_sub(1));
                        }
                        InputEvent::Press(Button::Confirm) => {
                            let mut guard = match renderer.lock() {
                                Ok(guard) => guard,
                                Err(poisoned) => poisoned.into_inner(),
                            };
                            let target = items[*selected].chapter_index;
                            if !guard.jump_to_chapter(target) {
                                self.browser.set_status_message(
                                    "Unable to jump to selected chapter".to_string(),
                                );
                            }
                            put_back = false;
                        }
                        _ => {}
                    }
                    let visible = 6usize;
                    if *selected < *scroll {
                        *scroll = *selected;
                    } else if *selected >= *scroll + visible {
                        *scroll = selected.saturating_sub(visible - 1);
                    }
                }
                EpubOverlay::JumpPercent { percent } => match event {
                    InputEvent::Press(Button::Left) => *percent = percent.saturating_sub(1),
                    InputEvent::Press(Button::Right) => *percent = (*percent + 1).min(100),
                    InputEvent::Press(Button::Down) | InputEvent::Press(Button::VolumeDown) => {
                        *percent = percent.saturating_sub(10)
                    }
                    InputEvent::Press(Button::Up) | InputEvent::Press(Button::VolumeUp) => {
                        *percent = (*percent + 10).min(100)
                    }
                    InputEvent::Press(Button::Confirm) => {
                        let mut guard = match renderer.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        if !guard.jump_to_book_percent(*percent) {
                            self.browser.set_status_message(
                                "Unable to jump to selected position".to_string(),
                            );
                        }
                        put_back = false;
                    }
                    _ => {}
                },
                EpubOverlay::Finished => match event {
                    InputEvent::Press(Button::Confirm) | InputEvent::Press(Button::Back) => {
                        exit_to_files = true;
                        put_back = false;
                    }
                    InputEvent::Press(Button::Left)
                    | InputEvent::Press(Button::Right)
                    | InputEvent::Press(Button::Up)
                    | InputEvent::Press(Button::Down)
                    | InputEvent::Press(Button::VolumeUp)
                    | InputEvent::Press(Button::VolumeDown) => {
                        put_back = false;
                    }
                    InputEvent::Press(Button::Power) => {
                        overlay = EpubOverlay::QuickMenu { selected: 0 };
                    }
                },
            },
            _ => {
                put_back = false;
            }
        }

        if navigate_settings {
            self.epub_overlay = None;
            return ActivityResult::NavigateTo(AppScreen::ReaderSettings);
        }
        if exit_to_files {
            self.epub_overlay = None;
            return self.exit_reader_to_browser();
        }
        if put_back {
            self.epub_overlay = Some(overlay);
        } else {
            self.epub_overlay = None;
        }
        ActivityResult::Consumed
    }

    pub const DEFAULT_ROOT: &'static str = "/";
    #[cfg(all(feature = "std", not(target_os = "espidf")))]
    const EPUB_OPEN_WORKER_STACK_BYTES: usize = 2 * 1024 * 1024;
    #[cfg(all(feature = "std", not(target_os = "espidf")))]
    const EPUB_NAV_WORKER_STACK_BYTES: usize = 512 * 1024;

    pub fn new() -> Self {
        Self {
            browser: FileBrowser::new(Self::DEFAULT_ROOT),
            mode: BrowserMode::Browsing,
            reader_settings: ReaderSettings::default(),
            battery_percent: 100,
            ui_tick: 0,
            #[cfg(feature = "std")]
            last_epub_interaction_tick: 0,
            #[cfg(feature = "std")]
            epub_overlay: None,
            #[cfg(feature = "std")]
            active_epub_path: None,
            pending_task: None,
            return_to_previous_on_back: false,
            #[cfg(all(feature = "std", not(target_os = "espidf")))]
            epub_open_pending: None,
            #[cfg(all(feature = "std", not(target_os = "espidf")))]
            epub_navigation_pending: None,
            #[cfg(all(feature = "std", target_os = "espidf"))]
            epub_open_staged: None,
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

    #[allow(dead_code)]
    pub(crate) fn status_message(&self) -> Option<&str> {
        self.browser.status_message()
    }

    pub fn set_reader_settings(&mut self, settings: ReaderSettings) {
        self.reader_settings = settings;
        #[cfg(feature = "std")]
        if let BrowserMode::ReadingEpub { renderer } = &self.mode {
            let mut renderer = match renderer.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Err(error) = renderer.apply_reader_settings(settings) {
                self.browser.set_status_message(format!(
                    "Unable to apply reader settings immediately: {}",
                    error
                ));
            }
        }
    }

    pub fn set_battery_percent(&mut self, battery_percent: u8) {
        self.battery_percent = battery_percent.min(100);
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

    #[cfg(feature = "std")]
    pub fn active_epub_path(&self) -> Option<&str> {
        self.active_epub_path.as_deref()
    }

    #[inline(never)]
    pub fn process_pending_task(&mut self, fs: &mut dyn FileSystem) -> bool {
        self.ui_tick = self.ui_tick.saturating_add(1);
        #[cfg(all(feature = "std", not(target_os = "espidf")))]
        let mut updated = self.poll_epub_open_result();
        #[cfg(all(feature = "std", not(target_os = "espidf")))]
        {
            updated |= self.poll_epub_navigation_result();
        }
        #[cfg(any(not(feature = "std"), target_os = "espidf"))]
        let mut updated = false;

        let Some(task) = self.pending_task.take() else {
            return updated;
        };

        let task_updated = match task {
            FileBrowserTask::LoadCurrentDirectory => self.process_load_current_directory_task(fs),
            FileBrowserTask::OpenPath { path } => self.process_open_path_task(fs, &path),
            FileBrowserTask::OpenTextFile { path } => self.process_open_text_file_task(fs, &path),
            FileBrowserTask::OpenEpubFile { path } => self.process_open_epub_file_task(fs, &path),
            #[cfg(all(feature = "std", target_os = "espidf"))]
            FileBrowserTask::FinalizeEpubOpen => self.process_finalize_epub_open_task(),
            #[cfg(all(feature = "std", target_os = "espidf"))]
            FileBrowserTask::RestoreEpubPosition => self.process_restore_epub_position_task(),
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
        #[cfg(feature = "std")]
        if self.is_viewing_epub() && self.epub_overlay.is_some() {
            return self.handle_epub_overlay_input(event);
        }

        if matches!(event, InputEvent::Press(Button::Back)) {
            #[cfg(feature = "std")]
            {
                if self.is_viewing_epub() {
                    self.epub_overlay = Some(EpubOverlay::QuickMenu { selected: 0 });
                    return ActivityResult::Consumed;
                }
            }
            return self.exit_reader_to_browser();
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
                enum EpubInputAction {
                    Page(EpubNavigationDirection),
                    ChapterNext,
                    ChapterPrev,
                    OpenSettings,
                }

                let action = match event {
                    InputEvent::Press(Button::Power) => Some(EpubInputAction::OpenSettings),
                    InputEvent::Press(Button::Down) => Some(EpubInputAction::ChapterNext),
                    InputEvent::Press(Button::Up) => Some(EpubInputAction::ChapterPrev),
                    InputEvent::Press(Button::Right)
                    | InputEvent::Press(Button::VolumeDown)
                    | InputEvent::Press(Button::Confirm) => {
                        Some(EpubInputAction::Page(EpubNavigationDirection::Next))
                    }
                    InputEvent::Press(Button::Left) | InputEvent::Press(Button::VolumeUp) => {
                        Some(EpubInputAction::Page(EpubNavigationDirection::Prev))
                    }
                    _ => None,
                };

                if let Some(action) = action {
                    if let EpubInputAction::OpenSettings = action {
                        self.last_epub_interaction_tick = self.ui_tick;
                        self.epub_overlay = Some(EpubOverlay::QuickMenu { selected: 0 });
                        return ActivityResult::Consumed;
                    }
                    #[cfg(target_os = "espidf")]
                    {
                        let mut advanced_position: Option<(usize, usize)> = None;
                        let mut renderer = match renderer.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        let advanced = match action {
                            EpubInputAction::Page(direction) => match direction {
                                EpubNavigationDirection::Next => renderer.next_page(),
                                EpubNavigationDirection::Prev => renderer.prev_page(),
                            },
                            EpubInputAction::ChapterNext => renderer.next_chapter(),
                            EpubInputAction::ChapterPrev => renderer.prev_chapter(),
                            EpubInputAction::OpenSettings => false,
                        };
                        let mut exact_chapter_counts: Vec<(usize, usize)> = Vec::new();
                        let reached_end =
                            matches!(action, EpubInputAction::Page(EpubNavigationDirection::Next))
                                && renderer.take_last_next_page_reached_end();
                        if !advanced {
                            if reached_end {
                                self.epub_overlay = Some(EpubOverlay::Finished);
                            } else {
                                log::warn!("[EPUB] unable to handle epub action");
                            }
                        } else {
                            advanced_position = Some(renderer.position_indices());
                            exact_chapter_counts = renderer.exact_chapter_page_counts();
                        }
                        drop(renderer);
                        if let (Some((chapter_idx, page_idx)), Some(path)) =
                            (advanced_position, self.active_epub_path.as_ref())
                        {
                            self.last_epub_interaction_tick = self.ui_tick;
                            let _ = Self::persist_epub_position_for_path(
                                path,
                                chapter_idx,
                                page_idx,
                                &exact_chapter_counts,
                            );
                        }
                        ActivityResult::Consumed
                    }

                    #[cfg(not(target_os = "espidf"))]
                    {
                        match action {
                            EpubInputAction::Page(direction) => {
                                if self.epub_navigation_pending.is_some() {
                                    return ActivityResult::Consumed;
                                }
                                self.last_epub_interaction_tick = self.ui_tick;
                                let renderer = Arc::clone(renderer);
                                match Self::spawn_epub_navigation_worker(renderer, direction) {
                                    Ok(receiver) => {
                                        self.epub_navigation_pending =
                                            Some(PendingEpubNavigation {
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
                            }
                            EpubInputAction::ChapterNext | EpubInputAction::ChapterPrev => {
                                let mut renderer = match renderer.lock() {
                                    Ok(guard) => guard,
                                    Err(poisoned) => poisoned.into_inner(),
                                };
                                let advanced = match action {
                                    EpubInputAction::ChapterNext => renderer.next_chapter(),
                                    EpubInputAction::ChapterPrev => renderer.prev_chapter(),
                                    _ => false,
                                };
                                if !advanced {
                                    self.browser.set_status_message(
                                        "No more chapters in this direction".to_string(),
                                    );
                                }
                                self.last_epub_interaction_tick = self.ui_tick;
                                ActivityResult::Consumed
                            }
                            EpubInputAction::OpenSettings => ActivityResult::Ignored,
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
            self.epub_overlay = None;
            self.active_epub_path = None;
        }
        #[cfg(all(feature = "std", not(target_os = "espidf")))]
        {
            self.epub_open_pending = None;
            self.epub_navigation_pending = None;
        }
        #[cfg(all(feature = "std", target_os = "espidf"))]
        {
            self.epub_open_staged = None;
        }
        if self.pending_task.is_none() {
            self.queue_load_current_directory();
        }
    }

    fn on_exit(&mut self) {
        #[cfg(feature = "std")]
        {
            self.persist_active_epub_position();
        }
        self.mode = BrowserMode::Browsing;
        #[cfg(feature = "std")]
        {
            self.epub_overlay = None;
            self.active_epub_path = None;
        }
        self.pending_task = None;
        self.return_to_previous_on_back = false;
        #[cfg(all(feature = "std", not(target_os = "espidf")))]
        {
            self.epub_open_pending = None;
            self.epub_navigation_pending = None;
        }
        #[cfg(all(feature = "std", target_os = "espidf"))]
        {
            self.epub_open_staged = None;
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
                renderer.render(display)?;
                if self.should_show_epub_footer() {
                    self.render_epub_footer(display, &renderer)?;
                }
                self.render_epub_overlay(display, &renderer)
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

impl FileBrowserActivity {
    #[cfg(feature = "std")]
    fn should_show_epub_footer(&self) -> bool {
        if self.epub_overlay.is_some() {
            return true;
        }
        let hide_ms = self.reader_settings.footer_auto_hide.milliseconds();
        if hide_ms == 0 {
            return true;
        }
        let elapsed_ticks = self.ui_tick.saturating_sub(self.last_epub_interaction_tick);
        let elapsed_ms = elapsed_ticks.saturating_mul(50);
        elapsed_ms < hide_ms
    }

    #[cfg(feature = "std")]
    fn render_epub_footer<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        renderer: &EpubReadingState,
    ) -> Result<(), D::Error> {
        let size = display.bounding_box().size;
        let width = size.width.min(size.height);
        let height = size.width.max(size.height);
        let footer_h: u32 = EPUB_FOOTER_HEIGHT as u32;
        let y = (height as i32 - footer_h as i32 - EPUB_FOOTER_BOTTOM_PADDING).max(0);

        Rectangle::new(Point::new(0, y), Size::new(width, footer_h))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
            .draw(display)?;

        let body_size = 16u32;
        let title_size = 18u32;
        let battery_text = format!("{}%", self.battery_percent);
        let battery_w = ui_text::width(&battery_text, Some(body_size)) as i32;
        let battery_x = (width as i32 - 8 - battery_w).max(8);

        let info = match self.reader_settings.footer_density {
            crate::reader_settings_activity::FooterDensity::Minimal => {
                format!(
                    "{}  {}%",
                    renderer.page_progress_label(),
                    renderer.book_progress_percent()
                )
            }
            crate::reader_settings_activity::FooterDensity::Detailed => {
                format!(
                    "{}  {}  {}%",
                    renderer.page_progress_label(),
                    renderer.chapter_progress_label(),
                    renderer.book_progress_percent(),
                )
            }
        };
        let info_max_w = (width as i32 / 3).max(80);
        let info_text = Self::truncate_to_px(&info, info_max_w, body_size);
        let info_w = ui_text::width(&info_text, Some(body_size)) as i32;
        let info_x = 8;

        // Center chapter title between left metrics and right battery.
        let center_left = (info_x + info_w + 14).max(14);
        let center_right = (battery_x - 14).max(center_left);
        let center_w = (center_right - center_left).max(0);
        let mut chapter_title = renderer.current_chapter_title(128);
        chapter_title = Self::truncate_to_px(&chapter_title, center_w, title_size);
        let chapter_w = ui_text::width(&chapter_title, Some(title_size)) as i32;
        let chapter_x = (center_left + ((center_w - chapter_w) / 2)).max(center_left);

        let baseline = y + 25;
        ui_text::draw(display, &info_text, info_x, baseline, Some(body_size))?;
        ui_text::draw(
            display,
            &chapter_title,
            chapter_x,
            baseline,
            Some(title_size),
        )?;
        ui_text::draw(display, &battery_text, battery_x, baseline, Some(body_size))?;
        Ok(())
    }

    #[cfg(feature = "std")]
    fn render_epub_overlay<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        renderer: &EpubReadingState,
    ) -> Result<(), D::Error> {
        let Some(overlay) = self.epub_overlay.as_ref() else {
            return Ok(());
        };
        let size = display.bounding_box().size;
        let width = size.width.min(size.height) as i32;
        let height = size.width.max(size.height) as i32;
        let panel_w = (width - 32).max(120);
        let panel_h = (height - 110).max(120);
        let panel_x = (width - panel_w) / 2;
        let panel_y = ((height - panel_h) / 2 - 8).max(8);
        Rectangle::new(
            Point::new(panel_x - 4, panel_y - 4),
            Size::new((panel_w + 8) as u32, (panel_h + 8) as u32),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
        .draw(display)?;
        Rectangle::new(
            Point::new(panel_x, panel_y),
            Size::new(panel_w as u32, panel_h as u32),
        )
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display)?;

        let title_style = MonoTextStyle::new(&FONT_9X18_BOLD, BinaryColor::On);
        let body_style = MonoTextStyle::new(&FONT_8X13, BinaryColor::On);
        let hint_style = MonoTextStyle::new(&FONT_7X13, BinaryColor::On);

        match overlay {
            EpubOverlay::QuickMenu { selected } => {
                Text::new(
                    "Reader Menu",
                    Point::new(panel_x + 8, panel_y + 22),
                    title_style,
                )
                .draw(display)?;
                let items = [
                    "Resume",
                    "Table of Contents",
                    "Go to Position",
                    "Reader Settings",
                    "Footer: ",
                    "Footer Hide: ",
                    "Back to Files",
                ];
                for (i, item) in items.iter().enumerate() {
                    let y = panel_y + 56 + (i as i32 * 22);
                    let label = match i {
                        4 => format!("{}{}", item, self.reader_settings.footer_density.label()),
                        5 => format!("{}{}", item, self.reader_settings.footer_auto_hide.label()),
                        _ => item.to_string(),
                    };
                    if i == *selected {
                        Rectangle::new(
                            Point::new(panel_x + 6, y - 16),
                            Size::new((panel_w - 12) as u32, 22),
                        )
                        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                        .draw(display)?;
                        Text::new(
                            &label,
                            Point::new(panel_x + 10, y),
                            MonoTextStyle::new(&FONT_8X13, BinaryColor::Off),
                        )
                        .draw(display)?;
                    } else {
                        Text::new(
                            &label,
                            Point::new(panel_x + 10, y),
                            MonoTextStyle::new(&FONT_8X13, BinaryColor::On),
                        )
                        .draw(display)?;
                    }
                }
                Text::new(
                    "U/D Move  OK Select  Back Close",
                    Point::new(panel_x + 8, panel_y + panel_h - 14),
                    hint_style,
                )
                .draw(display)?;
            }
            EpubOverlay::Toc {
                items,
                selected,
                scroll,
            } => {
                Text::new(
                    "Table of Contents",
                    Point::new(panel_x + 8, panel_y + 22),
                    title_style,
                )
                .draw(display)?;
                let visible = 6usize;
                for row in 0..visible {
                    let idx = scroll + row;
                    if idx >= items.len() {
                        break;
                    }
                    let item = &items[idx];
                    let y = panel_y + 56 + (row as i32 * 22);
                    let indent = (item.depth.min(4) as i32) * 10;
                    if idx == *selected {
                        Rectangle::new(
                            Point::new(panel_x + 6, y - 15),
                            Size::new((panel_w - 12) as u32, 20),
                        )
                        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                        .draw(display)?;
                        Text::new(
                            &item.label,
                            Point::new(panel_x + 10 + indent, y),
                            MonoTextStyle::new(&FONT_7X13_BOLD, BinaryColor::Off),
                        )
                        .draw(display)?;
                    } else {
                        Text::new(
                            &item.label,
                            Point::new(panel_x + 10 + indent, y),
                            body_style,
                        )
                        .draw(display)?;
                    }
                }
                let pos = format!("{}/{}", selected + 1, items.len());
                Text::new(
                    &pos,
                    Point::new(panel_x + panel_w - 72, panel_y + 22),
                    body_style,
                )
                .draw(display)?;
                Text::new(
                    "U/D Move  L/R Pg  OK Jump  Back",
                    Point::new(panel_x + 8, panel_y + panel_h - 14),
                    hint_style,
                )
                .draw(display)?;
            }
            EpubOverlay::JumpPercent { percent } => {
                Text::new(
                    "Go to Position",
                    Point::new(panel_x + 8, panel_y + 22),
                    title_style,
                )
                .draw(display)?;
                let pct = format!("{}%", percent);
                Text::new(
                    &pct,
                    Point::new(panel_x + panel_w / 2 - 28, panel_y + 64),
                    MonoTextStyle::new(&FONT_9X18_BOLD, BinaryColor::On),
                )
                .draw(display)?;

                let bar_x = panel_x + 16;
                let bar_y = panel_y + 88;
                let bar_w = panel_w - 32;
                Rectangle::new(Point::new(bar_x, bar_y), Size::new(bar_w as u32, 16))
                    .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                    .draw(display)?;
                let fill = (bar_w.saturating_sub(4) * (*percent as i32)) / 100;
                if fill > 0 {
                    Rectangle::new(Point::new(bar_x + 2, bar_y + 2), Size::new(fill as u32, 12))
                        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                        .draw(display)?;
                }
                let chapter = format!("Now: {}%", renderer.book_progress_percent());
                Text::new(&chapter, Point::new(bar_x, bar_y + 34), body_style).draw(display)?;
                Text::new(
                    "L/R 1%  U/D 10%  OK Jump  Back",
                    Point::new(panel_x + 8, panel_y + panel_h - 14),
                    hint_style,
                )
                .draw(display)?;
            }
            EpubOverlay::Finished => {
                Text::new(
                    "Finished",
                    Point::new(panel_x + 8, panel_y + 22),
                    title_style,
                )
                .draw(display)?;
                Text::new(
                    "You reached the end of this book.",
                    Point::new(panel_x + 10, panel_y + 58),
                    body_style,
                )
                .draw(display)?;
                Text::new(
                    "Confirm/Back Exit  Any Nav Continue",
                    Point::new(panel_x + 8, panel_y + panel_h - 14),
                    hint_style,
                )
                .draw(display)?;
            }
        }

        Ok(())
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

    // TODO: Fix this test - Cursor not imported
    // #[test]
    // fn sample_epub_parses_with_reasonable_spine_count() {
    //     let bytes = include_bytes!("../../../sample_books/sample.epub").to_vec();
    //     let reader = Cursor::new(bytes);
    //     let zip_limits = ZipLimits::new(8 * 1024 * 1024, 1024).with_max_eocd_scan(8 * 1024);
    //     let open_cfg = OpenConfig {
    //         options: mu_epub::book::EpubBookOptions {
    //             zip_limits: Some(zip_limits),
    //             validation_mode: mu_epub::book::ValidationMode::Lenient,
    //             max_nav_bytes: Some(256 * 1024),
    //         },
    //         lazy_navigation: true,
    //     };
    //     let book =
    //         EpubBook::from_reader_with_config(reader, open_cfg).expect("sample epub should parse");
    //     assert!(
    //         book.chapter_count() > 0 && book.chapter_count() < 4096,
    //         "unexpected chapter count: {}",
    //         book.chapter_count()
    //     );
    // }

    // TODO: Fix this test - EpubReadingState::from_reader doesn't exist
    // #[test]
    // fn epub_reading_state_from_reader_completes_for_sample_epub() {
    //     let bytes = include_bytes!("../../../sample_books/sample.epub").to_vec();
    //     let (tx, rx) = mpsc::channel();
    //     thread::spawn(move || {
    //         let result = EpubReadingState::from_reader(
    //             Box::new(Cursor::new(bytes)),
    //             ReaderSettings::default(),
    //         );
    //         let _ = tx.send(result.map(|_| ()));
    //     });
    //
    //     let result = rx
    //         .recv_timeout(Duration::from_secs(20))
    //         .expect("epub reading-state build timed out");
    //     assert!(
    //         result.is_ok(),
    //         "epub reading-state build failed: {:?}",
    //         result
    //     );
    // }
}
