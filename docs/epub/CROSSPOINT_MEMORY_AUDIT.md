# CrossPoint Memory Audit And EPUB Architecture Comparison

Date: 2026-03-06

Scope:
- CrossPoint reference implementation in `crosspoint-reader/`
- Current Rust EPUB stack in `einked/` and `epub-stream/`
- Focus on initialization order, EPUB open/render/cache flow, buffer allocation strategy, fragmentation risk, and architecture boundaries

## Executive Summary

CrossPoint is more robust on-device because it treats RAM as transient working space and SD as the long-lived backing store for book state. It streams metadata and chapter inputs to temporary files early, persists section/page artifacts to disk, and reloads only the current page on demand. It accepts some small temporary heap allocations, but it avoids keeping large EPUB-derived working sets alive across the session.

The current Rust stack is better factored at the library boundary, but its runtime shape is still more RAM-centric than CrossPoint's. `epub-stream` exposes caller-controlled scratch and has a temp-storage open path, and the reader now uses upstream binary chapter artifacts instead of app-owned page JSON, but the hot chapter path still wants a contiguous uncompressed chapter buffer and render prep still does too much one-shot chapter work on cache miss. The result is a system that is architecturally cleaner, but still more sensitive to heap shape and large-chapter first-open worst cases.

The practical target should be:
- keep metadata and structure caches on SD
- keep only the current chapter/page working set in RAM
- avoid full-chapter pinned buffers across the session
- remove JSON-heavy cache write/read paths from the embedded hot path
- shift from "page fetch by rerender" toward "chapter artifact + page lookup"

## Current Boundary And Execution Tracker

Latest confirmed device state from `flash.log`:
- the earlier post-`open_ready` heap OOM in ereader session assembly was real and drove the transient-session refactor
- the latest device failure moved earlier again and now crashes on the first temp-backed ZIP entry read:
  - `[EPUB-TEMP] open_begin`
  - `[EPUB-TEMP] zip_ready`
  - then `Stack protection fault`
- the highest-confidence root cause is the ZIP inflate state being constructed with `Box::new(InflateState::new(...))`, which builds a large `InflateState` on the stack before moving it to the heap
- the current fix is to enable `miniz_oxide`'s `with-alloc` feature and use its heap-first `InflateState::new_boxed(...)` constructor in `epub-stream/src/zip.rs`

This changes the immediate priority order. The next validation target is the ZIP open path itself, not the ereader session shape.

Latest follow-up after the ZIP fix:
- the ZIP/open stack fault is gone
- the remaining device fault is now in post-open reader work after `[EPUB-TEMP] open_ready`
- current mitigation work in flight:
  - always-on ESP release markers in the reader runtime
  - no-inline boundaries around page prepare and page bitmap raster
  - heap-owned transient worker members to avoid rebuilding `RenderEngine` and `EpubSessionBook` as large stack locals

### Highest-Value Refactor Order

1. `In progress`: remove the monolithic boxed ereader session allocation.
Current shape:
- `EpubSession` owns `EpubBook`, `RenderEngine`, cache store, resources, and reader state together
- that pushes the embedded reader toward one large contiguous allocation after book open

Current progress:
- the ereader session has been moved toward a compact handle shape
- heavy `EpubBook` and `RenderEngine` ownership is being removed from persistent session state
- cache-store creation is now deferred/lazy instead of happening during session open

Target shape:
- persistent reader state holds only:
  - book identity/path
  - pagination profile
  - chapter/page cursor
  - optional current page bitmap
  - cache root path or cache key
- heavy open/layout state becomes transient

Reason:
- this is now the most likely source of the `13160` byte device failure
- even if it is not the only allocation there, it is the wrong memory shape for a fragmented embedded heap

2. Defer cache-store creation until first actual cache use.
Current shape:
- cache store is created during session open

Target shape:
- opening a book should not allocate cache machinery
- create `FileRenderCacheStore` only on first page artifact read/write

Reason:
- the cache store is policy, not mandatory live state
- this is consistent with CrossPoint's “book opens first, section artifacts become active when needed” shape

