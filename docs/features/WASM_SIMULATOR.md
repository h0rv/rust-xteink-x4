# WASM Simulator

Browser-based simulator for developing the Xteink X4 UI without hardware.

## Why

- **Fast iteration** - Hot reload in browser, no flashing
- **No hardware needed** - Develop anywhere
- **Shareable** - Host on GitHub Pages for demos

## Architecture

```
crates/
├── xteink-ui/          # Shared UI (no_std, works everywhere)
├── xteink-sim-web/     # WASM browser simulator
├── xteink-sim-desktop/ # SDL desktop simulator (faster)
└── xteink-firmware/    # ESP32 target
```

The key insight: `xteink-ui` uses `embedded-graphics` which provides a `DrawTarget` trait. Any platform that implements this trait can render the UI.

## Usage

```bash
# Web (browser)
cd crates/xteink-sim-web
trunk serve --release

# Desktop (faster iteration)
cargo run -p xteink-sim-desktop
```

## Display Specs

- **Resolution:** 480 × 800 pixels
- **PPI:** 220
- **Diagonal:** 4.3"
- **Physical size:** 69 × 114 mm

## Controls

| Key | Action |
|-----|--------|
| Arrow keys / WASD | Move |
| Enter / Space | Confirm |
| Escape | Back |

## Dependencies

- `embedded-graphics` - Rendering primitives
- `embedded-graphics-web-simulator` - Canvas backend for WASM
- `embedded-graphics-simulator` - SDL backend for desktop
- `trunk` - WASM build tool with hot reload
