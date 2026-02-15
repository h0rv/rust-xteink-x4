# Agentic Coding Guidelines for rust-xteink-x4

This is a Rust workspace for the Xteink X4 e-ink reader firmware targeting ESP32-C3 (RISC-V).

## Build Commands

```bash
# Check all workspace crates (excludes firmware - needs ESP toolchain)
just check
cargo check --workspace --exclude xteink-firmware

# Check firmware (requires ESP toolchain installed)
just check-firmware
cargo check -p xteink-firmware

# Build firmware for ESP32
just build-firmware
cargo build -p xteink-firmware --release

# Format all code
just fmt
cargo fmt --all

# Lint with clippy (warnings treated as errors in CI)
just lint
cargo clippy --workspace --exclude xteink-firmware -- -D warnings

# Run all quality checks
just all          # Excludes firmware check
just all-full     # Includes firmware check
```

## Testing Commands

```bash
# Run all unit tests (in #[cfg(test)] modules)
just test-ui
cargo test -p xteink-ui --features std --target <host-target>

# Run only diff tests (fast subset)
just test-diff
cargo test -p xteink-ui --features std --target <host-target> diff

# Run a single unit test by name
cargo test -p xteink-ui --features std --target <host-target> <test_name>

# Run all integration/scenario tests
just sim-scenarios
cargo test -p xteink-scenario-harness --target <host-target>

# Run a single scenario test file
cargo test -p xteink-scenario-harness --target <host-target> --test <test_name>

# Run a specific test within a scenario file
cargo test -p xteink-scenario-harness --target <host-target> --test fundamental_epub_scroll <fn_name>
```

## Simulator Commands

```bash
# Run desktop simulator (SDL-based, fastest for UI iteration)
just sim-desktop
cargo run -p xteink-sim-desktop

# Run web simulator (WASM browser-based)
just sim-web
cd crates/xteink-sim-web && trunk serve --release

# Build web simulator
just build-web
```

## Flashing Commands

```bash
# Flash firmware to device (auto-detects port)
just flash

# Flash with monitor (always rebuilds)
just flash-monitor

# Clean flash (full rebuild with sdkconfig regeneration)
just flash-clean

# Just monitor serial output
just monitor
```

## Code Style Guidelines

### Imports
Order imports as follows:
1. Standard library (`core`, `alloc`, `std`)
2. External crates (alphabetical)
3. Internal crate modules (`crate::`)
4. Re-exports (`pub use`)

Example from `crates/xteink-ui/src/app.rs`:
```rust
extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

use crate::file_browser_activity::FileBrowserActivity;
use crate::input::InputEvent;
use crate::ui::{Activity, ActivityRefreshMode};
```

### Formatting
- Use `cargo fmt` with default settings (no custom `rustfmt.toml`)
- Format on save enabled in VS Code
- The project uses **nightly** Rust toolchain

### Types and Naming
- **Types**: PascalCase (`Ssd1677`, `RefreshMode`, `App`)
- **Functions/Variables**: snake_case (`send_command`, `display_width`)
- **Constants**: UPPER_SNAKE_CASE (`DISPLAY_WIDTH`, `HW_HEIGHT`)
- **Generic Parameters**: UpperCamelCase (`SPI`, `DC`, `RST`)

### Error Handling
- **Firmware**: Uses `.unwrap()` for hardware initialization (fail-fast on embedded)
- **UI Library**: Return `Result<T, E>` for fallible operations
- **Simulators**: Propagate errors with `?` operator
- **Display Operations**: Use `Infallible` error type where applicable

### Unsafe Code
- **Forbidden** in xteink-ui: `#![forbid(unsafe_code)]`
- Use safe abstractions over hardware registers

### Documentation
- Use `//!` for module-level documentation
- Use `///` for item-level documentation
- Document all public APIs
- Include units in comments (e.g., `480x800 @ 220 PPI`)

### Embedded-Specific Conventions
- Use `no_std` for UI library and driver: `#![cfg_attr(not(feature = "std"), no_std)]`
- Hardware abstraction via `embedded-graphics` traits (`DrawTarget`, `OriginDimensions`)
- Delay trait: `DelayNs` for non-blocking delays
- GPIO pins: Use `OutputPin`, `InputPin`, `SpiDevice` traits

### Memory Management
- Use `Box::new()` for heap-allocated buffers on embedded
- Prefer stack allocation for small structs
- Minimize allocations in hot paths

### Test Organization
- **Unit tests**: In `#[cfg(test)]` modules within source files
- **Integration tests**: In `crates/xteink-scenario-harness/tests/`
- Use `MockFileSystem` for filesystem-dependent tests
- Tests require `std` feature: `cargo test -p xteink-ui --features std`

### Workspace Structure
```
crates/
├── xteink-ui/              # Core UI (no_std, embedded-graphics)
├── xteink-firmware/        # ESP32 binary
├── xteink-sim-desktop/     # SDL simulator
├── xteink-sim-web/         # WASM simulator
└── xteink-scenario-harness/ # Integration test harness
ssd1677/                    # Display driver (no_std)
mu-epub/                    # EPUB parsing library
```

### Git Workflow
- All code must be formatted with `cargo fmt`
- Clippy warnings treated as errors in CI (`-D warnings`)
- Use conventional commits (implied by project structure)

### CI/Quality Gates
- Format check: `cargo fmt --package ssd1677 -- --check`
- Clippy: `cargo clippy --package ssd1677 -- -D warnings`
- Doc build: `RUSTDOCFLAGS='-D warnings' cargo doc --package ssd1677 --no-deps --all-features`
- Size check: Firmware binary must fit in partition (see `just size-check`)
