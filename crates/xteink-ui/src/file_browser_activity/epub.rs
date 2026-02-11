use super::*;

#[cfg(feature = "std")]
impl EpubReadingState {
    #[cfg(not(target_os = "espidf"))]
    const MAX_FONT_FACE_BYTES: usize = 512 * 1024;
    #[cfg(not(target_os = "espidf"))]
    const MAX_FONT_TOTAL_BYTES: usize = 2 * 1024 * 1024;
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
    #[allow(dead_code)]
    const PAGE_CACHE_LIMIT: usize = 0;
    #[cfg(not(target_os = "espidf"))]
    const PAGE_CACHE_LIMIT: usize = 8;
    const OUT_OF_RANGE_ERR: &'static str = "Requested EPUB page is out of range";

    fn from_reader(reader: Box<dyn ReadSeek>) -> Result<Self, String> {
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
        let known_total = self.chapter_page_counts.get(&self.chapter_idx).copied();
        let can_advance = known_total.is_none() || self.page_idx + 1 < known_total.unwrap_or(0);
        if can_advance {
            let next_idx = self.page_idx + 1;
            if let Ok(page) = self.load_page(self.chapter_idx, next_idx) {
                self.page_idx = next_idx;
                self.current_page = Some(page);
                return true;
            }
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

        let mut target_page: Option<RenderPage> = None;
        #[cfg(target_os = "espidf")]
        let config = RenderConfig::default().with_page_range(page_idx..page_idx + 1);
        #[cfg(not(target_os = "espidf"))]
        let config = RenderConfig::default()
            .with_page_range(page_idx..page_idx + 1)
            .with_cache(&self.render_cache);
        let mut session = self.engine.begin(chapter_idx, config);
        let mut layout_error: Option<String> = None;
        let chapter_opts = ChapterEventsOptions {
            max_items: Self::MAX_CHAPTER_EVENTS,
            ..ChapterEventsOptions::default()
        };

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
            )
            .map_err(|e| format!("Unable to stream EPUB chapter: {}", e))?;
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

        let page = target_page.ok_or_else(|| Self::OUT_OF_RANGE_ERR.to_string())?;
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
            let font_limits = FontLimits {
                max_faces: 16,
                max_bytes_per_font: Self::MAX_FONT_FACE_BYTES,
                max_total_font_bytes: Self::MAX_FONT_TOTAL_BYTES,
            };
            let Ok(embedded) = self.book.embedded_fonts_with_limits(font_limits) else {
                return;
            };
            for face in embedded {
                let italic = matches!(
                    face.style,
                    EmbeddedFontStyle::Italic | EmbeddedFontStyle::Oblique
                );
                let mut bytes = Vec::new();
                let Ok(_) = self.book.read_resource_into_with_limit(
                    &face.href,
                    &mut bytes,
                    Self::MAX_FONT_FACE_BYTES,
                ) else {
                    continue;
                };
                let registration = [FontFaceRegistration {
                    family: &face.family,
                    weight: face.weight,
                    italic,
                    data: &bytes,
                }];
                let _ = self.eg_renderer.register_faces(&registration);
            }
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
        if let Some(host_path) = Self::resolve_host_backed_epub_path(path) {
            return Ok(EpubOpenSource::HostPath(host_path));
        }

        let mut chunks: Vec<Vec<u8>> = Vec::new();
        let mut on_chunk = |chunk: &[u8]| -> Result<(), crate::filesystem::FileSystemError> {
            chunks.push(chunk.to_vec());
            Ok(())
        };
        fs.read_file_chunks(path, Self::EPUB_READ_CHUNK_BYTES, &mut on_chunk)
            .map_err(|e| format!("Unable to read EPUB: {}", e))?;

        if chunks.is_empty() {
            return Err("Unable to read EPUB: empty file".to_string());
        }

        Ok(EpubOpenSource::Chunks(chunks))
    }

    #[cfg(feature = "std")]
    #[inline(never)]
    fn spawn_epub_open_worker(
        fs: &mut dyn FileSystem,
        path: &str,
    ) -> Result<Receiver<Result<EpubReadingState, String>>, String> {
        let source = Self::prepare_epub_open_source(fs, path)?;
        let (tx, rx) = mpsc::channel();
        let builder = thread::Builder::new()
            .name("epub-open-worker".to_string())
            .stack_size(Self::EPUB_WORKER_STACK_BYTES);
        builder
            .spawn(move || {
                let result = match source {
                    EpubOpenSource::HostPath(path) => match File::open(&path) {
                        Ok(file) => EpubReadingState::from_reader(Box::new(file)),
                        Err(err) => Err(format!("Unable to read EPUB: {}", err)),
                    },
                    EpubOpenSource::Chunks(chunks) => EpubReadingState::from_reader(Box::new(
                        ChunkedEpubReader::from_chunks(chunks),
                    )),
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
        let (tx, rx) = mpsc::channel();
        let builder = thread::Builder::new()
            .name("epub-nav-worker".to_string())
            .stack_size(Self::EPUB_WORKER_STACK_BYTES);
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

        for candidate in candidates {
            if File::open(&candidate).is_ok() {
                return Some(candidate);
            }
        }
        None
    }
}
