# Xteink X4 Roadmap

Feature tracker for the Xteink X4 e-ink reader. Prioritized by user impact and
dependency order. Each section has a status, a brief rationale, and
implementation notes.

---

## 1. E-Ink Refresh Strategy

**Status:** Not started
**Priority:** Critical
**Why:** Black-bar selection indicators ghost after a few interactions, making
the UI unreadable. This must be solved before any visual polish matters.

### Requirements

- Periodic full refresh after N partial updates (configurable, default 10).
- Counter tracked per-activity so each screen gets a fresh baseline.
- Three-tier strategy matching the driver's existing modes:
  - `RefreshMode::Fast` for most interactions (~300 ms).
  - `RefreshMode::Partial` for periodic ghost cleanup (~1700 ms).
  - `RefreshMode::Full` on activity enter or manual trigger.
- Activity trait extended with `fn refresh_mode(&self) -> RefreshMode` so the
  firmware render loop can ask the current activity what mode to use.
- Expose "Refresh Frequency" in Reader Settings (already has the
  `RefreshFrequency` enum: 1, 5, 10, 15, 30 pages).

### Implementation Notes

- Add `pages_since_full_refresh: u32` to each activity (or to `App`).
- After each render, decrement counter; when it hits 0, return
  `RefreshMode::Partial` and reset.
- The firmware `update_display_*` functions already support all three modes.
- Diff-based updates (`compute_diff_region`) should still be used with
  `Fast` mode to minimize bandwidth.

### Reference

CrossPoint Reader uses a counter-based approach:
```
if pages_until_full_refresh <= 1 {
    display(HalfRefresh);
    pages_until_full_refresh = setting;
} else {
    display(FastRefresh);
    pages_until_full_refresh -= 1;
}
```

---

## 2. Auto-Detect Books from SD Card

**Status:** Not started
**Priority:** Critical
**Why:** Library is currently populated with mock data. Real books must be
discovered from the filesystem.

### Requirements

- On `LibraryActivity::on_enter()`, scan the SD card for supported files.
- Supported extensions: `.epub`, `.txt`, `.md`.
- Recursive scan from a configurable root (default `/books/`).
- Extract metadata where possible:
  - EPUB: title, author, cover ID from OPF (parsing already exists in
    `epub::metadata`).
  - TXT/MD: use filename as title, "Unknown" as author.
- Build `Vec<BookInfo>` from scan results.
- Cache scan results to avoid re-scanning on every enter (invalidate on
  timestamp change or manual refresh).
- Filter hidden files and system directories (`.`, `System Volume Information`).
- Show loading indicator during scan.

### Implementation Notes

- `FileSystem` trait already has `list_files()`. Add a recursive helper:
  `fn scan_books(fs: &mut dyn FileSystem, root: &str) -> Vec<BookInfo>`.
- EPUB metadata extraction: `extract_metadata()` already parses OPF. Wire it
  up to populate `BookInfo.title` and `BookInfo.author`.
- Need to add `read_file_bytes()` to `FileSystem` trait for binary reads
  (needed for EPUB zip parsing).
- `MockFileSystem` should be updated with mock book files for simulator.

---

## 3. Raw File Browser Activity

**Status:** Not started
**Priority:** High
**Why:** Users need direct access to the SD card filesystem beyond just the
book library.

### Requirements

- New `FileBrowserActivity` wrapping the existing `FileBrowser` component.
- Navigate directories, view files, open supported book formats.
- Breadcrumb or path display in header.
- Actions: Open, Delete, Info.
- Back button goes up one directory; long-press or Back at root returns to
  system menu.
- Add "Files" entry to the system menu.

### Implementation Notes

- `FileBrowser` component and `FileSystem` trait already exist.
- Register as `AppScreen::FileBrowser` in `app.rs`.
- Reuse `LibraryActivity` patterns for input handling and rendering.

---

## 4. Image Decoding & Cover Art

**Status:** Not started
**Priority:** High
**Why:** Cover thumbnails in the library list are currently black rectangles.
Book covers are essential for scannability.

### Requirements

- Decode BMP images (simplest format for 1-bit e-ink).
- Optional: decode JPEG/PNG (with `tinybmp`, `embedded-graphics` BMP, or a
  minimal decoder).
- Extract cover image from EPUB files (cover item path already detected in
  `EpubMetadata::get_cover_item()`).
- Render cover thumbnails in library list items (50px wide, aspect-fit).
- Cache decoded 1-bit thumbnails to SD card (e.g.,
  `/.xteink/covers/<hash>.bmp`).

### Implementation Notes

- `tinybmp` crate is `no_std` compatible and works with `embedded-graphics`.
- For EPUB covers: read the cover image bytes from the zip, decode, dither to
  1-bit, scale to thumbnail size, cache the result.
- `BufferedDisplay` already supports `DrawTarget` so images can be drawn
  directly.
