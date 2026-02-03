# Revised EPUB Implementation Plan (Production-Grade, Constrained Device)

Target: Xteink X4 (ESP32-C3, ~300KB usable heap, 16MB SD, 480x800 1-bit display)
Date: 2026-02-03

Goal: A production-level EPUB reader with real typography, chapter navigation, page counts, font size changes, font switching, and persistence — while respecting tight memory limits. Target is full language support via an optional full-shaping feature flag, with a lean Latin-only path for bring-up.

Non-goals (v1): full CSS2/3 layout, JavaScript, SVG, MathML, audio/video, complex floats/tables.

---

## Summary Strategy

1. Replace DOM-based EPUB parsing with a **streaming pipeline** that never holds full chapters, fonts, or pages in RAM.
2. Convert XHTML to a **compact token stream** stored on SD (structure cache). This decouples parsing from layout and enables fast reflow.
3. Build a **layout engine** that paginates from tokens and stores a **page map** for fast page turns and total page counts.
4. Implement a **font system** with strict size caps and fallback strategy (built-in fonts + optional embedded/user fonts).
5. Use **compact binary caches** on SD (postcard/msgpack) to keep RAM usage stable.

---

## Architecture Overview

EPUB (.epub)
- ZIP container on SD (streamed)
- `container.xml` -> `content.opf` (metadata + spine)
- XHTML chapters
- Optional CSS + fonts + images

Pipeline
1. Streaming ZIP reader: bounded buffer for entries.
2. XML/XHTML streaming parser: emits token stream.
3. Token cache on SD: per-chapter structure file.
4. Layout engine: token stream -> line breaks -> page map.
5. Renderer: glyph cache + framebuffer draw.

---

## Library Choices (Embedded-Focused)

- ZIP: `rc-zip` + `miniz_oxide`
- XML/XHTML: `quick-xml`
- Fonts (full): `ttf-parser` + `rustybuzz` + `fontdue` (or `swash` if quality outweighs size)
- Fonts (latin-only): `fontdue` only
- Cache format: `postcard` (default), `msgpack-rust` (optional)

These are the only libraries in scope that balance adoption and embedded suitability.

---

## Data Model (On-Device)

### Token Stream (Structure Cache)
Each chapter is stored as a stream of tokens on SD:
- `TextRun { text, style_id }`
- `ParagraphBreak`
- `Heading { level }`
- `ListItem { level }`
- `EmphasisOn/Off`, `StrongOn/Off`
- `ImageRef { path, width, height }`
- `SoftBreak`, `HardBreak`

Tokens are serialized with `postcard` (fixed schema, versioned header). Keep per-chapter cache under ~20KB.

### Page Map
For each chapter, store page offsets:
- `page_index -> token_offset`
- `token_offset -> text_offset`

This enables:
- fast page turns
- total page counts
- resume after reboot

---

## CSS / Styling Subset (Constrained but Useful)

Supported (v1):
- `font-size` (px, em)
- `font-family`
- `font-weight` (normal, bold)
- `font-style` (normal, italic)
- `text-align` (left/center/right/justify)
- `line-height`
- `margin-top/bottom` (block spacing)

Ignored (v1):
- float, table, grid, position, transform, text-shadow, background-image
- complex selectors; only tag, class, and inline style

---

## Fonts Strategy (Realistic on ESP32-C3)

1. **Built-in fonts** (default): stored in firmware or SD, always available. Keep small.
2. **User fonts**: loaded from `/fonts` on SD if <= size cap (start with 200KB, tune later).
3. **Embedded fonts**: parsed from EPUB resources only if <= size cap.
4. **Fallback**: if font is too large or fails, fall back to built-in.

Notes:
- Most TTF parsers expect the font blob in RAM. Large embedded fonts are not feasible without preprocessing.
- If full embedded font support is required, add an *optional desktop preprocessor* that subsets fonts. This is not in the immediate implementation path, but is part of the long-term design.

### Feature Flags (Language Support)
- `epub_latin` (default): lean layout with `fontdue`, minimal Unicode handling.
- `epub_full` (optional): `rustybuzz` + bidi + unicode line breaking for complex scripts.

Goal is to ship the Latin-only path first, with a clean upgrade path to full shaping.

---

## Pagination and Page Counts

