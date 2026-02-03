# Xteink X4 EPUB Implementation Plans

## Device Specifications & Constraints

### Hardware: ESP32-C3 E-Ink Reader
- **SoC**: ESP32-C3 (RISC-V RV32IMC, single-core)
- **CPU**: 160 MHz, no FPU (integer-only math)
- **RAM**: ~400KB total (system + application)
  - Application heap: ~300KB available
  - Stack: 64KB default
- **Flash**: 16MB external (via QSPI)
- **Display**: 480x800 pixels @ 220 PPI, 1-bit (48KB framebuffer)
- **Display Controller**: SSD1677
  - Full refresh: ~500ms
  - Partial refresh: ~120-200ms
- **Storage**: SD card via SPI (FAT32, Class 10 recommended)
- **Connectivity**: USB-C (power + serial), no WiFi/BLE currently enabled

### Software Environment
- **OS**: FreeRTOS (ESP-IDF v5.3.3)
- **Rust**: std available (via esp-idf-svc)
- **Power**: Battery-operated, sleep modes available
- **Toolchain**: riscv32imc-esp-espidf

### Critical Constraints

#### Memory
```
Total RAM:                    400KB
├── System reserved:          ~100KB
├── Display buffer:            48KB
├── Application heap target:  ~250KB
└── EPUB budget (safe):       ~100KB max
```

**OOM threshold**: Anything >150KB peak causes panic/abort

#### CPU Performance
- Single-threaded (no async parallelism)
- Floating-point: Emulated (slow)
- Context switches: ~10-20μs (FreeRTOS)
- Display refresh dominates UX timing

#### Storage I/O
- SD card: ~10-20MB/s read, ~5-10MB/s write
- Seek latency: ~1-5ms (FAT32)
- Cache line: 512 bytes (optimal read size)

#### Power
- Active reading: ~80-120mA
- Light sleep: ~5-10mA
- Deep sleep: ~0.1mA
- Target: 40+ hours reading per charge

---

## Architecture Approaches

### Approach 0: Status Quo (Current Implementation)

**What it is:** Uses `epub` crate (v2.1.5) with full DOM parsing.

**Implementation:**
```rust
// Current: loads ALL fonts + metadata
let mut doc = EpubDoc::new(path)?;
self.load_embedded_fonts(&mut doc)?;  // OOM here!
self.pages = self.paginate_all_chapters()?; // More OOM
```

**Memory Profile:**
- Peak: 150-200KB (DOM trees + font data + all pages)
- Persistent: 100KB+ (keeps everything loaded)
- **Result: CRASH (OOM)**

**Pros:**
- Simple code, one library
- Fast page turns once loaded

**Cons:**
- Crashes on real EPUBs
- Loads fonts into RAM (500KB+ possible)
- Parses entire chapter DOM (50-120KB)
- Caches all pages (30-60KB)

**Status:** ❌ **NON-WORKING** - Do not use

---

### Approach 1: Quick Fix (Minimum Viable)

**What it is:** Use `epub` crate but disable heavy features.

**Implementation:**
```rust
pub fn load(&mut self, path: &str) -> Result<(), String> {
    let mut doc = EpubDoc::new(path)?;
    
    // Only metadata, skip heavy operations
    self.spine = doc.spine.clone();
    self.title = doc.mdata("title")?.map(|t| t.value);
    
    // ❌ DON'T: self.load_embedded_fonts(&mut doc)?;
    // ❌ DON'T: Load all chapters upfront
    
    // ✅ DO: Load one chapter at a time
    self.load_chapter(0, &mut doc)?;
    Ok(())
}

fn load_chapter(&mut self, idx: usize, doc: &mut EpubDoc) -> Result<(), String> {
    doc.set_current_chapter(idx);
    let (html, _) = doc.get_current_str()?;
    let text = html_to_text(&html); // Strip tags
    
    // Layout just this chapter
    self.current_chapter = self.layout_text(&text)?;
    Ok(())
}

fn render(&self, display: &mut impl DrawTarget) {
    // Use built-in font, not EPUB fonts
    let style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
    for line in &self.current_chapter.lines {
        Text::new(&line.text, Point::new(10, line.y), style).draw(display)?;
    }
}
```

