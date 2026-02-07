# EPUB Specification Compliance Analysis

## Official EPUB Specifications

### Version History

| Version | Date | Status | Organization | Notes |
|---------|------|--------|--------------|-------|
| **EPUB 2.0** | 2007 | Deprecated | IDPF | Legacy, widely supported |
| **EPUB 3.0** | 2011 | Superseded | IDPF | Major update, HTML5 based |
| **EPUB 3.1** | 2017 | Superseded | IDPF → W3C | Controversial, not widely adopted |
| **EPUB 3.2** | 2019 | Superseded | W3C | Simplified 3.1 |
| **EPUB 3.3** | 2026-01-13 | **Current** | W3C | Latest recommendation |

### Official Sources
- **Current Spec:** https://www.w3.org/TR/epub-33/
- **GitHub:** https://github.com/w3c/epub-specs
- **Test Suite:** https://w3c.github.io/epub-tests/
- **Reading Systems:** https://www.w3.org/TR/epub-rs-33/
- **Accessibility:** https://www.w3.org/TR/epub-a11y-11/

---

## EPUB Structure Overview

### Required Components

#### 1. **OCF Container (ZIP)**
- MIME type file at root: `application/epub+zip`
- META-INF directory with `container.xml`
- Root directory with package document

#### 2. **Package Document (OPF)**
- XML file defining the publication
- Metadata (Dublin Core + EPUB-specific)
- Manifest (list of all resources)
- Spine (reading order)
- Guide (optional navigation hints - deprecated in 3.0+)

#### 3. **Content Documents**
- XHTML (primary) - HTML with XML syntax
- SVG (optional) - For graphics/fixed layout
- Media files (images, audio, video)

#### 4. **Navigation Document**
- XHTML with `epub:type="toc"` nav element
- Replaces NCX from EPUB 2.0
- Human and machine readable

#### 5. **Optional Components**
- Media Overlays (synchronized audio)
- Fonts (embedded)
- CSS stylesheets
- JavaScript
- Metadata (additional)

---

## ox4 Current Compliance

### ✅ What We Support (EPUB 3.3 Core)

#### **OCF Container**
- ✅ ZIP-based container format
- ✅ Streaming ZIP reader (4KB buffer)
- ✅ `container.xml` parsing
- ❓ MIME type file validation (not checked yet)
- ❓ META-INF directory structure

#### **Package Document (OPF)**
- ✅ XML parsing with `quick-xml` (SAX-style)
- ✅ Metadata extraction (title, author, language)
- ✅ Manifest parsing (resource list)
- ✅ Spine parsing (reading order)
- ❌ Full Dublin Core metadata support
- ❌ EPUB-specific metadata (cover, layout, etc.)
- ❌ Media overlay references
- ❌ Guide element (deprecated anyway)

#### **Content Documents**
- ✅ XHTML tokenization (SAX-style, no DOM)
- ✅ Basic HTML structure (p, h1-h6, em, strong)
- ✅ Text content extraction
- ⚠️ Limited CSS support (basic styling only)
- ❌ SVG content documents
- ❌ MathML
- ❌ Embedded fonts
- ❌ JavaScript/forms
- ❌ Images
- ❌ Audio/video
- ❌ Tables
- ❌ Lists (nested)
- ❌ Links (hyperlinks work but not tested)

#### **Navigation Document**
- ❌ Not implemented yet
- ✅ Can parse TOC from OPF spine (fallback)
- ❌ NCX navigation (EPUB 2.0 legacy)

#### **Layout Engine**
- ✅ Greedy line breaking
- ✅ Multi-page pagination
- ✅ Basic text styles (normal, bold, italic)
- ✅ Chapter-based reading
- ❌ Fixed layouts
- ❌ Spreads (two-page view)
- ❌ Complex typography
- ❌ Bidirectional text (RTL languages)
- ❌ Ruby annotations (East Asian)

---

## EPUB Version Targeting

### Recommended: **EPUB 3.2 Subset**

**Rationale:**
1. **EPUB 3.3 is too new** (published Jan 2026) - minimal adoption
2. **EPUB 3.2 is stable** (2019) - wide reading system support
3. **EPUB 2.0 is legacy** - lacks modern features, but still widely supported

**Target subset:**
- EPUB 3.2 core structure
- Basic XHTML content documents
- Simple metadata (Dublin Core minimum)
- Linear spine (no page-list, no landmarks yet)
- Skip: media overlays, scripting, fixed layouts, SVG

### Fallback Support: EPUB 2.0 Core

For maximum compatibility, we should parse:
- NCX (EPUB 2.0 navigation) as fallback
- Guide element (deprecated but still used)
- EPUB 2.0 OPF structure

---

## Compliance Gaps & Priority

### Critical (Must Fix)

**For Basic EPUB 3.2 Compliance:**

1. **Container validation** (Easy - 1 day)
   - Verify `mimetype` file exists
   - Check META-INF structure
   - Validate container.xml

2. **Navigation document** (Medium - 2-3 days)
   - Parse XHTML nav with `epub:type="toc"`
   - Extract TOC structure
   - Fallback to NCX for EPUB 2.0

3. **Metadata completeness** (Easy - 1 day)
   - Full Dublin Core (date, publisher, rights, etc.)
   - EPUB metadata (modified date, identifier scheme)
   - Cover image detection

### Important (Should Have)

**For Better User Experience:**

1. **Images** (Medium - 2-3 days)
   - JPEG/PNG rendering
   - Size/position handling
   - Memory-efficient loading

