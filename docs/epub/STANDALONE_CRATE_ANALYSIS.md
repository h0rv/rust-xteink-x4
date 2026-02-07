# Should We Ship the EPUB Parser as a Standalone Crate?

## TL;DR

**Yes, with caveats.** The custom EPUB implementation solves real problems that existing crates don't address, but it needs work before being a general-purpose library.

---

## Why We Built a Custom EPUB Parser

### Problem 1: Memory Constraints
**Existing solutions:**
- `epub` crate (GPL-3.0): Loads entire book + fonts into RAM (500KB+)
- Caused OOM crashes at 165KB on ESP32-C3 (400KB total RAM)
- No streaming support

**Our solution:**
- Streaming ZIP reader with 4KB buffer
- One chapter at a time (~32KB)
- SAX-style XML parsing (no DOM)
- **Total: ~60KB peak usage**

### Problem 2: License Issues
**Existing solutions:**
- `epub` crate: GPL-3.0 (copyleft, risky for firmware)
- `rbook`: MIT/Apache-2.0 but small community, not embedded-focused
- `epubie-lib`: Too new, untested

**Our solution:**
- MIT licensed
- No viral license concerns
- Clean room implementation

### Problem 3: Embedded-Hostile Design
**Existing solutions:**
- Assume unlimited memory
- Use `std::fs` for file access (not available on embedded)
- Load entire ZIP into memory
- DOM-based HTML parsing

**Our solution:**
- `no_std` compatible (with `std` feature for testing)
- Works with any `Read` trait (SD card, memory, etc.)
- Streaming architecture throughout
- Token-based processing (no DOM)

---

## Current Implementation

**Size:** ~3,350 lines of Rust

**Modules:**
- `streaming_zip.rs` - Streaming ZIP container reader
- `metadata.rs` - OPF metadata parsing (SAX-style)
- `spine.rs` - Chapter ordering and navigation
- `tokenizer.rs` - XHTML to token stream converter
- `layout.rs` - Text layout and pagination engine
- `tests.rs` - 23 unit tests

**Dependencies:**
- `quick-xml` - SAX parser (minimal allocation)
- `miniz_oxide` - Pure Rust deflate (no_std compatible)
- No heavy dependencies

---

## Would It Be Useful for Others?

### YES - Strong Use Cases

#### 1. **Embedded E-Readers**
- ESP32, STM32, RP2040 devices
- E-ink displays (where memory matters)
- Battery-powered reading devices
- DIY e-reader projects

#### 2. **WASM Applications**
- Browser-based EPUB readers
- Limited memory budgets
- Progressive loading requirements
- Client-side rendering

#### 3. **Mobile/IoT**
- Android/iOS readers (with memory constraints)
- Smartwatches with reading apps
- Embedded Linux devices
- Resource-constrained servers

#### 4. **Research & Education**
- Learning embedded Rust
- Studying streaming algorithms
- Understanding EPUB format
- E-ink display programming

### MAYBE - Weaker Use Cases

#### 1. **Desktop Applications**
- Can use existing crates with full memory
- Our streaming approach adds complexity
- **But:** Still useful if GPL license is an issue

#### 2. **Full-Featured Readers**
- We only support basic EPUB features
- No images, complex CSS, JavaScript, etc.
- **But:** Good foundation to build on

---

## What Would It Take to Ship?

### As-Is (Embedded-Only Crate)
**Difficulty:** Low  
**Timeline:** 1-2 days

**Tasks:**
1. Extract `epub` module to `ox4-epub` crate
2. Write comprehensive README
3. Add examples (embedded + WASM)
4. Document memory usage patterns
5. Publish to crates.io

**Target audience:** Embedded developers only

---

### As General-Purpose Library
**Difficulty:** Medium  
**Timeline:** 1-2 weeks

**Additional work needed:**

#### 1. **Feature Completeness**
- [ ] Image support (JPEG, PNG in EPUB)
- [ ] Better CSS support (basic layout properties)
- [ ] Footnotes and references
- [ ] Table support
- [ ] Nested lists
- [ ] Metadata extraction (cover, language, etc.)

#### 2. **API Design**
- [ ] Clean separation: parser vs. renderer
- [ ] Trait-based abstractions (filesystem, fonts)
- [ ] Builder pattern for configuration
- [ ] Error handling improvements
- [ ] Better chapter/page navigation API

#### 3. **Documentation**
- [ ] API documentation with examples
- [ ] Memory usage guidelines
- [ ] Performance characteristics
- [ ] Comparison with other crates
- [ ] Migration guide from `epub` crate