3. Stop keeping parser/layout machinery in long-lived UI state.
Current shape:
- `EpubBook` and `RenderEngine` live inside the reader session

Target shape:
- persistent session stores tiny reader state only
- uncached chapter work uses transient open/layout workers that are dropped immediately after:
  - page artifact build
  - first-page load
  - cache miss recovery

Reason:
- this is the maintainable embedded boundary
- it decouples UI state lifetime from parser/layout lifetime

4. Move from “session object” thinking to “reader service” thinking.
Target internal model:
- `open_book_compact(path) -> compact session state`
- `load_page_artifact(session, chapter, page) -> page`
- `build_chapter_artifact_if_missing(session, chapter) -> artifact metadata`

Reason:
- CrossPoint's real strength is artifact-backed page service, not a large live session object
- this also removes policy leakage from `HomeActivity`

5. After the session shape is fixed, attack the remaining upstream long pole.
Still remaining after the above:
- uncached chapter build still relies on a full contiguous uncompressed chapter buffer upstream

Reason:
- this is still the last major architectural risk for large chapters
- but it is now the second blocker, not the first one

### Local Validation Loop

Use local tooling for almost all iteration:
- `just ui-heap-profile-epub`
- `just ui-heap-profile-epub phase=epub_open_first_page fragment=1`
- `just test-ui-memory`
- `just epub-temp-open-profile`

What those local tools now cover:
- exact failing accessibility EPUB fixture
- temp-backed open path with embedded limits
- fragmented allocator regression on host
- host-side first-page runtime peak tracking
- DHAT ownership for temp-open allocations

What still requires device confirmation:
- ESP-specific allocator shape after temp-open succeeds
- any final stack/heap interactions that host allocators do not reproduce

### Robust Target State

The durable end state should be:
- `epub-stream` owns parsing, temp-backed open, chapter artifact build, and page artifact IO
- `einked-ereader` owns only reader cursor state and UI policy
- heavy book/layout/cache machinery is transient and file-backed
- steady-state live RAM is:
  - framebuffer
  - current page bitmap or current page data
  - bounded decode/layout scratch

That is the closest maintainable Rust equivalent to CrossPoint's behavior without copying CrossPoint's code structure directly.

## What CrossPoint Does Well

### 0. Its initialization is staged and uses stable global storage

CrossPoint's boot path is deliberately staged:
- storage and power setup happen before reader reopen logic
- display/framebuffer state is allocated as a stable subsystem cost
- reader reopen is guarded so a bad book does not trap the device in a boot loop

Relevant references:
- startup sequencing in `crosspoint-reader/src/main.cpp:227` and `crosspoint-reader/src/main.cpp:286`
- display/framebuffer setup in `crosspoint-reader/open-x4-sdk/libs/display/EInkDisplay/src/EInkDisplay.cpp:129`

This is worth calling out because CrossPoint spends fixed memory early and then tries hard not to create more long-lived EPUB memory afterward.

### 1. It streams core EPUB metadata work to disk immediately

CrossPoint does not load `container.xml`, `content.opf`, NCX, nav, or CSS fully into long-lived heap objects by default.

Examples:
- `container.xml` is size-checked and parsed through a streaming parser in `crosspoint-reader/lib/Epub/Epub.cpp:15`
- `content.opf` is streamed into `ContentOpfParser` in `crosspoint-reader/lib/Epub/Epub.cpp:47`
- NCX and NAV are first streamed to temp files, then parsed from file with a fixed 1 KB buffer in `crosspoint-reader/lib/Epub/Epub.cpp:152` and `crosspoint-reader/lib/Epub/Epub.cpp:209`
- CSS files are extracted to a temp file, parsed, cached, and then cleared in `crosspoint-reader/lib/Epub/Epub.cpp:257`

Why this matters:
- parsing does not require one large retained heap object per metadata file
- the temp-file step converts fragmented heap pressure into SD I/O
- the implementation intentionally spends I/O to keep heap shape survivable

### 2. It persists book structure early and reloads from cache

CrossPoint's `BookMetadataCache` is the core reason it feels stable on repeat opens.