- The `COVER_WIDTH: u32 = 50` constant is already defined in
  `library_activity.rs`.
- For dithering: Floyd-Steinberg or simple threshold (threshold is fine for
  covers on 1-bit displays).

### Dependency

Requires: #2 (Auto-Detect Books) for real book data.

---

## 5. Wallpaper / Sleep Screen

**Status:** Not started
**Priority:** Medium
**Why:** Personalization and a polished idle experience. E-ink displays retain
their image at zero power cost, so a good sleep screen is essentially free.

### Requirements

- Sleep screen displayed when device enters low-power mode.
- Sources (user-configurable in Settings):
  - **None** - blank / white screen.
  - **Custom image** - user places `wallpaper.bmp` in root of SD card.
  - **Random images** - user places `.bmp` files in `/wallpapers/` directory.
  - **Current book cover** - uses the cover of the last-read book.
- Image format: uncompressed BMP, 480x800, 1-bit or 24-bit (auto-dithered).
- Fit modes: Fit (letterbox) or Fill (crop).
- Add "Wallpaper" or "Sleep Screen" settings section to `SettingsActivity`.

### Implementation Notes

- On sleep trigger, render wallpaper to `BufferedDisplay` and do a
  `RefreshMode::Full` update.
- Reuse the image decoding pipeline from #4.
- Store preference in `Settings` struct (new field: `sleep_screen: SleepScreenMode`).
- Enum: `SleepScreenMode { None, CustomImage, RandomImage, BookCover }`.

### Dependency

Requires: #4 (Image Decoding) for BMP rendering.

---

## 6. WiFi Support & Web Server

**Status:** Not started
**Priority:** Medium
**Why:** Wireless book upload eliminates the need to physically access the SD
card. OTA updates enable shipping fixes without disassembly.

### Requirements

#### WiFi Connection
- WiFi scanning activity: scan networks, display SSIDs with signal strength.
- Credential entry (limited input on 5-button device - consider a generated
  AP mode where the user configures via phone/laptop).
- Credential storage on SD card.
- Connection status in system menu header.

#### Web Server Mode
- Separate activity/mode: "WiFi Transfer" in system menu.
- HTTP server on port 80:
  - `GET /` - status page with device info.
  - `GET /files` - file manager (HTML/JS served from flash or SD).
  - `POST /upload` - multipart file upload.
  - `GET /download?path=...` - file download.
  - `POST /delete` - delete file.
  - `POST /mkdir` - create directory.
- Display device IP, connection status, and transfer progress on e-ink screen.
- AP mode fallback: create "Xteink-XXXX" network if no saved networks.

#### OTA Updates
- Check for firmware updates from a configured URL.
- Download and flash via ESP-IDF OTA APIs.
- Show progress bar on display during update.

### Implementation Notes

- ESP-IDF provides WiFi and HTTP server APIs (`esp-idf-svc` crate).
- This is firmware-only code (`xteink-firmware`), not in the UI library.
- The UI library provides the activity screens; the firmware provides the
  network backend.
- WebSocket support for fast binary uploads (like CrossPoint Reader).
- UDP discovery broadcast so companion apps can find the device.
- Web UI should be minimal - just a file upload form and directory listing.

### Reference

CrossPoint Reader runs HTTP on port 80, WebSocket on port 81, and UDP
discovery on port 8134. Their web UI supports upload, download, delete,
rename, move, and mkdir.

---

## 7. Bluetooth Support

**Status:** Not started
**Priority:** Low
**Why:** Enables wireless page turning with external buttons, headphone
controls, or foot pedals. Nice-to-have for accessibility.

### Requirements

- BLE (Bluetooth Low Energy) support via ESP32-C3's built-in radio.
- HID profile: act as a BLE keyboard receiver for external page-turn buttons.
- Map BLE HID events to `InputEvent` (page forward/back).
- Pairing UI in settings.
- Optional: BLE serial for debugging/CLI access.

### Implementation Notes

- ESP32-C3 supports BLE 5.0 via `esp-idf-svc` or `esp32-nimble` crate.
- BLE and WiFi share the same radio; cannot use both simultaneously on
  ESP32-C3 (or performance degrades significantly).
- Start with BLE HID consumer for page-turn remotes.

---

## 8. OPDS Catalog Browser

**Status:** Not started
**Priority:** Low
**Why:** Browse and download books directly from Calibre, COPS, or other OPDS
servers without a computer.

### Requirements

- New activity: OPDS browser.
- Parse OPDS (Atom XML) feeds.
- Display book entries with title, author, cover thumbnail.
- Download books directly to SD card.
- Support HTTP Basic Auth for private servers.
- Configurable server URL in settings.

### Dependency

Requires: #6 (WiFi) for network access, #4 (Image Decoding) for covers.

---

## 9. Reading Progress Persistence