- On open: parse metadata + build chapter token cache for first chapter only.
- Page map is built incrementally:
  - immediate: current chapter
  - background: next/previous chapters when idle
- Total page count becomes accurate once all chapter page maps are built.
- Page maps persist to SD; subsequent opens are instant.

---

## Phased Implementation Plan

### Phase 0 — Stabilize + Instrument (1–2 days)
- Disable embedded font loading and full-book pagination in `EpubRenderer`.
- Move large buffers off stack; remove recursion in parsing.
- Add heap + stack watermarks to logs.
- Goal: load a real EPUB without stack overflow or OOM.

### Phase 1 — Streaming EPUB Core (3–5 days)
- Implement `EpubArchive` with streaming ZIP access.
- Parse `container.xml` and `content.opf` via `quick-xml`.
- Build spine list + metadata.
- Goal: open EPUB and list chapters with <80KB peak RAM.

### Phase 2 — XHTML Tokenizer + Structure Cache (4–7 days)
- Parse XHTML with `quick-xml` into tokens (no DOM).
- Implement CSS subset resolution for inline and basic selectors.
- Save tokens per chapter to SD (`.tok`).
- Goal: reconstruct chapter content purely from tokens.

### Phase 3 — Layout + Page Map (5–8 days)
- Greedy line breaking using font metrics.
- Render a single page from token stream.
- Build per-chapter page map and persist (`.pg`).
- Goal: page turn <200ms, total pages visible once maps exist.

### Phase 4 — Fonts + Shaping (5–10 days)
- Implement `epub_latin` path (default): `fontdue` rasterization + glyph LRU cache.
- Add font-size and font-family switching (re-layout current chapter).
- Implement `epub_full` path (feature flag): `ttf-parser` + `rustybuzz` + bidi + unicode line break.
- Goal: font change <5s for typical chapters.

### Phase 5 — UX Integration (4–7 days)
- TOC navigation from `nav.xhtml` or `toc.ncx`.
- Progress save/load (current chapter + page + settings).
- Bookmark storage.
- Goal: end-to-end reading workflow.

### Phase 6 — Performance + Power (ongoing)
- Background pagination when idle/charging.
- Prefetch next chapter tokens.
- Partial refresh tuning for page turns.

---

## Memory Budget (Target)

- ZIP + XML buffers: 8–16KB
- Chapter token cache (RAM): 20–32KB
- Glyph cache: 24–32KB
- Layout state: 8KB
- Metadata + spine: 4–8KB

Target peak: <= 120KB RAM beyond framebuffer.

---

## Acceptance Criteria

- Open EPUB: <2s (first open), <1s (cached)
- Page turn: <200ms
- Font size change: <5s
- Accurate total pages once background pagination finishes
- No stack overflow with real books
- Stable reading for 100+ pages without leaks

---

## Risks and Mitigations

- **Large embedded fonts**: enforce size cap; fall back; optional desktop subsetting (future tool).
- **Complex scripts**: shaping adds CPU/RAM; allow a compile-time `latin-only` profile.
- **CSS complexity**: only a subset is supported; document limitations.
- **SD performance**: keep reads aligned (512 bytes) and cache headers.

---

## Optional Desktop Preprocessor (Future)

Purpose: true global language coverage without exceeding RAM limits.

Workflow:
- Desktop tool reads EPUB, extracts used codepoints.
- Subsets one or more fonts to only required glyphs.
- Writes a cache bundle: tokens + page maps + subset fonts.

Device behavior:
- If cache bundle exists, use it (full shaping supported).
- If not, fall back to on-device fonts with size caps.

This is not required for initial implementation, but the on-device format and cache schema should be designed to allow it later.

---

## Proposed File/Module Layout

- `crates/xteink-ui/src/epub/`
  - `archive.rs` (streaming ZIP)
  - `opf.rs` (metadata/spine)
  - `xhtml.rs` (tokenizer)
  - `style.rs` (CSS subset)
  - `tokens.rs` (token definitions + serialization)
  - `layout.rs` (line break + pagination)
  - `render.rs` (glyph raster + draw)

- `crates/xteink-ui/src/epub_render.rs`
  - becomes orchestration layer for the new pipeline

---

## Next Step (If You Approve)

I will start with Phase 0 and Phase 1:
- fix the stack overflow path in `EpubRenderer`
- implement `EpubArchive` + OPF parsing skeleton
