use super::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use mu_epub::book::Locator;
use mu_epub::RenderPrepOptions;
#[cfg(feature = "std")]
use std::io::Cursor;
#[cfg(feature = "std")]
use std::sync::mpsc::{self, TryRecvError};
#[cfg(feature = "std")]
use std::thread;

#[cfg(feature = "std")]
#[derive(Clone)]
struct PersistedEpubState {
    chapter_idx: usize,
    page_idx: usize,
    chapter_counts: Vec<(usize, usize)>,
}

#[cfg(feature = "std")]
impl EpubReadingState {
    fn forced_font_family_for_settings(settings: ReaderSettings) -> Option<String> {
        let family = match settings.font_family {
            FontFamily::Serif => "serif",
            FontFamily::SansSerif => "sans-serif",
            FontFamily::Monospace => "monospace",
        };
        Some(family.to_string())
    }

    fn apply_common_render_config<'a>(&self, mut config: RenderConfig<'a>) -> RenderConfig<'a> {
        #[cfg(target_os = "espidf")]
        {
            config = config.with_embedded_fonts(false);
        }
        if let Some(family) = self.forced_font_family.as_ref() {
            config = config.with_forced_font_family(family.clone());
        }
        config
    }

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
    const MAX_CHAPTER_BUF_CAPACITY_BYTES: usize = 64 * 1024;
    #[cfg(not(target_os = "espidf"))]
    const MAX_CHAPTER_BUF_CAPACITY_BYTES: usize = 512 * 1024;
    const MAX_CHAPTER_BUF_GROW_RETRIES: usize = 8;
    const PAGE_LOAD_MAX_RETRIES: usize = 2;
    #[cfg(target_os = "espidf")]
    const EPUB_TEMP_DIR: &'static str = "/sd/.tmp";
    #[cfg(target_os = "espidf")]
    #[allow(dead_code)]
    const PAGE_CACHE_LIMIT: usize = 0;
    #[cfg(not(target_os = "espidf"))]
    const PAGE_CACHE_LIMIT: usize = 8;
    const OUT_OF_RANGE_ERR: &'static str = "Requested EPUB page is out of range";
    #[cfg(target_os = "espidf")]
    const INLINE_IMAGE_CACHE_LIMIT: usize = 8;
    #[cfg(not(target_os = "espidf"))]
    const INLINE_IMAGE_CACHE_LIMIT: usize = 24;
    #[cfg(target_os = "espidf")]
    const INLINE_IMAGE_SOURCE_MAX_BYTES: usize = 512 * 1024;
    #[cfg(not(target_os = "espidf"))]
    const INLINE_IMAGE_SOURCE_MAX_BYTES: usize = 4 * 1024 * 1024;
    #[cfg(target_os = "espidf")]
    const INLINE_IMAGE_MAX_DECODED_PIXELS: u64 = 2_000_000;
    #[cfg(not(target_os = "espidf"))]
    const INLINE_IMAGE_MAX_DECODED_PIXELS: u64 = 8_000_000;
    #[cfg(target_os = "espidf")]
    const INLINE_IMAGE_PREFETCH_PER_PAGE: usize = 1;
    #[cfg(not(target_os = "espidf"))]
    const INLINE_IMAGE_PREFETCH_PER_PAGE: usize = 8;

    fn create_render_options(settings: ReaderSettings) -> (RenderEngineOptions, RenderPrepOptions) {
        let mut opts = RenderEngineOptions::for_display(
            crate::DISPLAY_WIDTH as i32,
            crate::DISPLAY_HEIGHT as i32,
        );
        let mut layout = opts.layout;
        let side_margin = match settings.margin_size {
            crate::reader_settings_activity::MarginSize::Small => 8,
            crate::reader_settings_activity::MarginSize::Medium => 12,
            crate::reader_settings_activity::MarginSize::Large => 18,
        };
        layout.margin_left = side_margin;
        layout.margin_right = side_margin;
        // Keep a safety band at the top so ascenders/diacritics never clip on page starts.
        layout.margin_top = 14;
        layout.margin_bottom =
            (EPUB_FOOTER_HEIGHT + EPUB_FOOTER_BOTTOM_PADDING + EPUB_FOOTER_TOP_GAP).max(24);
        layout.first_line_indent_px = 0;
        layout.paragraph_gap_px = match settings.line_spacing {
            crate::reader_settings_activity::LineSpacing::Compact => 6,
            crate::reader_settings_activity::LineSpacing::Normal => 8,
            crate::reader_settings_activity::LineSpacing::Relaxed => 10,
        };
        layout.line_gap_px = match settings.line_spacing {
            crate::reader_settings_activity::LineSpacing::Compact => 1,
            crate::reader_settings_activity::LineSpacing::Normal => 2,
            crate::reader_settings_activity::LineSpacing::Relaxed => 4,
        };
        layout.typography.justification.enabled = matches!(
            settings.text_alignment,
            crate::reader_settings_activity::TextAlignment::Justified
        );
        layout.typography.hyphenation.soft_hyphen_policy = HyphenationMode::Discretionary;
        layout.typography.justification.strategy = if layout.typography.justification.enabled {
            JustificationStrategy::AdaptiveInterWord
        } else {
            JustificationStrategy::AlignLeft
        };
        layout.object_layout.cover_page_mode = CoverPageMode::Contain;
        layout.typography.justification.min_words = 4;
        layout.typography.justification.min_fill_ratio = 0.70;
        layout.typography.justification.max_space_stretch_ratio = 0.40;
        opts.layout = layout;

        let base_font = settings.font_size.epub_base_px();
        let text_scale = settings.font_size.epub_text_scale();
        let mut hints = opts.prep.layout_hints;
        hints.base_font_size_px = base_font;
        hints.text_scale = text_scale;
        hints.min_font_size_px = (base_font * 0.75).max(14.0);
        hints.max_font_size_px = (base_font * 2.4).min(80.0);
        match settings.line_spacing {
            crate::reader_settings_activity::LineSpacing::Compact => {
                hints.min_line_height = 1.05;
                hints.max_line_height = 1.20;
            }
            crate::reader_settings_activity::LineSpacing::Normal => {
                hints.min_line_height = 1.15;
                hints.max_line_height = 1.30;
            }
            crate::reader_settings_activity::LineSpacing::Relaxed => {
                hints.min_line_height = 1.25;
                hints.max_line_height = 1.45;
            }
        }
        opts.prep.layout_hints = hints;
        opts.prep.style.hints = hints;
        #[cfg(target_os = "espidf")]
        {
            opts.prep.fonts.max_faces = 8;
            opts.prep.fonts.max_bytes_per_font = 512 * 1024;
            opts.prep.fonts.max_total_font_bytes = 2 * 1024 * 1024;
            opts.prep.memory.max_entry_bytes = 2 * 1024 * 1024;
            opts.prep.memory.max_css_bytes = 256 * 1024;
            opts.prep.memory.max_nav_bytes = Self::MAX_NAV_BYTES;
        }
        let chapter_events_opts = opts.prep;
        (opts, chapter_events_opts)
    }

    fn create_engine(settings: ReaderSettings) -> (RenderEngine, RenderPrepOptions) {
        let (opts, chapter_events_opts) = Self::create_render_options(settings);
        (RenderEngine::new(opts), chapter_events_opts)
    }

    #[cfg(not(target_os = "espidf"))]
    pub(super) fn from_reader(
        reader: Box<dyn ReadSeek>,
        settings: ReaderSettings,
    ) -> Result<Self, String> {
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
        let (engine, chapter_events_opts) = Self::create_engine(settings);
        #[cfg(feature = "fontdue")]
        let (eg_renderer, layout_text_measurer) = Self::create_renderer();
        #[cfg(not(feature = "fontdue"))]
        let eg_renderer = Self::create_renderer();
        let mut state = Self {
            #[cfg(target_os = "espidf")]
            source_path: None,
            book,
            engine,
            chapter_events_opts,
            eg_renderer,
            #[cfg(feature = "fontdue")]
            layout_text_measurer,
            chapter_buf: Vec::with_capacity(Self::CHAPTER_BUF_CAPACITY_BYTES),
            chapter_scratch: ScratchBuffers::embedded(),
            current_page: None,
            page_cache: BTreeMap::new(),
            #[cfg(not(target_os = "espidf"))]
            render_cache: InMemoryRenderCache::default(),
            chapter_page_counts: BTreeMap::new(),
            chapter_page_counts_exact: BTreeSet::new(),
            non_renderable_chapters: BTreeSet::new(),
            forced_font_family: Self::forced_font_family_for_settings(settings),
            cover_image_sources: BTreeSet::new(),
            cover_image_bitmap: None,
            inline_image_cache: BTreeMap::new(),
            chapter_idx: 0,
            page_idx: 0,
            last_next_page_reached_end: false,
        };
        state.initialize_cover_image_cache();
        state.register_embedded_fonts();
        state.load_chapter_forward(0)?;
        log::info!("[EPUB] initial chapter/page loaded");
        Ok(state)
    }

    #[cfg(target_os = "espidf")]
    pub(super) fn from_sd_path_light(path: &str, settings: ReaderSettings) -> Result<Self, String> {
        log::debug!("[EPUB] opening reader (sd temp)");
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
        std::fs::create_dir_all(Self::EPUB_TEMP_DIR).map_err(|e| {
            format!(
                "Unable to create EPUB temp dir ({}): {}",
                Self::EPUB_TEMP_DIR,
                e
            )
        })?;
        let book = EpubBook::open_with_temp_storage(path, Self::EPUB_TEMP_DIR, open_cfg)
            .map_err(|e| format!("Unable to parse EPUB: {}", e))?;
        let (engine, chapter_events_opts) = Self::create_engine(settings);
        #[cfg(feature = "fontdue")]
        let (eg_renderer, layout_text_measurer) = Self::create_renderer();
        #[cfg(not(feature = "fontdue"))]
        let eg_renderer = Self::create_renderer();
        let mut state = Self {
            #[cfg(target_os = "espidf")]
            source_path: Some(path.to_string()),
            book,
            engine,
            chapter_events_opts,
            eg_renderer,
            #[cfg(feature = "fontdue")]
            layout_text_measurer,
            // Keep EPUB open deterministic on constrained heaps: defer large
            // working buffer allocations until first page load.
            chapter_buf: Vec::new(),
            chapter_scratch: ScratchBuffers {
                read_buf: Vec::new(),
                xml_buf: Vec::new(),
                text_buf: String::new(),
            },
            current_page: None,
            page_cache: BTreeMap::new(),
            #[cfg(not(target_os = "espidf"))]
            render_cache: InMemoryRenderCache::default(),
            chapter_page_counts: BTreeMap::new(),
            chapter_page_counts_exact: BTreeSet::new(),
            non_renderable_chapters: BTreeSet::new(),
            forced_font_family: Self::forced_font_family_for_settings(settings),
            cover_image_sources: BTreeSet::new(),
            cover_image_bitmap: None,
            inline_image_cache: BTreeMap::new(),
            chapter_idx: 0,
            page_idx: 0,
            last_next_page_reached_end: false,
        };
        state.initialize_cover_image_cache();
        state.register_embedded_fonts();
        Ok(state)
    }

    #[cfg(target_os = "espidf")]
    pub(super) fn ensure_initial_page_loaded(&mut self) -> Result<(), String> {
        if self.current_page.is_some() {
            return Ok(());
        }
        self.load_chapter_forward(0)?;
        log::info!("[EPUB] initial chapter/page loaded");
        Ok(())
    }

    fn load_chapter_exact(&mut self, chapter_idx: usize) -> Result<(), String> {
        log::debug!("[EPUB] load_chapter_exact idx={}", chapter_idx);
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
                Err(err) if Self::is_non_renderable_chapter_error(&err) => {
                    log::warn!(
                        "[EPUB] skipping chapter idx={} due to non-renderable error: {}",
                        idx,
                        err
                    );
                    self.non_renderable_chapters.insert(idx);
                    continue;
                }
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
                Err(err) if Self::is_non_renderable_chapter_error(&err) => {
                    log::warn!(
                        "[EPUB] skipping chapter idx={} due to non-renderable error: {}",
                        idx,
                        err
                    );
                    self.non_renderable_chapters.insert(idx as usize);
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

    fn is_non_renderable_chapter_error(err: &str) -> bool {
        Self::is_out_of_range_error(err)
            || err.contains("Unable to allocate EPUB chapter buffer")
            || err.contains("chapter buffer capped")
    }

    fn is_buffer_too_small_error(err: &str) -> bool {
        err.to_ascii_lowercase().contains("buffer too small")
    }

    fn grow_chapter_buffer(&mut self) -> Result<bool, String> {
        let current = self.chapter_buf.capacity();
        if current >= Self::MAX_CHAPTER_BUF_CAPACITY_BYTES {
            return Ok(false);
        }

        let next = current
            .max(Self::CHAPTER_BUF_CAPACITY_BYTES)
            .saturating_mul(2)
            .min(Self::MAX_CHAPTER_BUF_CAPACITY_BYTES);
        if next <= current {
            return Ok(false);
        }

        // `try_reserve` is relative to current length, not current capacity.
        // Reserve enough so effective total capacity can reach `next`.
        let len = self.chapter_buf.len();
        let additional = next.saturating_sub(len);
        self.chapter_buf.try_reserve(additional).map_err(|_| {
            format!(
                "Unable to allocate EPUB chapter buffer (requested {} bytes)",
                next
            )
        })?;
        let grown_to = self.chapter_buf.capacity();
        if grown_to <= current {
            return Ok(false);
        }
        log::warn!(
            "[EPUB] grew chapter buffer from {} to {} bytes",
            current,
            grown_to
        );
        Ok(true)
    }

    pub(super) fn current_chapter(&self) -> usize {
        let skipped_before = self
            .non_renderable_chapters
            .iter()
            .filter(|idx| **idx < self.chapter_idx)
            .count();
        Self::compute_current_chapter(self.chapter_idx, skipped_before)
    }

    pub(super) fn total_chapters(&self) -> usize {
        self.book
            .chapter_count()
            .saturating_sub(self.non_renderable_chapters.len())
            .max(1)
    }

    pub(super) fn current_page_number(&self) -> usize {
        self.page_idx + 1
    }

    pub(super) fn total_pages(&self) -> usize {
        self.chapter_page_counts
            .get(&self.chapter_idx)
            .copied()
            .unwrap_or_else(|| self.current_page_number().max(1))
    }

    pub(super) fn page_progress_label(&self) -> String {
        let current = self.current_page_number();
        let total = self.total_pages().max(current);
        Self::format_page_progress_label(
            current,
            total,
            self.chapter_page_counts_exact.contains(&self.chapter_idx),
        )
    }

    pub(super) fn chapter_progress_label(&self) -> String {
        Self::format_chapter_progress_label(self.current_chapter(), self.total_chapters())
    }

    pub(super) fn exact_chapter_page_counts(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for chapter_idx in self.chapter_page_counts_exact.iter().copied() {
            if let Some(count) = self.chapter_page_counts.get(&chapter_idx).copied() {
                out.push((chapter_idx, count.max(1)));
            }
        }
        out
    }

    pub(super) fn apply_exact_chapter_page_counts(&mut self, counts: &[(usize, usize)]) {
        for (chapter_idx, count) in counts.iter().copied() {
            if chapter_idx >= self.book.chapter_count() {
                continue;
            }
            let normalized = count.max(1);
            self.chapter_page_counts.insert(chapter_idx, normalized);
            self.chapter_page_counts_exact.insert(chapter_idx);
        }
    }

    #[cfg(not(target_os = "espidf"))]
    pub(super) fn prewarm_next_page(&mut self) {
        let chapter = self.chapter_idx;
        let next_page = self.page_idx.saturating_add(1);
        let _ = self.load_page_with_retries(chapter, next_page, 1);
    }

    pub(super) fn take_last_next_page_reached_end(&mut self) -> bool {
        core::mem::take(&mut self.last_next_page_reached_end)
    }

    pub(super) fn position_indices(&self) -> (usize, usize) {
        (self.chapter_idx, self.page_idx)
    }

    pub(super) fn book_progress_percent(&self) -> u8 {
        let (pages_before, current_pages, total_pages, on_final_exact_page) =
            self.estimated_global_page_metrics();
        Self::compute_book_progress_percent_from_pages(
            pages_before,
            self.page_idx,
            current_pages,
            total_pages,
            on_final_exact_page,
        )
    }

    pub(super) fn current_global_location(&self) -> usize {
        let (pages_before, current_pages, _total_pages, _on_final_exact_page) =
            self.estimated_global_page_metrics();
        let clamped = self.page_idx.min(current_pages.saturating_sub(1));
        pages_before.saturating_add(clamped).saturating_add(1)
    }

    pub(super) fn total_book_locations(&self) -> usize {
        let (_, _, total_pages, _) = self.estimated_global_page_metrics();
        total_pages.max(1)
    }

    pub(super) fn jump_to_global_location(&mut self, location: usize) -> bool {
        let target = location.saturating_sub(1);
        let Some((chapter_idx, page_idx)) = self.locate_from_global_location(target) else {
            return false;
        };
        self.restore_position(chapter_idx, page_idx)
    }

    fn compute_default_page_estimate(sum: usize, count: usize) -> usize {
        if count == 0 {
            8
        } else {
            (sum / count).clamp(1, 256)
        }
    }

    fn chapter_weight_from_counts(known_pages: Option<usize>, default_estimate: usize) -> usize {
        known_pages.unwrap_or(default_estimate).max(1)
    }

    fn default_chapter_page_estimate(&self) -> usize {
        let mut sum = 0usize;
        let mut count = 0usize;
        for chapter_idx in 0..self.book.chapter_count() {
            if self.non_renderable_chapters.contains(&chapter_idx) {
                continue;
            }
            if let Some(pages) = self.chapter_page_counts.get(&chapter_idx).copied() {
                sum = sum.saturating_add(pages.max(1));
                count = count.saturating_add(1);
            }
        }
        Self::compute_default_page_estimate(sum, count)
    }

    fn chapter_page_estimate_or_default(
        &self,
        chapter_idx: usize,
        default_estimate: usize,
    ) -> usize {
        Self::chapter_weight_from_counts(
            self.chapter_page_counts.get(&chapter_idx).copied(),
            default_estimate,
        )
    }

    pub(super) fn estimated_pages_for_chapter(&self, chapter_idx: usize) -> usize {
        if chapter_idx >= self.book.chapter_count() {
            return 1;
        }
        let fallback = self.default_chapter_page_estimate();
        self.chapter_page_estimate_or_default(chapter_idx, fallback)
            .max(1)
    }

    fn estimated_book_start_percent_for_chapter(&self, chapter_idx: usize) -> u8 {
        let fallback_pages = self.default_chapter_page_estimate();
        let mut pages_before = 0usize;
        let mut total_pages = 0usize;
        for idx in 0..self.book.chapter_count() {
            if self.non_renderable_chapters.contains(&idx) {
                continue;
            }
            let pages = self.chapter_page_estimate_or_default(idx, fallback_pages);
            if idx < chapter_idx {
                pages_before = pages_before.saturating_add(pages);
            }
            total_pages = total_pages.saturating_add(pages);
        }
        if total_pages == 0 {
            return 0;
        }
        ((pages_before as f32 / total_pages as f32) * 100.0).clamp(0.0, 100.0) as u8
    }

    fn estimated_global_page_metrics(&self) -> (usize, usize, usize, bool) {
        let mut pages_before = 0usize;
        let mut total_pages = 0usize;
        let mut current_pages = self.total_pages().max(1);
        let mut last_renderable = 0usize;
        let mut all_renderable_exact = true;
        let fallback_pages = self.default_chapter_page_estimate();
        for chapter_idx in 0..self.book.chapter_count() {
            if self.non_renderable_chapters.contains(&chapter_idx) {
                continue;
            }
            last_renderable = chapter_idx;
            let chapter_pages = self.chapter_page_estimate_or_default(chapter_idx, fallback_pages);
            let exact = self.chapter_page_counts_exact.contains(&chapter_idx);
            if !exact {
                all_renderable_exact = false;
            }
            if chapter_idx < self.chapter_idx {
                pages_before = pages_before.saturating_add(chapter_pages);
            }
            if chapter_idx == self.chapter_idx {
                current_pages = chapter_pages.max(self.current_page_number());
            }
            total_pages = total_pages.saturating_add(chapter_pages);
        }
        if total_pages == 0 {
            total_pages = 1;
            current_pages = 1;
        }
        let on_final_exact_page = all_renderable_exact
            && self.chapter_idx == last_renderable
            && self.chapter_page_counts_exact.contains(&self.chapter_idx)
            && self.current_page_number() >= current_pages;
        (
            pages_before,
            current_pages,
            total_pages,
            on_final_exact_page,
        )
    }

    fn locate_from_global_location(&self, target_global_page_idx: usize) -> Option<(usize, usize)> {
        let fallback_pages = self.default_chapter_page_estimate();
        let mut weighted = Vec::with_capacity(self.book.chapter_count());
        for chapter_idx in 0..self.book.chapter_count() {
            if self.non_renderable_chapters.contains(&chapter_idx) {
                continue;
            }
            let chapter_pages = self.chapter_page_estimate_or_default(chapter_idx, fallback_pages);
            weighted.push((chapter_idx, chapter_pages.max(1)));
        }
        Self::locate_in_weighted_pages(target_global_page_idx, &weighted)
    }

    fn locate_in_weighted_pages(
        target_global_page_idx: usize,
        weighted: &[(usize, usize)],
    ) -> Option<(usize, usize)> {
        let mut cursor = 0usize;
        let mut last: Option<(usize, usize)> = None;
        for (chapter_idx, units) in weighted.iter().copied() {
            let units = units.max(1);
            let chapter_end = cursor.saturating_add(units);
            if target_global_page_idx < chapter_end {
                return Some((chapter_idx, target_global_page_idx.saturating_sub(cursor)));
            }
            cursor = chapter_end;
            last = Some((chapter_idx, units.saturating_sub(1)));
        }
        last
    }

    fn compute_book_progress_percent_from_pages(
        pages_before_current_chapter: usize,
        page_idx_in_chapter: usize,
        current_chapter_pages: usize,
        total_book_pages_estimate: usize,
        on_final_exact_page: bool,
    ) -> u8 {
        if on_final_exact_page {
            return 100;
        }
        let total = total_book_pages_estimate.max(1);
        let current_chapter_pages = current_chapter_pages.max(1);
        let clamped_page_idx = page_idx_in_chapter.min(current_chapter_pages.saturating_sub(1));
        let global_page_idx = pages_before_current_chapter.saturating_add(clamped_page_idx);
        let pct = ((global_page_idx as f32 / total as f32) * 100.0).clamp(0.0, 99.0);
        pct as u8
    }

    fn compute_current_chapter(chapter_idx: usize, skipped_before: usize) -> usize {
        chapter_idx + 1 - skipped_before
    }

    fn format_page_progress_label(current: usize, total: usize, has_exact_total: bool) -> String {
        if has_exact_total {
            format!("p{}/{}", current, total.max(current))
        } else {
            format!("p{}", current)
        }
    }

    fn format_chapter_progress_label(current_chapter: usize, total_chapters: usize) -> String {
        format!("c{}/{}", current_chapter, total_chapters.max(1))
    }

    #[cfg(test)]
    fn compute_book_progress_percent_legacy(
        current_chapter: usize,
        total_chapters: usize,
        page_idx: usize,
        total_pages: usize,
        has_exact_current_total: bool,
    ) -> u8 {
        let total_chapters = total_chapters.max(1);
        let chapter_zero_based = current_chapter.saturating_sub(1).min(total_chapters - 1);
        let is_last_chapter = chapter_zero_based + 1 >= total_chapters;
        let total_pages = total_pages.max(1);
        let at_last_page = has_exact_current_total && page_idx + 1 >= total_pages;
        if is_last_chapter && at_last_page {
            return 100;
        }

        let page_portion = if has_exact_current_total {
            (page_idx as f32 / total_pages as f32) / total_chapters as f32
        } else {
            // Unknown chapter total: make progress monotonic while avoiding
            // overconfident percentages from temporary 1/1 placeholders.
            (page_idx as f32 / (page_idx + 2) as f32) / total_chapters as f32
        };
        let chapter_portion = chapter_zero_based as f32 / total_chapters as f32;
        ((chapter_portion + page_portion) * 100.0).clamp(0.0, 99.0) as u8
    }

    pub(super) fn current_chapter_title(&self, max_chars: usize) -> String {
        let fallback = format!("Chapter {}", self.current_chapter());
        if max_chars == 0 {
            return fallback;
        }
        let href = match self.book.chapter(self.chapter_idx) {
            Ok(chapter) => chapter.href,
            Err(_) => return fallback,
        };
        let href_key = Self::normalize_href_key(&href);

        let mut title = self
            .book
            .navigation()
            .and_then(|nav| {
                nav.toc_flat().into_iter().find_map(|(_, point)| {
                    let key = Self::normalize_href_key(&point.href);
                    if key == href_key {
                        Some(point.label.clone())
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| {
                href.rsplit('/')
                    .next()
                    .unwrap_or(href.as_str())
                    .split('#')
                    .next()
                    .unwrap_or(href.as_str())
                    .split('.')
                    .next()
                    .unwrap_or(href.as_str())
                    .replace(['_', '-'], " ")
            });

        title = Self::normalize_chapter_title_text(&title);
        if title.is_empty() {
            title = fallback;
        }
        let mut out = String::new();
        for (count, ch) in title.chars().enumerate() {
            if count + 1 >= max_chars {
                out.push('…');
                break;
            }
            out.push(ch);
        }
        if out.is_empty() {
            title
        } else {
            out
        }
    }

    fn normalize_href_key(href: &str) -> String {
        let mut key = href
            .split('#')
            .next()
            .unwrap_or(href)
            .rsplit('/')
            .next()
            .unwrap_or(href)
            .to_ascii_lowercase();
        if let Some(dot) = key.rfind('.') {
            key.truncate(dot);
        }
        key
    }

    fn normalize_chapter_title_text(title: &str) -> String {
        let mut out = String::with_capacity(title.len());
        let mut prev_ws = false;
        for ch in title.chars() {
            let normalized = match ch {
                '_' | '-' => ' ',
                c => c,
            };
            if normalized.is_whitespace() {
                if !prev_ws {
                    out.push(' ');
                }
                prev_ws = true;
            } else {
                out.push(normalized);
                prev_ws = false;
            }
        }
        out.trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace())
            .to_string()
    }

    pub(super) fn jump_to_chapter(&mut self, chapter_idx: usize) -> bool {
        if chapter_idx >= self.book.chapter_count() {
            return false;
        }
        let prev_chapter = self.chapter_idx;
        let prev_page = self.page_idx;
        self.current_page = None;
        if self.load_chapter_exact(chapter_idx).is_ok() {
            return true;
        }
        if let Ok(page) = self.load_page(prev_chapter, prev_page) {
            self.chapter_idx = prev_chapter;
            self.page_idx = prev_page;
            self.current_page = Some(page);
        }
        false
    }

    pub(super) fn jump_to_book_percent(&mut self, percent: u8) -> bool {
        let chapters = self.book.chapter_count();
        if chapters == 0 {
            return false;
        }

        // Prefer page-aware positioning. Unknown chapters use dynamic estimates
        // derived from observed chapter page counts to keep seek/progress stable.
        let fallback_pages = self.default_chapter_page_estimate();
        let mut weighted: Vec<(usize, usize)> = Vec::new();
        for chapter_idx in 0..chapters {
            if self.non_renderable_chapters.contains(&chapter_idx) {
                continue;
            }
            let weight = self.chapter_page_estimate_or_default(chapter_idx, fallback_pages);
            weighted.push((chapter_idx, weight));
        }
        if weighted.is_empty() {
            return false;
        }

        let Some((target_chapter, remaining, target_chapter_units)) =
            Self::select_target_from_weighted(percent, &weighted)
        else {
            return false;
        };

        // If this chapter has exact page counts, jump within the chapter too.
        if self.chapter_page_counts_exact.contains(&target_chapter) {
            let page_total = self
                .chapter_page_counts
                .get(&target_chapter)
                .copied()
                .unwrap_or(1)
                .max(1);
            let mut page_idx = (remaining * page_total) / target_chapter_units;
            if page_idx >= page_total {
                page_idx = page_total - 1;
            }
            return self.restore_position(target_chapter, page_idx);
        }

        self.jump_to_chapter(target_chapter)
    }

    fn select_target_from_weighted(
        percent: u8,
        weighted: &[(usize, usize)],
    ) -> Option<(usize, usize, usize)> {
        let &(first_chapter, first_units) = weighted.first()?;
        let total_units: usize = weighted.iter().map(|(_, w)| *w).sum::<usize>().max(1);
        let target_units = ((percent.min(100) as usize) * total_units) / 100;
        let mut remaining = target_units.min(total_units.saturating_sub(1));

        let mut target_chapter = first_chapter;
        let mut target_chapter_units = first_units.max(1);
        for (chapter_idx, units) in weighted.iter().copied() {
            let units = units.max(1);
            if remaining < units {
                target_chapter = chapter_idx;
                target_chapter_units = units;
                break;
            }
            remaining = remaining.saturating_sub(units);
            target_chapter = chapter_idx;
            target_chapter_units = units;
        }
        Some((target_chapter, remaining, target_chapter_units))
    }

    pub(super) fn restore_position(&mut self, chapter_idx: usize, page_idx: usize) -> bool {
        if chapter_idx >= self.book.chapter_count() {
            return false;
        }
        let mut target_page_idx = page_idx;
        if self.chapter_page_counts_exact.contains(&chapter_idx) {
            let chapter_total = self
                .chapter_page_counts
                .get(&chapter_idx)
                .copied()
                .unwrap_or(1)
                .max(1);
            target_page_idx = target_page_idx.min(chapter_total.saturating_sub(1));
        }
        let prev_chapter = self.chapter_idx;
        let prev_page = self.page_idx;
        self.current_page = None;

        if let Ok(page) = self.load_page(chapter_idx, target_page_idx) {
            self.chapter_idx = chapter_idx;
            self.page_idx = target_page_idx;
            self.current_page = Some(page);
            return true;
        }

        if self.load_chapter_exact(chapter_idx).is_ok() {
            // On-device memory is too constrained for linear page walks from 0..N
            // after a failed direct restore; this pattern can trigger OOM loops.
            #[cfg(not(target_os = "espidf"))]
            if target_page_idx > 0 {
                let mut idx = 1usize;
                while idx <= target_page_idx {
                    if let Ok(page) = self.load_page(chapter_idx, idx) {
                        self.page_idx = idx;
                        self.current_page = Some(page);
                        idx += 1;
                        continue;
                    }
                    break;
                }
            }
            return true;
        }

        if let Ok(page) = self.load_page(prev_chapter, prev_page) {
            self.chapter_idx = prev_chapter;
            self.page_idx = prev_page;
            self.current_page = Some(page);
        }
        false
    }

    pub(super) fn toc_items(&mut self) -> Vec<EpubTocItem> {
        let mut out = Vec::new();
        let current = self.chapter_idx;
        let mut flat_points: Vec<(usize, String, String)> = Vec::new();
        if let Ok(Some(nav)) = self.book.ensure_navigation() {
            for (depth, point) in nav.toc_flat() {
                if flat_points.len() >= 256 {
                    break;
                }
                flat_points.push((depth, point.href.clone(), point.label.clone()));
            }
        }
        if !flat_points.is_empty() {
            let mut session = self.book.reading_session();
            for (depth, href, label) in flat_points {
                if let Ok(loc) = session.resolve_locator(Locator::Href(href)) {
                    if out
                        .last()
                        .is_some_and(|prev: &EpubTocItem| prev.chapter_index == loc.chapter.index)
                    {
                        continue;
                    }
                    out.push(EpubTocItem {
                        chapter_index: loc.chapter.index,
                        depth,
                        label,
                        status: if loc.chapter.index < current {
                            EpubTocStatus::Done
                        } else if loc.chapter.index == current {
                            EpubTocStatus::Current
                        } else {
                            EpubTocStatus::Upcoming
                        },
                        start_percent: self
                            .estimated_book_start_percent_for_chapter(loc.chapter.index),
                    });
                }
            }
        }
        if out.is_empty() {
            for chapter in self.book.chapters() {
                out.push(EpubTocItem {
                    chapter_index: chapter.index,
                    depth: 0,
                    label: chapter.href,
                    status: if chapter.index < current {
                        EpubTocStatus::Done
                    } else if chapter.index == current {
                        EpubTocStatus::Current
                    } else {
                        EpubTocStatus::Upcoming
                    },
                    start_percent: self.estimated_book_start_percent_for_chapter(chapter.index),
                });
            }
        }
        out
    }

    pub(super) fn apply_reader_settings(&mut self, settings: ReaderSettings) -> Result<(), String> {
        let current_chapter = self.chapter_idx;
        let current_page = self.page_idx;
        let (engine, chapter_events_opts) = Self::create_engine(settings);
        self.engine = engine;
        self.chapter_events_opts = chapter_events_opts;
        self.forced_font_family = Self::forced_font_family_for_settings(settings);
        self.current_page = None;
        self.page_cache.clear();
        self.inline_image_cache.clear();
        self.chapter_page_counts.clear();
        self.chapter_page_counts_exact.clear();

        if let Ok(page) =
            self.load_page_with_retries(current_chapter, current_page, Self::PAGE_LOAD_MAX_RETRIES)
        {
            self.chapter_idx = current_chapter;
            self.page_idx = current_page;
            self.current_page = Some(page);
            return Ok(());
        }
        if self.load_chapter_forward(current_chapter).is_ok() {
            return Ok(());
        }
        self.load_chapter_backward(current_chapter.min(self.book.chapter_count().saturating_sub(1)))
    }

    pub(super) fn next_page(&mut self) -> bool {
        self.last_next_page_reached_end = false;
        let previous_chapter = self.chapter_idx;
        let previous_page = self.page_idx;
        // Free the currently rendered page before loading the next one to
        // maximize contiguous heap on constrained devices.
        self.current_page = None;
        let next_idx = self.page_idx + 1;
        let next_page_result =
            self.load_page_with_retries(self.chapter_idx, next_idx, Self::PAGE_LOAD_MAX_RETRIES);
        if let Ok(page) = next_page_result {
            self.page_idx = next_idx;
            self.current_page = Some(page);
            return true;
        }
        let next_page_err = next_page_result
            .err()
            .unwrap_or_else(|| "Unknown EPUB pagination error".to_string());
        let reached_end = if self.chapter_idx + 1 < self.book.chapter_count() {
            match self.load_chapter_forward(self.chapter_idx + 1) {
                Ok(()) => {
                    self.chapter_page_counts
                        .entry(previous_chapter)
                        .and_modify(|count| *count = (*count).max(previous_page + 1))
                        .or_insert(previous_page + 1);
                    self.chapter_page_counts_exact.insert(previous_chapter);
                    return true;
                }
                Err(err) => {
                    Self::is_out_of_range_error(&next_page_err)
                        && err == "No renderable pages found in remaining chapters"
                }
            }
        } else {
            Self::is_out_of_range_error(&next_page_err)
        };
        self.last_next_page_reached_end = reached_end;
        if reached_end {
            log::info!(
                "[EPUB] reached end of book at chapter={} page={}",
                previous_chapter,
                previous_page
            );
        }
        if let Ok(page) = self.load_page_with_retries(
            previous_chapter,
            previous_page,
            Self::PAGE_LOAD_MAX_RETRIES,
        ) {
            self.chapter_idx = previous_chapter;
            self.page_idx = previous_page;
            self.current_page = Some(page);
        }
        log::warn!(
            "[EPUB] next_page failed at chapter={} page={} err={}",
            previous_chapter,
            previous_page,
            next_page_err
        );
        false
    }

    pub(super) fn prev_page(&mut self) -> bool {
        let previous_chapter = self.chapter_idx;
        let previous_page = self.page_idx;
        // Free the currently rendered page before loading the previous one to
        // maximize contiguous heap on constrained devices.
        self.current_page = None;
        if self.page_idx > 0 {
            let prev_idx = self.page_idx - 1;
            if let Ok(page) =
                self.load_page_with_retries(self.chapter_idx, prev_idx, Self::PAGE_LOAD_MAX_RETRIES)
            {
                self.page_idx = prev_idx;
                self.current_page = Some(page);
                return true;
            }
        }
        if self.chapter_idx > 0 {
            // Walk backward until we find a renderable chapter. This handles
            // skipped/non-renderable chapters directly adjacent to the current one.
            let mut idx = self.chapter_idx as i32 - 1;
            while idx >= 0 {
                let candidate = idx as usize;
                match self.load_chapter_exact(candidate) {
                    Ok(()) => {
                        // `load_chapter_exact` loads page 0. Prefer jumping to the
                        // final page of this chapter. If exact totals are unknown,
                        // probe for the true last page index to avoid 0<->0 boundary loops.
                        let loaded_chapter = self.chapter_idx;
                        let mut last_page_idx = self
                            .chapter_page_counts
                            .get(&loaded_chapter)
                            .copied()
                            .unwrap_or(1)
                            .max(1)
                            .saturating_sub(1);
                        if !self.chapter_page_counts_exact.contains(&loaded_chapter) {
                            match self.discover_last_page_index(loaded_chapter) {
                                Ok(idx) => {
                                    last_page_idx = idx;
                                    self.chapter_page_counts.insert(loaded_chapter, idx + 1);
                                    self.chapter_page_counts_exact.insert(loaded_chapter);
                                }
                                Err(err) => {
                                    log::warn!(
                                        "[EPUB] prev_page unable to discover last page chapter={} err={}",
                                        loaded_chapter,
                                        err
                                    );
                                }
                            }
                        }
                        if last_page_idx == 0 {
                            return true;
                        }
                        self.page_idx = last_page_idx;
                        match self.load_page_with_retries(
                            loaded_chapter,
                            self.page_idx,
                            Self::PAGE_LOAD_MAX_RETRIES,
                        ) {
                            Ok(page) => {
                                self.current_page = Some(page);
                                return true;
                            }
                            Err(err) => {
                                // Persisted "exact" chapter totals can become stale after
                                // typography changes; downgrade to estimated and keep page 0.
                                if Self::is_out_of_range_error(&err) {
                                    self.chapter_page_counts_exact.remove(&loaded_chapter);
                                    self.chapter_page_counts.insert(loaded_chapter, 1);
                                }
                            }
                        }
                        // Keep already-loaded page 0 instead of failing hard.
                        self.page_idx = 0;
                        if self.chapter_idx == loaded_chapter && self.current_page.is_some() {
                            return true;
                        }
                        if let Ok(page) = self.load_page_with_retries(
                            loaded_chapter,
                            0,
                            Self::PAGE_LOAD_MAX_RETRIES,
                        ) {
                            self.current_page = Some(page);
                            return true;
                        }
                    }
                    Err(err) if Self::is_non_renderable_chapter_error(&err) => {
                        self.non_renderable_chapters.insert(candidate);
                    }
                    Err(err) => {
                        log::warn!(
                            "[EPUB] prev_page candidate chapter={} failed: {}",
                            candidate,
                            err
                        );
                    }
                }
                idx -= 1;
            }
        }
        if let Ok(page) = self.load_page_with_retries(
            previous_chapter,
            previous_page,
            Self::PAGE_LOAD_MAX_RETRIES,
        ) {
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

    fn discover_last_page_index(&mut self, chapter_idx: usize) -> Result<usize, String> {
        const MAX_PROBE_PAGE_INDEX: usize = 4096;

        let mut last_valid = 0usize;
        let mut invalid_upper = 1usize;

        loop {
            match self.load_page_with_retries(
                chapter_idx,
                invalid_upper,
                Self::PAGE_LOAD_MAX_RETRIES,
            ) {
                Ok(_) => {
                    last_valid = invalid_upper;
                    if invalid_upper >= MAX_PROBE_PAGE_INDEX {
                        return Ok(last_valid);
                    }
                    let next = invalid_upper.saturating_mul(2).min(MAX_PROBE_PAGE_INDEX);
                    if next == invalid_upper {
                        return Ok(last_valid);
                    }
                    invalid_upper = next;
                }
                Err(err) if Self::is_out_of_range_error(&err) => {
                    break;
                }
                Err(err) => return Err(err),
            }
        }

        let mut lo = last_valid;
        let mut hi = invalid_upper;
        while lo + 1 < hi {
            let mid = lo + (hi - lo) / 2;
            match self.load_page_with_retries(chapter_idx, mid, Self::PAGE_LOAD_MAX_RETRIES) {
                Ok(_) => lo = mid,
                Err(err) if Self::is_out_of_range_error(&err) => hi = mid,
                Err(err) => return Err(err),
            }
        }
        Ok(lo)
    }

    pub(super) fn next_chapter(&mut self) -> bool {
        if self.chapter_idx + 1 >= self.book.chapter_count() {
            return false;
        }

        let previous_chapter = self.chapter_idx;
        let previous_page = self.page_idx;
        self.current_page = None;

        if self.load_chapter_forward(self.chapter_idx + 1).is_ok() {
            return true;
        }

        if let Ok(page) = self.load_page_with_retries(
            previous_chapter,
            previous_page,
            Self::PAGE_LOAD_MAX_RETRIES,
        ) {
            self.chapter_idx = previous_chapter;
            self.page_idx = previous_page;
            self.current_page = Some(page);
        }
        false
    }

    pub(super) fn prev_chapter(&mut self) -> bool {
        if self.chapter_idx == 0 {
            return false;
        }

        let previous_chapter = self.chapter_idx;
        let previous_page = self.page_idx;
        self.current_page = None;

        if self.load_chapter_backward(self.chapter_idx - 1).is_ok() {
            return true;
        }

        if let Ok(page) = self.load_page_with_retries(
            previous_chapter,
            previous_page,
            Self::PAGE_LOAD_MAX_RETRIES,
        ) {
            self.chapter_idx = previous_chapter;
            self.page_idx = previous_page;
            self.current_page = Some(page);
        }
        false
    }

    pub(super) fn render<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        if let Some(page) = self.current_page.as_ref() {
            self.eg_renderer.render_page(page, display)?;
            self.render_inline_images(page, display)?;
            Ok(())
        } else {
            display.clear(BinaryColor::Off)
        }
    }

    fn inline_image_cache_key(src: &str, width: u32, height: u32) -> String {
        format!("{}@{}x{}", src, width.max(1), height.max(1))
    }

    fn adaptive_inline_threshold(pixels: &[u8]) -> u8 {
        if pixels.is_empty() {
            return 128;
        }
        let sum: u64 = pixels.iter().map(|px| *px as u64).sum();
        let avg = (sum / pixels.len() as u64) as i32;
        avg.clamp(78, 178) as u8
    }

    fn normalize_image_src_key(src: &str) -> String {
        src.trim()
            .split('#')
            .next()
            .unwrap_or(src)
            .trim()
            .to_ascii_lowercase()
    }

    fn initialize_cover_image_cache(&mut self) {
        #[cfg(target_os = "espidf")]
        {
            // Keep EPUB open deterministic: only read lightweight cover refs and
            // reuse a precomputed compact artifact from library scan cache.
            if let Ok(Some(cover_ref)) = self.book.cover_image_ref() {
                self.cover_image_sources
                    .insert(Self::normalize_image_src_key(&cover_ref.href));
                self.cover_image_sources
                    .insert(Self::normalize_image_src_key(&cover_ref.zip_path));
                let file_name = basename(&cover_ref.href);
                if !file_name.is_empty() {
                    self.cover_image_sources
                        .insert(Self::normalize_image_src_key(file_name));
                }
            }
            self.cover_image_bitmap = self
                .source_path
                .as_deref()
                .and_then(Self::load_compact_cover_bitmap_for_book_path);
            return;
        }
        #[cfg(not(target_os = "espidf"))]
        {
            let mut cover_bytes = Vec::new();
            let cover_ref = match self.book.read_cover_image_into(&mut cover_bytes) {
                Ok(Some(cover)) => Some(cover),
                Ok(None) => None,
                Err(err) => {
                    log::debug!("[EPUB] cover image read failed: {}", err);
                    None
                }
            };
            let Some(cover_ref) = cover_ref else {
                return;
            };
            if let Some(bitmap) = Self::decode_inline_image_bitmap(
                &cover_bytes,
                crate::DISPLAY_WIDTH,
                crate::DISPLAY_HEIGHT,
            ) {
                self.cover_image_bitmap = Some(bitmap);
                self.cover_image_sources
                    .insert(Self::normalize_image_src_key(&cover_ref.href));
                self.cover_image_sources
                    .insert(Self::normalize_image_src_key(&cover_ref.zip_path));
                let file_name = basename(&cover_ref.href);
                if !file_name.is_empty() {
                    self.cover_image_sources
                        .insert(Self::normalize_image_src_key(file_name));
                }
            }
        }
    }

    #[cfg(target_os = "espidf")]
    fn cover_cache_root() -> &'static str {
        "/sd/.xteink/covers"
    }

    #[cfg(target_os = "espidf")]
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

    #[cfg(target_os = "espidf")]
    fn cover_cache_path(path: &str, size: u64) -> String {
        format!(
            "{}/{:016x}.compact",
            Self::cover_cache_root(),
            Self::cover_cache_key(path, size)
        )
    }

    #[cfg(target_os = "espidf")]
    fn book_path_aliases(path: &str) -> [String; 3] {
        let trimmed = path.trim();
        let without_sd = trimmed.strip_prefix("/sd").unwrap_or(trimmed);
        let with_sd = if trimmed.starts_with("/sd/") {
            trimmed.to_string()
        } else if trimmed.starts_with('/') {
            format!("/sd{}", trimmed)
        } else {
            format!("/sd/{}", trimmed)
        };
        [trimmed.to_string(), without_sd.to_string(), with_sd]
    }

    #[cfg(target_os = "espidf")]
    fn decode_compact_cover_bitmap(encoded: &str) -> Option<InlineImageBitmap> {
        let (dims, hex) = encoded.trim().split_once(':')?;
        let (w, h) = dims.split_once('x')?;
        let width = w.parse::<u32>().ok()?;
        let height = h.parse::<u32>().ok()?;
        if width == 0
            || height == 0
            || width > crate::DISPLAY_WIDTH
            || height > crate::DISPLAY_HEIGHT
        {
            return None;
        }
        let pixel_count = (width as usize).checked_mul(height as usize)?;
        let packed_len = pixel_count.checked_add(7)? / 8;
        if hex.len() != packed_len.checked_mul(2)? {
            return None;
        }
        let mut packed = vec![0u8; packed_len];
        for (idx, chunk) in hex.as_bytes().chunks_exact(2).enumerate() {
            let hi = match chunk[0] {
                b'0'..=b'9' => chunk[0] - b'0',
                b'a'..=b'f' => chunk[0] - b'a' + 10,
                b'A'..=b'F' => chunk[0] - b'A' + 10,
                _ => return None,
            };
            let lo = match chunk[1] {
                b'0'..=b'9' => chunk[1] - b'0',
                b'a'..=b'f' => chunk[1] - b'a' + 10,
                b'A'..=b'F' => chunk[1] - b'A' + 10,
                _ => return None,
            };
            packed[idx] = (hi << 4) | lo;
        }
        let mut pixels = vec![255u8; pixel_count];
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                let byte = packed[idx / 8];
                let bit = 7 - (idx % 8);
                let is_black = (byte & (1 << bit)) != 0;
                pixels[idx] = if is_black { 0 } else { 255 };
            }
        }
        Some(InlineImageBitmap {
            width,
            height,
            pixels,
            threshold: 128,
        })
    }

    #[cfg(target_os = "espidf")]
    fn load_compact_cover_bitmap_for_book_path(path: &str) -> Option<InlineImageBitmap> {
        let file_size = std::fs::metadata(path).ok()?.len();
        for candidate in Self::book_path_aliases(path) {
            if candidate.is_empty() {
                continue;
            }
            let cache_path = Self::cover_cache_path(&candidate, file_size);
            let Ok(encoded) = std::fs::read_to_string(cache_path) else {
                continue;
            };
            if let Some(bitmap) = Self::decode_compact_cover_bitmap(&encoded) {
                return Some(bitmap);
            }
        }
        None
    }

    fn resolve_cover_image_bitmap_for_src(&self, src: &str) -> Option<&InlineImageBitmap> {
        let bitmap = self.cover_image_bitmap.as_ref()?;
        let key = Self::normalize_image_src_key(src);
        if key.is_empty() {
            return None;
        }
        if self.cover_image_sources.contains(&key) {
            return Some(bitmap);
        }
        let file_name = basename(&key);
        if !file_name.is_empty()
            && self
                .cover_image_sources
                .contains(&Self::normalize_image_src_key(file_name))
        {
            return Some(bitmap);
        }
        None
    }

    fn decode_inline_image_bitmap(
        bytes: &[u8],
        max_width: u32,
        max_height: u32,
    ) -> Option<InlineImageBitmap> {
        if bytes.len() > Self::INLINE_IMAGE_SOURCE_MAX_BYTES {
            return None;
        }
        let cursor = Cursor::new(bytes);
        let reader = ImageReader::new(cursor).with_guessed_format().ok()?;
        let (src_w, src_h) = reader.into_dimensions().ok()?;
        if src_w == 0 || src_h == 0 {
            return None;
        }
        if (src_w as u64).saturating_mul(src_h as u64) > Self::INLINE_IMAGE_MAX_DECODED_PIXELS {
            return None;
        }
        let decoded = image::load_from_memory(bytes).ok()?;
        let resized = decoded.thumbnail(max_width.max(1), max_height.max(1));
        let gray = resized.to_luma8();
        let (width, height) = gray.dimensions();
        if width == 0 || height == 0 {
            return None;
        }
        let pixels = gray.into_raw();
        if pixels.is_empty() {
            return None;
        }
        Some(InlineImageBitmap {
            width,
            height,
            threshold: Self::adaptive_inline_threshold(&pixels),
            pixels,
        })
    }

    fn prefetch_inline_images_for_page(&mut self, page: &RenderPage) {
        let commands: &[DrawCommand] = if !page.content_commands.is_empty() {
            &page.content_commands
        } else {
            &page.commands
        };
        let mut loaded = 0usize;
        for cmd in commands {
            if loaded >= Self::INLINE_IMAGE_PREFETCH_PER_PAGE {
                break;
            }
            let DrawCommand::ImageObject(obj) = cmd else {
                continue;
            };
            if obj.src.is_empty() || obj.width == 0 || obj.height == 0 {
                continue;
            }
            let key = Self::inline_image_cache_key(&obj.src, obj.width, obj.height);
            if self.inline_image_cache.contains_key(&key) {
                continue;
            }
            let bytes = match self.book.read_resource(&obj.src) {
                Ok(bytes) => bytes,
                Err(err) => {
                    log::debug!(
                        "[EPUB] inline image resource read failed src={} err={}",
                        obj.src,
                        err
                    );
                    continue;
                }
            };
            if bytes.len() > Self::INLINE_IMAGE_SOURCE_MAX_BYTES {
                log::debug!(
                    "[EPUB] inline image skipped src={} bytes={} limit={}",
                    obj.src,
                    bytes.len(),
                    Self::INLINE_IMAGE_SOURCE_MAX_BYTES
                );
                continue;
            }
            if let Some(bitmap) = Self::decode_inline_image_bitmap(&bytes, obj.width, obj.height) {
                self.inline_image_cache.insert(key, bitmap);
                loaded += 1;
            }
        }
        while self.inline_image_cache.len() > Self::INLINE_IMAGE_CACHE_LIMIT {
            let Some(first_key) = self.inline_image_cache.keys().next().cloned() else {
                break;
            };
            self.inline_image_cache.remove(&first_key);
        }
    }

    fn render_inline_image_object<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        obj: &ImageObjectCommand,
        bitmap: &InlineImageBitmap,
    ) -> Result<(), D::Error> {
        if obj.width == 0 || obj.height == 0 || bitmap.width == 0 || bitmap.height == 0 {
            return Ok(());
        }
        let inner_x = obj.x + 1;
        let inner_y = obj.y + 1;
        let inner_w = obj.width.saturating_sub(2).max(1);
        let inner_h = obj.height.saturating_sub(2).max(1);
        let draw_w = inner_w.min(bitmap.width);
        let draw_h = inner_h.min(bitmap.height);
        let offset_x = ((inner_w as i32 - draw_w as i32).max(0)) / 2;
        let offset_y = ((inner_h as i32 - draw_h as i32).max(0)) / 2;

        Rectangle::new(Point::new(inner_x, inner_y), Size::new(inner_w, inner_h))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
            .draw(display)?;

        for dy in 0..draw_h {
            let src_y = (dy as u64 * bitmap.height as u64 / draw_h as u64) as u32;
            for dx in 0..draw_w {
                let src_x = (dx as u64 * bitmap.width as u64 / draw_w as u64) as u32;
                let idx = (src_y * bitmap.width + src_x) as usize;
                if idx >= bitmap.pixels.len() {
                    continue;
                }
                let color = if bitmap.pixels[idx] < bitmap.threshold {
                    BinaryColor::On
                } else {
                    BinaryColor::Off
                };
                Pixel(
                    Point::new(
                        inner_x + offset_x + dx as i32,
                        inner_y + offset_y + dy as i32,
                    ),
                    color,
                )
                .draw(display)?;
            }
        }

        Rectangle::new(
            Point::new(obj.x, obj.y),
            Size::new(obj.width.max(1), obj.height.max(1)),
        )
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display)?;
        Ok(())
    }

    fn render_inline_image_placeholder<D: DrawTarget<Color = BinaryColor>>(
        &self,
        display: &mut D,
        obj: &ImageObjectCommand,
    ) -> Result<(), D::Error> {
        let outer = Rectangle::new(
            Point::new(obj.x, obj.y),
            Size::new(obj.width.max(1), obj.height.max(1)),
        );
        outer
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
            .draw(display)?;
        outer
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;

        let inner_w = obj.width.saturating_sub(8).max(12);
        let inner_h = obj.height.saturating_sub(10).max(12);
        let icon_w = inner_w.min(20);
        let icon_h = inner_h.min(24);
        let icon_x = obj.x + ((obj.width as i32 - icon_w as i32).max(0) / 2);
        let icon_y = obj.y + ((obj.height as i32 - icon_h as i32).max(0) / 2);

        Rectangle::new(Point::new(icon_x, icon_y), Size::new(icon_w, icon_h))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;

        // Small top bar + two text lines for a simple "book page" icon.
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
            .draw(display)?;
        Ok(())
    }

    fn render_inline_images<D: DrawTarget<Color = BinaryColor>>(
        &self,
        page: &RenderPage,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let commands: &[DrawCommand] = if !page.content_commands.is_empty() {
            &page.content_commands
        } else {
            &page.commands
        };
        for cmd in commands {
            let DrawCommand::ImageObject(obj) = cmd else {
                continue;
            };
            let key = Self::inline_image_cache_key(&obj.src, obj.width, obj.height);
            if let Some(bitmap) = self.inline_image_cache.get(&key) {
                self.render_inline_image_object(display, obj, bitmap)?;
            } else if let Some(bitmap) = self.resolve_cover_image_bitmap_for_src(&obj.src) {
                self.render_inline_image_object(display, obj, bitmap)?;
            } else {
                self.render_inline_image_placeholder(display, obj)?;
            }
        }
        Ok(())
    }

    fn load_current_page(&mut self) -> Result<(), String> {
        let page = self.load_page_with_retries(
            self.chapter_idx,
            self.page_idx,
            Self::PAGE_LOAD_MAX_RETRIES,
        )?;
        self.current_page = Some(page);
        Ok(())
    }

    fn is_retryable_page_error(err: &str) -> bool {
        let lower = err.to_ascii_lowercase();
        lower.contains("buffer too small")
            || lower.contains("allocate")
            || lower.contains("alloc")
            || lower.contains("out of memory")
            || lower.contains("unable to stream epub chapter")
            || lower.contains("unable to layout epub chapter")
    }

    fn recover_after_page_load_failure(&mut self) {
        self.current_page = None;
        self.page_cache.clear();
        self.inline_image_cache.clear();
        self.chapter_scratch.read_buf.clear();
        self.chapter_scratch.xml_buf.clear();
        self.chapter_scratch.text_buf.clear();
        self.chapter_buf.shrink_to(Self::CHAPTER_BUF_CAPACITY_BYTES);
    }

    fn load_page_with_retries(
        &mut self,
        chapter_idx: usize,
        page_idx: usize,
        retries: usize,
    ) -> Result<RenderPage, String> {
        let mut attempt = 0usize;
        loop {
            match self.load_page(chapter_idx, page_idx) {
                Ok(page) => return Ok(page),
                Err(err) => {
                    if attempt >= retries || !Self::is_retryable_page_error(&err) {
                        return Err(err);
                    }
                    attempt = attempt.saturating_add(1);
                    log::warn!(
                        "[EPUB] retryable load failure ch={} pg={} attempt={}/{} err={}",
                        chapter_idx,
                        page_idx,
                        attempt,
                        retries,
                        err
                    );
                    self.recover_after_page_load_failure();
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            }
        }
    }

    fn load_page(&mut self, chapter_idx: usize, page_idx: usize) -> Result<RenderPage, String> {
        if let Some(page) = self.page_cache.get(&(chapter_idx, page_idx)) {
            log::debug!(
                "[EPUB] page cache hit chapter={} page={}",
                chapter_idx,
                page_idx
            );
            return Ok(page.clone());
        }
        log::debug!(
            "[EPUB] load_page start chapter={} page={} cache_entries={}",
            chapter_idx,
            page_idx,
            self.page_cache.len()
        );

        let chapter_opts = ChapterEventsOptions {
            max_items: Self::MAX_CHAPTER_EVENTS,
            render: self.chapter_events_opts,
        };

        if let Ok(required_bytes) = self.book.chapter_uncompressed_size(chapter_idx) {
            if required_bytes > self.chapter_buf.capacity() {
                if required_bytes > Self::MAX_CHAPTER_BUF_CAPACITY_BYTES {
                    return Err(format!(
                        "Unable to stream EPUB chapter: required {} bytes exceeds chapter buffer cap {} bytes",
                        required_bytes,
                        Self::MAX_CHAPTER_BUF_CAPACITY_BYTES
                    ));
                }
                let len = self.chapter_buf.len();
                let additional = required_bytes.saturating_sub(len);
                self.chapter_buf.try_reserve(additional).map_err(|_| {
                    format!(
                        "Unable to allocate EPUB chapter buffer (required {} bytes)",
                        required_bytes
                    )
                })?;
                log::debug!(
                    "[EPUB] pre-sized chapter buffer to {} bytes for chapter={}",
                    self.chapter_buf.capacity(),
                    chapter_idx
                );
            }
        }

        let mut grow_retries = 0usize;
        let page = loop {
            let mut target_page: Option<RenderPage> = None;
            #[cfg(target_os = "espidf")]
            let config = self.apply_common_render_config(
                RenderConfig::default().with_page_range(page_idx..page_idx + 1),
            );
            #[cfg(not(target_os = "espidf"))]
            let config = self.apply_common_render_config(
                RenderConfig::default()
                    .with_page_range(page_idx..page_idx + 1)
                    .with_cache(&self.render_cache),
            );
            #[cfg(feature = "fontdue")]
            let config = config.with_text_measurer(Arc::new(self.layout_text_measurer.clone()));
            #[cfg(target_os = "espidf")]
            let mut session = Box::new(self.engine.begin(chapter_idx, config));
            #[cfg(not(target_os = "espidf"))]
            let mut session = self.engine.begin(chapter_idx, config);
            let book_language = self.book.language();
            if !book_language.trim().is_empty() {
                session.set_hyphenation_language(book_language);
            }
            let mut layout_error: Option<String> = None;

            let stream_result = self.book.chapter_events_with_scratch(
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
            );

            if let Err(err) = stream_result {
                let err_string = err.to_string();
                if Self::is_buffer_too_small_error(&err_string) {
                    if grow_retries >= Self::MAX_CHAPTER_BUF_GROW_RETRIES {
                        return Err(format!(
                            "Unable to stream EPUB chapter after {} buffer growth retries (capacity={} bytes): {}",
                            grow_retries,
                            self.chapter_buf.capacity(),
                            err_string
                        ));
                    }
                    if self.grow_chapter_buffer()? {
                        grow_retries += 1;
                        continue;
                    }
                    return Err(format!(
                        "Unable to stream EPUB chapter: chapter buffer capped at {} bytes ({})",
                        self.chapter_buf.capacity(),
                        err_string
                    ));
                }
                return Err(format!("Unable to stream EPUB chapter: {}", err_string));
            }
            log::debug!("[EPUB] chapter_events streamed chapter={}", chapter_idx);

            if let Some(err) = layout_error {
                return Err(format!("Unable to layout EPUB chapter: {}", err));
            }

            #[cfg(target_os = "espidf")]
            {
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
            }

            #[cfg(not(target_os = "espidf"))]
            {
                session
                    .finish()
                    .map_err(|e| format!("Unable to finalize EPUB chapter layout: {}", e))?;
                session.drain_pages(|page| {
                    if target_page.is_none() {
                        target_page = Some(page);
                    }
                });
            }

            break target_page.ok_or_else(|| Self::OUT_OF_RANGE_ERR.to_string())?;
        };
        log::debug!(
            "[EPUB] load_page ok chapter={} page={} total_in_chapter={:?}",
            chapter_idx,
            page_idx,
            page.metrics.chapter_page_count
        );
        self.prefetch_inline_images_for_page(&page);

        if let Some(count) = page.metrics.chapter_page_count {
            self.chapter_page_counts.insert(chapter_idx, count.max(1));
            self.chapter_page_counts_exact.insert(chapter_idx);
        } else {
            // Keep chapter page totals monotonic from observed pages without forcing
            // a full chapter reflow on constrained devices.
            let observed_min = page_idx + 1;
            let existing = self
                .chapter_page_counts
                .get(&chapter_idx)
                .copied()
                .unwrap_or(0);
            self.chapter_page_counts
                .insert(chapter_idx, existing.max(observed_min));
            #[cfg(not(target_os = "espidf"))]
            if existing == 0 {
                if let Ok(count) = self.compute_chapter_page_count(chapter_idx) {
                    self.chapter_page_counts
                        .insert(chapter_idx, count.max(observed_min));
                    self.chapter_page_counts_exact.insert(chapter_idx);
                }
            }
        }

        #[cfg(not(target_os = "espidf"))]
        {
            self.page_cache
                .insert((chapter_idx, page_idx), page.clone());
            self.trim_page_cache();
        }
        Ok(page)
    }

    #[cfg(not(target_os = "espidf"))]
    fn compute_chapter_page_count(&mut self, chapter_idx: usize) -> Result<usize, String> {
        let chapter_opts = ChapterEventsOptions {
            max_items: Self::MAX_CHAPTER_EVENTS,
            render: self.chapter_events_opts,
        };
        let mut count = 0usize;
        let mut config = self.apply_common_render_config(RenderConfig::default());
        #[cfg(feature = "fontdue")]
        {
            config = config.with_text_measurer(Arc::new(self.layout_text_measurer.clone()));
        }
        #[cfg(target_os = "espidf")]
        let mut session = Box::new(self.engine.begin(chapter_idx, config));
        #[cfg(not(target_os = "espidf"))]
        let mut session = self.engine.begin(chapter_idx, config);
        let book_language = self.book.language();
        if !book_language.trim().is_empty() {
            session.set_hyphenation_language(book_language);
        }
        let mut layout_error: Option<String> = None;
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
                    if let Err(err) = session.push(item) {
                        layout_error = Some(err.to_string());
                        return Ok(());
                    }
                    session.drain_pages(|_| {
                        count += 1;
                    });
                    Ok(())
                },
            )
            .map_err(|err| format!("Unable to stream EPUB chapter: {}", err))?;
        if let Some(err) = layout_error {
            return Err(format!("Unable to layout EPUB chapter: {}", err));
        }
        session
            .finish()
            .map_err(|err| format!("Unable to finalize EPUB chapter layout: {}", err))?;
        session.drain_pages(|_| {
            count += 1;
        });
        if count == 0 {
            return Err(Self::OUT_OF_RANGE_ERR.to_string());
        }
        Ok(count)
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

    #[cfg(feature = "fontdue")]
    fn create_renderer() -> (ReaderRenderer, BookerlyFontBackend) {
        let cfg = EgRenderConfig {
            image_fallback: if cfg!(target_os = "espidf") {
                ImageFallbackPolicy::OutlineOnly
            } else {
                ImageFallbackPolicy::OutlineWithAltText
            },
            ..EgRenderConfig::default()
        };
        let backend = BookerlyFontBackend::default();
        let limits = if cfg!(target_os = "espidf") {
            ImageRegistryLimits {
                max_images: 8,
                max_total_pixels: 128 * 1024,
            }
        } else {
            ImageRegistryLimits::default()
        };
        (
            EgRenderer::with_backend_and_image_limits(cfg, backend.clone(), limits),
            backend,
        )
    }

    #[cfg(not(feature = "fontdue"))]
    fn create_renderer() -> ReaderRenderer {
        let cfg = EgRenderConfig {
            image_fallback: if cfg!(target_os = "espidf") {
                ImageFallbackPolicy::OutlineOnly
            } else {
                ImageFallbackPolicy::OutlineWithAltText
            },
            ..EgRenderConfig::default()
        };
        let limits = if cfg!(target_os = "espidf") {
            ImageRegistryLimits {
                max_images: 8,
                max_total_pixels: 128 * 1024,
            }
        } else {
            ImageRegistryLimits::default()
        };
        EgRenderer::with_backend_and_image_limits(
            cfg,
            mu_epub_embedded_graphics::MonoFontBackend,
            limits,
        )
    }

    fn register_embedded_fonts(&mut self) {
        #[cfg(target_os = "espidf")]
        {
            // Embedded font resource loading can require large contiguous
            // allocations (font blobs), which is not reliable on constrained
            // ESP heaps during open. Keep open deterministic and use bundled
            // fonts on device.
            #[allow(clippy::needless_return)]
            return;
        }

        #[cfg(not(target_os = "espidf"))]
        #[cfg(feature = "fontdue")]
        {
            let limits = self.chapter_events_opts.fonts;
            let faces = match self.book.embedded_fonts_with_options(limits) {
                Ok(faces) => faces,
                Err(err) => {
                    log::warn!("[EPUB] unable to enumerate embedded fonts: {}", err);
                    return;
                }
            };
            if faces.is_empty() {
                return;
            }

            let mut total_loaded = 0usize;
            let mut face_data = Vec::with_capacity(faces.len().min(limits.max_faces));
            let mut face_meta: Vec<(String, usize, u16, bool)> =
                Vec::with_capacity(faces.len().min(limits.max_faces));
            for face in faces.iter().take(limits.max_faces) {
                let bytes = match self.book.read_resource(&face.href) {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        log::warn!(
                            "[EPUB] skipping embedded font {} (read failed: {})",
                            face.href,
                            err
                        );
                        continue;
                    }
                };
                if bytes.len() > limits.max_bytes_per_font {
                    continue;
                }
                if total_loaded.saturating_add(bytes.len()) > limits.max_total_font_bytes {
                    break;
                }
                total_loaded += bytes.len();
                face_data.push(bytes);
                face_meta.push((
                    face.family.clone(),
                    face_data.len() - 1,
                    face.weight,
                    matches!(
                        face.style,
                        mu_epub::EmbeddedFontStyle::Italic | mu_epub::EmbeddedFontStyle::Oblique
                    ),
                ));
            }
            let mut registrations = Vec::with_capacity(face_meta.len());
            for (family, data_idx, weight, italic) in face_meta.iter() {
                let data = face_data[*data_idx].as_slice();
                registrations.push(mu_epub_embedded_graphics::FontFaceRegistration {
                    family,
                    weight: *weight,
                    italic: *italic,
                    data,
                });
            }
            if registrations.is_empty() {
                return;
            }
            let accepted = self.eg_renderer.register_faces(&registrations);
            log::info!(
                "[EPUB] registered embedded fonts: accepted={} attempted={}",
                accepted,
                registrations.len()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EpubReadingState;
    use crate::reader_settings_activity::ReaderSettings;
    use std::io::Cursor;

    #[test]
    fn current_chapter_skips_non_renderable_before_current() {
        assert_eq!(EpubReadingState::compute_current_chapter(0, 0), 1);
        assert_eq!(EpubReadingState::compute_current_chapter(1, 0), 2);
        assert_eq!(EpubReadingState::compute_current_chapter(3, 1), 3);
        assert_eq!(EpubReadingState::compute_current_chapter(5, 2), 4);
    }

    #[test]
    fn page_progress_label_respects_exact_vs_estimated_totals() {
        assert_eq!(
            EpubReadingState::format_page_progress_label(3, 10, true),
            "p3/10"
        );
        assert_eq!(
            EpubReadingState::format_page_progress_label(3, 10, false),
            "p3"
        );
    }

    #[test]
    fn chapter_progress_label_uses_current_and_total() {
        assert_eq!(
            EpubReadingState::format_chapter_progress_label(2, 7),
            "c2/7"
        );
        assert_eq!(
            EpubReadingState::format_chapter_progress_label(1, 0),
            "c1/1"
        );
    }

    #[test]
    fn book_progress_percent_hits_100_only_on_known_final_page() {
        let p = EpubReadingState::compute_book_progress_percent_legacy(5, 5, 9, 10, true);
        assert_eq!(p, 100);

        let p = EpubReadingState::compute_book_progress_percent_legacy(5, 5, 8, 10, true);
        assert!(p < 100);

        let p = EpubReadingState::compute_book_progress_percent_legacy(5, 5, 99, 1, false);
        assert!(p < 100);
    }

    #[test]
    fn book_progress_percent_monotonic_inside_chapter() {
        let p0 = EpubReadingState::compute_book_progress_percent_legacy(2, 5, 0, 10, true);
        let p1 = EpubReadingState::compute_book_progress_percent_legacy(2, 5, 3, 10, true);
        let p2 = EpubReadingState::compute_book_progress_percent_legacy(2, 5, 9, 10, true);
        assert!(p0 <= p1 && p1 <= p2);

        let u0 = EpubReadingState::compute_book_progress_percent_legacy(2, 5, 0, 1, false);
        let u1 = EpubReadingState::compute_book_progress_percent_legacy(2, 5, 2, 1, false);
        let u2 = EpubReadingState::compute_book_progress_percent_legacy(2, 5, 8, 1, false);
        assert!(u0 <= u1 && u1 <= u2);
    }

    #[test]
    fn global_page_percent_smooths_chapter_boundary_jump() {
        // 4 chapters with page totals [9, 8, 7, 1]:
        // last page of chapter 3 should be high but not 100.
        let c3_last =
            EpubReadingState::compute_book_progress_percent_from_pages(9 + 8, 6, 7, 25, false);
        assert!(c3_last < 100);
        // first page of last one-page chapter can be 100 only when exact final.
        let c4_only =
            EpubReadingState::compute_book_progress_percent_from_pages(24, 0, 1, 25, true);
        assert_eq!(c4_only, 100);
    }

    #[test]
    fn default_page_estimate_uses_average_and_bounds() {
        assert_eq!(EpubReadingState::compute_default_page_estimate(0, 0), 8);
        assert_eq!(EpubReadingState::compute_default_page_estimate(48, 6), 8);
        assert_eq!(EpubReadingState::compute_default_page_estimate(3, 5), 1);
        assert_eq!(
            EpubReadingState::compute_default_page_estimate(999_999, 1),
            256
        );
    }

    #[test]
    fn chapter_weight_prefers_known_pages_else_default_estimate() {
        assert_eq!(
            EpubReadingState::chapter_weight_from_counts(Some(17), 9),
            17
        );
        assert_eq!(EpubReadingState::chapter_weight_from_counts(None, 9), 9);
        assert_eq!(EpubReadingState::chapter_weight_from_counts(Some(0), 9), 1);
        assert_eq!(EpubReadingState::chapter_weight_from_counts(None, 0), 1);
    }

    #[test]
    fn select_target_from_weighted_stays_stable_across_boundary() {
        let weighted = [(0usize, 9usize), (1, 8), (2, 7), (3, 1)];
        let target = EpubReadingState::select_target_from_weighted(73, &weighted)
            .expect("target should resolve");
        assert_eq!(target.0, 2);
        assert!(target.1 < target.2);
    }

    #[test]
    fn select_target_from_weighted_hits_final_only_at_hundred() {
        let weighted = [(0usize, 9usize), (1, 8), (2, 7), (3, 1)];
        let ninety_nine = EpubReadingState::select_target_from_weighted(99, &weighted)
            .expect("target should resolve");
        let hundred = EpubReadingState::select_target_from_weighted(100, &weighted)
            .expect("target should resolve");
        assert_eq!(ninety_nine.0, 2);
        assert_eq!(hundred.0, 3);
    }

    #[test]
    fn locate_in_weighted_pages_maps_global_index_to_chapter_page() {
        let weighted = [(0usize, 3usize), (1, 2), (2, 1)];
        assert_eq!(
            EpubReadingState::locate_in_weighted_pages(0, &weighted),
            Some((0, 0))
        );
        assert_eq!(
            EpubReadingState::locate_in_weighted_pages(2, &weighted),
            Some((0, 2))
        );
        assert_eq!(
            EpubReadingState::locate_in_weighted_pages(3, &weighted),
            Some((1, 0))
        );
        assert_eq!(
            EpubReadingState::locate_in_weighted_pages(5, &weighted),
            Some((2, 0))
        );
    }

    #[test]
    fn locate_in_weighted_pages_clamps_out_of_range_to_last_known_page() {
        let weighted = [(0usize, 4usize), (1, 1)];
        assert_eq!(
            EpubReadingState::locate_in_weighted_pages(999, &weighted),
            Some((1, 0))
        );
        assert_eq!(EpubReadingState::locate_in_weighted_pages(0, &[]), None);
    }

    #[test]
    fn prev_page_discovers_last_page_for_unknown_previous_chapter_total() {
        let bytes = include_bytes!("../../../../sample_books/pg84-frankenstein.epub").to_vec();
        let mut state =
            EpubReadingState::from_reader(Box::new(Cursor::new(bytes)), ReaderSettings::default())
                .expect("frankenstein should open");
        state
            .load_chapter_forward(0)
            .expect("first renderable chapter should load");

        let chapter_count = state.book.chapter_count();
        let mut picked: Option<(usize, usize, usize)> = None;
        for chapter_idx in 0..chapter_count.saturating_sub(1) {
            let Ok(page_total) = state.compute_chapter_page_count(chapter_idx) else {
                continue;
            };
            if page_total <= 1 {
                continue;
            }
            if state.load_chapter_forward(chapter_idx + 1).is_ok() {
                let next_renderable = state.chapter_idx;
                if next_renderable > chapter_idx {
                    picked = Some((chapter_idx, page_total, next_renderable));
                    break;
                }
            }
        }
        let (prev_chapter, prev_page_total, next_renderable) =
            picked.expect("need a multi-page chapter followed by renderable chapter");

        // Simulate unknown chapter total as seen on constrained devices.
        state.chapter_page_counts.insert(prev_chapter, 1);
        state.chapter_page_counts_exact.remove(&prev_chapter);
        state
            .load_chapter_forward(next_renderable)
            .expect("target next renderable chapter should load");
        assert_eq!(state.chapter_idx, next_renderable);
        assert_eq!(state.page_idx, 0);

        assert!(state.prev_page(), "prev_page should cross chapter boundary");
        assert_eq!(state.chapter_idx, prev_chapter);
        assert_eq!(state.page_idx, prev_page_total - 1);

        assert!(state.next_page(), "next_page should return to next chapter");
        assert_eq!(state.chapter_idx, next_renderable);
        assert_eq!(state.page_idx, 0);
    }

    #[test]
    fn chapter_navigation_progresses_monotonically_without_looping() {
        let bytes = include_bytes!("../../../../sample_books/pg84-frankenstein.epub").to_vec();
        let mut state =
            EpubReadingState::from_reader(Box::new(Cursor::new(bytes)), ReaderSettings::default())
                .expect("frankenstein should open");
        state
            .load_chapter_forward(0)
            .expect("first renderable chapter should load");

        let start = state.chapter_idx;
        assert!(state.next_chapter(), "next_chapter should advance");
        let advanced = state.chapter_idx;
        assert!(advanced > start, "chapter index should increase");

        assert!(state.prev_chapter(), "prev_chapter should go back");
        assert_eq!(state.chapter_idx, start);

        let mut last = state.chapter_idx;
        let mut steps = 0usize;
        while state.next_chapter() {
            assert!(
                state.chapter_idx > last,
                "chapter index must strictly increase"
            );
            last = state.chapter_idx;
            steps += 1;
            assert!(steps < 128, "chapter navigation loop detected");
        }
    }
}

impl FileBrowserActivity {
    #[inline(never)]
    pub(super) fn process_open_epub_file_task(
        &mut self,
        fs: &mut dyn FileSystem,
        path: &str,
    ) -> bool {
        #[cfg(feature = "std")]
        {
            self.active_epub_path = Some(path.to_string());
            if let Err(err) = Self::persist_last_active_content_path(path) {
                log::warn!("[EPUB] unable to persist last active content path: {}", err);
            }
        }
        self.invalidate_browser_tasks();
        #[cfg(all(feature = "std", not(target_os = "espidf")))]
        {
            self.mode = BrowserMode::OpeningEpub;
            self.browser
                .set_status_message(format!("Opening EPUB: {}", basename(path)));
            match Self::spawn_epub_open_worker(fs, path, self.reader_settings) {
                Ok(receiver) => {
                    self.epub_open_pending = Some(PendingEpubOpen { receiver });
                    self.epub_open_started_tick = Some(self.ui_tick);
                }
                Err(error) => {
                    self.mode = BrowserMode::Browsing;
                    self.handle_epub_runtime_failure(error);
                    self.active_epub_path = None;
                    self.epub_open_started_tick = None;
                }
            }
        }

        #[cfg(all(feature = "std", target_os = "espidf"))]
        {
            self.mode = BrowserMode::OpeningEpub;
            self.browser
                .set_status_message("Opening EPUB...".to_string());
            match Self::prepare_epub_open_source(fs, path) {
                Ok(EpubOpenSource::HostPath(host_path)) => {
                    match EpubReadingState::from_sd_path_light(&host_path, self.reader_settings) {
                        Ok(state) => {
                            let renderer = Arc::new(Mutex::new(state));
                            self.invalidate_browser_tasks();
                            self.mode = BrowserMode::ReadingEpub {
                                renderer: Arc::clone(&renderer),
                            };
                            match Self::spawn_epub_initial_load_worker(renderer) {
                                Ok(receiver) => {
                                    self.pending_epub_initial_load =
                                        Some(PendingEpubInitialLoad { receiver });
                                    self.epub_initial_load_started_tick = Some(self.ui_tick);
                                    self.reset_epub_failure_streak();
                                }
                                Err(error) => {
                                    self.mode = BrowserMode::Browsing;
                                    self.handle_epub_runtime_failure(error);
                                    self.active_epub_path = None;
                                }
                            }
                        }
                        Err(error) => {
                            self.mode = BrowserMode::Browsing;
                            self.handle_epub_runtime_failure(error);
                            self.active_epub_path = None;
                        }
                    }
                }
                Err(error) => {
                    self.mode = BrowserMode::Browsing;
                    self.handle_epub_runtime_failure(error);
                    self.active_epub_path = None;
                }
                #[allow(unreachable_patterns)]
                _ => {
                    self.mode = BrowserMode::Browsing;
                    self.handle_epub_runtime_failure(
                        "Unable to open EPUB: unsupported source".to_string(),
                    );
                    self.active_epub_path = None;
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

    #[cfg(all(feature = "std", not(target_os = "espidf")))]
    pub(super) fn poll_epub_open_result(&mut self) -> bool {
        let recv_result = match self.epub_open_pending.as_mut() {
            Some(pending) => pending.receiver.try_recv(),
            None => return false,
        };

        match recv_result {
            Ok(EpubOpenWorkerEvent::Phase(phase)) => {
                self.browser
                    .set_status_message(format!("Opening EPUB: {}", phase));
                true
            }
            Ok(EpubOpenWorkerEvent::Done(Ok(renderer))) => {
                self.epub_open_pending = None;
                self.epub_open_started_tick = None;
                self.epub_navigation_pending = None;
                self.epub_navigation_started_tick = None;
                self.restore_active_epub_position(&renderer);
                #[cfg(not(target_os = "espidf"))]
                if let Ok(mut guard) = renderer.lock() {
                    guard.prewarm_next_page();
                }
                self.invalidate_browser_tasks();
                self.mode = BrowserMode::ReadingEpub { renderer };
                self.reset_epub_failure_streak();
                true
            }
            Ok(EpubOpenWorkerEvent::Done(Err(error))) => {
                self.epub_open_pending = None;
                self.epub_open_started_tick = None;
                self.mode = BrowserMode::Browsing;
                self.handle_epub_runtime_failure(error);
                self.active_epub_path = None;
                true
            }
            Err(TryRecvError::Empty) => {
                if let Some(start_tick) = self.epub_open_started_tick {
                    let elapsed = self.ui_tick.saturating_sub(start_tick);
                    if elapsed > Self::EPUB_OPEN_TIMEOUT_TICKS {
                        self.epub_open_pending = None;
                        self.epub_open_started_tick = None;
                        self.mode = BrowserMode::Browsing;
                        self.handle_epub_runtime_failure(
                            "Unable to open EPUB: timed out".to_string(),
                        );
                        self.active_epub_path = None;
                        return true;
                    }
                    if elapsed > 0 && elapsed % Self::EPUB_OPEN_HEARTBEAT_TICKS == 0 {
                        let seconds = (elapsed.saturating_mul(50)) / 1000;
                        self.browser
                            .set_status_message(format!("Opening EPUB... {}s", seconds));
                        return true;
                    }
                }
                false
            }
            Err(TryRecvError::Disconnected) => {
                self.epub_open_pending = None;
                self.epub_open_started_tick = None;
                self.mode = BrowserMode::Browsing;
                self.handle_epub_runtime_failure(
                    "Unable to open EPUB: worker disconnected".to_string(),
                );
                self.active_epub_path = None;
                true
            }
        }
    }

    #[cfg(all(feature = "std", target_os = "espidf"))]
    pub(super) fn poll_epub_open_result(&mut self) -> bool {
        false
    }

    #[cfg(all(feature = "std", target_os = "espidf"))]
    pub(super) fn process_pending_epub_initial_load(&mut self) -> bool {
        let recv_result = match self.pending_epub_initial_load.as_mut() {
            Some(pending) => pending.receiver.try_recv(),
            None => return false,
        };

        match recv_result {
            Ok(Ok(())) => {
                self.pending_epub_initial_load = None;
                self.epub_initial_load_started_tick = None;
                let renderer = match &self.mode {
                    BrowserMode::ReadingEpub { renderer } => Arc::clone(renderer),
                    _ => return false,
                };
                self.restore_active_epub_position(&renderer);
                self.reset_epub_failure_streak();
                true
            }
            Ok(Err(err)) => {
                self.pending_epub_initial_load = None;
                self.epub_initial_load_started_tick = None;
                self.mode = BrowserMode::Browsing;
                self.handle_epub_runtime_failure(format!("Unable to open EPUB: {}", err));
                self.active_epub_path = None;
                true
            }
            Err(TryRecvError::Empty) => {
                if let Some(start_tick) = self.epub_initial_load_started_tick {
                    let elapsed = self.ui_tick.saturating_sub(start_tick);
                    if elapsed > Self::EPUB_INITIAL_LOAD_TIMEOUT_TICKS {
                        self.pending_epub_initial_load = None;
                        self.epub_initial_load_started_tick = None;
                        self.mode = BrowserMode::Browsing;
                        self.handle_epub_runtime_failure(
                            "Unable to open EPUB: initial page load timed out".to_string(),
                        );
                        self.active_epub_path = None;
                        return true;
                    }
                }
                false
            }
            Err(TryRecvError::Disconnected) => {
                self.pending_epub_initial_load = None;
                self.epub_initial_load_started_tick = None;
                self.mode = BrowserMode::Browsing;
                self.handle_epub_runtime_failure(
                    "Unable to open EPUB: initial page worker disconnected".to_string(),
                );
                self.active_epub_path = None;
                true
            }
        }
    }

    #[cfg(feature = "std")]
    pub(super) fn poll_epub_navigation_result(&mut self) -> bool {
        let recv_result = match self.epub_navigation_pending.as_mut() {
            Some(pending) => pending.receiver.try_recv(),
            None => return false,
        };

        match recv_result {
            Ok(Ok(outcome)) => {
                let direction = self
                    .epub_navigation_pending
                    .as_ref()
                    .map(|pending| pending.direction)
                    .unwrap_or(EpubNavigationDirection::Next);
                self.epub_navigation_pending = None;
                self.epub_navigation_started_tick = None;
                if !outcome.advanced {
                    if matches!(direction, EpubNavigationDirection::Next) && outcome.reached_end {
                        self.epub_overlay = Some(EpubOverlay::Finished);
                    } else {
                        log::warn!("[EPUB] unable to advance {} page", direction.label());
                    }
                } else {
                    self.persist_active_epub_position();
                    self.reset_epub_failure_streak();
                }
                outcome.advanced || outcome.reached_end
            }
            Ok(Err(error)) => {
                log::warn!("[EPUB] page turn worker failed: {}", error);
                self.epub_navigation_pending = None;
                self.epub_navigation_started_tick = None;
                self.handle_epub_runtime_failure(error);
                true
            }
            Err(TryRecvError::Empty) => {
                if let Some(start_tick) = self.epub_navigation_started_tick {
                    let elapsed = self.ui_tick.saturating_sub(start_tick);
                    if elapsed > Self::EPUB_NAV_TIMEOUT_TICKS {
                        self.epub_navigation_pending = None;
                        self.epub_navigation_started_tick = None;
                        self.handle_epub_runtime_failure(
                            "Unable to change EPUB page: timed out".to_string(),
                        );
                        return true;
                    }
                }
                false
            }
            Err(TryRecvError::Disconnected) => {
                self.epub_navigation_pending = None;
                self.epub_navigation_started_tick = None;
                self.handle_epub_runtime_failure(
                    "Unable to change EPUB page: worker disconnected".to_string(),
                );
                true
            }
        }
    }

    #[cfg(feature = "std")]
    fn prepare_epub_open_source(
        fs: &mut dyn FileSystem,
        path: &str,
    ) -> Result<EpubOpenSource, String> {
        #[cfg(target_os = "espidf")]
        let _ = fs;

        if let Some(host_path) = Self::resolve_host_backed_epub_path(path) {
            return Ok(EpubOpenSource::HostPath(host_path));
        }

        #[cfg(target_os = "espidf")]
        {
            Err(format!(
                "Unable to open EPUB: file not reachable via VFS path ({})",
                path
            ))
        }

        #[cfg(not(target_os = "espidf"))]
        {
            let bytes = fs
                .read_file_bytes(path)
                .map_err(|e| format!("Unable to read EPUB: {}", e))?;
            if bytes.is_empty() {
                return Err("Unable to read EPUB: empty file".to_string());
            }

            Ok(EpubOpenSource::Bytes(bytes))
        }
    }

    #[cfg(all(feature = "std", not(target_os = "espidf")))]
    #[inline(never)]
    fn spawn_epub_open_worker(
        fs: &mut dyn FileSystem,
        path: &str,
        settings: ReaderSettings,
    ) -> Result<Receiver<EpubOpenWorkerEvent>, String> {
        let source = Self::prepare_epub_open_source(fs, path)?;
        log::info!(
            "[EPUB] spawn open worker stack={}B path={}",
            Self::EPUB_OPEN_WORKER_STACK_BYTES,
            path
        );
        let (tx, rx) = mpsc::channel();
        let builder = thread::Builder::new()
            .name("epub-open-worker".to_string())
            .stack_size(Self::EPUB_OPEN_WORKER_STACK_BYTES);
        builder
            .spawn(move || {
                let _ = tx.send(EpubOpenWorkerEvent::Phase("Preparing"));
                let result =
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match source {
                        EpubOpenSource::HostPath(path) => {
                            let _ = tx.send(EpubOpenWorkerEvent::Phase("Parsing"));
                            #[cfg(target_os = "espidf")]
                            {
                                EpubReadingState::from_sd_path_light(&path, settings).and_then(
                                    |mut state| {
                                        state.ensure_initial_page_loaded()?;
                                        Ok(state)
                                    },
                                )
                            }
                            #[cfg(not(target_os = "espidf"))]
                            {
                                match File::open(&path) {
                                    Ok(file) => {
                                        EpubReadingState::from_reader(Box::new(file), settings)
                                    }
                                    Err(err) => Err(format!("Unable to read EPUB: {}", err)),
                                }
                            }
                        }
                        #[cfg(not(target_os = "espidf"))]
                        EpubOpenSource::Bytes(bytes) => {
                            let _ = tx.send(EpubOpenWorkerEvent::Phase("Parsing"));
                            EpubReadingState::from_reader(Box::new(Cursor::new(bytes)), settings)
                        }
                    })) {
                        Ok(result) => result,
                        Err(_) => Err("Unable to open EPUB: worker panicked".to_string()),
                    };
                let result = result.map(|state| Arc::new(Mutex::new(state)));
                let _ = tx.send(EpubOpenWorkerEvent::Done(result));
            })
            .map_err(|e| format!("Unable to start EPUB worker: {}", e))?;
        Ok(rx)
    }

    #[cfg(feature = "std")]
    pub(super) fn spawn_epub_navigation_worker(
        renderer: Arc<Mutex<EpubReadingState>>,
        direction: EpubNavigationDirection,
    ) -> Result<Receiver<Result<EpubNavigationOutcome, String>>, String> {
        log::info!(
            "[EPUB] spawn nav worker stack={}B direction={}",
            Self::EPUB_NAV_WORKER_STACK_BYTES,
            direction.label()
        );
        let (tx, rx) = mpsc::channel();
        let builder = thread::Builder::new()
            .name("epub-nav-worker".to_string())
            .stack_size(Self::EPUB_NAV_WORKER_STACK_BYTES);
        builder
            .spawn(move || {
                let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match renderer
                    .lock()
                {
                    Ok(mut renderer) => match direction {
                        EpubNavigationDirection::Next => {
                            let advanced = renderer.next_page();
                            let reached_end = renderer.take_last_next_page_reached_end();
                            Ok(EpubNavigationOutcome {
                                advanced,
                                reached_end,
                            })
                        }
                        EpubNavigationDirection::Prev => Ok(EpubNavigationOutcome {
                            advanced: renderer.prev_page(),
                            reached_end: false,
                        }),
                    },
                    Err(_) => Err("Unable to change EPUB page: worker poisoned".to_string()),
                }));
                let _ = match run {
                    Ok(result) => tx.send(result),
                    Err(_) => tx.send(Err(
                        "Unable to change EPUB page: worker panicked".to_string()
                    )),
                };
            })
            .map_err(|e| format!("Unable to start EPUB navigation worker: {}", e))?;
        Ok(rx)
    }

    #[cfg(all(feature = "std", target_os = "espidf"))]
    fn spawn_epub_initial_load_worker(
        renderer: Arc<Mutex<EpubReadingState>>,
    ) -> Result<Receiver<Result<(), String>>, String> {
        let (tx, rx) = mpsc::channel();
        let builder = thread::Builder::new()
            .name("epub-init-worker".to_string())
            .stack_size(Self::EPUB_NAV_WORKER_STACK_BYTES);
        builder
            .spawn(move || {
                let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match renderer
                    .lock()
                {
                    Ok(mut renderer) => renderer.ensure_initial_page_loaded(),
                    Err(poisoned) => poisoned.into_inner().ensure_initial_page_loaded(),
                }));
                let _ = match run {
                    Ok(result) => tx.send(result),
                    Err(_) => tx.send(Err(
                        "Unable to open EPUB: initial page worker panicked".to_string()
                    )),
                };
            })
            .map_err(|e| format!("Unable to start EPUB initial-load worker: {}", e))?;
        Ok(rx)
    }

    #[cfg(feature = "std")]
    const EPUB_STATE_DIR: &'static str = if cfg!(target_os = "espidf") {
        "/sd/.xteink"
    } else {
        "/tmp/.xteink"
    };

    #[cfg(feature = "std")]
    const EPUB_STATE_FILE: &'static str = if cfg!(target_os = "espidf") {
        "/sd/.xteink/reader_state.tsv"
    } else {
        "/tmp/.xteink/reader_state.tsv"
    };

    #[cfg(feature = "std")]
    const EPUB_LAST_SESSION_FILE: &'static str = if cfg!(target_os = "espidf") {
        "/sd/.xteink/last_session.tsv"
    } else {
        "/tmp/.xteink/last_session.tsv"
    };
    #[cfg(feature = "std")]
    const EPUB_BOOKMARKS_FILE: &'static str = if cfg!(target_os = "espidf") {
        "/sd/.xteink/bookmarks.tsv"
    } else {
        "/tmp/.xteink/bookmarks.tsv"
    };

    #[cfg(feature = "std")]
    const EPUB_STATE_MAX_BOOKS: usize = 256;
    #[cfg(feature = "std")]
    const EPUB_STATE_MAX_CHAPTER_COUNTS: usize = 512;

    #[cfg(feature = "std")]
    pub(super) fn persist_active_epub_position(&mut self) {
        let Some(path) = self.active_epub_path.clone() else {
            return;
        };
        let BrowserMode::ReadingEpub { renderer } = &self.mode else {
            return;
        };
        let (chapter_idx, page_idx) = match renderer.lock() {
            Ok(guard) => guard.position_indices(),
            Err(poisoned) => poisoned.into_inner().position_indices(),
        };
        let chapter_counts = match renderer.lock() {
            Ok(guard) => guard.exact_chapter_page_counts(),
            Err(poisoned) => poisoned.into_inner().exact_chapter_page_counts(),
        };
        if let Err(err) =
            Self::persist_epub_position_for_path(&path, chapter_idx, page_idx, &chapter_counts)
        {
            log::warn!("[EPUB] unable to persist reading position: {}", err);
        }
    }

    pub(super) fn restore_active_epub_position(&mut self, renderer: &Arc<Mutex<EpubReadingState>>) {
        let Some(path) = self.active_epub_path.as_ref() else {
            return;
        };
        let Some(saved) = Self::load_epub_state_for_path(path) else {
            return;
        };
        let restored = match renderer.lock() {
            Ok(mut guard) => {
                guard.apply_exact_chapter_page_counts(&saved.chapter_counts);
                guard.restore_position(saved.chapter_idx, saved.page_idx)
            }
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                guard.apply_exact_chapter_page_counts(&saved.chapter_counts);
                guard.restore_position(saved.chapter_idx, saved.page_idx)
            }
        };
        if restored {
            log::info!(
                "[EPUB] restored position path={} chapter={} page={}",
                path,
                saved.chapter_idx,
                saved.page_idx
            );
        } else {
            log::warn!(
                "[EPUB] failed to restore position path={} chapter={} page={}",
                path,
                saved.chapter_idx,
                saved.page_idx
            );
        }
    }

    #[cfg(feature = "std")]
    fn load_epub_state_for_path(path: &str) -> Option<PersistedEpubState> {
        let raw = std::fs::read_to_string(Self::EPUB_STATE_FILE).ok()?;
        for line in raw.lines() {
            let mut fields = line.split('\t');
            let Some(saved_path) = fields.next() else {
                continue;
            };
            if saved_path != path {
                continue;
            }
            let chapter_idx = fields.next().and_then(|v| v.parse::<usize>().ok())?;
            let page_idx = fields.next().and_then(|v| v.parse::<usize>().ok())?;
            let chapter_counts = fields
                .next()
                .map(Self::decode_chapter_counts)
                .unwrap_or_default();
            return Some(PersistedEpubState {
                chapter_idx,
                page_idx,
                chapter_counts,
            });
        }
        None
    }

    #[cfg(feature = "std")]
    pub(super) fn persist_epub_position_for_path(
        path: &str,
        chapter_idx: usize,
        page_idx: usize,
        chapter_counts: &[(usize, usize)],
    ) -> Result<(), String> {
        std::fs::create_dir_all(Self::EPUB_STATE_DIR).map_err(|e| e.to_string())?;
        let mut state: BTreeMap<String, PersistedEpubState> = BTreeMap::new();
        if let Ok(raw) = std::fs::read_to_string(Self::EPUB_STATE_FILE) {
            for line in raw.lines() {
                let mut fields = line.split('\t');
                let Some(saved_path) = fields.next() else {
                    continue;
                };
                let Some(saved_chapter) = fields.next().and_then(|v| v.parse::<usize>().ok())
                else {
                    continue;
                };
                let Some(saved_page) = fields.next().and_then(|v| v.parse::<usize>().ok()) else {
                    continue;
                };
                let saved_counts = fields
                    .next()
                    .map(Self::decode_chapter_counts)
                    .unwrap_or_default();
                state.insert(
                    saved_path.to_string(),
                    PersistedEpubState {
                        chapter_idx: saved_chapter,
                        page_idx: saved_page,
                        chapter_counts: saved_counts,
                    },
                );
            }
        }
        state.insert(
            path.to_string(),
            PersistedEpubState {
                chapter_idx,
                page_idx,
                chapter_counts: chapter_counts
                    .iter()
                    .copied()
                    .take(Self::EPUB_STATE_MAX_CHAPTER_COUNTS)
                    .collect(),
            },
        );
        while state.len() > Self::EPUB_STATE_MAX_BOOKS {
            let Some(oldest_key) = state.keys().next().cloned() else {
                break;
            };
            state.remove(&oldest_key);
        }
        let mut out = String::new();
        for (saved_path, saved) in state {
            out.push_str(&saved_path);
            out.push('\t');
            out.push_str(&saved.chapter_idx.to_string());
            out.push('\t');
            out.push_str(&saved.page_idx.to_string());
            out.push('\t');
            out.push_str(&Self::encode_chapter_counts(&saved.chapter_counts));
            out.push('\n');
        }
        std::fs::write(Self::EPUB_STATE_FILE, out).map_err(|e| e.to_string())?;
        Self::persist_last_active_content_path(path)?;
        Self::persist_last_active_epub_path(path, chapter_idx, page_idx)?;
        Ok(())
    }

    #[cfg(feature = "std")]
    pub(super) fn persist_epub_bookmark_for_path(
        path: &str,
        chapter_idx: usize,
        page_idx: usize,
    ) -> Result<(), String> {
        std::fs::create_dir_all(Self::EPUB_STATE_DIR).map_err(|e| e.to_string())?;
        let mut bookmarks: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        if let Ok(raw) = std::fs::read_to_string(Self::EPUB_BOOKMARKS_FILE) {
            for line in raw.lines() {
                let mut fields = line.split('\t');
                let Some(saved_path) = fields.next() else {
                    continue;
                };
                let Some(saved_chapter) = fields.next().and_then(|v| v.parse::<usize>().ok())
                else {
                    continue;
                };
                let Some(saved_page) = fields.next().and_then(|v| v.parse::<usize>().ok()) else {
                    continue;
                };
                bookmarks.insert(saved_path.to_string(), (saved_chapter, saved_page));
            }
        }

        bookmarks.insert(path.to_string(), (chapter_idx, page_idx));
        while bookmarks.len() > Self::EPUB_STATE_MAX_BOOKS {
            let Some(oldest_key) = bookmarks.keys().next().cloned() else {
                break;
            };
            bookmarks.remove(&oldest_key);
        }

        let mut out = String::new();
        for (saved_path, (saved_chapter, saved_page)) in bookmarks {
            out.push_str(&saved_path);
            out.push('\t');
            out.push_str(&saved_chapter.to_string());
            out.push('\t');
            out.push_str(&saved_page.to_string());
            out.push('\n');
        }
        std::fs::write(Self::EPUB_BOOKMARKS_FILE, out).map_err(|e| e.to_string())
    }

    #[cfg(feature = "std")]
    pub(super) fn load_epub_bookmark_for_path(path: &str) -> Option<(usize, usize)> {
        let raw = std::fs::read_to_string(Self::EPUB_BOOKMARKS_FILE).ok()?;
        for line in raw.lines() {
            let mut fields = line.split('\t');
            let Some(saved_path) = fields.next() else {
                continue;
            };
            if saved_path != path {
                continue;
            }
            let chapter_idx = fields.next().and_then(|v| v.parse::<usize>().ok())?;
            let page_idx = fields.next().and_then(|v| v.parse::<usize>().ok())?;
            return Some((chapter_idx, page_idx));
        }
        None
    }

    #[cfg(feature = "std")]
    fn persist_last_active_epub_path(
        path: &str,
        chapter_idx: usize,
        page_idx: usize,
    ) -> Result<(), String> {
        let mut out = String::new();
        out.push_str(path);
        out.push('\t');
        out.push_str(&chapter_idx.to_string());
        out.push('\t');
        out.push_str(&page_idx.to_string());
        out.push('\n');
        std::fs::write(Self::EPUB_LAST_SESSION_FILE, out).map_err(|e| e.to_string())
    }

    #[cfg(feature = "std")]
    fn encode_chapter_counts(counts: &[(usize, usize)]) -> String {
        let mut out = String::new();
        for (idx, (chapter_idx, count)) in counts.iter().copied().enumerate() {
            if idx > 0 {
                out.push(',');
            }
            out.push_str(&chapter_idx.to_string());
            out.push(':');
            out.push_str(&count.max(1).to_string());
        }
        out
    }

    #[cfg(feature = "std")]
    fn decode_chapter_counts(raw: &str) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for item in raw.split(',') {
            let mut parts = item.split(':');
            let Some(chapter_idx) = parts.next().and_then(|v| v.parse::<usize>().ok()) else {
                continue;
            };
            let Some(count) = parts.next().and_then(|v| v.parse::<usize>().ok()) else {
                continue;
            };
            out.push((chapter_idx, count.max(1)));
        }
        out
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

        candidates
            .into_iter()
            .find(|candidate| std::fs::File::open(candidate).is_ok())
    }
}
