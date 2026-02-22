# Xteink X4 Firmware Comparison

Comprehensive feature comparison of firmware options for the Xteink X4 e-reader.

## Document Support

| Feature | Official | ox4 | CrossPoint | MicroReader |
|---------|:--------:|:---:|:----------:|:-----------:|
| EPUB 2/3 | ✓ | ✓ | ✓ | ✓ |
| TXT | ✓ | ✓ | ✓ | ✓ |
| PDF | ✓ | — | — | — |
| XTC (pre-converted) | — | — | ✓ | — |
| Image files | — | ✓ | — | — |
| EPUB embedded images | ✓ | ✓ | ✓ (limited) | — |
| EPUB embedded fonts | — | ✓ | — | — |

## Typography & Layout

| Feature | Official | ox4 | CrossPoint | MicroReader |
|---------|:--------:|:---:|:----------:|:-----------:|
| Antialiased fonts (grayscale) | ✓ | ✓ | ✓ | ✓ |
| 4-level grayscale display | ✓ | ✓ | ✓ | ✓ |
| Custom fonts | ✓ | ✓ | ✓ (planned) | ✓ |
| Open Dyslexic font | — | — | ✓ | — |
| Hyphenation | ✓ | ✓ (English) | ✓ (multi-lang) | ✓ (EN/DE) |
| Text justification | ✓ | ✓ | ✓ | — |
| Line spacing options | ✓ | ✓ | ✓ | ✓ |
| Margin configuration | ✓ | ✓ | ✓ | ✓ |
| Font size options | ✓ | ✓ | ✓ | ✓ |
| Knuth-Plass line breaking | — | — | — | ✓ |
| Embedded CSS support | ✓ | — | ✓ (toggle) | — |

## Navigation & Reading

| Feature | Official | ox4 | CrossPoint | MicroReader |
|---------|:--------:|:---:|:----------:|:-----------:|
| File browser | ✓ | ✓ | ✓ | ✓ |
| Nested folder navigation | ✓ | ✓ | ✓ | — |
| Chapter navigation | ✓ | ✓ | ✓ | ✓ |
| Table of contents | ✓ | ✓ | ✓ | ✓ |
| Bookmarks | ✓ | ✓ | — | — |
| Reading position persistence | ✓ | ✓ | ✓ | ✓ |
| Jump to percentage | ✓ | ✓ | ✓ | — |
| Screen rotation | ✓ | ✓ | ✓ | — |
| Invert colors (dark mode) | ✓ | ✓ | — | — |

## Library Management

| Feature | Official | ox4 | CrossPoint | MicroReader |
|---------|:--------:|:---:|:----------:|:-----------:|
| Library view | ✓ | ✓ | ✓ | — |
| Book cover thumbnails | ✓ | ✓ | ✓ | — |
| Recent books list | ✓ | — | ✓ | — |
| Sort by title/author/recent | ✓ | ✓ | — | — |
| Mark as unread | — | ✓ | — | — |
| Delete book from device | ✓ | ✓ | ✓ | — |

## Connectivity

| Feature | Official | ox4 | CrossPoint | MicroReader |
|---------|:--------:|:---:|:----------:|:-----------:|
| WiFi book upload | — | ✓ | ✓ | — |
| OTA firmware updates | ✓ | — | ✓ | — |
| Calibre wireless transfer | — | — | ✓ | — |
| OPDS catalog browser | — | — | ✓ | — |
| KOReader sync | — | — | ✓ | — |
| WebDAV | — | — | ✓ | — |
| Web file management UI | — | ✓ | ✓ | — |
| WiFi AP mode | — | ✓ | ✓ | — |
| WiFi station mode | — | ✓ | ✓ | — |
| mDNS discovery | — | ✓ | — | — |

## Power & Display

| Feature | Official | ox4 | CrossPoint | MicroReader |
|---------|:--------:|:---:|:----------:|:-----------:|
| Sleep screen | ✓ | ✓ | ✓ | ✓ |
| Custom sleep images | — | ✓ | ✓ | — |
| Cover as sleep screen | — | ✓ | ✓ | — |
| Auto-sleep timeout | ✓ | ✓ | ✓ | — |
| Battery indicator | ✓ | ✓ | ✓ | ✓ |
| Charging status | ✓ | ✓ | ✓ | — |
| Refresh modes (full/partial) | ✓ | ✓ | ✓ | ✓ |
| Configurable refresh frequency | ✓ | ✓ | ✓ | — |
| Sunlight fading fix | — | — | ✓ | — |

