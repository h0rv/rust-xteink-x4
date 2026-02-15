use super::*;
use mu_epub::book::Locator;
use mu_epub::RenderPrepOptions;

#[cfg(feature = "std")]
impl EpubReadingState {
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
    #[cfg(target_os = "espidf")]
    const EPUB_TEMP_DIR: &'static str = "/sd/.tmp";
    #[cfg(target_os = "espidf")]
    #[allow(dead_code)]
    const PAGE_CACHE_LIMIT: usize = 0;
    #[cfg(not(target_os = "espidf"))]
    const PAGE_CACHE_LIMIT: usize = 8;
    const OUT_OF_RANGE_ERR: &'static str = "Requested EPUB page is out of range";

    fn create_render_options(settings: ReaderSettings) -> (RenderEngineOptions, RenderPrepOptions) {
        let mut opts = RenderEngineOptions::for_display(
            crate::DISPLAY_WIDTH as i32,
            crate::DISPLAY_HEIGHT as i32,
        );
        let mut layout = opts.layout;
        let side_margin = match settings.margin_size {
            crate::reader_settings_activity::MarginSize::Small => 8,
            crate::reader_settings_activity::MarginSize::Medium => 16,
            crate::reader_settings_activity::MarginSize::Large => 24,
        };
        layout.margin_left = side_margin;
        layout.margin_right = side_margin;
        layout.margin_top = 18;
        layout.margin_bottom = 50;
        layout.first_line_indent_px = 10;
        layout.paragraph_gap_px = match settings.line_spacing {
            crate::reader_settings_activity::LineSpacing::Compact => 5,
            crate::reader_settings_activity::LineSpacing::Normal => 7,
            crate::reader_settings_activity::LineSpacing::Relaxed => 10,
        };
        layout.line_gap_px = match settings.line_spacing {
            crate::reader_settings_activity::LineSpacing::Compact => 0,
            crate::reader_settings_activity::LineSpacing::Normal => 1,
            crate::reader_settings_activity::LineSpacing::Relaxed => 3,
        };
        layout.typography.justification.enabled = matches!(
            settings.text_alignment,
            crate::reader_settings_activity::TextAlignment::Justified
        );
        layout.typography.justification.min_words = 5;
        layout.typography.justification.min_fill_ratio = 0.62;
        opts.layout = layout;

        let base_font = settings.font_size.epub_base_px();
        let text_scale = settings.font_size.epub_text_scale();
        let mut hints = opts.prep.layout_hints;
        hints.base_font_size_px = base_font;
        hints.text_scale = text_scale;
        hints.min_font_size_px = (base_font * 0.8).max(12.0);
        hints.max_font_size_px = (base_font * 2.8).min(72.0);
        let line_height = settings.line_spacing.multiplier() as f32 / 10.0;
        hints.min_line_height = line_height.max(1.1);
        hints.max_line_height = (line_height + 0.3).min(2.4);
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
        let mut state = Self {
            book,
            engine,
            chapter_events_opts,
            eg_renderer: Self::create_renderer(),
            chapter_buf: Vec::with_capacity(Self::CHAPTER_BUF_CAPACITY_BYTES),
            chapter_scratch: ScratchBuffers::embedded(),
            current_page: None,
            page_cache: BTreeMap::new(),
            #[cfg(not(target_os = "espidf"))]
            render_cache: InMemoryRenderCache::default(),
            chapter_page_counts: BTreeMap::new(),
            chapter_page_counts_exact: BTreeSet::new(),
            non_renderable_chapters: BTreeSet::new(),
            chapter_idx: 0,
            page_idx: 0,
        };
        state.register_embedded_fonts();
        state.load_chapter_forward(0)?;
        log::info!("[EPUB] initial chapter/page loaded");
        Ok(state)
    }

