# Einked Migration Parity Tracker

## Objective
Restore firmware UX/runtime behavior to at least pre-einked quality while keeping the new architecture:
- `einked` remains generic and device-agnostic.
- firmware owns device/hardware constraints and policies.
- `einked-ereader` is app-level and configurable, not target-coupled.

## Current High-Risk Gaps
- [~] EPUB open/read path still unstable on device memory pressure.
- [x] Library scan/listing parity is incomplete (pre-migration recursive behavior missing).
- [~] Feed offline/enable UX needs fully event-driven behavior (single clear flow).
- [~] Refresh/runtime policy has migration-era debug artifacts and needs final hardening.

## Pre-Migration Behavior To Match
- [~] EPUB: streaming/chapter event rendering, bounded buffers, safe page/chapter navigation.
- [x] Library: recursive discovery of supported books under `/books` and stable first-render population.
- [~] Feed: explicit offline/network-required state, clear enable action, auto-advance on connectivity.
- [~] Display: reliable first frame and stable `fast/partial/full` cadence for interaction types.

## Workstreams

### A) EPUB Streaming + Memory (Owner: Subagent A)
- [x] Eliminate whole-file/chapter materialization on firmware path.
- [x] Keep allocations bounded and recover gracefully on low memory.
- [x] Preserve chapter/page navigation UX parity (left/right pages, aux chapter jump).
- [ ] Validate no OOM on open/navigation in `flash.log`.

### B) Library Scan Parity (Owner: Subagent B)
- [x] Restore recursive scan capability in generic storage abstraction.
- [x] Keep API generic (no firmware-specific coupling in `einked`).
- [x] Ensure first library render includes discovered books as before.
- [ ] Validate nested `/books/**` content appears.

### C) Feed Wi-Fi Event UX (Owner: Subagent C)
- [x] Feed open always shows network-required screen when offline.
- [x] Confirm triggers enable request exactly once and shows deterministic state.
- [x] Auto-transition to feed entries when Wi-Fi becomes active (no manual re-confirm loop).
- [x] Keep firmware bridge generic via settings/flags, no device-specific UI coupling.

### D) Refresh/Runtime Hardening (Owner: Subagent D)
- [x] Keep first-frame reliability.
- [x] Remove temporary migration debug logic that changes behavior.
- [ ] Preserve hint-driven refresh mapping for normal interaction.
- [ ] Ensure no blank-frame regressions on idle/power-on/sleep-wake.

## Validation Gates
- [x] `cargo check --workspace --exclude xteink-firmware`
- [ ] `just check-firmware`
- [ ] `just check-firmware-minireader` (isolated side-path; no hard migration)
- [ ] Device flash + log verification for:
  - [ ] boot first-frame visible
  - [ ] open EPUB without crash
  - [ ] page/chapter nav stable
  - [ ] feed offline->online transition works
  - [ ] library populated from `/books` recursively

## Notes
- Do not reintroduce `xteink`-specific assumptions inside `einked`/`einked-ereader`.
- Keep constraints configurable in `DeviceConfig` or equivalent app config supplied by firmware.
- Added generic `FeedClient` bridge in `einked-ereader`; firmware now supplies `FirmwareFeedClient` backed by `FeedService`.
- Latest `flash.log` root cause was EPUB open OOM (`allocation of 8192 bytes failed`) in `RenderPrep`; added embedded memory budgets + bounded EPUB open options and page-windowed loading to reduce peak heap.
- Updated EPUB path to keep a persistent `epub-stream` session (book + engine) per open reader modal, removing reopen/reparse per page turn and removing EPUB fallback byte-buffer loading.
- Latest `flash.log` still shows EPUB navigation OOM on second page turn (`memory allocation of 43280 bytes failed` after two `Right` presses), so EPUB stability and nav validation gates remain open.
- Latest crash log confirms idle largest contiguous 8-bit block is only `32768` bytes before EPUB open, so a single ~43KB allocation aborts.
- Reduced ESP memory footprint in `einked` command history buffers (`FRAME_PREV_CAPACITY` and inline draw-text capacity) to increase available contiguous heap for EPUB open/page work.
- Added `FileStore::native_path` optional hook for future direct VFS-backed EPUB open optimizations without coupling `einked` to firmware specifics.
- Restored pre-migration EPUB tuning model in `einked-ereader`:
  - ESP EPUB open now uses `EpubBook::open_with_temp_storage(..., "/sd/.tmp", ...)` with lazy navigation.
  - Page extraction now uses `chapter_events_with_scratch` + bounded `chapter_buf` growth/retries, matching the earlier stable streaming behavior instead of full-chapter prepare/materialize.
- Additional hardening pass (2026-02-24):
  - Re-applied pre-migration ESP open behavior of deferring EPUB working-buffer allocation (`chapter_buf` + `ScratchBuffers`) until first page stream, instead of eager allocation at open.
  - Reduced ESP initial EPUB page-window prefetch pressure (`EPUB_PAGE_WINDOW=2` on ESP only).
  - Released large non-reader list allocations right before EPUB open to recover contiguous heap for parser/open phases.
- Isolated runtime swap added in firmware (2026-02-24):
  - Default firmware path remains `einked-ereader`.
  - `minireader-ui` feature compiles and runs `einked-minireader` without touching the default UI path.
  - Added `just` commands for side-by-side validation: `check-firmware-minireader`, `build-firmware-minireader`, `flash-minireader`.
