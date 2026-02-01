# Agentic Coding Guidelines for rust-xteink-x4

This is a Rust workspace for the Xteink X4 e-ink reader firmware targeting ESP32-C3.

## Build Commands

```bash
# Check all workspace crates (excludes firmware - needs ESP toolchain)
just check
cargo check --workspace --exclude xteink-firmware

# Check firmware (requires ESP toolchain installed)
just check-firmware
cd crates/xteink-firmware && cargo check

# Build firmware for ESP32
just build-firmware
cd crates/xteink-firmware && cargo build --release

# Format all code
just fmt
cargo fmt --all

# Lint with clippy (warnings treated as errors in CI)
just lint
cargo clippy --workspace --exclude xteink-firmware -- -D warnings

# Run desktop simulator (for UI testing without hardware)
just sim-desktop
cargo run -p xteink-sim-desktop

# Run web simulator (WASM)
just sim-web
cd crates/xteink-sim-web && trunk serve --release

# Flash firmware to device
just flash

# Clean build artifacts
just clean
cargo clean
```

## Testing

This project uses simulators for testing rather than traditional unit tests:

- **Desktop Simulator** (`xteink-sim-desktop`): SDL-based, fastest for UI iteration
- **Web Simulator** (`xteink-sim-web`): WASM browser-based

To test changes: Run the desktop simulator for rapid UI iteration.

## Code Style Guidelines

### Imports
Order imports as follows:
1. Standard library (`core`, `alloc`, `std`)
2. External crates (alphabetical)
3. Internal crate modules (`crate::`)
4. Re-exports (`pub use`)

Example from `crates/ssd1677/src/lib.rs`:
```rust
extern crate alloc;

use alloc::boxed::Box;
use core::convert::Infallible;

use embedded_graphics_core::{...};
use embedded_hal::delay::DelayNs;
```

### Formatting
- Use `cargo fmt` with default settings
- No custom `rustfmt.toml` - stick to standard Rust style
- Format on save enabled in VS Code

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

### Documentation
- Use `//!` for module-level documentation
- Use `///` for item-level documentation
- Document all public APIs
- Include units in comments (e.g., `480x800 @ 220 PPI`)

### Embedded-Specific Conventions
- Use `no_std` for UI library and driver (`#![cfg_attr(not(feature = "std"), no_std)]`)
- Hardware abstraction via `embedded-graphics` traits (`DrawTarget`, `OriginDimensions`)
- Delay trait: `DelayNs` for non-blocking delays
- GPIO pins: Use `OutputPin`, `InputPin`, `SpiDevice` traits

### Memory Management
- Use `Box::new()` for heap-allocated buffers on embedded
- Prefer stack allocation for small structs
- Minimize allocations in hot paths

### Workspace Structure
```
crates/
├── xteink-ui/        # Core UI (no_std, embedded-graphics)
├── ssd1677/          # Display driver (no_std)
├── xteink-firmware/  # ESP32 binary
├── xteink-sim-desktop/  # SDL simulator
└── xteink-sim-web/      # WASM simulator
```

### Git Workflow
- All code is formatted with `cargo fmt`
- Clippy warnings are treated as errors in CI (`-D warnings`)
- The project uses nightly Rust toolchain
