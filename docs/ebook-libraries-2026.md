# E-Book Reading Libraries for Rust — 2026 Edition

**Date:** February 2026  
**Target Device:** Xteink X4 (ESP32-C3, 480x800 e-ink, 400KB RAM)

---

## 📚 EPUB Parsing Libraries

### 🏆 RECOMMENDED: `epub` (v2.1.5)
**Status:** ✅ Actively maintained, widely used  
**License:** GPL-3.0  
**Size:** ~1MB, 918 lines  
**Downloads:** ~4,000/month  
**MSRV:** Rust 1.85.0

**Why use it:**
- Mature and battle-tested (19 releases, 158 commits)
- Simple, clean API for reading EPUB navigation
- Supports spine-based reading (sequential chapters)
- Gets cover images, metadata, resources
- Used by 26 other crates

**Example usage:**
```rust
use epub::doc::EpubDoc;

let mut doc = EpubDoc::new("book.epub")?;

// Get metadata
let title = doc.mdata("title");
let author = doc.mdata("creator");

// Navigate via spine
let chapter_content = doc.get_current_str()?; // Current chapter as string

doc.go_next(); // Next chapter
let next_chapter = doc.get_current_str()?;

// Jump to specific chapter
doc.set_current_chapter(5);
```

**Key features:**
- HashMap-based metadata access
- Spine-based navigation (sequential reading)
- Resource access (images, CSS, fonts)
- Cover image extraction

**Cons:**
- GPL-3.0 license (copyleft - must open-source derivatives)
- Does NOT parse HTML content (gives raw XHTML strings)

---

### Alternative: `rbook` (v0.6.10)
**Status:** ✅ Active, MIT/Apache-2.0  
**License:** Apache-2.0  
**Downloads:** Growing  
**100% documented**

**Why use it:**
- Dual-licensed (Apache-2.0 / MIT) - more permissive than GPL
- Uses `lol_html` for content parsing (streaming HTML parser)
- Modern design, very well documented

**Best for:** If GPL is a concern, this is the best permissively-licensed alternative

---

### Alternative: `epubie-lib` (v0.1.1)
**Status:** ⚠️ New (June 2025), MIT licensed  
**Size:** Tiny (29KB, 498 lines)

**Why use it:**
- Made for "content by chapter" rather than TOC
- Very lightweight
- Good for embedded/constrained devices

**Best for:** Minimal footprint, simple EPUB reading

---

## 📝 Text Layout Libraries

### 🏆 RECOMMENDED: `cosmic-text` (v0.17.1)
**Status:** ✅ Very active, POP!_OS / System76 backed  
**License:** MIT/Apache-2.0  
**Dependencies:** Uses `harfrust`, `skrifa`, `fontdb`, `swash`

**Why use it:**
- Complete text stack: shaping, layout, rasterization
- Supports complex scripts (Arabic, CJK, emoji)
- Font fallback system
- Line breaking, bidirectional text
- Optional Swash integration for glyph rasterization
- No GPU required - software rendering
- `no_std` compatible with right features

**Example usage:**
```rust
use cosmic_text::{FontSystem, SwashCache, Buffer, Metrics, Attrs, Shaping};

// Create once per app
let mut font_system = FontSystem::new();
let mut swash_cache = SwashCache::new();

// Text metrics (font size, line height)
let metrics = Metrics::new(14.0, 20.0);
let mut buffer = Buffer::new(&mut font_system, metrics);

// Set text area size (480x700 for reading area)
buffer.set_size(Some(480.0), Some(700.0));

// Add text
let attrs = Attrs::new();
buffer.set_text(
    "Your chapter content here...",
    &attrs,
    Shaping::Advanced,
    None
);

// Layout
buffer.shape_until_scroll(true);

// Access layout for rendering
for run in buffer.layout_runs() {
    for glyph in run.glyphs.iter() {
        // glyph.x, glyph.y, glyph.font_id, glyph.glyph_id
        // Render to your e-ink display
    }
}

// Draw (or use SwashCache for bitmap glyphs)
buffer.draw(&mut swash_cache, text_color, |x, y, w, h, color| {
    // Your pixel drawing code here
});
```

**Key features:**
- Font discovery via `fontdb`
- Text shaping via `harfrust` (HarfBuzz port)
- Layout with wrapping, alignment
- Glyph rasterization via `swash` (optional)
- Syntax highlighting support (via syntect)
- Buffer/Editor abstractions

**Cons:**
- Larger dependency tree (~15 deps)
- May need feature-gating for `no_std`

---

### Alternative: `parley` (v0.7.0)
**Status:** ✅ Active, Linebender ecosystem  
**License:** Apache-2.0/MIT  
**Last release:** Nov 24, 2025

**Why use it:**
- Next-generation text layout (designed for Druid, Masonry, Vello)
- Uses same stack: Fontique, HarfRust, Skrifa, ICU4X
- Rich text support (spans, styles)
- Part of the Linebender ecosystem (actively developed)
- Better for complex UI applications

**Best for:** If you're building a more complex UI (not just reading)

**Cons:**
- Higher-level API - more complex
- MSRV 1.88 (newer Rust required)

---

## 🏗️ Architecture Recommendation for Xteink X4

### Tier 1: Start Here (MVP)

```toml
[dependencies]
# EPUB parsing - widely used, simple API
epub = "2.1.5"

# Text layout - complete stack
cosmic-text = { version = "0.17.1", default-features = false, features = ["swash"] }

# HTML stripping (for clean text from EPUB chapters)
html2text = "0.14.0"
# OR
scraper = "0.23.0"  # If you need to parse HTML structure
```

### Tier 2: Enhanced (After MVP)