**Memory Profile:**
- Peak: 60-80KB (one chapter + metadata)
- Persistent: 40KB (spine + current chapter)
- Font memory: 0KB (uses built-in)

**Pros:**
- ✅ Works this week
- ✅ Minimal code changes
- ✅ <100KB RAM usage
- ✅ Fast to implement (1-2 days)

**Cons:**
- ❌ No embedded font support (EPUB fonts ignored)
- ❌ Re-layout on font/size change (3-5s)
- ❌ Complex HTML → plain text only
- ❌ No images in EPUBs

**Use Case:** Reading simple text-only EPUBs immediately.

**Status:** ⚠️ **VIABLE SHORT-TERM** - Good for MVP

---

### Approach 2: Streaming Reflow (Recommended)

**What it is:** Custom streaming EPUB reader with lazy loading.

**Architecture:**
```
┌─────────────────────────────────────────────────────┐
│  EPUB File (.epub)                                  │
│  └─ ZIP archive on SD card                           │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│  StreamingZip (custom no_std or std)              │
│  ├─ Central directory cache: ~4KB                 │
│  ├─ Decompress buffer: 4KB                          │
│  └─ Stream one file at a time                       │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│  SAX XML Parser (quick-xml)                         │
│  ├─ Event buffer: 4KB                              │
│  ├─ No DOM - streaming extract                      │
│  └─ Extract: text + minimal structure               │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│  Chapter Cache (32KB)                              │
│  ├─ Plain text: ~28KB                              │
│  ├─ Structure markers: ~4KB                          │
│  └─ Only current chapter in RAM                     │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│  Layout Engine (greedy wrap)                        │
│  ├─ Current page only: ~8KB                        │
│  ├─ Line buffer: 50 lines × 80 chars                 │
│  └─ Font metrics cache                               │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│  Display (48KB framebuffer)                         │
│  └─ Rendered page                                   │
└─────────────────────────────────────────────────────┘
```

**Libraries:**
```toml
[dependencies]
# ZIP streaming (std-compatible)
zip = { version = "2.1", default-features = false, features = ["deflate"] }

# OR custom streaming (if zip has issues)
# miniz_oxide = "0.8"  # no_std deflate

# XML SAX parsing
quick-xml = { version = "0.37", features = ["encoding"] }

# Fonts with LRU cache
fontdue = { version = "0.9", features = ["hashbrown"] }

# Fixed collections
heapless = "0.8"
```

**Memory Budget:**
```
ZIP central directory:     4KB
Decompression buffer:      4KB
XML parsing buffer:        8KB
Chapter text cache:       32KB
Glyph LRU cache:          32KB
Layout state:              8KB
Metadata:                  4KB
────────────────────────────────
TOTAL:                    92KB
```

**Implementation Phases:**

**Phase 1 (Week 1): Core Streaming**
- Custom `StreamingZip` (or fix `zip` crate usage)
- SAX XML parser for `content.opf` (metadata)
- Basic chapter text extraction

**Phase 2 (Week 2): Layout & Cache**
- Greedy text wrap layout
- LRU glyph cache (256 glyphs)
- Single-chapter pagination

**Phase 3 (Week 3): Polish**
- Font size change handling
- TOC navigation
- Progress saving

**Phase 4 (Week 4): Fonts & Images**
- Lazy embedded font loading
- Selective image support

**Performance:**
- Open EPUB: 1-2 seconds
- Chapter load: 1-2 seconds
- Page turn: 100-200ms
- Font resize: 3-5 seconds

**Pros:**
- ✅ <100KB RAM (safe margin)
- ✅ Supports font changes
- ✅ Can use embedded fonts (lazy loaded)
- ✅ Good typography (text wrap)
- ✅ Industry-standard algorithm (greedy wrap)

**Cons:**
- ⚠️ 1-2 weeks development
- ⚠️ Font resize takes 3-5s
- ⚠️ Complex EPUBs may need simplification