Examples:
- it loads cached book metadata first in `crosspoint-reader/lib/Epub/Epub.cpp:342`
- if cache is missing, it builds the spine and TOC in passes and writes them to temp binary files immediately in `crosspoint-reader/lib/Epub/Epub.cpp:368`
- it then builds `book.bin` with a compact LUT and metadata payload in `crosspoint-reader/lib/Epub/Epub/BookMetadataCache.cpp:99`
- for large books, it uses a hash-indexed spine lookup and a one-pass ZIP size fill rather than loading all ZIP stats into RAM in `crosspoint-reader/lib/Epub/Epub/BookMetadataCache.cpp:51` and `crosspoint-reader/lib/Epub/Epub/BookMetadataCache.cpp:176`

Why this matters:
- CrossPoint pays the indexing cost once and reloads a compact representation afterward
- it explicitly chooses CPU and SD work over heap retention
- large-book handling is special-cased around RAM limits, not treated like a normal desktop path

### 3. It caches chapter pagination output as section files on SD

This is the biggest divergence from the current Rust path.

CrossPoint does not keep full rendered chapters in RAM. It builds a per-section file keyed by layout parameters and stores pages to that file as they are produced.

Examples:
- section cache validity is keyed by font, viewport, line compression, paragraph spacing, hyphenation, embedded-style flag, and image mode in `crosspoint-reader/lib/Epub/Epub/Section.cpp:36` and `crosspoint-reader/lib/Epub/Epub/Section.cpp:63`
- the chapter HTML is first streamed to a temp file in `crosspoint-reader/lib/Epub/Epub/Section.cpp:134`
- pages are serialized incrementally as they are completed in `crosspoint-reader/lib/Epub/Epub/Section.cpp:19`
- a LUT of page offsets is appended so a single page can be reloaded without reparsing the chapter in `crosspoint-reader/lib/Epub/Epub/Section.cpp:224`
- page reload is direct file seek plus deserialize in `crosspoint-reader/lib/Epub/Epub/Section.cpp:253`

Why this matters:
- live RAM only needs the current page object, not a whole chapter page vector
- page turns are largely "seek and deserialize" instead of "rerender chapter and drain one page"
- fragmentation pressure moves from live heap to bounded file I/O

### 4. It treats the framebuffer as stable, not as the problem

CrossPoint keeps a full display framebuffer and draws into it directly.

Examples:
- renderer grabs the display framebuffer once in `crosspoint-reader/lib/GfxRenderer/GfxRenderer.cpp:18`
- rendering writes pixels directly into that buffer in `crosspoint-reader/lib/GfxRenderer/GfxRenderer.cpp:159`
- the buffer is flushed only at the end in `crosspoint-reader/lib/GfxRenderer/GfxRenderer.cpp:816`

Why this matters:
- CrossPoint's stability is not coming from avoiding a framebuffer
- the stable full-screen buffer is treated as a fixed cost
- the real optimization effort is spent on EPUB-side artifacts and transient row buffers

### 5. It uses small, scoped allocations for image decoding

CrossPoint does allocate during rendering, but mostly in row-sized or operation-sized chunks.

Examples:
- bitmap render allocates row buffers sized to the image row and frees them after use in `crosspoint-reader/lib/GfxRenderer/GfxRenderer.cpp:595`
- 1-bit bitmap render does the same in `crosspoint-reader/lib/GfxRenderer/GfxRenderer.cpp:678`
- scanline polygon fill allocates only a small node list and frees it immediately in `crosspoint-reader/lib/GfxRenderer/GfxRenderer.cpp:748`

This is not zero-allocation, but it is scoped allocation. The important point is that these allocations do not become a long-lived fragmented EPUB session footprint.

### 6. It has explicit heap-aware policy in a few risky places

CrossPoint is willing to skip work when memory is not there.

Examples:
- CSS parsing is skipped below a heap threshold and large CSS files are skipped outright in `crosspoint-reader/lib/Epub/Epub.cpp:258`
- large spine handling avoids preloading central-directory metadata in `crosspoint-reader/lib/Epub/Epub/BookMetadataCache.cpp:176`

