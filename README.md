# ox4

**Rust-powered e-reader firmware for Xteink X4**

A complete e-reader firmware written in Rust for the Xteink X4 (ESP32-C3 based 4.3" e-ink device).

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![ESP32-C3](https://img.shields.io/badge/ESP32--C3-RISC--V-green.svg)](https://www.espressif.com/en/products/socs/esp32-c3)

---

## Features

### EPUB Reader
- Streaming architecture handles large books with <60KB RAM usage
- Custom EPUB parser optimized for embedded systems
- Chapter navigation with OPF metadata parsing
- Text layout engine with line breaking and pagination
- Progress tracking and position persistence

### User Interface
- File browser for SD card navigation
- Library view with book metadata
- Settings system (font size, line spacing, margins, refresh modes)
- Multiple UI paradigm designs available

### Display & Input
- SSD1677 driver for 480×800 e-ink display
- Partial refresh support (<200ms page turns)
- Multiple refresh modes (full, half, fast)
- 7-button input (4 directional + confirm + back + power)
- Differential update optimization

### Storage
- SD card support via FAT32 filesystem
- EPUB and TXT file formats
- File operations (read, list, navigate)

### Development
- Desktop simulator (SDL-based)
- Web simulator (WASM-based)
- Hot reload development workflow
- No hardware required for UI development

### Architecture
- `no_std` core library for portability
- `embedded-graphics` trait compatibility
- Workspace-based project structure
- Memory-efficient streaming and buffering
- Comprehensive unit test coverage

---

## Quick Start

### One-Command Setup

```bash
# Install Rust first
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install just task runner
cargo install just

# Bootstrap everything
just setup
```

This will install all dependencies, set up the ESP toolchain, configure git hooks, and verify your environment.

### Manual Setup (Alternative)

<details>
<summary>Click to expand manual installation steps</summary>

**Rust toolchain (1.85+)**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install ESP tools
cargo install espflash espup
```

**ESP-IDF toolchain**
```bash
# Install ESP-IDF and Rust targets
espup install
```

**Development tools (optional)**
```bash
# For web simulator
cargo install trunk

# For task runner
cargo install just
```

**Linux: Serial port access**
```bash
# Add user to serial port group
sudo usermod -aG uucp $USER      # Arch
sudo usermod -aG dialout $USER   # Debian/Ubuntu
# Then log out and back in
```

</details>

### Build & Flash Firmware

```bash
# Build and flash
just flash

# Clean build and flash
just flash-clean

# Monitor serial output only
just monitor
```

### Run Simulators

```bash
# Desktop simulator (fastest for development)
just sim-desktop

# Web simulator (browser-based)
just sim-web
# Open http://localhost:8080
```

---

## Project Structure

```
ox4/
├── crates/
│   ├── xteink-firmware/     # ESP32-C3 firmware binary
│   └── xteink-scenario-harness/ # Integration test harness
│
├── einked/                  # Generic UI library + e-reader app + simulators
│   └── crates/
│       ├── einked-ereader   # E-reader app crate (reusable UI)
│       ├── einked-sim-desktop
│       └── einked-sim-web
│
├── docs/                    # Documentation
│   ├── epub/                # EPUB implementation
│   ├── ui/                  # UI design
│   ├── hardware/            # Hardware specs
│   └── features/            # Feature documentation
│
└── justfile                 # Task runner commands
```

---

## Development Commands

### Setup & Dependencies

```bash
# Bootstrap development environment
just setup

# Check system dependencies
just check-deps
```

### Build & Check

```bash
# Run all checks (format + lint + check)
just all

# Check all crates (excludes firmware)
just check

# Check firmware (requires ESP toolchain)
just check-firmware

# Format code
just fmt

# Lint with clippy
just lint
```

### Testing

```bash
# Run all tests
just test

# Run UI tests only
just test-ui

# Run EPUB tests
cargo test -p einked-ereader --target <host-target>
```

### Building

```bash
# Build firmware for device
just build-firmware

# Build web simulator
just build-web
```

### Flashing

```bash
# Flash firmware (incremental build)
just flash

# Flash with full rebuild
just flash-monitor

# Clean flash (regenerate sdkconfig)
just flash-clean

# Monitor serial output only
just monitor
```

### Utilities

```bash
# Get board info
just board-info

# Backup full flash (16MB, ~25 min)
just backup

# Clean build artifacts
just clean

# Clean firmware only
just clean-firmware
```

---

## Hardware Specifications

- **Device:** Xteink X4 (4.3" e-ink reader)
- **CPU:** ESP32-C3 (RISC-V, 160MHz, single core)
- **RAM:** 400KB SRAM (327KB usable)
- **Flash:** 16MB
- **Display:** 480×800 e-ink, 220 PPI, B&W (SSD1677 controller)
- **Storage:** microSD (up to 512GB)
- **Buttons:** 7 total (Left, Right, Up/Down, Confirm, Back, Power)
- **Battery:** 650mAh Li-ion
- **Connectivity:** WiFi 2.4GHz, Bluetooth
- **Port:** USB-C

See [docs/PLAN.md](docs/PLAN.md) for detailed hardware specs and pin mappings.

---

## Implementation Status

| Phase | Status | Description |
|-------|--------|-------------|
| Phase 1 | Complete | Display driver (SSD1677) |
| Phase 2 | Complete | Button input (ADC + GPIO) |
| Phase 3 | Complete | SD card support (FATFS) |
| Phase 4 | Complete | EPUB reader (ready for testing) |
| Phase 5 | In Progress | Power management |

### EPUB Implementation

- Streaming ZIP parser (4KB buffer)
- OPF metadata parsing
- XHTML tokenizer (SAX-style, no DOM)
- Layout engine with line breaking and pagination
- Chapter navigation
- Memory usage: ~60KB (target: <100KB)
- Page turn latency: ~100-150ms (target: <200ms)
- Unit test coverage: 23 tests

See [docs/epub/implementation-status.md](docs/epub/implementation-status.md) for details.

---

## Documentation

- [Getting Started](docs/PLAN.md) - Project overview and learning resources
- [EPUB Implementation](docs/epub/) - Architecture and status
- [UI Design](docs/ui/) - Interface paradigms and mockups
- [Hardware](docs/hardware/) - Display driver review
- [Features](docs/features/) - Simulator and future ideas
- [Glossary](docs/GLOSSARY.md) - Technical terms

---

## Simulator Controls

| Key | Button |
|-----|--------|
| ← / A | Left |
| → / D | Right |
| ↑ / W | Volume Up |
| ↓ / S | Volume Down |
| Enter / Space | Confirm |
| Escape | Back |
| P | Power |

---

## Technology Stack

**Core**
- Rust (1.85+, nightly)
- ESP32-C3 (RISC-V)
- ESP-IDF (via `esp-idf-svc`)

**UI & Graphics**
- `embedded-graphics` - Drawing primitives
- `embedded-text` - Text rendering
- `fontdue` - Font rasterization

**EPUB**
- Custom streaming ZIP implementation
- `quick-xml` - SAX-style XML parser
- Custom HTML tokenizer

**Simulators**
- SDL2 via `embedded-graphics-simulator`
- WASM via `embedded-graphics-web-simulator`

---

## Contributing

See [AGENTS.md](AGENTS.md) for code style guidelines.

Key requirements:
- Follow standard Rust formatting (`cargo fmt`)
- Pass clippy lints (`cargo clippy -- -D warnings`)
- Use `no_std` patterns for UI/runtime crates where applicable
- Test changes in simulators before device
- Document memory usage for embedded code

---

## License

MIT License - See [LICENSE](LICENSE) for details.

---

## Acknowledgments

- Original hardware info: [CidVonHighwind/xteink-x4-sample](https://github.com/CidVonHighwind/xteink-x4-sample)
- Display driver concepts from `epd-waveshare` crate
- ESP-RS community

---

## Resources

- ESP-RS Book: https://docs.esp-rs.org/book/
- Embedded Rust Book: https://docs.rust-embedded.org/book/
- ESP32-C3 Documentation: https://www.espressif.com/en/products/socs/esp32-c3
- Matrix Chat: [#esp-rs:matrix.org](https://matrix.to/#/#esp-rs:matrix.org)
