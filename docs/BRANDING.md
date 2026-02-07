# ox4 Branding Guide

## Name

**ox4** (pronounced "ox-four" or "ocks-four")

### Meaning
- **ox** - Oxide (Rust programming language)
- **4** - Xteink X4 hardware platform

### Tagline
*"Rust-powered reading"*

---

## Visual Identity

### ASCII Logo

```
       _  _   
  ___ ( \/ )  
 / _ \ \  /   
| (_) |/  \   
 \___//_/\_\  
```

### Minimal Version
```
ox4
```

---

## Naming Conventions

### Repositories
- Main firmware: `ox4`
- GitHub org (if created): `ox4-project`

### Crates
- Firmware binary: `ox4-firmware` or `ox4`
- UI library: `ox4-ui`
- Desktop simulator: `ox4-sim-desktop`
- Web simulator: `ox4-sim-web`
- SSD1677 driver (if published): `ssd1677` (generic) or `ox4-display`

### Documentation
- "the ox4 firmware"
- "ox4 e-reader"
- Avoid: "the ox4" (sounds awkward)

---

## Messaging

### One-line Description
"Rust-powered e-reader firmware for Xteink X4"

### Short Description (Tweet-length)
"ox4: Open-source e-reader firmware written in Rust for ESP32-C3 devices. EPUB support, <60KB RAM, fast refresh, dual simulators."

### Key Benefits
1. **Memory efficient** - <60KB RAM usage, runs on constrained hardware
2. **Fast** - <200ms page turns, optimized refresh modes
3. **Developer-friendly** - Desktop/web simulators, no device needed
4. **Open source** - MIT licensed, hackable, extensible
5. **Pure Rust** - Type-safe, modern toolchain, `no_std` core

---

## Color Palette (Suggested)

Since e-ink is B&W, keep branding minimal:

- **Primary:** `#CE422B` (Rust orange)
- **Secondary:** `#1A1A1A` (E-ink black)
- **Accent:** `#FFFFFF` (E-ink white)
- **Code:** `#2A2A2A` (Dark gray)

---

## Social Media

### Hashtags
- `#ox4`
- `#RustLang`
- `#EmbeddedRust`
- `#ESP32`
- `#EinkReader`
- `#OpenHardware`

### GitHub Topics
```
rust, embedded, esp32, esp32-c3, e-ink, e-reader, epub, 
firmware, no-std, risc-v, xteink
```

---

## Comparisons (Positioning)

### vs. Official Firmware
- **ox4:** Open source, hackable, Rust-based
- **Official:** Closed source, stable, manufacturer support

### vs. crosspoint-reader
- **ox4:** Modern Rust architecture, streaming EPUB, memory-optimized
- **crosspoint-reader:** Existing project (research their positioning)

### Unique Selling Points
1. First Rust-based Xteink X4 firmware
2. Dual simulator support (desktop + web)
3. Custom streaming EPUB engine
4. <60KB RAM for large books
5. `no_std` core for portability

---

## Community Guidelines

### Target Audience
- Embedded Rust enthusiasts
- E-reader hackers
- ESP32 developers
- Open hardware advocates
- Xteink X4 owners wanting more control

### Tone
- **Technical** but approachable
- **Educational** - explain embedded concepts
- **Honest** - acknowledge limitations
- **Collaborative** - welcome contributions

---

## Release Naming (Future)

Suggested scheme: **Material names** (like Android)

Examples:
- v0.1.0: "Paper"
- v0.2.0: "Vellum"
- v0.3.0: "Parchment"
- v1.0.0: "Codex"

Or: **Rust oxidation states**
- v0.1.0: "Ferrous" (Fe²⁺)
- v0.2.0: "Ferric" (Fe³⁺)
- v1.0.0: "Stable"

---

## Example Usage

### Repository Description
```
ox4 - Rust-powered e-reader firmware for Xteink X4 (ESP32-C3)
```

### Cargo.toml
```toml
[package]
name = "ox4-firmware"
description = "Rust-powered e-reader firmware for Xteink X4"
repository = "https://github.com/USERNAME/ox4"
keywords = ["embedded", "esp32", "e-ink", "e-reader", "epub"]
categories = ["embedded", "no-std"]
```

### Commit Messages
```
feat(epub): add streaming ZIP parser
fix(display): correct partial refresh timing
docs(ox4): update installation guide
```

---

*Last Updated: 2026-02-07*