That is not elegant, but it is embedded-realistic.

### 7. It is not allocation-free, but its worst spikes are localized

CrossPoint still has a few memory-risky paths:
- `ZipFile::readFileToMemory()` can allocate both compressed input and final output buffers for some reads in `crosspoint-reader/lib/ZipFile/ZipFile.cpp:384`
- the guide-cover fallback uses that byte-buffer path in `crosspoint-reader/lib/Epub/Epub.cpp:86`
- extracted image files and pixel caches can accumulate on SD until the book cache is cleared in `crosspoint-reader/lib/Epub/Epub/Section.cpp:193` and `crosspoint-reader/lib/Epub/Epub/blocks/ImageBlock.cpp:22`

The important distinction is that these are exception paths or cache growth issues. They are not the core steady-state page-turn model. The mainline reader flow still relies on disk-backed section artifacts rather than chapter-sized live buffers.

## What The Current Rust Stack Does Well

### 1. The core EPUB library is more cleanly separated

`epub-stream` is cleaner than CrossPoint structurally.

Examples:
- `open_with_temp_storage` is a dedicated low-RAM open path in `epub-stream/src/book.rs:822`
- `chapter_events_with_scratch` is explicitly caller-buffer-driven and documents its memory behavior in `epub-stream/src/book.rs:1784`
- the render layer exposes a neutral persisted page schema in `epub-stream/crates/epub-stream-render/src/persisted.rs:10`

This is a better API shape than CrossPoint in terms of library design. The problem is that the runtime still does not fully exploit that architecture on device.

### 2. Open-time metadata flow is already moving in the right direction

The temp-storage open path is conceptually aligned with CrossPoint:
- container and OPF are streamed to temp files in `epub-stream/src/book.rs:878` and `epub-stream/src/book.rs:900`
- navigation can be deferred with `lazy_navigation` in `epub-stream/src/book.rs:932`
- zip scratch is kept in explicit vectors in `epub-stream/src/book.rs:870`

That part is good and should be kept.

### 3. Reader state ownership is cleaner than before

The current ereader now keeps live reader cursor state in `EpubSession.reader` rather than leaking it through modal state, and it no longer keeps a live `Vec<RenderPage>` window in session state.

Examples:
- session-owned reader state lives in `einked/crates/einked-ereader/src/lib.rs`
- modal state is just `EpubReader` mode in `einked/crates/einked-ereader/src/lib.rs`
- `EpubResources` now keeps only the optional page bitmap, not a retained page window, in `einked/crates/einked-ereader/src/lib.rs`

This is better than the older split modal/session design.

### 4. The current stack already degrades under pressure better than before

Examples:
- page bitmap allocation is lazy in `einked/crates/einked-ereader/src/lib.rs`
- streamed image rasterization degrades cleanly to non-streamed page rasterization when image decoding fails in `einked/crates/einked-ereader/src/lib.rs`
- embedded render prep now skips intrinsic image-dimension pre-scan, so image assets stay on-demand instead of being inventoried up front in `epub-stream/src/render_prep.rs`
- EPUB open/navigation are now synchronous on the reader path rather than bouncing between main and worker thread policies in `einked/crates/einked-ereader/src/lib.rs`

This is the right survival behavior. It just does not yet remove the underlying chapter-buffer problem.

## Where We Diverge From CrossPoint

## Current Refactor Status

Completed in the current refactor:
- temp-backed EPUB open now uses unique temp files with scoped cleanup in `epub-stream/src/book.rs`
- `epub-stream-render` now owns a binary chapter-artifact format with page-count and page-at-a-time lookup
- `einked-ereader` now uses the upstream artifact store instead of an app-owned per-page JSON cache
- native-path opens now prefer the temp-storage path on both host and device
- the leaked raw-pointer page bitmap wrapper has been replaced with an owned buffer
- embedded temp open now parses spine first and only retains manifest entries needed for spine/nav/cover lookup
- the ereader session no longer retains a live `Vec<RenderPage>` page window or text fallback page; only reader position plus optional bitmap survive between page turns
- embedded render prep no longer inventories chapter image sources and intrinsic image dimensions up front
- the reader no longer switches EPUB navigation/open between synchronous and worker-thread execution models