    #[cfg(target_os = "espidf")]
    pub(super) fn from_sd_path_light(path: &str, settings: ReaderSettings) -> Result<Self, String> {
        log::info!("[EPUB] opening reader (sd temp)");
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
        log::info!("[EPUB] open ok: chapters={}", book.chapter_count());
        log::info!("[EPUB] creating render engine");
        let (engine, chapter_events_opts) = Self::create_engine(settings);
        log::info!("[EPUB] render engine ready");
        let mut state = Self {
            book,
            engine,
            chapter_events_opts,
            eg_renderer: Self::create_renderer(),
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
            chapter_idx: 0,
            page_idx: 0,
        };
        log::info!("[EPUB] reader state allocated (lazy buffers)");
        state.register_embedded_fonts();
        log::info!("[EPUB] reader state ready");
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
                Err(err) if Self::is_out_of_range_error(&err) => {
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
                Err(err) if Self::is_out_of_range_error(&err) => {
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
        self.chapter_idx + 1 - skipped_before
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
        if self.chapter_page_counts_exact.contains(&self.chapter_idx) {
            format!("p{}/{}", current, total)
        } else {
            format!("p{}", current)
        }
    }

    pub(super) fn chapter_progress_label(&self) -> String {
        format!("c{}/{}", self.current_chapter(), self.total_chapters())
    }

    pub(super) fn position_indices(&self) -> (usize, usize) {
        (self.chapter_idx, self.page_idx)
    }

    pub(super) fn book_progress_percent(&self) -> u8 {
        let total_chapters = self.total_chapters().max(1);
        let chapter_zero_based = self
            .current_chapter()
            .saturating_sub(1)
            .min(total_chapters - 1);
        let is_last_chapter = chapter_zero_based + 1 >= total_chapters;
        let total_pages = self.total_pages().max(1);
        let at_last_page = self.chapter_page_counts_exact.contains(&self.chapter_idx)
            && self.current_page_number() >= total_pages;
        if is_last_chapter && at_last_page {
            return 100;
        }

        let page_portion = if self.chapter_page_counts_exact.contains(&self.chapter_idx) {
            (self.page_idx as f32 / total_pages as f32) / total_chapters as f32
        } else {
            // Unknown chapter total: make progress monotonic while avoiding
            // overconfident percentages from temporary 1/1 placeholders.
            (self.page_idx as f32 / (self.page_idx + 2) as f32) / total_chapters as f32
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
        let mut title = href
            .rsplit('/')
            .next()
            .unwrap_or(href.as_str())
            .split('#')
            .next()
            .unwrap_or(href.as_str())
            .split('.')
            .next()
            .unwrap_or(href.as_str())
            .replace(['_', '-'], " ");
        if title.is_empty() {
            title = fallback;
        }
        let mut out = String::new();
        let mut count = 0usize;
        for ch in title.chars() {
            if count + 1 >= max_chars {
                out.push('…');
                break;
            }
            out.push(ch);
            count += 1;
        }
        if out.is_empty() {
            title
        } else {
            out
        }
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
        let pct = percent.min(100) as usize;
        let mut target = (pct * chapters) / 100;
        if target >= chapters {
            target = chapters - 1;
        }
        self.jump_to_chapter(target)
    }

    pub(super) fn restore_position(&mut self, chapter_idx: usize, page_idx: usize) -> bool {
        if chapter_idx >= self.book.chapter_count() {
            return false;
        }
        let prev_chapter = self.chapter_idx;
        let prev_page = self.page_idx;
        self.current_page = None;

        if let Ok(page) = self.load_page(chapter_idx, page_idx) {
            self.chapter_idx = chapter_idx;
            self.page_idx = page_idx;
            self.current_page = Some(page);
            return true;
        }

        if self.load_chapter_exact(chapter_idx).is_ok() {
            if page_idx > 0 {
                let mut idx = 1usize;
                while idx <= page_idx {
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
        self.current_page = None;
        self.page_cache.clear();
        self.chapter_page_counts.clear();
        self.chapter_page_counts_exact.clear();

        if let Ok(page) = self.load_page(current_chapter, current_page) {
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
        let previous_chapter = self.chapter_idx;
        let previous_page = self.page_idx;
        // Free the currently rendered page before loading the next one to
        // maximize contiguous heap on constrained devices.
        self.current_page = None;
        let next_idx = self.page_idx + 1;
        if let Ok(page) = self.load_page(self.chapter_idx, next_idx) {
            self.page_idx = next_idx;
            self.current_page = Some(page);
            return true;
        }
        if self.chapter_idx + 1 < self.book.chapter_count()
            && self.load_chapter_forward(self.chapter_idx + 1).is_ok()
        {
            self.chapter_page_counts
                .entry(previous_chapter)
                .and_modify(|count| *count = (*count).max(previous_page + 1))
                .or_insert(previous_page + 1);
            self.chapter_page_counts_exact.insert(previous_chapter);
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

    pub(super) fn prev_page(&mut self) -> bool {
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
                if total_prev <= 1 {
                    // `load_chapter_backward` already loaded page 0 for this chapter.
                    // Avoid re-loading the same page, which needlessly re-streams the
                    // chapter and can trigger allocation failures on constrained heaps.
                    return true;
                }
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

        if let Ok(page) = self.load_page(previous_chapter, previous_page) {
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

        if let Ok(page) = self.load_page(previous_chapter, previous_page) {
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

        let chapter_opts = ChapterEventsOptions {
            max_items: Self::MAX_CHAPTER_EVENTS,
            render: self.chapter_events_opts,
            ..ChapterEventsOptions::default()
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
                log::info!(
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
            let config = RenderConfig::default().with_page_range(page_idx..page_idx + 1);
            #[cfg(not(target_os = "espidf"))]
            let config = RenderConfig::default()
                .with_page_range(page_idx..page_idx + 1)
                .with_cache(&self.render_cache);
            let mut session = self.engine.begin(chapter_idx, config);
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
            log::info!("[EPUB] chapter_events streamed chapter={}", chapter_idx);

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
        log::info!(
            "[EPUB] load_page ok chapter={} page={} total_in_chapter={:?}",
            chapter_idx,
            page_idx,
            page.metrics.chapter_page_count
        );

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
            ..ChapterEventsOptions::default()
        };
        let mut count = 0usize;
        let mut session = self.engine.begin(chapter_idx, RenderConfig::default());
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

    fn create_renderer() -> ReaderRenderer {
        let cfg = EgRenderConfig::default();
        #[cfg(all(feature = "std", feature = "fontdue"))]
        {
            EgRenderer::with_backend(cfg, BookerlyFontBackend::default())
        }
        #[cfg(all(feature = "std", not(feature = "fontdue")))]
        {
            EgRenderer::with_backend(cfg, mu_epub_embedded_graphics::MonoFontBackend)
        }
    }

    fn register_embedded_fonts(&mut self) {
        #[cfg(target_os = "espidf")]
        {
            // Embedded font resource loading can require large contiguous
            // allocations (font blobs), which is not reliable on constrained
            // ESP heaps during open. Keep open deterministic and use bundled
            // fonts on device.
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
        }
        #[cfg(all(feature = "std", not(target_os = "espidf")))]
        {
            match Self::spawn_epub_open_worker(fs, path, self.reader_settings) {
                Ok(receiver) => {
                    self.epub_open_pending = Some(PendingEpubOpen { receiver });
                    self.mode = BrowserMode::OpeningEpub;
                    self.browser
                        .set_status_message(format!("Opening EPUB: {}", basename(path)));
                }
                Err(error) => {
                    self.mode = BrowserMode::Browsing;
                    self.browser.set_status_message(error);
                    self.active_epub_path = None;
                }
            }
        }

        #[cfg(all(feature = "std", target_os = "espidf"))]
        {
            match Self::prepare_epub_open_source(fs, path) {
                Ok(EpubOpenSource::HostPath(host_path)) => {
                    self.mode = BrowserMode::OpeningEpub;
                    self.browser
                        .set_status_message(format!("Opening EPUB: {}", basename(path)));
                    match EpubReadingState::from_sd_path_light(&host_path, self.reader_settings) {
                        Ok(renderer) => {
                            self.epub_open_staged = Some(Arc::new(Mutex::new(renderer)));
                            self.queue_task(FileBrowserTask::FinalizeEpubOpen);
                        }
                        Err(error) => {
                            self.mode = BrowserMode::Browsing;
                            self.browser.set_status_message(error);
                            self.active_epub_path = None;
                        }
                    }
                }
                Err(error) => {
                    self.mode = BrowserMode::Browsing;
                    self.browser.set_status_message(error);
                    self.active_epub_path = None;
                }
                #[allow(unreachable_patterns)]
                _ => {
                    self.mode = BrowserMode::Browsing;
                    self.browser
                        .set_status_message("Unable to open EPUB: unsupported source".to_string());
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
            Ok(Ok(renderer)) => {
                self.epub_open_pending = None;
                self.epub_navigation_pending = None;
                let renderer = Arc::new(Mutex::new(renderer));
                self.restore_active_epub_position(&renderer);
                self.mode = BrowserMode::ReadingEpub { renderer };
                true
            }
            Ok(Err(error)) => {
                self.epub_open_pending = None;
                self.mode = BrowserMode::Browsing;
                self.browser.set_status_message(error);
                self.active_epub_path = None;
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.epub_open_pending = None;
                self.mode = BrowserMode::Browsing;
                self.browser
                    .set_status_message("Unable to open EPUB: worker disconnected".to_string());
                self.active_epub_path = None;
                true
            }
        }
    }

    #[cfg(all(feature = "std", not(target_os = "espidf")))]
    pub(super) fn poll_epub_navigation_result(&mut self) -> bool {
        let recv_result = match self.epub_navigation_pending.as_mut() {
            Some(pending) => pending.receiver.try_recv(),
            None => return false,
        };

        match recv_result {
            Ok(Ok(advanced)) => {
                let direction = self
                    .epub_navigation_pending
                    .as_ref()
                    .map(|pending| pending.direction.label())
                    .unwrap_or("page");
                self.epub_navigation_pending = None;
                if !advanced {
                    log::warn!("[EPUB] unable to advance {} page", direction);
                } else {
                    self.persist_active_epub_position();
                }
                advanced
            }
            Ok(Err(error)) => {
                log::warn!("[EPUB] page turn worker failed: {}", error);
                self.epub_navigation_pending = None;
                self.browser.set_status_message(error);
                false
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.epub_navigation_pending = None;
                self.browser.set_status_message(
                    "Unable to change EPUB page: worker disconnected".to_string(),
                );
                false
            }
        }
    }

    #[cfg(all(feature = "std", target_os = "espidf"))]
    pub(super) fn process_finalize_epub_open_task(&mut self) -> bool {
        let Some(renderer) = self.epub_open_staged.take() else {
            self.mode = BrowserMode::Browsing;
            self.browser
                .set_status_message("Unable to open EPUB: staged state missing".to_string());
            return true;
        };

        let init_result = match renderer.lock() {
            Ok(mut guard) => guard.ensure_initial_page_loaded(),
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                guard.ensure_initial_page_loaded()
            }
        };

        match init_result {
            Ok(()) => {
                self.restore_active_epub_position(&renderer);
                self.mode = BrowserMode::ReadingEpub { renderer };
            }
            Err(error) => {
                self.mode = BrowserMode::Browsing;
                self.browser.set_status_message(error);
                self.active_epub_path = None;
            }
        }

        true
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
    ) -> Result<Receiver<Result<EpubReadingState, String>>, String> {
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
                let result = match source {
                    EpubOpenSource::HostPath(path) => match File::open(&path) {
                        Ok(file) => EpubReadingState::from_reader(Box::new(file), settings),
                        Err(err) => Err(format!("Unable to read EPUB: {}", err)),
                    },
                    EpubOpenSource::Bytes(bytes) => {
                        EpubReadingState::from_reader(Box::new(Cursor::new(bytes)), settings)
                    }
                };
                let _ = tx.send(result);
            })
            .map_err(|e| format!("Unable to start EPUB worker: {}", e))?;
        Ok(rx)
    }

    #[cfg(all(feature = "std", not(target_os = "espidf")))]
    pub(super) fn spawn_epub_navigation_worker(
        renderer: Arc<Mutex<EpubReadingState>>,
        direction: EpubNavigationDirection,
    ) -> Result<Receiver<Result<bool, String>>, String> {
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
                let advanced = match renderer.lock() {
                    Ok(mut renderer) => match direction {
                        EpubNavigationDirection::Next => renderer.next_page(),
                        EpubNavigationDirection::Prev => renderer.prev_page(),
                    },
                    Err(_) => {
                        let _ = tx.send(Err(
                            "Unable to change EPUB page: worker poisoned".to_string()
                        ));
                        return;
                    }
                };
                let _ = tx.send(Ok(advanced));
            })
            .map_err(|e| format!("Unable to start EPUB navigation worker: {}", e))?;
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
    const EPUB_STATE_MAX_BOOKS: usize = 256;

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
        if let Err(err) = Self::persist_epub_position_for_path(&path, chapter_idx, page_idx) {
            log::warn!("[EPUB] unable to persist reading position: {}", err);
        }
    }

    #[cfg(feature = "std")]
    pub(super) fn restore_active_epub_position(&mut self, renderer: &Arc<Mutex<EpubReadingState>>) {
        let Some(path) = self.active_epub_path.as_ref() else {
            return;
        };
        let Some((chapter_idx, page_idx)) = Self::load_epub_position_for_path(path) else {
            return;
        };
        let restored = match renderer.lock() {
            Ok(mut guard) => guard.restore_position(chapter_idx, page_idx),
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                guard.restore_position(chapter_idx, page_idx)
            }
        };
        if restored {
            log::info!(
                "[EPUB] restored position path={} chapter={} page={}",
                path,
                chapter_idx,
                page_idx
            );
        }
    }

    #[cfg(feature = "std")]
    fn load_epub_position_for_path(path: &str) -> Option<(usize, usize)> {
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
            return Some((chapter_idx, page_idx));
        }
        None
    }

    #[cfg(feature = "std")]
    pub(super) fn persist_epub_position_for_path(
        path: &str,
        chapter_idx: usize,
        page_idx: usize,
    ) -> Result<(), String> {
        std::fs::create_dir_all(Self::EPUB_STATE_DIR).map_err(|e| e.to_string())?;
        let mut state: BTreeMap<String, (usize, usize)> = BTreeMap::new();
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
                state.insert(saved_path.to_string(), (saved_chapter, saved_page));
            }
        }
        state.insert(path.to_string(), (chapter_idx, page_idx));
        while state.len() > Self::EPUB_STATE_MAX_BOOKS {
            let Some(oldest_key) = state.keys().next().cloned() else {
                break;
            };
            state.remove(&oldest_key);
        }
        let mut out = String::new();
        for (saved_path, (saved_chapter, saved_page)) in state {
            out.push_str(&saved_path);
            out.push('\t');
            out.push_str(&saved_chapter.to_string());
            out.push('\t');
            out.push_str(&saved_page.to_string());
            out.push('\n');
        }
        std::fs::write(Self::EPUB_STATE_FILE, out).map_err(|e| e.to_string())
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
            .find(|candidate| File::open(candidate).is_ok())
    }
}