**Status:** Not started
**Priority:** High
**Why:** Users expect to resume exactly where they left off after power cycling.

### Requirements

- Save reading position per book (chapter index, byte offset, page number).
- Persist to SD card: `/.xteink/progress/<path_hash>.bin`.
- Load on book open; save on page turn and book close.
- Store `last_read` timestamp for "Recent" sort in library.
- Minimal struct: `{ chapter: u16, offset: u32, page: u16, timestamp: u64 }`.

### Implementation Notes

- Use a simple binary format (fixed-size struct, no serialization library
  needed for `no_std`).
- Write to a temp file then rename for atomic updates.
- The `BookInfo::last_read` field already exists but is unpopulated.

---

## 10. Bookmarks & Annotations

**Status:** Not started
**Priority:** Low
**Why:** Power readers want to mark passages and return to them later.

### Requirements

- Add/remove bookmarks at current position.
- Bookmark list activity per book.
- Navigate to bookmarked position.
- Optional: short text annotations (difficult with limited input, but
  possible with character picker).
- Persist to SD card alongside progress data.

---

## 11. Reading Statistics

**Status:** Not started
**Priority:** Low
**Why:** Gamification and self-awareness for reading habits.

### Requirements

- Track pages read, time spent reading, sessions per day.
- Display in Information activity or dedicated Stats activity.
- Persist daily aggregates to SD card.
- Simple visualizations: bar chart of pages/day (last 7 days), total stats.

---

## 12. Dictionary Lookup

**Status:** Not started
**Priority:** Low
**Why:** Look up words without leaving the reader.

### Requirements

- Offline dictionary stored on SD card (StarDict or similar format).
- Word selection in reader (cursor-based, given no touch).
- Popup with definition.
- Manage dictionaries via web server upload.

---

## 13. Search Within Book

**Status:** Not started
**Priority:** Low
**Why:** Find specific passages in the current book.

### Requirements

- Text search within current EPUB/TXT.
- Character-by-character input with rotary selector (A-Z picker).
- Results list with context snippets.
- Navigate to result.

---

## 14. Multi-Format Support

**Status:** Partial (EPUB + TXT exist)
**Priority:** Medium
**Why:** PDF and comic formats expand the device's usefulness.

### Requirements

- **PDF**: Reflow or page-image rendering. Difficult on ESP32 due to memory
  constraints. Consider server-side conversion via web server.
- **FB2**: XML-based, similar complexity to EPUB.
- **CBZ/CBR**: Comic archives (ZIP/RAR of images). Simpler than PDF - just
  image rendering with navigation.

### Dependency

Requires: #4 (Image Decoding) for comic/PDF page images.

---

## 15. Power Management

**Status:** Not started
**Priority:** Medium
**Why:** Battery life is critical for a portable reader.

### Requirements

- Deep sleep between interactions (wake on button press).
- Configurable auto-sleep timeout.
- Battery prediction ("~3 weeks remaining" based on usage patterns).
- Low battery warning and graceful shutdown.

### Implementation Notes

- ESP32-C3 deep sleep: ~5 uA.
- Wake sources: GPIO (buttons), timer.
- Save state before sleep, restore on wake.
- E-ink retains image during sleep (zero power for display).

---

## Priority Summary

| # | Feature | Priority | Depends On |
|---|---------|----------|------------|
| 1 | E-Ink Refresh Strategy | Critical | - |
| 2 | Auto-Detect Books | Critical | - |
| 3 | Raw File Browser | High | - |
| 4 | Image Decoding & Covers | High | #2 |
| 5 | Wallpaper / Sleep Screen | Medium | #4 |
| 6 | WiFi & Web Server | Medium | - |
| 7 | Bluetooth | Low | - |
| 8 | OPDS Browser | Low | #6, #4 |
| 9 | Reading Progress Persistence | High | - |
| 10 | Bookmarks & Annotations | Low | #9 |
| 11 | Reading Statistics | Low | #9 |
| 12 | Dictionary Lookup | Low | - |
| 13 | Search Within Book | Low | - |
| 14 | Multi-Format Support | Medium | #4 |
| 15 | Power Management | Medium | - |

### Suggested Build Order

```
Phase 1 - Core functionality
  #1 E-Ink Refresh Strategy
  #2 Auto-Detect Books
  #9 Reading Progress Persistence
  #3 Raw File Browser

Phase 2 - Visual polish
  #4 Image Decoding & Covers
  #5 Wallpaper / Sleep Screen

Phase 3 - Connectivity
  #6 WiFi & Web Server
  #15 Power Management

Phase 4 - Enrichment
  #7 Bluetooth
  #8 OPDS Browser
  #14 Multi-Format Support
  #10 Bookmarks & Annotations
  #11 Reading Statistics
  #12 Dictionary Lookup
  #13 Search Within Book
```

---

*Last updated: 2026-02-07*