Still meaningfully behind CrossPoint:
- first-open and first-cache-build still require a full chapter reflow before page turns become cheap
- `HomeActivity` still owns too much EPUB session/open/navigation policy
- metadata/index caching is still weaker than CrossPoint's `book.bin`-style structure cache
- `epub-stream` still retains more package metadata than a true compact section index
- `chapter_events_with_scratch` still requires a full contiguous uncompressed chapter buffer on cache miss

## 1. First-open still pays too much live chapter work

This is now narrower than it was before.

The ereader no longer allocates a chapter-sized buffer in app code for page navigation. It now asks `RenderEngine` for a page-range render and uses the upstream artifact cache for reuse. That closes the old `chapter_buf` anti-pattern in the app layer.

The remaining gap is that a cache miss still means:
- restream the chapter through render prep
- reflow the chapter
- build or extend the chapter artifact

CrossPoint also pays a one-time chapter pagination cost, but its section-file model is the core runtime path rather than an added cache behind a windowed reader model.

Impact:
- the repeated page-turn path is much better than before because it can hit a persisted chapter artifact
- the worst remaining risk is now first-open and first-cache-build cost, not the old app-side chapter buffer
- if large chapters still fail, the next work belongs upstream in render prep/layout and artifact build strategy, not back in UI-layer scratch management

Target:
- keep reducing first-open chapter reflow cost and verify the remaining largest single allocations in DHAT/device logs
- if needed, add a more explicitly file-backed chapter-prep/artifact-build path upstream

## 2. The cache shape is now much closer, but the runtime model still is not

What is fixed:
- the app-side per-page JSON cache is gone
- `epub-stream-render` now persists a compact binary chapter artifact with page payloads and a LUT-like page lookup API
- the ereader now asks the upstream cache for page count and individual pages

CrossPoint behavior:
- a section file stores all pages for one chapter/layout key plus a LUT in `crosspoint-reader/lib/Epub/Epub/Section.cpp:224`
- a page turn is a seek into that file in `crosspoint-reader/lib/Epub/Epub/Section.cpp:258`

Impact:
- the artifact format divergence is mostly closed
- the remaining divergence is behavioral: the reader now navigates by cursor state and artifact lookups, but `HomeActivity` still owns too much of that policy
- artifact build is still coupled to full chapter reflow on miss

Target:
- extract the remaining artifact/open/navigation policy out of `HomeActivity` so the reader path becomes one deterministic session machine

## 3. Reader session state is now close; the remaining gap is first-build cost

Current behavior:
- the ereader session now keeps only chapter/page cursor state and an optional page bitmap
- page turns go through artifact lookup or page-at-a-time rerender; they do not keep a live `RenderPage` window around
- the remaining expensive path is the first cache miss for a chapter, not steady-state page-turn retention

CrossPoint also keeps only the current page live. The main remaining difference is that CrossPoint's section build path is more aggressively file-backed end to end.

Impact:
- our steady-state in-memory state is now much closer to CrossPoint
- the long pole is still the one-time chapter build path, especially for large chapters

Target:
- move the remaining chapter-build peak down by replacing full-chapter contiguous prep with a more file-backed or chunked path

## 4. Our caches and session policy still leak into the app layer

The Rust code is cleaner than before, but `HomeActivity` still owns too much EPUB orchestration.

Examples:
- open policy, cache creation, bitmap allocation, and navigation policy still largely live in `einked/crates/einked-ereader/src/lib.rs`
- `release_non_reader_state` still explicitly drops files and feed sources before EPUB open in `einked/crates/einked-ereader/src/lib.rs`

CrossPoint is not cleaner globally, but its reader path is more opinionated around one concrete artifact model: build cache, open section, load page.

Impact:
- the app layer still knows too much about EPUB runtime policy
- it is harder to reason about memory budgets because policy is spread across open/render/UI code

Target:
- move open/load/cache/page-artifact policy behind a dedicated ereader session module
- `HomeActivity` should tell the session "open book", "next page", "change style", not own buffer/caching decisions