## Status Bar & Progress

| Feature | Official | ox4 | CrossPoint | MicroReader |
|---------|:--------:|:---:|:----------:|:-----------:|
| Status bar options | ✓ | ✓ | ✓ | — |
| Page number display | ✓ | ✓ | ✓ | ✓ |
| Chapter progress | ✓ | ✓ | ✓ | ✓ |
| Book progress percentage | ✓ | ✓ | ✓ | ✓ |
| Hide battery percentage | — | — | ✓ | — |
| Auto-hide footer | — | ✓ | — | — |

## Controls & Input

| Feature | Official | ox4 | CrossPoint | MicroReader |
|---------|:--------:|:---:|:----------:|:-----------:|
| Button remapping | — | ✓ | ✓ | ✓ |
| Side button swap | — | ✓ | ✓ | ✓ |
| Short power button actions | — | ✓ | ✓ | — |
| Long-press chapter skip | — | — | ✓ | ✓ |
| Screenshot capture | — | — | ✓ | — |
| Volume button page turn | — | ✓ | ✓ | ✓ |

## System & UI

| Feature | Official | ox4 | CrossPoint | MicroReader |
|---------|:--------:|:---:|:----------:|:-----------:|
| Multi-language UI | ✓ | — | ✓ | — |
| UI themes | — | — | ✓ | — |
| Device info screen | ✓ | ✓ | — | — |
| Boot loop recovery | — | — | ✓ | — |
| Cache management | — | — | ✓ | — |
| Settings persistence | ✓ | ✓ | ✓ | ✓ |

## Development & Architecture

| Feature | Official | ox4 | CrossPoint | MicroReader |
|---------|:--------:|:---:|:----------:|:-----------:|
| Open source | — | ✓ (MIT) | ✓ | ✓ |
| Language | Closed | Rust | C++ | C++ |
| Platform | Proprietary | ESP-IDF | PlatformIO | PlatformIO |
| Desktop simulator | — | ✓ (SDL) | — | — |
| Web simulator (WASM) | — | ✓ | — | — |
| Unit test framework | — | ✓ | ✓ | ✓ |
| `no_std` core library | — | ✓ | — | — |
| Memory usage | ? | <60KB | SD cache | Minimal |
| EPUB parser | Proprietary | Streaming | Cached | Streaming |

## Support

| Feature | Official | ox4 | CrossPoint | MicroReader |
|---------|:--------:|:---:|:----------:|:-----------:|
| Official manufacturer support | ✓ | — | — | — |
| Warranty coverage | ✓ | — | — | — |
| Regular updates | ✓ | — | ✓ | — |
| Active development | ✓ | ✓ | ✓ | — |
| Community support | — | ✓ | ✓ | ✓ |

---

## Summary

### Official Firmware
- Factory-installed, supported by Xteink
- **PDF support** (unique among options)
- Preserves warranty
- Closed source with official updates
- Recommended for most users

### ox4 (This Project)
- **Rust** with memory safety guarantees
- **Streaming EPUB** architecture (~60KB RAM)
- **Desktop + Web simulators** for rapid development
- `no_std` core library for portability
- Embedded font support from EPUBs
- Dark mode / color inversion
- Modern UI with library view and cover thumbnails
- Button remapping (swap L/R, swap U/D, volume for pages)
- Custom sleep screen images

### CrossPoint Reader
- **Most feature-complete** community firmware
- **OPDS browser** + **Calibre integration** + **KOReader sync**
- **OTA updates** with web-based flashing
- Multi-language UI (12+ languages)
- Extensive customization options
- Multiple UI themes
- Active development community

### MicroReader
- **Minimal**, focused EPUB/TXT reader
- **Knuth-Plass** optimal line breaking
- Small codebase, easy to understand
- English/German hyphenation
- Launcher-compatible design
- 4-level grayscale rendering

---

## Installation

| Firmware | Method |
|----------|--------|
| Official | Pre-installed / Flash from https://xteink.dve.al/ |
| ox4 | `just flash` (requires ESP toolchain) |
| CrossPoint | Web flash at https://xteink.dve.al/ |
| MicroReader | PlatformIO: `pio run -t upload` |

## Resources

- Official: https://www.xteink.com
- ox4: This repository
- CrossPoint: https://github.com/daveallie/crosspoint-reader
- MicroReader: https://github.com/CidVonHighwind/microreader
- Community hub: https://www.readme.club/firmware