#### 4. **Testing**
- [ ] Test with real EPUB corpus
- [ ] Benchmark suite
- [ ] Fuzzing for ZIP/XML parsers
- [ ] Cross-platform testing

#### 5. **Polish**
- [ ] Consistent naming conventions
- [ ] Remove ox4-specific assumptions
- [ ] Configurable display dimensions
- [ ] Optional font rendering

---

## Recommended Approach

### Phase 1: Embedded-First Crate (Ship Now)
**Crate name:** `epub-streaming` or `epub-embedded`

**Pitch:**
> "Memory-efficient EPUB parser for embedded systems. Streaming architecture 
> designed for devices with <400KB RAM. MIT licensed."

**Target users:**
- ESP32/STM32/RP2040 developers
- E-ink display projects
- WASM applications
- Anyone avoiding GPL licenses

**Unique selling points:**
1. **<60KB peak memory** - Real measured usage
2. **Streaming everything** - No full file in RAM
3. **`no_std` compatible** - Works without allocator
4. **MIT licensed** - No copyleft concerns
5. **Proven on real hardware** - Used in ox4 firmware

---

### Phase 2: General-Purpose Features (Later)
Once embedded use case is validated, add:
- Image support
- Better CSS
- Desktop convenience APIs
- More complete EPUB 3.0 support

But keep the memory-efficient core intact.

---

## Crate Structure Proposal

```rust
epub-streaming/
├── Cargo.toml
├── README.md
├── CHANGELOG.md
├── LICENSE
├── examples/
│   ├── embedded_reader.rs    # ESP32 example
│   ├── wasm_reader.rs         # Browser example
│   └── parse_metadata.rs      # Simple usage
├── src/
│   ├── lib.rs
│   ├── zip.rs                 # Streaming ZIP
│   ├── metadata.rs            # OPF parsing
│   ├── spine.rs               # Chapter navigation
│   ├── tokenizer.rs           # XHTML tokenizer
│   ├── layout.rs              # Optional layout engine
│   └── error.rs
└── tests/
    └── integration_tests.rs
```

**Features:**
```toml
[features]
default = []
std = ["alloc"]
alloc = []
layout = ["fontdue"]  # Optional layout engine
```

---

## Comparison Matrix

| Crate | License | Memory | Streaming | Embedded | Stars |
|-------|---------|--------|-----------|----------|-------|
| **epub** | GPL-3.0 | High (500KB+) | No | No | ~50 |
| **rbook** | MIT/Apache | Medium | Partial | No | ~10 |
| **epub-streaming** | MIT | Low (<60KB) | Yes | Yes | NEW |

---

## Should We Do It?

### Arguments FOR:
1. **Fills a real gap** - No embedded-friendly EPUB parser exists
2. **Proven implementation** - Already works on real hardware
3. **Community value** - Helps DIY e-reader community
4. **Reusable in other projects** - Not ox4-specific
5. **Learning resource** - Good example of streaming embedded code
6. **MIT licensed** - No restrictions

### Arguments AGAINST:
1. **Maintenance burden** - Now responsible for external API
2. **Still incomplete** - Missing images, complex CSS
3. **Breaking changes likely** - API not fully stabilized
4. **Small audience** - Embedded EPUB readers are niche
5. **Distraction from ox4** - Time spent on external crate

---

## Recommendation

**Ship it as `epub-streaming` targeting embedded systems.**

**Why:**
1. The embedded use case is **proven and working**
2. It solves a **real problem** that existing crates don't
3. The code is **already modular** (just needs extraction)
4. **Low maintenance** if scoped to embedded-only initially
5. **Helps the community** (Rust embedded ecosystem)

**Timeline:**
- Week 1: Extract, document, test
- Week 2: Publish v0.1.0, gather feedback
- Month 2+: Add features based on user needs

**Success metrics:**
- 5+ embedded projects using it within 6 months
- Positive feedback from ESP32/STM32 community
- <10 issues reported (indicates good quality)
- Featured in Awesome Embedded Rust list

---

## Next Steps (If We Ship)

1. Create `epub-streaming` repo
2. Extract ox4 EPUB code (keep in sync with vendoring)
3. Write killer README with memory graphs
4. Add 3 examples (ESP32, WASM, simple parser)
5. Publish v0.1.0 to crates.io
6. Announce on:
   - r/rust
   - ESP-RS Matrix
   - Embedded Rust Matrix
   - This Week in Rust
7. Monitor feedback, iterate

---

*Decision: Ship as embedded-first crate, expand later based on demand.*
