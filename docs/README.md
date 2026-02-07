# ox4 Documentation

Documentation for **ox4** - Rust-powered e-reader firmware for the Xteink X4 (ESP32-C3 based e-ink reader).

---

## Quick Links

### Core Documentation
- **[PLAN.md](./PLAN.md)** - Master project plan with hardware specs and critical path
- **[GLOSSARY.md](./GLOSSARY.md)** - Terms and definitions for embedded development

### By Category

#### 📖 EPUB Implementation
- **[implementation-status.md](./epub/implementation-status.md)** - Current status and testing guide
- **[architecture-plan.md](./epub/architecture-plan.md)** - Streaming EPUB implementation plan
- **[library-research.md](./epub/library-research.md)** - Rust library research for EPUB/typography

#### 🎨 UI/UX Design
- **[paradigms.md](./ui/paradigms.md)** - UI interaction patterns for e-ink + 6 buttons
- **[mockups.md](./ui/mockups.md)** - Complete UI mockups (5 paradigms)
- **[creative-concepts.md](./ui/creative-concepts.md)** - Creative UI concepts

#### 🔧 Hardware
- **[ssd1677-code-review.md](./hardware/ssd1677-code-review.md)** - Display driver code review

#### ✨ Features
- **[wasm-simulator.md](./features/wasm-simulator.md)** - Browser-based UI simulator
- **[future-ideas.md](./features/future-ideas.md)** - Future feature ideas

---

## Documentation Guide

### New to the project?
1. Start with **[PLAN.md](./PLAN.md)** for hardware specs and project phases
2. Read **[epub/architecture-plan.md](./epub/architecture-plan.md)** to understand EPUB implementation approach
3. Check **[GLOSSARY.md](./GLOSSARY.md)** for unfamiliar terms

### Working on EPUB support?
1. **[epub/implementation-status.md](./epub/implementation-status.md)** - Current status and testing
2. **[epub/architecture-plan.md](./epub/architecture-plan.md)** - Implementation details
3. **[epub/library-research.md](./epub/library-research.md)** - Library choices

### Designing UI features?
1. **[ui/paradigms.md](./ui/paradigms.md)** - Understand the 5 UI paradigms
2. **[ui/mockups.md](./ui/mockups.md)** - See complete screen flows
3. **[features/wasm-simulator.md](./features/wasm-simulator.md)** - Test in browser

---

## Key Decisions Documented

| Decision | Document | Status |
|----------|----------|--------|
| EPUB Architecture | [epub/architecture-plan.md](./epub/architecture-plan.md) | ✅ **Streaming Reflow** implemented |
| Library Stack | [epub/library-research.md](./epub/library-research.md) | ✅ `quick-xml` + `rc-zip` + `fontdue` |
| Implementation Status | [epub/implementation-status.md](./epub/implementation-status.md) | ✅ Complete, ready for testing |
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

**Key Constraint:** Streaming architecture keeps memory usage under 100KB (achieved: ~60KB).

---

## Current Status

**Phase 1: Display Driver** - ✅ Complete (SSD1677 working)  
**Phase 2: Button Input** - ✅ Complete  
**Phase 3: SD Card** - ✅ Complete (FATFS + CLI)  
**Phase 4: EPUB Reader** - ✅ Complete (Ready for device testing)

---

## Directory Structure

```
docs/
├── README.md              # This file
├── PLAN.md                # Master project plan
├── GLOSSARY.md            # Terms and definitions
│
├── epub/                  # EPUB implementation docs
│   ├── implementation-status.md
│   ├── architecture-plan.md
│   └── library-research.md
│
├── ui/                    # UI/UX design docs
│   ├── paradigms.md
│   ├── mockups.md
│   └── creative-concepts.md
│
├── hardware/              # Hardware-specific docs
│   └── ssd1677-code-review.md
│
└── features/              # Feature documentation
    ├── wasm-simulator.md
    └── future-ideas.md
```

---

*Last Updated: 2026-02-07*
