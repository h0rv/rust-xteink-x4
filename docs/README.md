# Xteink X4 Documentation

Documentation for the Xteink X4 e-ink reader firmware - an ESP32-C3 based e-reader written in Rust.

---

## Quick Links

### For Developers
- **[PLAN.md](./PLAN.md)** - Master project plan with hardware specs and critical path
- **[GLOSSARY.md](./GLOSSARY.md)** - Terms and definitions
- **[FUTURE_FEATURES.md](./FUTURE_FEATURES.md)** - Ideas for future development

### EPUB Implementation
- **[EPUB_ARCHITECTURE_COMPARISON.md](./EPUB_ARCHITECTURE_COMPARISON.md)** - Compare all 6 possible approaches with tradeoffs
- **[epub-plan-revised-2026-02-03.md](./epub-plan-revised-2026-02-03.md)** - Detailed streaming EPUB implementation plan
- **[epub-library-scan-2026-02-03.md](./epub-library-scan-2026-02-03.md)** - Rust library research for EPUB/typography

### UI/UX Design
- **[ui-paradigms.md](./ui-paradigms.md)** - UI interaction patterns
- **[ui-mockups-complete.md](./ui-mockups-complete.md)** - Complete UI mockups
- **[ui-creative-complete.md](./ui-creative-complete.md)** - Creative UI concepts

### Hardware
- **[ssd1677-code-review.md](./ssd1677-code-review.md)** - Display driver review
- **[ebook-libraries-2026.md](./ebook-libraries-2026.md)** - E-ink library ecosystem review

---

## Documentation Guide

### New to the project?
1. Start with **[PLAN.md](./PLAN.md)** for hardware specs and project phases
2. Read **[EPUB_ARCHITECTURE_COMPARISON.md](./EPUB_ARCHITECTURE_COMPARISON.md)** to understand EPUB implementation options
3. Check **[GLOSSARY.md](./GLOSSARY.md)** for unfamiliar terms

### Working on EPUB support?
2. **[epub-plan-revised-2026-02-03.md](./epub-plan-revised-2026-02-03.md)** - Implementation details
3. **[epub-library-scan-2026-02-03.md](./epub-library-scan-2026-02-03.md)** - Library choices

---

## Key Decisions Documented

| Decision | Document | Status |
|----------|----------|--------|
| EPUB Architecture | [EPUB_ARCHITECTURE_COMPARISON.md](./EPUB_ARCHITECTURE_COMPARISON.md) | ✅ **Approach 2: Streaming Reflow** recommended |
| Library Stack | [epub-library-scan-2026-02-03.md](./epub-library-scan-2026-02-03.md) | ✅ `quick-xml` + `zip` + `fontdue` |
| Implementation Phases | [epub-plan-revised-2026-02-03.md](./epub-plan-revised-2026-02-03.md) | ✅ 6 phases defined |
| Hardware Specs | [PLAN.md](./PLAN.md) | ✅ ESP32-C3, 480x800, SSD1677 |

---

## Memory Constraints Summary

```
ESP32-C3 Available Resources:
├── Total RAM:                400KB
├── Application heap:         ~300KB
├── Safe EPUB budget:         ~100KB
├── Display buffer:            48KB
└── OOM threshold:           >150KB

Target: Peak usage <100KB for EPUB operations
```

**Key Constraint:** Current implementation crashes at ~165KB (OOM). New architecture must stay under 100KB.

---

## Current Status

**Phase 1: Display Driver** - ✅ Complete (SSD1677 working)  
**Phase 2: Button Input** - ✅ Complete  
**Phase 3: SD Card** - ✅ Complete (FATFS + CLI)  
**Phase 4: EPUB Reader** - 🔄 In Progress (Architecture selected, implementation starting)

---

*Last Updated: 2026-02-03*
