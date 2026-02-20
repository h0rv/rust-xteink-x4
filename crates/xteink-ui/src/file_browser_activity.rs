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
#[cfg(all(feature = "std", not(target_os = "espidf")))]
use std::fs::File;
#[cfg(feature = "std")]
use std::sync::mpsc::Receiver;
#[cfg(feature = "std")]
use std::sync::{Arc, Mutex};

#[cfg(feature = "std")]
use embedded_graphics::{
    mono_font::MonoTextStyle,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
#[cfg(feature = "std")]
use image::ImageReader;
#[cfg(feature = "std")]
use mu_epub::book::{ChapterEventsOptions, OpenConfig};
#[cfg(feature = "std")]
use mu_epub::{EpubBook, RenderPrepOptions, ScratchBuffers, ZipLimits};
#[cfg(feature = "std")]
use mu_epub_embedded_graphics::{EgRenderConfig, EgRenderer};
#[cfg(feature = "std")]
use mu_epub_render::{
    DrawCommand, ImageObjectCommand, RenderConfig, RenderEngine, RenderEngineOptions, RenderPage,
};
#[cfg(all(feature = "std", not(target_os = "espidf")))]
use mu_epub_render::{PaginationProfileId, RenderCacheStore};
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
use crate::ui::theme::{layout, ui_font_body, ui_font_small, ui_font_title, ui_text};
use crate::ui::{Activity, ActivityResult};

#[cfg(feature = "std")]
mod epub;

// Re-export layout constants under the names used by epub.rs
#[cfg(feature = "std")]
pub(super) use crate::ui::theme::layout::EPUB_FOOTER_BOTTOM_PAD as EPUB_FOOTER_BOTTOM_PADDING;
#[cfg(feature = "std")]
pub(super) use crate::ui::theme::layout::EPUB_FOOTER_H as EPUB_FOOTER_HEIGHT;
#[cfg(feature = "std")]
pub(super) use crate::ui::theme::layout::EPUB_FOOTER_TOP_GAP;

#[derive(Debug, Clone)]
enum FileBrowserTask {
    LoadCurrentDirectory,
    OpenPath { path: String },
    OpenTextFile { path: String },
    OpenImageFile { path: String },
    OpenAnyFile { path: String },
    OpenEpubFile { path: String },
}

#[derive(Debug, Clone)]
struct PendingBrowserTask {
    epoch: u32,
    task: FileBrowserTask,
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
    ViewingImage {
        title: String,
        viewer: ImageViewer,
    },
    #[cfg(feature = "std")]
    ReadingEpub {
        renderer: Arc<Mutex<EpubReadingState>>,
    },
}

#[cfg(feature = "std")]
struct ImageViewer {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    threshold: u8,
}

#[cfg(feature = "std")]
#[derive(Clone)]
struct InlineImageBitmap {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    threshold: u8,
}

#[cfg(feature = "std")]
impl ImageViewer {
    const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
    const MAX_DECODED_PIXELS: u64 = 8_000_000;

    fn from_bytes(data: &[u8], max_width: u32, max_height: u32) -> Result<Self, String> {
        if data.len() > Self::MAX_IMAGE_BYTES {
            return Err("Image file is too large".to_string());
        }
        let cursor = std::io::Cursor::new(data);
        let reader = ImageReader::new(cursor)
            .with_guessed_format()
            .map_err(|e| format!("Unable to detect image format: {}", e))?;
        let (src_w, src_h) = reader
            .into_dimensions()
            .map_err(|e| format!("Unable to read image dimensions: {}", e))?;
        if src_w == 0 || src_h == 0 {
            return Err("Image has invalid dimensions".to_string());
        }
        if (src_w as u64).saturating_mul(src_h as u64) > Self::MAX_DECODED_PIXELS {
            return Err("Image dimensions are too large".to_string());
        }
        let decoded =
            image::load_from_memory(data).map_err(|e| format!("Unable to decode image: {}", e))?;
        let resized = decoded.thumbnail(max_width.max(1), max_height.max(1));
        let gray = resized.to_luma8();
        let (width, height) = gray.dimensions();
        if width == 0 || height == 0 {
            return Err("Image decode returned empty frame".to_string());
        }
        let pixels = gray.into_raw();
        if pixels.is_empty() {
            return Err("Image decode returned no pixels".to_string());
        }
        let threshold = Self::adaptive_threshold(&pixels);
        Ok(Self {
            width,
            height,
            pixels,
            threshold,
        })
    }

    #[cfg(target_os = "espidf")]
    fn from_path(path: &str, max_width: u32, max_height: u32) -> Result<Self, String> {
        let decoded = image::open(path).map_err(|e| format!("Unable to decode image: {}", e))?;
        let (src_w, src_h) = image::GenericImageView::dimensions(&decoded);
        if src_w == 0 || src_h == 0 {
            return Err("Image has invalid dimensions".to_string());
        }
        if (src_w as u64).saturating_mul(src_h as u64) > Self::MAX_DECODED_PIXELS {
            return Err("Image dimensions are too large".to_string());
        }
        let resized = decoded.thumbnail(max_width.max(1), max_height.max(1));
        let gray = resized.to_luma8();
        let (width, height) = gray.dimensions();
        if width == 0 || height == 0 {
            return Err("Image decode returned empty frame".to_string());
        }
        let pixels = gray.into_raw();
        if pixels.is_empty() {
            return Err("Image decode returned no pixels".to_string());
        }
        let threshold = Self::adaptive_threshold(&pixels);
        Ok(Self {
            width,
            height,
            pixels,
            threshold,
        })
    }

    fn adaptive_threshold(pixels: &[u8]) -> u8 {
        if pixels.is_empty() {
            return 128;
        }
        let sum: u64 = pixels.iter().map(|px| *px as u64).sum();
        let avg = (sum / pixels.len() as u64) as i32;
        avg.clamp(78, 178) as u8
    }

    fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        title: &str,
    ) -> Result<(), D::Error> {
        let size = display.bounding_box().size;
        let width = size.width.min(size.height);
        let height = size.width.max(size.height);
        let header_h = 24i32;
        let footer_h = 18i32;
        let y_top = header_h;
        let y_bottom = (height as i32 - footer_h).max(y_top + 1);
        let content_h = (y_bottom - y_top).max(1);

        display.clear(BinaryColor::Off)?;
        let header_style = MonoTextStyle::new(ui_font_small(), BinaryColor::On);
        let footer_style = MonoTextStyle::new(ui_font_small(), BinaryColor::On);
        let title =
            FileBrowserActivity::truncate_to_px(title, width as i32 - 2 * layout::GAP_SM, 12);
        Text::new(&title, Point::new(layout::GAP_SM, 14), header_style).draw(display)?;
        Rectangle::new(Point::new(0, header_h), Size::new(width, 1))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;

        let offset_x = ((width as i32 - self.width as i32).max(0)) / 2;
        let offset_y = y_top + ((content_h - self.height as i32).max(0)) / 2;
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = (y * self.width + x) as usize;
                let luma = self.pixels[idx];
                let color = if luma < self.threshold {
                    BinaryColor::On
                } else {
                    BinaryColor::Off
                };
                Pixel(Point::new(offset_x + x as i32, offset_y + y as i32), color).draw(display)?;
            }
        }

        Rectangle::new(Point::new(0, y_bottom), Size::new(width, 1))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;
        Text::new(
            "Back: Close",
            Point::new(layout::GAP_SM, height as i32 - 4),
            footer_style,
        )
        .draw(display)?;
        Ok(())
    }
}

