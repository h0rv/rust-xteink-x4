# Embedded Rust Tasking on ESP32-C3 (2026 Quick Guide)

Last updated: 2026-02-10

This project runs on ESP32-C3 with ESP-IDF. The core rule is: keep `xteink-ui` safe and platform-agnostic, and put RTOS-specific control in firmware boundaries.

## Verified facts from current docs

- ESP-IDF `pthread` is implemented as wrappers around FreeRTOS features.
- C++ `std::thread` in ESP-IDF is realized using `pthread`.
- In IDF FreeRTOS, task creation stack units are **bytes**, not words (unlike vanilla FreeRTOS).
- `esp_pthread_set_cfg()` / `esp_pthread_get_default_config()` can tune default thread stack size, priority, name, core pinning, and stack allocation capabilities for subsequent `pthread_create()`.
- In `esp-idf-hal`, raw FreeRTOS `task::create` is explicitly documented as a niche API; default recommendation is safe `std::thread`.

Inference from sources:
- Rust `std::thread` behavior on ESP-IDF follows the same practical constraints as C++ `std::thread` here, because both go through the ESP-IDF pthread/RTOS integration layer.

## 2026 best-practice decision path

1. Safe-first default:
- Use `std::thread` + channels for background work.
- Configure thread defaults centrally with `esp_pthread_set_cfg` (stack/priority/core/caps) instead of ad-hoc values at each callsite.
- Keep UI/input paths (`handle_input`, `render`) free of blocking IO/parsing.

2. Move to direct FreeRTOS APIs only when there is a clear reason:
- Need static allocation (`xTaskCreateStatic*`) for determinism.
- Need exact RTOS-level affinity/priority behavior not covered by pthread config.
- Need ISR-oriented primitives or zero-allocation control paths.

3. Contain unsafe:
- Keep any raw FreeRTOS FFI in a firmware-only boundary module (for example `xteink-firmware/src/rtos_bridge.rs`).
- Expose only safe APIs to the rest of the app.
- Do not introduce RTOS `unsafe` into crates that enforce `#![forbid(unsafe_code)]` (for example `xteink-ui`).

4. Prefer long-lived workers over spawn-per-request:
- Use one dedicated worker task/thread + queue for heavy domains (EPUB parse/layout).
- Avoid repeated create/destroy cycles that increase allocation pressure and fragmentation risk.

5. Measure continuously:
- Log `uxTaskGetStackHighWaterMark(NULL)`.
- Log `heap_caps_get_largest_free_block(MALLOC_CAP_8BIT)` and minimum free heap.
- Gate stack/heap tuning on measured low-water marks, not assumptions.

## Anti-patterns

- Parsing EPUB in the UI loop.
- Using desktop stack assumptions on ESP32-C3.
- Spawning transient workers for every heavy operation.
- Allocating full-book buffers when streaming is viable.
- Treating SD card capacity as RAM (it is storage, not execution memory).

## Repo guidance right now

1. Keep `xteink-ui` safe and cross-target.
2. Use deferred tasks and bounded per-tick work in app/activity layers.
3. For firmware-only RTOS specialization, implement a small safe adapter in `xteink-firmware`, not in shared UI code.
4. For EPUB: prefer one long-lived background worker + queue over repeated thread creation attempts.

## Current implementation notes (this repo)

- Firmware sets global pthread defaults with `ThreadSpawnConfiguration` (`stack_size=56KiB`, low priority) so `std::thread` tasks have a sane floor on ESP-IDF.
- EPUB open and page-turn work run in background worker threads; the UI loop now polls completion and redraws when work completes.
- `xteink-ui` keeps `#![forbid(unsafe_code)]` and does not call raw FreeRTOS APIs directly.
- `mu-epub-render` cache hooks are enabled for non-ESP targets, where full-session chapter caching is safe; ESP-IDF keeps conservative memory behavior to avoid page-cache OOM spikes.

## Source links (checked 2026-02-10)

- ESP-IDF POSIX/pthread support (ESP32-C3, stable): https://docs.espressif.com/projects/esp-idf/en/stable/esp32c3/api-reference/system/pthread.html
- ESP-IDF FreeRTOS (IDF) (ESP32-C3, stable): https://docs.espressif.com/projects/esp-idf/en/stable/esp32c3/api-reference/system/freertos_idf.html
- ESP-IDF heap memory allocation (ESP32-C3, stable): https://docs.espressif.com/projects/esp-idf/en/stable/esp32c3/api-reference/system/mem_alloc.html
- ESP-IDF call function with external stack (ESP32-C3, stable): https://docs.espressif.com/projects/esp-idf/en/stable/esp32c3/api-reference/system/esp_function_with_shared_stack.html
- esp-idf-hal `task::create` docs: https://docs.esp-rs.org/esp-idf-hal/esp_idf_hal/task/fn.create.html
- Rust `std::thread::Builder::stack_size`: https://doc.rust-lang.org/std/thread/struct.Builder.html
- Embassy executor docs: https://docs.embassy.dev/embassy-executor/
- Embassy book: https://embassy.dev/book/
