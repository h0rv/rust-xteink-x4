# Debugging & Observability Plan (2026)

## Goals
- Cut hardware flash loops by validating most UI logic on host/sim.
- Make device failures diagnosable from logs/metrics without guesswork.
- Keep watchdogs enabled in production while supporting a safe debug profile.

## 1) Development Profiles

### `prod` profile (default)
- Watchdogs enabled.
- Normal log verbosity.
- Assertions/panics configured for production behavior.

### `debug-hw` profile
- Higher log verbosity and richer diagnostics.
- Relaxed watchdog windows (or explicit temporary disable when isolating startup faults).
- Extra timing/memory probes around long operations.

Implementation:
- Add separate config defaults, e.g. `sdkconfig.debug.defaults`.
- Add `just` commands for profile-specific build/flash.

## 2) Structured Logging

Use machine-parseable event lines, not free-form-only text.

Required event fields:
- `ts_ms` (monotonic milliseconds)
- `evt` (event id)
- `mod` (module/component)
- key fields per event (e.g. `mode=Full dur_ms=3560`)

Minimum event set:
- `BOOT_START`, `BOOT_DONE`, `RESET_REASON`
- `LOOP_HEARTBEAT`
- `INPUT_SAMPLE`, `INPUT_BUTTON`
- `UI_NAV`, `UI_REDRAW`, `UI_NO_REDRAW`
- `DISPLAY_UPDATE_BEGIN`, `DISPLAY_UPDATE_END`, `DISPLAY_UPDATE_FAIL`
- `FS_OP_BEGIN`, `FS_OP_END`, `FS_OP_FAIL`
- `EPUB_OPEN_BEGIN`, `EPUB_OPEN_END`, `EPUB_OPEN_FAIL`
- `WDT_WARNING`, `WDT_RESET_HINT`
- `HEAP_SNAPSHOT`, `STACK_WATERMARK`

## 3) Metrics To Collect

### Reliability
- Reset count and reset reason distribution.
- Watchdog resets/hour.
- Time-to-failure from boot.

### Performance
- Input-to-redraw latency (p50/p95).
- Redraw-to-display-commit latency.
- Full/partial/fast refresh durations.

### Memory
- Free heap and minimum free heap.
- Largest free block.
- Per-task stack high-water mark.

### Content/IO
- Library scan duration + file count.
- Book open success/failure rate by type.
- EPUB chapter load/tokenize/render durations.
- SD mount/read error rates.

## 4) Required Debugging Instrumentation

Add timers and probes around:
- Startup display init/reset/update.
- Main event loop iteration.
- Button sampling and debounce decisions.
- `app.handle_input(...)`.
- Deferred task processing (`process_deferred_tasks`).
- Display update calls (`full/partial/fast/region`).
- EPUB open/chapter parse path.

Add periodic heartbeat:
- Every 1-2 seconds: loop alive + current screen + free/min heap.

Add on-boot diagnostics:
- Build SHA/version/date.
- Active profile/config (prod/debug-hw).
- Effective watchdog settings.

## 5) Host-First Validation

Move behavior checks to host tests where possible:
- Scenario tests for navigation and back-stack behavior.
- File/browser/library open flows.
- Settings apply behavior (including font profile propagation).
- Deferred task processing sequences.

Add `just` fast-loop commands:
- `just test-ui-fast` for critical scenario subset.
- `just test-ui-scenarios` for full UI flow set.
- `just sim-desktop` for visual checks.

## 6) Device-Specific Validation Checklist

Use flash/device only for:
- ADC button thresholds and debounce behavior.
- Display timing artifacts (ghosting, partial/full cadence).
- SD card timing and mount reliability.
- Sleep/wake and power button long-press behavior.

Per flash session collect:
- `flash.log`
- reset reason
- first-render duration
- first 10 input events with ADC values
- one full navigation/open/back flow trace

## 7) Immediate Implementation Backlog

1. Add profile split (`prod` vs `debug-hw`) in `sdkconfig` defaults and `just` commands.
2. Add structured event helper macros in firmware.
3. Instrument startup + loop + input + display + deferred tasks.
4. Add host scenario tests for current regressions.
5. Add a simple log parser script/command for `flash.log` summary:
   - reset reason
   - watchdog hints
   - event timing summaries
6. Re-enable production watchdog settings after blocking sections are measured and bounded.

## 8) Production Guardrails

- Never ship with watchdogs disabled.
- Keep debug-only logs/config gated by profile.
- Require a stable 30+ minute soak run with no watchdog reset.
- Require scenario test pass before flash validation.