**Use Case:** Production EPUB reader with full features.

**Status:** ✅ **RECOMMENDED** - Best balance of features/safety

---

### Approach 3: Binary Cache (Crosspoint-Style)

**What it is:** Pre-render EPUB pages to binary cache on SD card.

**Architecture (inspired by crosspoint-reader):**
```
First Open:
EPUB → Parse all chapters → Render to binary cache → Display
(5-30 seconds initial delay)

Subsequent Opens:
Binary cache → Direct display
(1 second, instant page turns)
```

**Cache Format (XTC - XTeink Cache):**
```
/books/book.xtc/
├── header.json          # Metadata
├── spine.msgpack        # Page LUT offsets
├── chapter_000/
│   ├── pages.bin        # Pre-rendered pages
│   └── lut.bin          # Page seek table
├── chapter_001/
│   └── ...
```

**Memory Profile:**
- Peak (cache creation): 150KB (OOM risk!)
- Steady state: 50KB (read from cache)
- Page turn: 0KB CPU, just memcpy

**Pros:**
- ✅ Fastest page turns (<100ms)
- ✅ No CPU during reading
- ✅ Excellent battery life
- ✅ Survives sleep/resume instantly

**Cons:**
- ❌ Font change = full re-cache (30s)
- ❌ Initial open very slow (5-30s)
- ❌ Heavy SD writes (wear)
- ❌ Complex cache invalidation
- ❌ Cache corruption risk

**Use Case:** Users who pick one font and read many books.

**Status:** ⚠️ **VIABLE BUT COMPLEX** - High effort, tradeoffs

---

### Approach 4: Hybrid (Best of Both)

**What it is:** Store structure, not pixels. Fast re-layout.

**Architecture:**
```
EPUB Parse → Extract Structure (text + styles) → Save to SD
                                  ↓
                        ┌─────────────────────┐
                        │ Structure Cache     │
                        │ (per chapter)       │
                        │ - Text segments     │
                        │ - Style markers     │
                        │ - 5-15KB/chapter    │
                        └─────────────────────┘
                                  ↓
                        ┌─────────────────────┐
                        │ Layout Engine       │
                        │ (fast re-layout)    │
                        │ 1-2s per chapter    │
                        └─────────────────────┘
```

**Benefits:**
- Font change: 1-2s (re-layout from structure)
- Initial open: 3-6s (parse + save structure)
- Page turn: 100-200ms (layout current page)
- Memory: 80-100KB

**Pros:**
- ✅ Faster font change than Approach 2
- ✅ No heavy SD writes like Approach 3
- ✅ Supports rich formatting

**Cons:**
- ⚠️ Most complex implementation
- ⚠️ 2-3 weeks development
- ⚠️ Structure format needs design

**Status:** ⚠️ **OVERKILL** - Benefits don't justify complexity

---

### Approach 5: Server-Side Rendering

**What it is:** Convert EPUBs on desktop, transfer simplified format.

**Toolchain:**
```bash
# Desktop tool (Rust or Python)
$ xteink-convert book.epub
→ Generates: book.xtb (XTeink Book)

# Format: Compressed text + metadata + simple markup
```

**Device Reader:**
- Reads `.xtb` files (no EPUB parsing)
- Simple format = tiny code
- Fast loading

**Pros:**
- ✅ Minimal device code
- ✅ Can use heavy desktop libraries
- ✅ Fastest on-device performance
- ✅ Handles complex EPUBs

**Cons:**
- ❌ Requires desktop preprocessing
- ❌ Extra step for users
- ❌ Not "native" EPUB support

**Use Case:** Power users with computer access.

**Status:** ⚠️ **WORKAROUND** - Not true EPUB support

---

## Comparison Matrix