2. **Better CSS** (Medium - 3-4 days)
   - Font-family, font-size, color
   - Text alignment, indentation
   - Basic box model (margins, padding)

3. **Links** (Easy - 1 day)
   - Internal chapter links
   - Footnote navigation
   - External URL handling (optional)

4. **Lists** (Easy - 1 day)
   - Ordered/unordered lists
   - Nested list support
   - List markers

### Nice to Have (Future)

**For Advanced Features:**

1. **Fixed layouts** (Hard - 1+ week)
2. **Media overlays** (Hard - 1+ week)
3. **SVG content documents** (Medium - 3-5 days)
4. **Embedded fonts** (Medium - 3-5 days)
5. **MathML** (Hard - 2+ weeks)
6. **Scripting** (Very Hard - ongoing)
7. **Forms** (Medium - 1 week)

---

## Validation & Testing

### Official Tools

**EPUBCheck** (https://github.com/w3c/epubcheck)
- Official EPUB validator
- Current version: 5.1.0 (supports EPUB 3.3)
- Command line and library available
- We should run our EPUBs through this

**EPUB Test Suite** (https://w3c.github.io/epub-tests/)
- W3C official test suite
- Covers EPUB 3.2 and 3.3
- Browser-based testing

### Test Corpus

Recommended test books:
1. **Moby Dick** (Project Gutenberg) - Simple text
2. **Alice in Wonderland** (with images)
3. **Technical manual** (with tables, lists)
4. **Foreign language** (test UTF-8, RTL)
5. **W3C test EPUBs** (from official suite)

---

## Recommended Compliance Path

### Phase 1: EPUB 2.0 Core (Current ✅)
**Status:** ~60% complete
- ✅ Container structure
- ✅ OPF parsing (basic)
- ✅ Spine/manifest
- ✅ XHTML tokenization
- ✅ Basic layout

### Phase 2: EPUB 3.2 Minimum (Next)
**Goal:** Pass EPUBCheck for simple books
**Timeline:** 1-2 weeks

1. Container validation
2. Navigation document
3. Complete metadata
4. Images
5. Lists
6. Better CSS

### Phase 3: EPUB 3.2 Standard (Future)
**Goal:** Handle 80% of real-world EPUBs
**Timeline:** 1-2 months

1. Fonts
2. Fixed layouts
3. Tables
4. SVG
5. Better typography

### Phase 4: EPUB 3.3 Features (Long-term)
**Goal:** Cutting-edge features
**Timeline:** Ongoing

1. Media overlays
2. Scripting
3. Advanced CSS
4. MathML
5. Accessibility features

---

## Specification Documents to Read

### Essential Reading

1. **EPUB 3.3 Core** (https://www.w3.org/TR/epub-33/)
   - Sections 3-7: Container, Package, Content, Navigation
   - ~100 pages, but detailed

2. **EPUB 3 Reading Systems** (https://www.w3.org/TR/epub-rs-33/)
   - Understand how readers should behave
   - Helps design our renderer

3. **EPUB Accessibility 1.1** (https://www.w3.org/TR/epub-a11y-11/)
   - Important for inclusive design
   - Semantic HTML, ARIA, alt text

### Reference Documents

1. **OCF (Open Container Format)**
   - https://www.w3.org/TR/epub-33/#sec-ocf
   - ZIP structure, container.xml

2. **OPF (Open Packaging Format)**
   - https://www.w3.org/TR/epub-33/#sec-package-doc
   - Metadata, manifest, spine

3. **XHTML Content Documents**
   - https://www.w3.org/TR/epub-33/#sec-xhtml
   - HTML subset, extensions

4. **EPUBCheck Source**
   - https://github.com/w3c/epubcheck
   - Learn validation rules

---

## Action Items

### Immediate (Before v1.0)

1. [ ] Download and study EPUB 3.3 spec (Sections 3-7)
2. [ ] Run test EPUBs through our parser
3. [ ] Identify which EPUB version we support (2.0 vs 3.2)
4. [ ] Document compliance level in README
5. [ ] Add EPUBCheck validation to CI (if releasing standalone crate)

### Short-term (v1.0 → v1.1)

1. [ ] Implement navigation document parsing
2. [ ] Add container validation
3. [ ] Complete metadata support
4. [ ] Add images
5. [ ] Test with W3C test suite

### Long-term (v2.0+)

1. [ ] Full EPUB 3.2 compliance
2. [ ] Begin EPUB 3.3 features
3. [ ] Accessibility compliance
4. [ ] EPUBCheck validation passing

---

## Compliance Statement (Current)

**ox4 EPUB Parser:**
- **Target Spec:** EPUB 3.2 (subset)
- **Compliance Level:** ~40% (basic reading only)
- **Version Support:** EPUB 2.0 spine + EPUB 3.2 structure
- **Known Limitations:**
  - No navigation document parsing
  - No images, audio, video
  - Limited CSS (text styling only)
  - No fixed layouts
  - No media overlays
  - No scripting
  - ASCII/UTF-8 text only (no complex scripts)

**Best suited for:**
- Plain text fiction/non-fiction
- Simple formatting books
- Memory-constrained devices
- Educational/DIY projects

**Not suitable for:**
- Technical books (tables, code)
- Children's books (images)
- Textbooks (complex layouts)
- Magazines (fixed layouts)
- Interactive content

---

*Last Updated: 2026-02-07*
*Based on: EPUB 3.3 (W3C Recommendation, 2026-01-13)*