## 5. The upstream render cache API is now on the right side of the boundary

This gap is largely closed.

`epub-stream-render` now owns the embedded chapter-artifact format and page-oriented cache API. That was the right refactor direction because it keeps pagination/render identity with the renderer instead of the UI crate.

What remains:
- verify that the artifact writer/load path stays bounded under real firmware memory profiles
- avoid drifting back toward whole-chapter compatibility paths in the embedded hot path
- decide whether metadata/index caching should also move lower or remain a thin app/session concern

## Encapsulation And Design Issues In The Current Stack

### Good boundaries

- `epub-stream` separates open, metadata, zip, and chapter streaming better than CrossPoint
- `epub-stream-render` now owns the neutral persisted page schema instead of the app crate
- the reader's live cursor state is now session-owned

### Weak boundaries

1. `HomeActivity` is still acting as controller, resource manager, cache policy owner, worker-thread policy owner, and renderer adapter all at once.
2. `EpubResources` mixes stable session state with optional rendering acceleration state. `page_window` and `page_bitmap` have different lifetimes and should not necessarily live behind the same policy object.
3. `release_non_reader_state` is a symptom of global heap competition rather than a well-encapsulated resource budget.

### Anti-patterns still present

1. Open-time and navigation-time policies are still being selected from UI code.
2. Page retrieval still falls back to rerender/reflow on cache miss instead of treating artifact build as a more explicit session concern.
3. The reader still keeps live page windows as state instead of using artifact-backed page lookup as the single source of truth.
4. `EpubResources` still couples window state and bitmap acceleration in one object.

## Additional Production Risks

### Temp-storage hygiene is fixed, but temp-root policy is still basic

What is fixed:
- unique temp filenames per open
- cleanup on both success and failure
- temp-backed open is used earlier in the session flow

What still deserves attention:
- temp-root selection is now centralized, but still simple and path-based
- there is still room for a more explicit lower-level session/open abstraction so UI code stops caring about native-path resolution details

### Host and ESP behavior diverge more than they should

This gap is smaller now because host native-path opens also prefer the temp-storage path. The remaining divergence is in the fallback path when a `FileStore` cannot provide a native path.

## What We Should Be Striving For

## Phase 1: Make the runtime shape match CrossPoint's strengths

### A. Persist a chapter artifact, not just page JSON

Target artifact per `(book, pagination_profile, chapter)`:
- header with version and layout key
- chapter page count
- page LUT
- page payloads in compact binary
- optional image/object sidecar references

Then:
- page turns become "seek page N"
- reopen becomes "load artifact if present, rebuild if stale"
- the live RAM set is current page + small decode scratch

### B. Move from full-chapter decompression to file-backed or chunk-backed prep

Best end state:
- EPUB entry is decompressed incrementally into a temp artifact or into a chunked prep pipeline
- render prep consumes chunks or a file reader
- no requirement for one `Vec<u8>` equal to full chapter size

This is the single most important upstream change.

### C. Keep only stable fixed-cost graphics state in RAM

This means:
- framebuffer stays
- optional page bitmap stays lazy
- image decode buffers stay bounded and scoped
- no persistent chapter-sized buffers

### D. Use binary caches on device

Recommended split:
- metadata/index cache: compact binary
- chapter/page artifact cache: compact binary
- debug/export cache: optional JSON on host only

### E. Centralize EPUB runtime policy in one session module

The app should not need to know:
- when to use a worker thread
- how big chapter scratch is
- when snapshot writes should be skipped
- whether page navigation comes from cache artifact or rerender

Those belong in a dedicated reader/session layer.

## Phase 2: Use upstream cache APIs, but change their embedded shape

`epub-stream-render` should remain the owner of persisted render schema. But for embedded, it needs:
- a chapter artifact writer that can append pages and a final LUT
- a page-at-a-time reload API
- a compact binary format
- a lower-allocation cache load path than "read whole JSON file into `Vec<u8>`"

## Concrete Divergence Checklist

Use this as the working tracker.

