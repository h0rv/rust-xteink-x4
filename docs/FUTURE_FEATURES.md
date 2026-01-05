# Future Features

Post-MVP ideas. Don't touch until basic e-reader works.

## Content Formats

- **EPUB support** - XML parsing, CSS subset, chapter navigation
- **PDF support** - Harder, probably needs external library
- **Comic/manga** - Image scaling, panel detection

## Connectivity

- **WiFi sync** - Download books from Calibre server
- **OTA updates** - Flash new firmware over WiFi
- **Bluetooth remote** - Page turn with external button

## Reading Features

- **Dictionary lookup** - Offline dictionary, select word to define
- **Multiple fonts** - User-selectable typefaces and sizes
- **Reading stats** - Pages per day, time spent, streaks
- **Bookmarks** - Multiple per book, with notes
- **Search** - Find text within current book

## Power

- **Aggressive sleep** - Sleep between page turns, wake on button
- **Battery prediction** - "3 weeks remaining" based on usage patterns

## Platform

- **Multi-chip support** - Same code on ESP32, STM32, RP2040 via embedded-hal
- **Plugin system** - User scripts in Lua or WASM for custom behavior

## Developer Experience

- **WASM simulator enhancements** - File picker, virtual SD card, network mocking
- **Formal verification** - Use Kani/Prusti to prove memory safety in critical paths

## Hardware Variants

- **Larger displays** - 6", 7.8" e-ink panels
- **Color e-ink** - When affordable panels exist
- **Touch input** - If display supports it

---

Pick one at a time. Ship small increments.
