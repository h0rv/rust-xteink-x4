use super::*;

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

    #[cfg(not(target_os = "espidf"))]
    pub(super) fn from_reader(reader: Box<dyn ReadSeek>) -> Result<Self, String> {
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
        let mut state = Self {
            book,
            engine: RenderEngine::new(RenderEngineOptions::for_display(
                crate::DISPLAY_WIDTH as i32,
                crate::DISPLAY_HEIGHT as i32,
            )),
            eg_renderer: Self::create_renderer(),
            chapter_buf: Vec::with_capacity(Self::CHAPTER_BUF_CAPACITY_BYTES),
            chapter_scratch: ScratchBuffers::embedded(),
            current_page: None,
            page_cache: BTreeMap::new(),
            #[cfg(not(target_os = "espidf"))]
            render_cache: InMemoryRenderCache::default(),
            chapter_page_counts: BTreeMap::new(),
            chapter_idx: 0,
            page_idx: 0,
        };
        state.register_embedded_fonts();
        state.load_chapter_forward(0)?;
        log::info!("[EPUB] initial chapter/page loaded");
        Ok(state)
    }

    #[cfg(target_os = "espidf")]
    pub(super) fn from_sd_path(path: &str) -> Result<Self, String> {
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
        let mut state = Self {
            book,
            engine: RenderEngine::new(RenderEngineOptions::for_display(
                crate::DISPLAY_WIDTH as i32,
                crate::DISPLAY_HEIGHT as i32,
            )),
            eg_renderer: Self::create_renderer(),
            chapter_buf: Vec::with_capacity(Self::CHAPTER_BUF_CAPACITY_BYTES),
            chapter_scratch: ScratchBuffers::embedded(),
            current_page: None,
            page_cache: BTreeMap::new(),
            #[cfg(not(target_os = "espidf"))]
            render_cache: InMemoryRenderCache::default(),
            chapter_page_counts: BTreeMap::new(),
            chapter_idx: 0,
            page_idx: 0,
        };
        state.register_embedded_fonts();
        state.load_chapter_forward(0)?;
        log::info!("[EPUB] initial chapter/page loaded");
        Ok(state)
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
                Err(err) if Self::is_out_of_range_error(&err) => continue,
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
        self.chapter_idx + 1
    }

    pub(super) fn total_chapters(&self) -> usize {
        self.book.chapter_count()
    }

    pub(super) fn current_page_number(&self) -> usize {
        self.page_idx + 1
    }

    pub(super) fn total_pages(&self) -> usize {
        self.chapter_page_counts
            .get(&self.chapter_idx)
            .copied()
            .unwrap_or(1)
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
            ..ChapterEventsOptions::default()
        };
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
            self.chapter_page_counts.insert(chapter_idx, count);
        }

        #[cfg(not(target_os = "espidf"))]
        {
            self.page_cache
                .insert((chapter_idx, page_idx), page.clone());
            self.trim_page_cache();
        }
        Ok(page)
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
        #[cfg(all(feature = "std", feature = "fontdue", not(target_os = "espidf")))]
        {
            EgRenderer::with_backend(cfg, BookerlyFontBackend::default())
        }
        #[cfg(any(
            all(feature = "std", not(feature = "fontdue")),
            all(feature = "std", target_os = "espidf")
        ))]
        {
            EgRenderer::with_backend(cfg, mu_epub_embedded_graphics::MonoFontBackend)
        }
    }

    fn register_embedded_fonts(&mut self) {
        #[cfg(target_os = "espidf")]
        {
            // On-device we default to bundled font families (e.g. Bookerly) and
            // avoid eager runtime TTF parsing to keep EPUB open deterministic.
        }

        #[cfg(not(target_os = "espidf"))]
        {
            // Keep EPUB open deterministic across host and embedded flows by
            // avoiding eager runtime TTF parsing during open.
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
            match Self::spawn_epub_open_worker(fs, path) {
                Ok(receiver) => {
                    self.epub_open_pending = Some(PendingEpubOpen { receiver });
                    self.mode = BrowserMode::OpeningEpub;
                    self.browser
                        .set_status_message(format!("Opening EPUB: {}", basename(path)));
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

    #[cfg(feature = "std")]
    pub(super) fn poll_epub_open_result(&mut self) -> bool {
        let recv_result = match self.epub_open_pending.as_mut() {
            Some(pending) => pending.receiver.try_recv(),
            None => return false,
        };

        match recv_result {
            Ok(Ok(renderer)) => {
                self.epub_open_pending = None;
                self.epub_navigation_pending = None;
                self.mode = BrowserMode::ReadingEpub {
                    renderer: Arc::new(Mutex::new(renderer)),
                };
                true
            }
            Ok(Err(error)) => {
                self.epub_open_pending = None;
                self.mode = BrowserMode::Browsing;
                self.browser.set_status_message(error);
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.epub_open_pending = None;
                self.mode = BrowserMode::Browsing;
                self.browser
                    .set_status_message("Unable to open EPUB: worker disconnected".to_string());
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
            Ok(Ok(advanced)) => {
                let direction = self
                    .epub_navigation_pending
                    .as_ref()
                    .map(|pending| pending.direction.label())
                    .unwrap_or("page");
                self.epub_navigation_pending = None;
                if !advanced {
                    log::warn!("[EPUB] unable to advance {} page", direction);
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

    #[cfg(feature = "std")]
    #[inline(never)]
    fn spawn_epub_open_worker(
        fs: &mut dyn FileSystem,
        path: &str,
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
                    EpubOpenSource::HostPath(path) => {
                        #[cfg(target_os = "espidf")]
                        {
                            EpubReadingState::from_sd_path(&path)
                        }
                        #[cfg(not(target_os = "espidf"))]
                        {
                            match File::open(&path) {
                                Ok(file) => EpubReadingState::from_reader(Box::new(file)),
                                Err(err) => Err(format!("Unable to read EPUB: {}", err)),
                            }
                        }
                    }
                    #[cfg(not(target_os = "espidf"))]
                    EpubOpenSource::Bytes(bytes) => {
                        EpubReadingState::from_reader(Box::new(Cursor::new(bytes)))
                    }
                };
                let _ = tx.send(result);
            })
            .map_err(|e| format!("Unable to start EPUB worker: {}", e))?;
        Ok(rx)
    }

    #[cfg(feature = "std")]
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