```toml
# For font embedding (e-ink devices need specific fonts)
# Include fonts as binary blobs
# Use fontdb to load them

# For image support in books (EPUB images)
image = { version = "0.25.5", default-features = false, features = ["png", "jpeg"] }

# For better HTML parsing
lol_html = "2.2.0"  # Streaming HTML parser (Cloudflare)
# OR
tendril = "0.4.3"   # For html5ever (more memory)
```

---

## 🔧 Implementation Strategy

### Phase 1: Plain Text Reading (Week 1)

1. **Load EPUB:**
```rust
use epub::doc::EpubDoc;

pub struct Book {
    doc: EpubDoc,
    current_chapter: usize,
}

impl Book {
    pub fn open(path: &str) -> Result<Self, Box<dyn Error>> {
        let doc = EpubDoc::new(path)?;
        Ok(Self { doc, current_chapter: 0 })
    }
    
    pub fn current_text(&mut self) -> Option<String> {
        self.doc.get_current_str().ok()
    }
    
    pub fn next_chapter(&mut self) -> bool {
        self.doc.go_next()
    }
}
```

2. **Render with cosmic-text:**
```rust
use cosmic_text::{FontSystem, Buffer, Metrics, Attrs, Shaping, Wrap};

pub struct TextRenderer {
    font_system: FontSystem,
    buffer: Buffer,
}

impl TextRenderer {
    pub fn new() -> Self {
        let mut font_system = FontSystem::new();
        let metrics = Metrics::new(16.0, 24.0); // 16pt font, 24pt line height
        let buffer = Buffer::new(&mut font_system, metrics);
        
        Self { font_system, buffer }
    }
    
    pub fn set_text(&mut self, text: &str, width: f32, height: f32) {
        let mut buffer = self.buffer.borrow_with(&mut self.font_system);
        buffer.set_size(Some(width), Some(height));
        buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);
        buffer.set_wrap(Wrap::Word); // Word wrapping
        buffer.shape_until_scroll(true);
    }
    
    pub fn layout_lines(&self) -> impl Iterator<Item = cosmic_text::LayoutRun> {
        self.buffer.layout_runs()
    }
}
```

### Phase 2: HTML Content (Week 2-3)

EPUB chapters are XHTML. Strip to plain text first:

```rust
use html2text::from_read;

pub fn html_to_text(html: &str) -> String {
    from_read(html.as_bytes(), 80) // 80 char width
}

// Or use scraper for structured parsing:
use scraper::{Html, Selector};

pub fn extract_paragraphs(html: &str) -> Vec<String> {
    let document = Html::parse_fragment(html);
    let selector = Selector::parse("p").unwrap();
    
    document.select(&selector)
        .map(|p| p.text().collect::<String>())
        .collect()
}
```

### Phase 3: Pagination (Week 3-4)

E-ink requires page-based navigation, not scrolling:

```rust
pub struct Paginator {
    text: String,
    page_size: usize, // Characters per page (approximate)
    pages: Vec<String>,
}

impl Paginator {
    pub fn paginate(&mut self, text: String, chars_per_page: usize) {
        // Simple approach: split by character count
        // Better: use cosmic-text to measure actual rendered lines
        self.pages = text.chunks(chars_per_page)
            .map(|chunk| chunk.to_string())
            .collect();
    }
    
    pub fn page(&self, n: usize) -> Option<&str> {
        self.pages.get(n).map(|s| s.as_str())
    }
}
```

**Better approach with cosmic-text:**
```rust
pub fn paginate_buffer(buffer: &Buffer, lines_per_page: usize) -> Vec<Vec<cosmic_text::LayoutRun>> {
    let all_lines: Vec<_> = buffer.layout_runs().collect();
    all_lines.chunks(lines_per_page).map(|c| c.to_vec()).collect()
}
```

---

## 📊 Library Comparison Matrix

| Library | License | Size | Activity | Pros | Cons |
|---------|---------|------|----------|------|------|
| **epub** | GPL-3.0 | 1MB | Very High | Mature, simple API | Copyleft license |
| **rbook** | Apache-2.0 | <1MB | High | Permissive, well documented | Newer, smaller community |
| **cosmic-text** | MIT/Apache | 2-3MB | Very High | Complete text stack | Many dependencies |
| **parley** | MIT/Apache | 2MB | High | Rich text, Linebender | Complex API |
| **epubie-lib** | MIT | 29KB | Medium | Tiny, minimal | Very new |

---

## 🎯 Recommended Stack for Xteink X4

### Minimal Viable Reader:
```toml
[dependencies]
# EPUB reading
epub = "2.1.5"

# Text layout (disable default features for no_std)
cosmic-text = { version = "0.17.1", default-features = false, features = ["swash"] }

# HTML to text conversion
html2text = "0.14"

# Embedded font (choose ONE good font)
# Include as binary: include_bytes!("fonts/NotoSerif-Regular.ttf")
```

### Memory Considerations:
- **EPUB parsing:** Low memory (streaming ZIP)
- **Chapter text:** ~50-100KB per chapter (load one at a time)
- **Font database:** ~20-30KB for embedded fonts
- **cosmic-text buffer:** ~10-20KB for layout state
- **Total:** <200KB for reading subsystem

### License Strategy:
If GPL-3.0 is problematic, use **rbook** instead of **epub**:
```toml
[dependencies]
rbook = "0.6.10"  # Apache-2.0 (permissive)
```

---

## 🚀 Next Steps

1. **Create book loader module** using `epub` crate
2. **Set up cosmic-text** with embedded fonts (Noto Serif or similar)
3. **Implement page rendering** using `embedded-graphics` DrawTarget
4. **Add pagination logic** (measure text, split into screen-sized chunks)
5. **Test on device** with real EPUB files

See `docs/ui-creative-complete.md` for UI mockups to integrate with these libraries.
