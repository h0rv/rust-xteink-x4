# E-Reader Minimum Gaps Execution Tracker

Scope: execution tracker for minimum gaps required to make the device a usable daily e-reader.

## 1. Reliable On-Device Resume
- Status: `In progress (core path landed)`
- Owner: `TBD`
- Acceptance criteria:
  - Last reading position persists to storage on page turn and on clean exit.
  - After reboot, opening the same book resumes at the saved position (chapter + offset/page index).
  - Corrupt/missing resume state falls back to book start without crash.
- Next concrete engineering tasks:
  - Verify on physical ESP32-C3 that restored positions are stable across cold reboot and power-loss.
  - Add write throttling/debounce to reduce SD write amplification under rapid page turns.
  - Add scenario test for reboot-resume flow once harness supports reboot simulation.
  - Add telemetry/log counters for restore success/fallback rates.

## 2. Bookmarks
- Status: `In progress (single-slot bookmark landed)`
- Owner: `TBD`
- Acceptance criteria:
  - User can add/remove bookmark at current position from reader UI.
  - Bookmark list for a book is persisted and survives reboot.
  - Selecting a bookmark opens the exact bookmarked position.
- Next concrete engineering tasks:
  - Extend from current single bookmark per book to multi-bookmark list per book.
  - Add delete/rename actions and a small bookmark picker overlay.
  - Add tests for stale chapter/page fallback and persistence corruption handling.
  - Decide cap policy (max bookmarks per book / total bookmarks).

## 3. EPUB Image Rendering On Device
- Status: `In progress (device path hardened; memory guards expanded)`
- Owner: `TBD`
- Acceptance criteria:
  - Inline EPUB images render in reading flow on hardware and simulator.
  - Oversized images are scaled to fit content area without layout corruption.
  - Unsupported image formats fail gracefully (text remains readable, no panic).
- Next concrete engineering tasks:
  - Add fixture tests for oversized/decompression-bomb inline images.
  - Add on-device validation pass for mixed text+image chapters across multiple books.
  - Add telemetry counters for inline image cache hit/miss and decode failures.
  - Validate memory ceiling under rapid page-turn with image-heavy chapters.

## 4. Hyphenation/Typography
- Status: `In progress (language + policy wiring landed)`
- Owner: `TBD`
- Acceptance criteria:
  - Long words no longer overflow line bounds in normal body text.
  - Paragraph spacing/line-height defaults are readable and consistent.
  - Rendering remains deterministic across simulator and device for same content.
- Next concrete engineering tasks:
  - Tune typography defaults (line height, paragraph spacing, margins) in reader theme.
  - Add layout golden tests for edge-case words and narrow-width pages.
  - Benchmark pagination/render cost to confirm acceptable turn latency.

## Worklog (Latest)
- Wired `ActivityResult::NavigateTo/NavigateBack` in `App` so reader overlay actions no longer drop navigation intents silently.
- Hardened library scan startup path: scan errors now keep existing library state and avoid persisting empty snapshots.
- Added image memory guardrails before decode (`file_info` size gate + host-path size checks).
- Hardened inline EPUB image decode path with dimension/pixel ceilings before decode and more aggressive cache cleanup on retry recovery.
- Moved ESP-IDF initial EPUB page load off the deferred UI tick into a worker path with timeout/disconnect handling.
- Added per-file text resume persistence (`text_state.tsv`) and restore-on-open for text files.
- Added explicit last-opened-content semantics (`last_content.tsv`) used by app startup auto-resume (not EPUB-only).

## 5. Library Recents/Search
- Status: `Not started`
- Owner: `TBD`
- Acceptance criteria:
  - Library shows a recents section ordered by last-opened timestamp.
  - Search filters library by title/author with case-insensitive matching.
  - Metadata/index updates after library scan without full app restart.
- Next concrete engineering tasks:
  - Persist `last_opened_at` on open/resume and expose sorted recents query.
  - Add lightweight in-memory search index over scanned library metadata.
  - Implement library UI states for empty, query active, and no-results.
  - Add tests for ranking, prefix/substring matching, and metadata refresh behavior.
