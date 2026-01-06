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

## Performance

- **Embassy async runtime** - Use `embassy-executor` on ESP-IDF for cooperative multitasking
- **EPUB pre-caching** - Parse/render next pages in background while user reads
- **Page cache on SD** - Store rendered pages to SD, instant page turns
- **Streaming parser** - Parse EPUB incrementally, don't load entire file to RAM

```
┌─────────────────┐     ┌──────────────────────┐
│ Foreground      │     │ Background (async)   │
│ - Display page  │     │ - Parse next chapter │
│ - Handle input  │     │ - Render to buffer   │
│                 │     │ - Cache to SD        │
└─────────────────┘     └──────────────────────┘
```

Dependencies:
```toml
embassy-executor = { version = "0.7", features = ["task-arena-size-32768"] }
embassy-time = "0.4"
esp-idf-svc = { version = "0.51", features = ["embassy-time-driver"] }
```

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
