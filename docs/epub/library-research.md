# EPUB + Typography Library Scan (Rust) — 2026-02-03

Scope: Rust libraries that can realistically power a production-grade EPUB reader on a constrained device. Focus is on *embedded-suitable building blocks* (streaming ZIP, XML/XHTML parsing, font parsing/shaping/raster, compact serialization). This is not a drop-in EPUB renderer list because none currently meet the “high-stars + embedded-ready + full features” bar.

As-of date: 2026-02-03. GitHub star counts are read from repo pages and are approximate.

---

## Key Takeaways

- There is **no high-adoption, embedded-focused, full EPUB renderer** in Rust today. Most EPUB crates are small or license-constrained.
- The feasible path is to assemble a stack from **streaming ZIP + SAX/XML + font stack + custom layout**.
- For on-device caches, **postcard** or **MessagePack** are better than JSON. **bincode** is archived.

---

## EPUB-Specific Crates (Low Adoption / License Risk)

These do not currently meet the “high-star embedded-ready” threshold. Useful for reference, but likely not the core of the firmware implementation.

- `epub` (crate) — GPL-3.0; license risk for firmware distribution.
- `rbook` — permissive license; smaller community. GitHub stars are low.
- `epubie-lib` — new/small; likely not battle-tested.

Recommendation: Avoid a full dependency on these crates for firmware. Use them only for desktop tooling or as reference implementations.

---

## Streaming ZIP (EPUB container)

- `rc-zip` — ~397 stars. Sans-I/O ZIP reader with a state-machine design, good for streaming and bounded buffers.
  Repo: https://github.com/bearcove/rc-zip

- `zip2` (zip-rs) — ~276 stars. General-purpose ZIP crate; heavier; still useful if memory is carefully capped.
  Repo: https://github.com/zip-rs/zip2

- `rawzip` — minimalist ZIP reader (smaller adoption). Useful if you want to manage deflate yourself.
  Docs: https://docs.rs/rawzip

- `miniz_oxide` — ~230 stars. Pure-Rust deflate; supports `no_std` + `alloc`.
  Repo: https://github.com/Frommi/miniz_oxide

Embedded fit: `rc-zip` + `miniz_oxide` is the most promising streaming combo.

---

## XML / XHTML Parsing

- `quick-xml` — ~1.5k stars. High-performance pull parser; good for SAX-style streaming of OPF, NCX, and XHTML.
  Repo: https://github.com/tafia/quick-xml

Embedded fit: strong. This is the primary XML parser to build on.

---

## Fonts, Shaping, Rasterization

- `ttf-parser` — ~752 stars. Zero-allocation, `no_std`-friendly font parser. Good for reading tables and metrics.
  Repo: https://github.com/harfbuzz/ttf-parser

- `rustybuzz` — ~656 stars. HarfBuzz port for shaping complex scripts.
  Repo: https://github.com/harfbuzz/rustybuzz

- `fontdue` — ~1.6k stars. `no_std` glyph rasterizer with a simple layout model (no complex shaping).
  Repo: https://github.com/mooman219/fontdue

- `swash` — ~819 stars. High-quality shaping + rasterization stack; heavier but very capable.
  Repo: https://github.com/dfrg/swash

- `parley` — ~507 stars. Rich text layout (Linebender ecosystem), heavier dependency tree.
  Repo: https://github.com/linebender/parley

- `fontations` — ~721 stars. Google Fonts’ Rust font stack (e.g., `read-fonts`, `skrifa`). Powerful but not embedded-lean.
  Repo: https://github.com/googlefonts/fontations

Embedded fit: use `ttf-parser` + `rustybuzz` + `fontdue` for a lean stack, or `swash` if quality trumps size.

---

## Compact Serialization (Better Than JSON)

- `postcard` — ~1.3k stars. `no_std`, compact, stable. Good for caches and settings on SD.
  Repo: https://github.com/jamesmunns/postcard

- `msgpack-rust` — ~1.4k stars. MessagePack codec; compact + self-describing.
  Repo: https://github.com/3Hren/msgpack-rust

- `rkyv` — ~4k stars. Zero-copy deserialization. Powerful but requires careful versioning discipline.
  Repo: https://github.com/rkyv/rkyv

- `ciborium` — ~351 stars. Active CBOR implementation under Enarx org.
  Repo: https://github.com/enarx/ciborium

- `bincode` — archived (Aug 15, 2025). Not recommended for new work.
  Repo: https://github.com/bincode-org/bincode

Embedded fit: `postcard` for fixed schemas; `msgpack-rust` for self-describing caches.

---

## Recommended Core Stack (Embedded-Focused)

- ZIP: `rc-zip` + `miniz_oxide`
- XML/XHTML: `quick-xml`
- Fonts: `ttf-parser` + `rustybuzz` + `fontdue`
- Cache format: `postcard` (or `msgpack-rust` if you want self-describing data)

Notes:
- Embedded font support is limited by RAM; fonts above a size cap should be skipped or preprocessed.
- Full CSS support is not realistic on-device; implement a controlled subset.