- Done: `epub-stream/src/book.rs`
  Temp-storage open now uses unique temp files and scoped cleanup.

- Done: `epub-stream/crates/epub-stream-render/src/render_engine.rs`
  Render cache API now supports append/finalize/page-load against a binary chapter artifact.

- Done: `einked/crates/einked-ereader/src/lib.rs`
  Page load now uses upstream artifact lookup instead of app-side per-page JSON cache.

- Remaining: `einked/crates/einked-ereader/src/lib.rs`
  Reader navigation still rebuilds/stores live page windows instead of treating artifact lookup as the primary state model.

- Remaining: `einked/crates/einked-ereader/src/lib.rs`
  Open/runtime policy still lives in `HomeActivity` and still competes with unrelated app state.

- Remaining: upstream render prep / first-open profiling
  First-open and first-artifact-build still need tighter measurement and possibly more file-backed staging if large chapters remain risky.

## Next Steps To Diverge Less From CrossPoint

This is the recommended order of work. The goal is not to clone CrossPoint's code. The goal is to adopt the same memory shape where it matters.

### Completed foundation

1. Move embedded page/chapter caching ownership into `epub-stream-render`.
2. Replace per-page JSON with a binary chapter artifact.
3. Add page-at-a-time cache lookup APIs.
4. Fix temp-file hygiene and move temp-backed open earlier in the session flow.
5. Remove the leaked raw-pointer page bitmap owner.

### Remaining highest-leverage work: make the ereader navigate by artifact, not by live page window rebuild

1. On open, attempt to load chapter artifact for the current profile.
2. If present, load only the requested page and adjacent metadata.
3. If missing or stale, build the artifact once, then switch over to page-by-page lookup.
4. Reduce `EpubPageWindow` to a UI convenience, not the primary persistence/navigation model.

Why this is next:
- this is the point where runtime behavior actually starts to look like CrossPoint
- once done, `EPUB_PAGE_WINDOW = 1` stops implying "rerender constantly"

### Long pole: reduce first-open and first-artifact-build memory cost

1. Profile the current artifact-build miss path on real books and DHAT/device logs.
2. Preferred end states:
   - more file-backed chapter-prep/artifact-build staging
   - lower peak memory during full-chapter reflow
   - less repeated work during first-open on large chapters

Why this is the long pole:
- page turns and reopen behavior are already on a much better path
- the next real risk is the first cache miss on a large chapter
- that is the place where CrossPoint still has the stronger memory shape

### In parallel: clean up runtime boundaries

1. Move EPUB open/cache/navigation policy out of `HomeActivity` into a dedicated session module.
2. Separate persistent reader state from optional render acceleration state more explicitly.
3. Revisit whether `release_non_reader_state` can be replaced by a more explicit reader memory budget.

Why this matters:
- these are the main design/encapsulation problems left in the current stack
- they are not the first source of fragmentation, but they make the system harder to evolve safely

## Minimal Device-Usable Milestone

The smallest change set that should materially improve device usability is:

1. Chapter artifact cache with LUT in `epub-stream-render`
2. `einked-ereader` switched to artifact-first page lookup
3. robust temp-file handling

That milestone does not fully solve large-chapter contiguous-buffer risk, but it should remove the current "rerender chapter for page misses" behavior and make the device meaningfully more stable.

## Full CrossPoint-Parity Direction

To really converge toward CrossPoint's runtime shape:

1. metadata/index cache on SD
2. chapter artifact cache on SD
3. page-at-a-time reload from artifact
4. bounded image decode scratch
5. no chapter-sized persistent session allocations
6. no chapter-sized contiguous hot-path allocations

At that point, the remaining difference is mostly implementation language and API cleanliness, not runtime memory behavior.

## Bottom Line

CrossPoint is not winning because it has a better framebuffer or a dramatically better parser API. It is winning because its runtime memory shape is more embedded-native:
- stream into temp files
- persist compact book and section artifacts to SD
- reload one page at a time
- keep heap working sets short-lived and small

The current Rust stack already has the cleaner long-term architecture. What it still lacks is CrossPoint's file-backed artifact discipline in the hot path. That is the gap to close.
