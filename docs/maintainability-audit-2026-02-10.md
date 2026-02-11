# Maintainability Audit (2026-02-10)

## Scope

- Workspace-level architecture and module boundaries.
- Runtime-critical EPUB path and firmware task/heap configuration boundaries.
- Test brittleness affecting iteration speed.

## Priority Findings

1. `P1` Monolithic EPUB logic mixed with activity navigation in one file.
: `/Users/robby/projects/rust-xteink-x4/crates/xteink-ui/src/file_browser_activity.rs` previously combined UI routing, EPUB parsing, render/cache policies, worker lifecycle, and tests. This slowed safe iteration on any single concern.

2. `P1` Firmware runtime policy code embedded directly in app entrypoint.
: `/Users/robby/projects/rust-xteink-x4/crates/xteink-firmware/src/main.rs` held thread stack defaults and heap diagnostics inline with boot/input/render loop code, making critical runtime policy harder to reason about and reuse.

3. `P2` Async behavior tests relied on timing/order details.
: `file_browser_activity` tests assumed deterministic directory ordering and single-tick task completion, causing flaky failures during async evolution.

## Refactors Applied In This Pass

1. Split EPUB implementation details into a dedicated submodule.
: Added `/Users/robby/projects/rust-xteink-x4/crates/xteink-ui/src/file_browser_activity/epub.rs` and moved EPUB rendering/open/navigation worker implementations there, keeping `/Users/robby/projects/rust-xteink-x4/crates/xteink-ui/src/file_browser_activity.rs` focused on activity orchestration.

2. Isolated firmware runtime diagnostics and pthread policy.
: Added `/Users/robby/projects/rust-xteink-x4/crates/xteink-firmware/src/runtime_diagnostics.rs` and moved heap logging + pthread default configuration out of `/Users/robby/projects/rust-xteink-x4/crates/xteink-firmware/src/main.rs`.

3. Hardened async-sensitive tests.
: Updated `file_browser_activity` tests to use path-driven opens and bounded task draining rather than relying on directory order and exact tick count.

## Recommended Next Slices

1. Split large UI activities by concern (`library_activity`, `reader_settings_activity`, `settings_activity`).
: Extract each activity into `model`, `input`, and `render` submodules with narrow state transition APIs.

2. Introduce typed UI render commands for shared components.
: Replace repeated direct drawing logic with reusable component render command structs to reduce duplication and improve testability.

3. Decompose firmware `main` loop into explicit subsystems.
: Move input state machine, render scheduling, and CLI handling into separate modules and use a small orchestrator in `main`.

4. Add deterministic integration-style tests for deferred task scheduling.
: Cover `process_deferred_tasks` and EPUB open/page-turn flow with bounded polling helpers and explicit expected transitions.