| Approach | RAM | Dev Time | Font Change | Page Turn | Complexity | Recommendation |
|----------|-----|----------|-------------|-----------|------------|----------------|
| **0: Status Quo** | 150KB+ | 0d | N/A | N/A | Low | ❌ Crashes |
| **1: Quick Fix** | 60KB | 1-2d | 3-5s | 200ms | Low | ⚠️ MVP |
| **2: Streaming** | 90KB | 1-2w | 3-5s | 200ms | Medium | ✅ **Best** |
| **3: Binary Cache** | 50KB | 2-3w | 30s | 50ms | High | ⚠️ Complex |
| **4: Hybrid** | 90KB | 2-3w | 1-2s | 200ms | High | ⚠️ Overkill |
| **5: Server-Side** | 20KB | 1w | N/A | 100ms | Low | ⚠️ Not native |

---

## Decision Guide

### Choose Approach 1 (Quick Fix) if:
- Need EPUBs working THIS WEEK
- Accepting simple text-only rendering
- OK with built-in fonts only
- Want to validate EPUB UX before heavy investment

### Choose Approach 2 (Streaming) if:
- Building production EPUB reader
- Need embedded font support
- Want proper typography
- Accepting 1-2 week development
- Comfortable with 3-5s font resize time

### Choose Approach 3 (Binary Cache) if:
- Page turn speed is absolute priority
- Users rarely change fonts
- OK with slow initial opens
- Want maximum battery life
- Willing to handle cache complexity

### Avoid Approach 4 (Hybrid) unless:
- Font changes happen frequently
- 3-5s is too slow for resize
- Have 3+ weeks to develop
- Want to over-engineer (not recommended)

### Choose Approach 5 (Server-Side) if:
- Desktop preprocessing is acceptable
- Want minimal device complexity
- Building for tech-savvy users
- True EPUB support not required

---

## Recommended Path Forward

### Immediate (This Week): Approach 1
1. Disable `load_embedded_fonts()` in current code
2. Use `MonoTextStyle` built-in fonts
3. Load chapters one at a time
4. Test with real EPUBs on device
5. Validate memory usage stays <80KB

**Goal:** Working EPUB reader for simple books.

### Short-Term (Next 2 Weeks): Approach 2
1. Implement `StreamingZip` with `zip` crate
2. Add SAX parser with `quick-xml`
3. Build LRU glyph cache
4. Create layout engine
5. Integrate into App state machine

**Goal:** Production EPUB reader with full features.

### Long-Term (If Needed): Optimization
- Profile real-world usage
- Add binary cache for frequently-read books
- Optimize based on user feedback
- Consider Approach 3 only if page turns are problematic

---

## Technical References

### Crosspoint-Reader Analysis
The `crosspoint-reader/` C++ firmware demonstrates:
- Streaming ZIP with central directory cursor
- SAX-style XML parsing (Expat)
- Two-tier caching (temp files → binary cache)
- Page serialization format
- Aggressive resource cleanup
- 400KB RAM constraint handling

**Key Insight:** They cache rendered pages (Approach 3), accepting the font-change penalty.

### Memory-Safe Patterns
1. **Fixed buffers** for I/O (4-8KB)
2. **Arena allocation** per chapter
3. **LRU caches** with bounded size
4. **Streaming over loading**
5. **Lazy initialization**

### Performance Targets
- Open EPUB: <2s
- Chapter load: <2s  
- Page turn: <200ms
- Font resize: <5s
- Battery: 40+ hours reading

---

## Appendix: Failed Approaches

### ❌ Full DOM Parsing (xml-rs)
- Loads entire XML tree into RAM
- 50-120KB per document
- Causes OOM on typical EPUBs

### ❌ Loading All Embedded Fonts
- TTF fonts: 50-500KB each
- 2-3 fonts typical: 150KB-1.5MB
- Immediately OOM on ESP32-C3

### ❌ Caching All Chapters
- War and Peace: 1200 pages
- 50 chapters × 50KB = 2.5MB
- Far exceeds RAM

### ❌ Uncompressed EPUB Storage
- EPUBs are ZIP (compressed)
- Extracting doubles storage
- No benefit, wastes SD space

---

*Last Updated: 2026-02-03*
*Based on: ESP32-C3 analysis, crosspoint-reader architecture review, Rust ecosystem evaluation*