#[cfg(feature = "std")]
#[derive(Clone)]
struct EpubTocItem {
    chapter_index: usize,
    depth: usize,
    label: String,
    status: EpubTocStatus,
    start_percent: u8,
}

#[cfg(feature = "std")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum EpubTocStatus {
    Done,
    Current,
    Upcoming,
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
        collapsed: BTreeSet<usize>,
    },
    JumpLocation {
        chapter: usize,
        page: usize,
    },
    JumpGlobalLocation {
        location: usize,
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
enum EpubOpenWorkerEvent {
    Phase(&'static str),
    Done(Result<Arc<Mutex<EpubReadingState>>, String>),
}

#[cfg(all(feature = "std", not(target_os = "espidf")))]
struct PendingEpubOpen {
    receiver: Receiver<EpubOpenWorkerEvent>,
}

#[cfg(feature = "std")]
struct PendingEpubNavigation {
    receiver: Receiver<Result<EpubNavigationOutcome, String>>,
    direction: EpubNavigationDirection,
}

#[cfg(feature = "std")]
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
    #[cfg(target_os = "espidf")]
    source_path: Option<String>,
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
    cover_image_sources: BTreeSet<String>,
    cover_image_bitmap: Option<InlineImageBitmap>,
    inline_image_cache: BTreeMap<String, InlineImageBitmap>,
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
    pending_task: Option<PendingBrowserTask>,
    browser_task_epoch: u32,
    return_to_previous_on_back: bool,
    #[cfg(all(feature = "std", target_os = "espidf"))]
    pending_epub_initial_load: bool,
    #[cfg(all(feature = "std", not(target_os = "espidf")))]
    epub_open_pending: Option<PendingEpubOpen>,
    #[cfg(all(feature = "std", not(target_os = "espidf")))]
    epub_open_started_tick: Option<u32>,
    #[cfg(feature = "std")]
    epub_navigation_pending: Option<PendingEpubNavigation>,
    #[cfg(feature = "std")]
    epub_navigation_started_tick: Option<u32>,
    #[cfg(feature = "std")]
    epub_failure_streak: u8,
    #[cfg(feature = "std")]
    epub_failure_window_start_tick: u32,
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

    #[cfg(feature = "std")]
    fn toc_visible_indices(items: &[EpubTocItem], collapsed: &BTreeSet<usize>) -> Vec<usize> {
        let mut visible = Vec::with_capacity(items.len());
        let mut hidden_depth: Option<usize> = None;
        for (idx, item) in items.iter().enumerate() {
            if let Some(depth) = hidden_depth {
                if item.depth > depth {
                    continue;
                }
                hidden_depth = None;
            }
            visible.push(idx);
            if collapsed.contains(&idx) {
                hidden_depth = Some(item.depth);
            }
        }
        visible
    }

    #[cfg(feature = "std")]
    fn toc_has_children(items: &[EpubTocItem], idx: usize) -> bool {
        let Some(current) = items.get(idx) else {
            return false;
        };
        items
            .get(idx + 1)
            .is_some_and(|next| next.depth > current.depth)
    }

    fn exit_reader_to_browser(&mut self) -> ActivityResult {
        #[cfg(feature = "std")]
        {
            self.persist_active_epub_position();
        }
        #[cfg(all(feature = "std", not(target_os = "espidf")))]
        {
            self.epub_open_pending = None;
            self.epub_open_started_tick = None;
        }
        #[cfg(feature = "std")]
        {
            self.epub_navigation_pending = None;
            self.epub_navigation_started_tick = None;
            self.reset_epub_failure_streak();
        }
        self.mode = BrowserMode::Browsing;
        self.invalidate_browser_tasks();
        #[cfg(feature = "std")]
        {
            self.epub_overlay = None;
            self.active_epub_path = None;
            #[cfg(not(target_os = "espidf"))]
            {
                self.epub_open_started_tick = None;
            }
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
        if matches!(event, InputEvent::Press(Button::Back))
            && !matches!(overlay, EpubOverlay::Finished)
        {
            self.epub_overlay = None;
            return ActivityResult::Consumed;
        }

        let mut navigate_settings = false;
        let mut exit_to_files = false;
        let mut put_back = true;
        match &mut self.mode {
            BrowserMode::ReadingEpub { renderer } => match &mut overlay {
                EpubOverlay::QuickMenu { selected } => {
                    const COUNT: usize = 9;
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
                                        let current_idx = items
                                            .iter()
                                            .position(|item| item.status == EpubTocStatus::Current)
                                            .unwrap_or(0);
                                        let visible = 6usize;
                                        let initial_scroll =
                                            current_idx.saturating_sub(visible / 2);
                                        overlay = EpubOverlay::Toc {
                                            items,
                                            selected: current_idx,
                                            scroll: initial_scroll,
                                            collapsed: BTreeSet::new(),
                                        };
                                    }
                                }
                                2 => {
                                    let guard = match renderer.lock() {
                                        Ok(guard) => guard,
                                        Err(poisoned) => poisoned.into_inner(),
                                    };
                                    overlay = EpubOverlay::JumpLocation {
                                        chapter: guard.current_chapter(),
                                        page: guard.current_page_number(),
                                    };
                                }
                                3 => {
                                    let guard = match renderer.lock() {
                                        Ok(guard) => guard,
                                        Err(poisoned) => poisoned.into_inner(),
                                    };
                                    overlay = EpubOverlay::JumpGlobalLocation {
                                        location: guard.current_global_location(),
                                    };
                                }
                                4 => {
                                    let guard = match renderer.lock() {
                                        Ok(guard) => guard,
                                        Err(poisoned) => poisoned.into_inner(),
                                    };
                                    overlay = EpubOverlay::JumpPercent {
                                        percent: guard.book_progress_percent(),
                                    };
                                }
                                5 => {
                                    navigate_settings = true;
                                    put_back = false;
                                }
                                6 => {
                                    self.reader_settings.footer_density =
                                        self.reader_settings.footer_density.next_wrapped();
                                    self.set_reader_settings(self.reader_settings);
                                }
                                7 => {
                                    self.reader_settings.footer_auto_hide =
                                        self.reader_settings.footer_auto_hide.next_wrapped();
                                    self.set_reader_settings(self.reader_settings);
                                }
                                8 => {
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
                    collapsed,
                } => {
                    let mut visible_rows = Self::toc_visible_indices(items, collapsed);
                    if visible_rows.is_empty() {
                        return ActivityResult::Consumed;
                    }
                    let selected_visible = visible_rows
                        .iter()
                        .position(|idx| *idx == *selected)
                        .unwrap_or(0);
                    match event {
                        InputEvent::Press(Button::Up) | InputEvent::Press(Button::VolumeUp) => {
                            let next_visible = if selected_visible == 0 {
                                visible_rows.len().saturating_sub(1)
                            } else {
                                selected_visible - 1
                            };
                            *selected = visible_rows[next_visible];
                        }
                        InputEvent::Press(Button::Down) | InputEvent::Press(Button::VolumeDown) => {
                            let next_visible = (selected_visible + 1) % visible_rows.len();
                            *selected = visible_rows[next_visible];
                        }
                        InputEvent::Press(Button::Left) => {
                            if collapsed.remove(selected) {
                                visible_rows = Self::toc_visible_indices(items, collapsed);
                            } else if Self::toc_has_children(items, *selected) {
                                collapsed.insert(*selected);
                                visible_rows = Self::toc_visible_indices(items, collapsed);
                            } else if let Some(current) = items.get(*selected) {
                                if let Some((parent_idx, _)) =
                                    items.iter().enumerate().rev().find(|(idx, item)| {
                                        *idx < *selected && item.depth < current.depth
                                    })
                                {
                                    *selected = parent_idx;
                                }
                            }
                        }
                        InputEvent::Press(Button::Right) => {
                            if collapsed.remove(selected) {
                                visible_rows = Self::toc_visible_indices(items, collapsed);
                            } else if Self::toc_has_children(items, *selected) {
                                let current_depth = items[*selected].depth;
                                if let Some((idx, _)) =
                                    items.iter().enumerate().find(|(idx, item)| {
                                        *idx > *selected && item.depth > current_depth
                                    })
                                {
                                    *selected = idx;
                                }
                            }
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
                    let selected_visible = visible_rows
                        .iter()
                        .position(|idx| *idx == *selected)
                        .unwrap_or(0);
                    if selected_visible < *scroll {
                        *scroll = selected_visible;
                    } else if selected_visible >= *scroll + visible {
                        *scroll = selected_visible.saturating_sub(visible - 1);
                    }
                }
                EpubOverlay::JumpLocation { chapter, page } => match event {
                    InputEvent::Press(Button::Up) | InputEvent::Press(Button::VolumeUp) => {
                        let guard = match renderer.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        let max_chapters = guard.total_chapters().max(1);
                        *chapter = chapter.saturating_add(1).min(max_chapters);
                        *page = 1;
                    }
                    InputEvent::Press(Button::Down) | InputEvent::Press(Button::VolumeDown) => {
                        *chapter = chapter.saturating_sub(1).max(1);
                        *page = 1;
                    }
                    InputEvent::Press(Button::Left) => {
                        *page = page.saturating_sub(1).max(1);
                    }
                    InputEvent::Press(Button::Right) => {
                        let guard = match renderer.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        let chapter_zero = chapter.saturating_sub(1);
                        let max_pages = guard.estimated_pages_for_chapter(chapter_zero).max(1);
                        *page = page.saturating_add(1).min(max_pages);
                    }
                    InputEvent::Press(Button::Confirm) => {
                        let mut guard = match renderer.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        let max_chapters = guard.total_chapters().max(1);
                        *chapter = (*chapter).clamp(1, max_chapters);
                        let target_chapter = chapter.saturating_sub(1);
                        let max_pages = guard.estimated_pages_for_chapter(target_chapter).max(1);
                        *page = (*page).clamp(1, max_pages);
                        if !guard.restore_position(target_chapter, page.saturating_sub(1)) {
                            self.browser.set_status_message(
                                "Unable to jump to selected location".to_string(),
                            );
                        }
                        put_back = false;
                    }
                    _ => {}
                },
                EpubOverlay::JumpGlobalLocation { location } => match event {
                    InputEvent::Press(Button::Left) => {
                        *location = location.saturating_sub(1).max(1);
                    }
                    InputEvent::Press(Button::Right) => {
                        let guard = match renderer.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        let max_location = guard.total_book_locations().max(1);
                        *location = location.saturating_add(1).min(max_location);
                    }
                    InputEvent::Press(Button::Down) | InputEvent::Press(Button::VolumeDown) => {
                        *location = location.saturating_sub(10).max(1);
                    }
                    InputEvent::Press(Button::Up) | InputEvent::Press(Button::VolumeUp) => {
                        let guard = match renderer.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        let max_location = guard.total_book_locations().max(1);
                        *location = location.saturating_add(10).min(max_location);
                    }
                    InputEvent::Press(Button::Confirm) => {
                        let mut guard = match renderer.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        let max_location = guard.total_book_locations().max(1);
                        *location = (*location).clamp(1, max_location);
                        if !guard.jump_to_global_location(*location) {
                            self.browser.set_status_message(
                                "Unable to jump to selected location".to_string(),
                            );
                        }
                        put_back = false;
                    }
                    _ => {}
                },
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
    #[cfg(feature = "std")]
    const EPUB_OPEN_WORKER_STACK_BYTES: usize = if cfg!(target_os = "espidf") {
        56 * 1024
    } else {
        2 * 1024 * 1024
    };
    #[cfg(feature = "std")]
    const EPUB_NAV_WORKER_STACK_BYTES: usize = if cfg!(target_os = "espidf") {
        48 * 1024
    } else {
        512 * 1024
    };
    #[cfg(feature = "std")]
    const EPUB_OPEN_TIMEOUT_TICKS: u32 = if cfg!(target_os = "espidf") {
        2400 // ~120s at 50ms loop delay
    } else {
        900 // ~45s at 50ms loop delay
    };
    #[cfg(feature = "std")]
    const EPUB_OPEN_HEARTBEAT_TICKS: u32 = 20; // ~1s at 50ms loop delay
    #[cfg(feature = "std")]
    const EPUB_NAV_TIMEOUT_TICKS: u32 = if cfg!(target_os = "espidf") {
        240 // ~12s
    } else {
        120 // ~6s
    };
    #[cfg(feature = "std")]
    const EPUB_FAILURE_STREAK_LIMIT: u8 = 3;
    #[cfg(feature = "std")]
    const EPUB_FAILURE_WINDOW_TICKS: u32 = 600; // ~30s

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
            browser_task_epoch: 1,
            return_to_previous_on_back: false,
            #[cfg(all(feature = "std", target_os = "espidf"))]
            pending_epub_initial_load: false,
            #[cfg(all(feature = "std", not(target_os = "espidf")))]
            epub_open_pending: None,
            #[cfg(all(feature = "std", not(target_os = "espidf")))]
            epub_open_started_tick: None,
            #[cfg(feature = "std")]
            epub_navigation_pending: None,
            #[cfg(feature = "std")]
            epub_navigation_started_tick: None,
            #[cfg(feature = "std")]
            epub_failure_streak: 0,
            #[cfg(feature = "std")]
            epub_failure_window_start_tick: 0,
        }
    }

    pub fn current_path(&self) -> &str {
        self.browser.current_path()
    }

    pub fn is_viewing_text(&self) -> bool {
        matches!(self.mode, BrowserMode::ReadingText { .. })
    }

    pub fn is_viewing_image(&self) -> bool {
        #[cfg(feature = "std")]
        {
            matches!(self.mode, BrowserMode::ViewingImage { .. })
        }

        #[cfg(not(feature = "std"))]
        {
            false
        }
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

    pub(crate) fn has_pending_task(&self) -> bool {
        self.pending_task.is_some()
    }

    #[cfg(feature = "std")]
    pub(crate) fn has_epub_runtime_work(&self) -> bool {
        #[cfg(target_os = "espidf")]
        {
            self.pending_epub_initial_load || self.epub_navigation_pending.is_some()
        }
        #[cfg(not(target_os = "espidf"))]
        {
            self.epub_open_pending.is_some() || self.epub_navigation_pending.is_some()
        }
    }

    #[cfg(not(feature = "std"))]
    pub(crate) fn has_epub_runtime_work(&self) -> bool {
        false
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

    #[cfg(feature = "std")]
    fn reset_epub_failure_streak(&mut self) {
        self.epub_failure_streak = 0;
        self.epub_failure_window_start_tick = self.ui_tick;
    }

    #[cfg(feature = "std")]
    fn handle_epub_runtime_failure(&mut self, message: String) {
        let now = self.ui_tick;
        if self.epub_failure_streak == 0
            || now.saturating_sub(self.epub_failure_window_start_tick)
                > Self::EPUB_FAILURE_WINDOW_TICKS
        {
            self.epub_failure_streak = 0;
            self.epub_failure_window_start_tick = now;
        }
        self.epub_failure_streak = self.epub_failure_streak.saturating_add(1);

        if self.epub_failure_streak >= Self::EPUB_FAILURE_STREAK_LIMIT {
            log::warn!(
                "[EPUB] safe fallback after repeated failures; last error: {}",
                message
            );
            self.epub_failure_streak = 0;
            self.epub_failure_window_start_tick = now;
            self.epub_overlay = None;
            #[cfg(target_os = "espidf")]
            {
                self.pending_epub_initial_load = false;
            }
            #[cfg(not(target_os = "espidf"))]
            {
                self.epub_open_pending = None;
                self.epub_open_started_tick = None;
            }
            self.epub_navigation_pending = None;
            self.epub_navigation_started_tick = None;
            self.active_epub_path = None;
            self.mode = BrowserMode::Browsing;
            self.invalidate_browser_tasks();
            self.browser
                .set_status_message("Reader recovered from repeated errors".to_string());
            return;
        }

        self.browser.set_status_message(message);
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

    /// Returns current EPUB overall progress percent (0-100).
    pub fn epub_book_progress_percent(&self) -> Option<u8> {
        #[cfg(feature = "std")]
        {
            if let BrowserMode::ReadingEpub { renderer } = &self.mode {
                let renderer = match renderer.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                return Some(renderer.book_progress_percent());
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
        #[cfg(feature = "std")]
        let mut updated = self.poll_epub_open_result();
        #[cfg(all(feature = "std", target_os = "espidf"))]
        {
            updated |= self.process_pending_epub_initial_load();
        }
        #[cfg(feature = "std")]
        {
            updated |= self.poll_epub_navigation_result();
        }
        #[cfg(not(feature = "std"))]
        let mut updated = false;

        let Some(pending) = self.pending_task.take() else {
            return updated;
        };

        if pending.epoch != self.browser_task_epoch {
            return updated;
        }

        let task_updated = match pending.task {
            FileBrowserTask::LoadCurrentDirectory => self.process_load_current_directory_task(fs),
            FileBrowserTask::OpenPath { path } => self.process_open_path_task(fs, &path),
            FileBrowserTask::OpenTextFile { path } => self.process_open_text_file_task(fs, &path),
            FileBrowserTask::OpenImageFile { path } => self.process_open_image_file_task(fs, &path),
            FileBrowserTask::OpenAnyFile { path } => self.process_open_any_file_task(fs, &path),
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

    #[cfg(feature = "std")]
    #[inline(never)]
    fn process_open_image_file_task(&mut self, fs: &mut dyn FileSystem, path: &str) -> bool {
        let title = basename(path).to_string();
        let max_w = crate::DISPLAY_WIDTH;
        let max_h = crate::DISPLAY_HEIGHT.saturating_sub(44).max(1);
        #[cfg(target_os = "espidf")]
        let mut path_error: Option<String> = None;

        #[cfg(target_os = "espidf")]
        if let Some(host_path) = Self::resolve_host_backed_image_path(path) {
            match ImageViewer::from_path(&host_path, max_w, max_h) {
                Ok(viewer) => {
                    self.mode = BrowserMode::ViewingImage { title, viewer };
                    return true;
                }
                Err(error) => {
                    path_error = Some(error);
                }
            }
        }

        let bytes = match fs.read_file_bytes(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.browser
                    .set_status_message(format!("Unable to open image: {}", error));
                self.mode = BrowserMode::Browsing;
                return true;
            }
        };
        match ImageViewer::from_bytes(&bytes, max_w, max_h) {
            Ok(viewer) => {
                self.mode = BrowserMode::ViewingImage { title, viewer };
            }
            Err(error) => {
                #[cfg(target_os = "espidf")]
                if let Some(path_error) = path_error {
                    self.browser.set_status_message(format!(
                        "Unable to decode image: {} / fallback: {}",
                        path_error, error
                    ));
                    self.mode = BrowserMode::Browsing;
                    return true;
                }
                self.browser
                    .set_status_message(format!("Unable to decode image: {}", error));
                self.mode = BrowserMode::Browsing;
            }
        }
        true
    }

    #[cfg(not(feature = "std"))]
    #[inline(never)]
    fn process_open_image_file_task(&mut self, _fs: &mut dyn FileSystem, path: &str) -> bool {
        self.mode = BrowserMode::Browsing;
        self.browser
            .set_status_message(format!("Unsupported file type: {}", basename(path)));
        true
    }

    #[inline(never)]
    fn process_open_any_file_task(&mut self, fs: &mut dyn FileSystem, path: &str) -> bool {
        const PREVIEW_MAX_BYTES: usize = 256 * 1024;
        let title = basename(path).to_string();
        let content = match fs.read_file(path) {
            Ok(text) => text,
            Err(_) => match fs.read_file_bytes(path) {
                Ok(bytes) => {
                    let truncated = if bytes.len() > PREVIEW_MAX_BYTES {
                        &bytes[..PREVIEW_MAX_BYTES]
                    } else {
                        &bytes[..]
                    };
                    let mut text = String::from_utf8_lossy(truncated).to_string();
                    if bytes.len() > PREVIEW_MAX_BYTES {
                        text.push_str("\n\n[File preview truncated]");
                    }
                    text
                }
                Err(error) => {
                    self.browser
                        .set_status_message(format!("Unable to open file: {}", error));
                    self.mode = BrowserMode::Browsing;
                    return true;
                }
            },
        };
        self.mode = BrowserMode::ReadingText {
            title,
            viewer: TextViewer::new(content),
        };
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

    fn invalidate_browser_tasks(&mut self) {
        self.browser_task_epoch = self.browser_task_epoch.wrapping_add(1);
        self.pending_task = None;
        #[cfg(all(feature = "std", target_os = "espidf"))]
        {
            self.pending_epub_initial_load = false;
        }
    }

    fn queue_task(&mut self, task: FileBrowserTask) {
        self.pending_task = Some(PendingBrowserTask {
            epoch: self.browser_task_epoch,
            task,
        });
    }

    fn queue_load_current_directory(&mut self) {
        self.queue_task(FileBrowserTask::LoadCurrentDirectory);
    }

    pub fn request_open_path(&mut self, path: impl Into<String>) {
        self.mode = BrowserMode::Browsing;
        self.return_to_previous_on_back = true;
        let path = path.into();
        if Self::is_text_file(&path) || Self::is_epub_file(&path) || Self::is_image_file(&path) {
            self.mode = BrowserMode::Browsing;
            // Open directly without first loading parent directory, so
            // library-open doesn't flash filesystem browser UI.
            self.queue_open_file(path);
        } else {
            self.mode = BrowserMode::Browsing;
            self.queue_task(FileBrowserTask::OpenPath { path });
        }
    }

    fn queue_open_file(&mut self, path: String) {
        if Self::is_text_file(&path) {
            self.queue_task(FileBrowserTask::OpenTextFile { path });
        } else if Self::is_image_file(&path) {
            self.queue_task(FileBrowserTask::OpenImageFile { path });
        } else if cfg!(feature = "std") && Self::is_epub_file(&path) {
            self.queue_task(FileBrowserTask::OpenEpubFile { path });
        } else {
            self.queue_task(FileBrowserTask::OpenAnyFile { path });
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

    fn is_image_file(path: &str) -> bool {
        let lower = path.to_lowercase();
        lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".png")
            || lower.ends_with(".bmp")
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
            BrowserMode::ViewingImage { .. } => ActivityResult::Ignored,
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
                    match action {
                        EpubInputAction::Page(direction) => {
                            if self.epub_navigation_pending.is_some() {
                                return ActivityResult::Consumed;
                            }
                            self.last_epub_interaction_tick = self.ui_tick;
                            let renderer = Arc::clone(renderer);
                            match Self::spawn_epub_navigation_worker(renderer, direction) {
                                Ok(receiver) => {
                                    self.epub_navigation_pending = Some(PendingEpubNavigation {
                                        receiver,
                                        direction,
                                    });
                                    self.epub_navigation_started_tick = Some(self.ui_tick);
                                    ActivityResult::Consumed
                                }
                                Err(error) => {
                                    self.handle_epub_runtime_failure(error);
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
                            } else {
                                drop(renderer);
                                self.reset_epub_failure_streak();
                                self.last_epub_interaction_tick = self.ui_tick;
                                return ActivityResult::Consumed;
                            }
                            ActivityResult::Consumed
                        }
                        EpubInputAction::OpenSettings => ActivityResult::Ignored,
                    }
                } else {
                    ActivityResult::Ignored
                }
            }
            #[cfg(feature = "std")]
            BrowserMode::OpeningEpub => {
                if matches!(event, InputEvent::Press(Button::Back)) {
                    #[cfg(not(target_os = "espidf"))]
                    {
                        self.epub_open_pending = None;
                        self.epub_open_started_tick = None;
                    }
                    self.epub_navigation_pending = None;
                    self.epub_navigation_started_tick = None;
                    self.mode = BrowserMode::Browsing;
                    self.browser
                        .set_status_message("Canceled EPUB open".to_string());
                }
                ActivityResult::Consumed
            }
            BrowserMode::Browsing => ActivityResult::Ignored,
        }
    }
}

impl Activity for FileBrowserActivity {
    fn on_enter(&mut self) {
        self.mode = BrowserMode::Browsing;
        self.invalidate_browser_tasks();
        #[cfg(feature = "std")]
        {
            self.epub_overlay = None;
            self.active_epub_path = None;
        }
        #[cfg(all(feature = "std", not(target_os = "espidf")))]
        {
            self.epub_open_pending = None;
            self.epub_open_started_tick = None;
        }
        #[cfg(feature = "std")]
        {
            self.epub_navigation_pending = None;
            self.epub_navigation_started_tick = None;
            self.reset_epub_failure_streak();
        }
        self.queue_load_current_directory();
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
        self.invalidate_browser_tasks();
        self.return_to_previous_on_back = false;
        #[cfg(all(feature = "std", not(target_os = "espidf")))]
        {
            self.epub_open_pending = None;
        }
        #[cfg(feature = "std")]
        {
            self.epub_navigation_pending = None;
            self.epub_navigation_started_tick = None;
            self.reset_epub_failure_streak();
        }
    }

    fn handle_input(&mut self, event: InputEvent) -> ActivityResult {
        if self.is_viewing_text()
            || self.is_viewing_image()
            || self.is_viewing_epub()
            || self.is_opening_epub()
        {
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

            if Self::is_text_file(&path) || Self::is_epub_file(&path) || Self::is_image_file(&path)
            {
                self.queue_open_file(path);
                return ActivityResult::Consumed;
            }
            self.queue_open_file(path);
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
            BrowserMode::OpeningEpub => self.render_opening_epub(display),
            BrowserMode::ReadingText { title, viewer } => viewer.render(display, title),
            #[cfg(feature = "std")]
            BrowserMode::ViewingImage { title, viewer } => viewer.render(display, title),
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
    fn render_opening_epub<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        self.browser.render(display)
    }

    #[cfg(all(feature = "std", target_os = "espidf"))]
    fn resolve_host_backed_image_path(path: &str) -> Option<String> {
        let mut candidates: Vec<String> = Vec::new();
        candidates.push(path.to_string());
        if path.starts_with('/') {
            candidates.push(format!("/sd{}", path));
        } else {
            candidates.push(format!("/sd/{}", path));
        }
        candidates
            .into_iter()
            .find(|candidate| std::fs::File::open(candidate).is_ok())
    }

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
        let panel_w = (width - layout::OVERLAY_PANEL_MARGIN * 2).max(120);
        let panel_h = (height - 110).max(120);
        let panel_x = (width - panel_w) / 2;
        let panel_y = ((height - panel_h) / 2 - layout::GAP_SM).max(layout::GAP_SM);
        Rectangle::new(
            Point::new(
                panel_x - layout::OVERLAY_BORDER_PAD,
                panel_y - layout::OVERLAY_BORDER_PAD,
            ),
            Size::new(
                (panel_w + layout::OVERLAY_BORDER_PAD * 2) as u32,
                (panel_h + layout::OVERLAY_BORDER_PAD * 2) as u32,
            ),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
        .draw(display)?;
        Rectangle::new(
            Point::new(panel_x, panel_y),
            Size::new(panel_w as u32, panel_h as u32),
        )
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display)?;

        let title_style = MonoTextStyle::new(ui_font_title(), BinaryColor::On);
        let body_style = MonoTextStyle::new(ui_font_body(), BinaryColor::On);
        let hint_style = MonoTextStyle::new(ui_font_small(), BinaryColor::On);

        match overlay {
            EpubOverlay::QuickMenu { selected } => {
                Text::new(
                    "Reader Menu",
                    Point::new(panel_x + layout::GAP_SM, panel_y + layout::OVERLAY_TITLE_Y),
                    title_style,
                )
                .draw(display)?;
                let items = [
                    "Resume",
                    "Table of Contents",
                    "Go to Chapter/Page",
                    "Go to Location",
                    "Go to Position",
                    "Reader Settings",
                    "Footer: ",
                    "Footer Hide: ",
                    "Back to Files",
                ];
                for (i, item) in items.iter().enumerate() {
                    let y =
                        panel_y + layout::OVERLAY_CONTENT_Y + (i as i32 * layout::OVERLAY_ROW_H);
                    let label = match i {
                        6 => format!("{}{}", item, self.reader_settings.footer_density.label()),
                        7 => format!("{}{}", item, self.reader_settings.footer_auto_hide.label()),
                        _ => item.to_string(),
                    };
                    if i == *selected {
                        Rectangle::new(
                            Point::new(
                                panel_x + layout::OVERLAY_SELECT_INSET,
                                y - layout::OVERLAY_ROW_H + layout::OVERLAY_SELECT_INSET,
                            ),
                            Size::new(
                                (panel_w - layout::OVERLAY_SELECT_INSET * 2) as u32,
                                layout::OVERLAY_ROW_H as u32,
                            ),
                        )
                        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                        .draw(display)?;
                        Text::new(
                            &label,
                            Point::new(panel_x + layout::INNER_PAD, y),
                            MonoTextStyle::new(ui_font_body(), BinaryColor::Off),
                        )
                        .draw(display)?;
                    } else {
                        Text::new(
                            &label,
                            Point::new(panel_x + layout::INNER_PAD, y),
                            MonoTextStyle::new(ui_font_body(), BinaryColor::On),
                        )
                        .draw(display)?;
                    }
                }
                Text::new(
                    "U/D Move  OK Select  Back Close",
                    Point::new(
                        panel_x + layout::GAP_SM,
                        panel_y + panel_h - layout::OVERLAY_HINT_BOTTOM,
                    ),
                    hint_style,
                )
                .draw(display)?;
            }
            EpubOverlay::Toc {
                items,
                selected,
                scroll,
                collapsed,
            } => {
                Text::new(
                    "Table of Contents",
                    Point::new(panel_x + layout::GAP_SM, panel_y + layout::OVERLAY_TITLE_Y),
                    title_style,
                )
                .draw(display)?;
                let visible_indices = Self::toc_visible_indices(items, collapsed);
                let visible = 6usize;
                for row in 0..visible {
                    let visible_idx = scroll + row;
                    if visible_idx >= visible_indices.len() {
                        break;
                    }
                    let idx = visible_indices[visible_idx];
                    let item = &items[idx];
                    let y =
                        panel_y + layout::OVERLAY_CONTENT_Y + (row as i32 * layout::OVERLAY_ROW_H);
                    let indent = (item.depth.min(4) as i32) * layout::INNER_PAD;
                    let status = match item.status {
                        EpubTocStatus::Done => "✓",
                        EpubTocStatus::Current => ">",
                        EpubTocStatus::Upcoming => " ",
                    };
                    let pct = format!("{}%", item.start_percent);
                    let pct_x = panel_x + panel_w - 48;
                    let label_x = panel_x + layout::INNER_PAD + indent;
                    let label_max_px = (pct_x - layout::GAP_SM - label_x).max(16);
                    let collapse_mark = if Self::toc_has_children(items, idx) {
                        if collapsed.contains(&idx) {
                            "+"
                        } else {
                            "-"
                        }
                    } else {
                        " "
                    };
                    let label = Self::truncate_to_px(
                        &format!("{}{} {}", status, collapse_mark, item.label),
                        label_max_px,
                        ui_font_body().character_size.height,
                    );
                    if idx == *selected {
                        Rectangle::new(
                            Point::new(
                                panel_x + layout::OVERLAY_SELECT_INSET,
                                y - layout::OVERLAY_ROW_H + layout::OVERLAY_SELECT_INSET + 1,
                            ),
                            Size::new(
                                (panel_w - layout::OVERLAY_SELECT_INSET * 2) as u32,
                                (layout::OVERLAY_ROW_H - 2) as u32,
                            ),
                        )
                        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                        .draw(display)?;
                        Text::new(
                            &label,
                            Point::new(label_x, y),
                            MonoTextStyle::new(ui_font_body(), BinaryColor::Off),
                        )
                        .draw(display)?;
                        Text::new(
                            &pct,
                            Point::new(pct_x, y),
                            MonoTextStyle::new(ui_font_small(), BinaryColor::Off),
                        )
                        .draw(display)?;
                    } else {
                        Text::new(&label, Point::new(label_x, y), body_style).draw(display)?;
                        Text::new(&pct, Point::new(pct_x, y), hint_style).draw(display)?;
                    }
                }
                let selected_visible = visible_indices
                    .iter()
                    .position(|idx| *idx == *selected)
                    .map(|v| v + 1)
                    .unwrap_or(1);
                let pos = format!("{}/{}", selected_visible, visible_indices.len().max(1));
                Text::new(
                    &pos,
                    Point::new(panel_x + panel_w - 72, panel_y + layout::OVERLAY_TITLE_Y),
                    body_style,
                )
                .draw(display)?;
                Text::new(
                    "U/D Move  L/R Fold  OK Jump  Back",
                    Point::new(
                        panel_x + layout::GAP_SM,
                        panel_y + panel_h - layout::OVERLAY_HINT_BOTTOM,
                    ),
                    hint_style,
                )
                .draw(display)?;
            }
            EpubOverlay::JumpLocation { chapter, page } => {
                Text::new(
                    "Go to Chapter/Page",
                    Point::new(panel_x + layout::GAP_SM, panel_y + layout::OVERLAY_TITLE_Y),
                    title_style,
                )
                .draw(display)?;
                let chapter_text = format!("Chapter {}", chapter);
                let page_text = format!("Page {}", page);
                Text::new(
                    &chapter_text,
                    Point::new(
                        panel_x + layout::INNER_PAD,
                        panel_y + layout::OVERLAY_CONTENT_Y,
                    ),
                    body_style,
                )
                .draw(display)?;
                Text::new(
                    &page_text,
                    Point::new(
                        panel_x + layout::INNER_PAD,
                        panel_y + layout::OVERLAY_CONTENT_Y + layout::OVERLAY_ROW_H,
                    ),
                    body_style,
                )
                .draw(display)?;
                Text::new(
                    "U/D Chapter  L/R Page  OK Jump",
                    Point::new(
                        panel_x + layout::GAP_SM,
                        panel_y + panel_h - layout::OVERLAY_HINT_BOTTOM - layout::GAP_SM,
                    ),
                    hint_style,
                )
                .draw(display)?;
                Text::new(
                    "Vol +/- chapter  Back Cancel",
                    Point::new(
                        panel_x + layout::GAP_SM,
                        panel_y + panel_h - layout::OVERLAY_HINT_BOTTOM + 8,
                    ),
                    hint_style,
                )
                .draw(display)?;
            }
            EpubOverlay::JumpGlobalLocation { location } => {
                Text::new(
                    "Go to Location",
                    Point::new(panel_x + layout::GAP_SM, panel_y + layout::OVERLAY_TITLE_Y),
                    title_style,
                )
                .draw(display)?;
                let location_text = format!("Location {}", location);
                Text::new(
                    &location_text,
                    Point::new(
                        panel_x + layout::INNER_PAD,
                        panel_y + layout::OVERLAY_CONTENT_Y,
                    ),
                    body_style,
                )
                .draw(display)?;
                Text::new(
                    "L/R +/-1  U/D +/-10  OK Jump",
                    Point::new(
                        panel_x + layout::GAP_SM,
                        panel_y + panel_h - layout::OVERLAY_HINT_BOTTOM - layout::GAP_SM,
                    ),
                    hint_style,
                )
                .draw(display)?;
                Text::new(
                    "Back Cancel",
                    Point::new(
                        panel_x + layout::GAP_SM,
                        panel_y + panel_h - layout::OVERLAY_HINT_BOTTOM + 8,
                    ),
                    hint_style,
                )
                .draw(display)?;
            }
            EpubOverlay::JumpPercent { percent } => {
                Text::new(
                    "Go to Position",
                    Point::new(panel_x + layout::GAP_SM, panel_y + layout::OVERLAY_TITLE_Y),
                    title_style,
                )
                .draw(display)?;
                let pct = format!("{}%", percent);
                Text::new(
                    &pct,
                    Point::new(
                        panel_x + panel_w / 2 - 28,
                        panel_y + layout::OVERLAY_CONTENT_Y + layout::GAP_SM,
                    ),
                    MonoTextStyle::new(ui_font_title(), BinaryColor::On),
                )
                .draw(display)?;

                let bar_x = panel_x + layout::OVERLAY_PANEL_MARGIN;
                let bar_y =
                    panel_y + layout::OVERLAY_CONTENT_Y + layout::OVERLAY_ROW_H + layout::INNER_PAD;
                let bar_w = panel_w - layout::OVERLAY_PANEL_MARGIN * 2;
                Rectangle::new(
                    Point::new(bar_x, bar_y),
                    Size::new(bar_w as u32, layout::OVERLAY_BAR_H as u32),
                )
                .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                .draw(display)?;
                let fill = (bar_w.saturating_sub(4) * (*percent as i32)) / 100;
                if fill > 0 {
                    Rectangle::new(
                        Point::new(bar_x + 2, bar_y + 2),
                        Size::new(fill as u32, (layout::OVERLAY_BAR_H - 4) as u32),
                    )
                    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                    .draw(display)?;
                }
                let chapter = format!("Now: {}%", renderer.book_progress_percent());
                Text::new(
                    &chapter,
                    Point::new(bar_x, bar_y + layout::OVERLAY_BAR_H + layout::GAP_MD),
                    body_style,
                )
                .draw(display)?;
                Text::new(
                    "L/R 1%  U/D 10%  OK Jump  Back",
                    Point::new(
                        panel_x + layout::GAP_SM,
                        panel_y + panel_h - layout::OVERLAY_HINT_BOTTOM,
                    ),
                    hint_style,
                )
                .draw(display)?;
            }
            EpubOverlay::Finished => {
                Text::new(
                    "Finished",
                    Point::new(panel_x + layout::GAP_SM, panel_y + layout::OVERLAY_TITLE_Y),
                    title_style,
                )
                .draw(display)?;
                Text::new(
                    "You reached the end of this book.",
                    Point::new(
                        panel_x + layout::INNER_PAD,
                        panel_y + layout::OVERLAY_CONTENT_Y + 2,
                    ),
                    body_style,
                )
                .draw(display)?;
                Text::new(
                    "Confirm/Back Exit  Any Nav Continue",
                    Point::new(
                        panel_x + layout::GAP_SM,
                        panel_y + panel_h - layout::OVERLAY_HINT_BOTTOM,
                    ),
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
    use image::codecs::jpeg::JpegEncoder;
    use image::codecs::png::PngEncoder;
    use image::{ColorType, ImageEncoder};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use std::time::Instant;

    fn tiny_jpeg() -> Vec<u8> {
        let rgb = [0u8, 0, 0, 255, 255, 255, 200, 200, 200, 64, 64, 64];
        let mut out = Vec::new();
        let mut encoder = JpegEncoder::new_with_quality(&mut out, 80);
        encoder
            .encode(&rgb, 2, 2, image::ExtendedColorType::Rgb8)
            .expect("jpeg encode should succeed");
        out
    }

    fn tiny_png() -> Vec<u8> {
        let rgba = [
            0u8, 0, 0, 255, 255, 255, 255, 255, 200, 200, 200, 255, 64, 64, 64, 255,
        ];
        let mut out = Vec::new();
        let encoder = PngEncoder::new(&mut out);
        encoder
            .write_image(&rgba, 2, 2, ColorType::Rgba8.into())
            .expect("png encode should succeed");
        out
    }

    fn create_fs() -> MockFileSystem {
        let mut fs = MockFileSystem::empty();
        fs.add_directory("/");
        fs.add_directory("/docs");
        fs.add_file("/docs/readme.txt", b"hello");
        fs.add_file("/docs/image.jpg", &tiny_jpeg());
        fs.add_file("/docs/image.png", &tiny_png());
        fs.add_file("/docs/blob.bin", b"\x00\x01\x02\x03\xFF\xFE");
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
    fn image_file_opens_image_viewer() {
        let mut activity = FileBrowserActivity::new();
        let mut fs = create_fs();
        activity.on_enter();
        activity.request_open_path("/docs/image.jpg");
        let _ = drain_pending_tasks(&mut activity, &mut fs);
        assert!(activity.is_viewing_image());
    }

    #[test]
    fn png_file_opens_image_viewer() {
        let mut activity = FileBrowserActivity::new();
        let mut fs = create_fs();
        activity.on_enter();
        activity.request_open_path("/docs/image.png");
        let _ = drain_pending_tasks(&mut activity, &mut fs);
        assert!(activity.is_viewing_image());
    }

    #[test]
    fn unknown_file_opens_text_preview() {
        let mut activity = FileBrowserActivity::new();
        let mut fs = create_fs();
        activity.on_enter();
        activity.request_open_path("/docs/blob.bin");
        let _ = drain_pending_tasks(&mut activity, &mut fs);
        assert!(activity.is_viewing_text());
    }

    #[test]
    fn back_returns_to_previous_screen_after_library_open_fallback() {
        let mut activity = FileBrowserActivity::new();
        let mut fs = create_fs();
        activity.on_enter();
        assert!(activity.process_pending_task(&mut fs));

        // Simulate library-initiated open of image file.
        activity.request_open_path("/docs/image.jpg");
        assert!(activity.process_pending_task(&mut fs));
        assert!(activity.is_viewing_image());

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
    fn finished_overlay_back_exits_epub_reader() {
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
        assert!(activity.process_pending_task(&mut fs));
        let start = Instant::now();
        while activity.is_opening_epub() && start.elapsed() < Duration::from_secs(20) {
            let _ = activity.process_pending_task(&mut fs);
            thread::sleep(Duration::from_millis(1));
        }
        assert!(activity.is_viewing_epub());

        activity.epub_overlay = Some(EpubOverlay::Finished);
        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert!(matches!(
            result,
            ActivityResult::Consumed | ActivityResult::NavigateBack
        ));
        assert!(!activity.is_viewing_epub());
        assert!(activity.epub_overlay.is_none());
    }

    #[test]
    fn opening_epub_back_cancels_pending_open() {
        let mut activity = FileBrowserActivity::new();
        let (_tx, rx) = mpsc::channel::<EpubOpenWorkerEvent>();
        activity.mode = BrowserMode::OpeningEpub;
        activity.epub_open_pending = Some(PendingEpubOpen { receiver: rx });
        activity.epub_open_started_tick = Some(1);

        let result = activity.handle_input(InputEvent::Press(Button::Back));
        assert_eq!(result, ActivityResult::Consumed);
        assert!(!activity.is_opening_epub());
        assert!(activity.epub_open_pending.is_none());
        assert!(activity.epub_open_started_tick.is_none());
    }

    #[test]
    fn process_pending_task_ignores_stale_task_epoch() {
        let mut activity = FileBrowserActivity::new();
        let mut fs = create_fs();
        activity.on_enter();
        assert!(activity.process_pending_task(&mut fs));

        activity.mode = BrowserMode::ReadingText {
            title: "sample".to_string(),
            viewer: TextViewer::new("body".to_string()),
        };
        let stale_epoch = activity.browser_task_epoch;
        activity.pending_task = Some(PendingBrowserTask {
            epoch: stale_epoch,
            task: FileBrowserTask::LoadCurrentDirectory,
        });
        activity.browser_task_epoch = activity.browser_task_epoch.wrapping_add(1);

        assert!(!activity.process_pending_task(&mut fs));
        assert!(activity.is_viewing_text());
    }

    #[test]
    fn opening_epub_times_out_when_worker_stalls() {
        let mut activity = FileBrowserActivity::new();
        let (_tx, rx) = mpsc::channel::<EpubOpenWorkerEvent>();
        activity.mode = BrowserMode::OpeningEpub;
        activity.epub_open_pending = Some(PendingEpubOpen { receiver: rx });
        activity.epub_open_started_tick = Some(0);
        activity.ui_tick = FileBrowserActivity::EPUB_OPEN_TIMEOUT_TICKS + 1;

        assert!(activity.poll_epub_open_result());
        assert!(!activity.is_opening_epub());
        assert!(activity.epub_open_pending.is_none());
        assert!(activity.epub_open_started_tick.is_none());
    }

    #[test]
    fn repeated_epub_runtime_failures_trigger_safe_fallback() {
        let mut activity = FileBrowserActivity::new();
        activity.mode = BrowserMode::OpeningEpub;
        activity.active_epub_path = Some("/books/failing.epub".to_string());
        activity.epub_overlay = Some(EpubOverlay::Finished);
        activity.epub_failure_window_start_tick = 0;
        activity.ui_tick = 10;

        for idx in 0..FileBrowserActivity::EPUB_FAILURE_STREAK_LIMIT {
            activity.handle_epub_runtime_failure(format!("failure-{idx}"));
            activity.ui_tick = activity.ui_tick.saturating_add(1);
        }

        assert!(matches!(activity.mode, BrowserMode::Browsing));
        assert!(activity.active_epub_path.is_none());
        assert!(activity.epub_overlay.is_none());
        assert!(activity
            .status_message()
            .is_some_and(|msg| msg.contains("recovered")));
    }

    #[test]
    fn epub_navigation_timeout_sets_status_and_clears_pending() {
        let mut activity = FileBrowserActivity::new();
        let (_tx, rx) = mpsc::channel::<Result<EpubNavigationOutcome, String>>();
        activity.epub_navigation_pending = Some(PendingEpubNavigation {
            receiver: rx,
            direction: EpubNavigationDirection::Next,
        });
        activity.epub_navigation_started_tick = Some(0);
        activity.ui_tick = FileBrowserActivity::EPUB_NAV_TIMEOUT_TICKS + 1;

        assert!(activity.poll_epub_navigation_result());
        assert!(activity.epub_navigation_pending.is_none());
        assert!(activity.epub_navigation_started_tick.is_none());
        assert!(activity
            .status_message()
            .is_some_and(|msg| msg.contains("Unable to change EPUB page")));
    }

    #[test]
    fn toc_visible_indices_hide_descendants_of_collapsed_rows() {
        let items = vec![
            EpubTocItem {
                chapter_index: 0,
                depth: 0,
                label: "A".to_string(),
                status: EpubTocStatus::Current,
                start_percent: 0,
            },
            EpubTocItem {
                chapter_index: 1,
                depth: 1,
                label: "A.1".to_string(),
                status: EpubTocStatus::Upcoming,
                start_percent: 10,
            },
            EpubTocItem {
                chapter_index: 2,
                depth: 2,
                label: "A.1.a".to_string(),
                status: EpubTocStatus::Upcoming,
                start_percent: 20,
            },
            EpubTocItem {
                chapter_index: 3,
                depth: 0,
                label: "B".to_string(),
                status: EpubTocStatus::Upcoming,
                start_percent: 30,
            },
        ];
        let mut collapsed = BTreeSet::new();
        collapsed.insert(0usize);
        let visible = FileBrowserActivity::toc_visible_indices(&items, &collapsed);
        assert_eq!(visible, vec![0, 3]);
    }

    #[test]
    fn toc_has_children_detects_next_deeper_item() {
        let items = vec![
            EpubTocItem {
                chapter_index: 0,
                depth: 0,
                label: "A".to_string(),
                status: EpubTocStatus::Current,
                start_percent: 0,
            },
            EpubTocItem {
                chapter_index: 1,
                depth: 1,
                label: "A.1".to_string(),
                status: EpubTocStatus::Upcoming,
                start_percent: 10,
            },
            EpubTocItem {
                chapter_index: 2,
                depth: 0,
                label: "B".to_string(),
                status: EpubTocStatus::Upcoming,
                start_percent: 30,
            },
        ];
        assert!(FileBrowserActivity::toc_has_children(&items, 0));
        assert!(!FileBrowserActivity::toc_has_children(&items, 1));
        assert!(!FileBrowserActivity::toc_has_children(&items, 2));
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
