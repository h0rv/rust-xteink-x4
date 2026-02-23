# OPDS & Feed Browser

Browse and download ebooks from online catalogs, and read articles from RSS feeds directly on the device.

## Overview

Support for:
- **OPDS catalogs** - Browse and download ebooks wirelessly (Project Gutenberg, Standard Ebooks, etc.)
- **RSS/Atom feeds** - Read articles from news sources (Hacker News, Longform, etc.)
- **Jina.ai Reader** - Extract clean article content from web pages

## Features

- Preloaded OPDS catalogs (Project Gutenberg, Standard Ebooks, Feedbooks)
- Preloaded RSS feeds (Hacker News, Longform)
- Browse categories and book listings
- View book/article details (title, author, description)
- Download EPUBs directly to device
- Read articles with content extracted via Jina.ai Reader

## Architecture

### Components

```
┌─────────────────────────────────────────────┐
│           FeedBrowserActivity (UI)          │
│   - Source list, catalog browsing, download │
├─────────────────────────────────────────────┤
│           FeedService (firmware)            │
│   - HTTP fetch, parsing, download manager   │
├─────────────────────────────────────────────┤
│           feed-rs crate                     │
│   - RSS/Atom/OPDS parsing                   │
└─────────────────────────────────────────────┘
```

### Feed Types

```rust
pub enum FeedType {
    Opds,  // Downloadable ebooks
    Rss,   // Articles to read
}
```

### Content Extraction (RSS)

For RSS articles, use Jina.ai Reader to extract clean text:

```rust
// https://r.jina.ai/{article_url} returns clean markdown/text
pub fn get_reader_url(article_url: &str) -> String {
    format!("https://r.jina.ai/{}", article_url)
}
```

### Data Flow

```
User selects source
       ↓
HTTP GET feed URL
       ↓
Parse OPDS/RSS (feed-rs)
       ↓
Display entries in list
       ↓
User selects entry
       ↓
┌─────────────────────────────────┐
│ OPDS: Show detail + Download    │
│ RSS:  Fetch article via Jina.ai │
│       → Display in text viewer  │
└─────────────────────────────────┘
```

## Implementation

### Phase 1: Core Infrastructure ✓

**1.1 Dependencies**

```toml
# crates/xteink-firmware/Cargo.toml
[dependencies]
feed-rs = "2.3"
```

**1.2 Feed Data Structures**

```rust
// crates/xteink-ui/src/feed.rs

pub enum FeedType {
    Opds,
    Rss,
}

pub struct FeedSource {
    pub name: String,
    pub url: String,
    pub feed_type: FeedType,
}

pub struct OpdsCatalog {
    pub title: String,
    pub entries: Vec<OpdsEntry>,
}

pub struct OpdsEntry {
    pub title: String,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub download_url: Option<String>,
    pub format: Option<String>,
}

pub const JINA_READER_BASE: &str = "https://r.jina.ai/";

pub fn get_reader_url(article_url: &str) -> String {
    format!("{}{}", JINA_READER_BASE, article_url)
}
```

### Phase 2: UI ✓

**2.1 Feed Browser Activity**

```rust
pub struct FeedBrowserActivity {
    sources: Vec<(&'static str, &'static str, FeedType)>,
    current_catalog: Option<OpdsCatalog>,
    state: BrowserState,
}

enum BrowserState {
    SourceList,
    Loading,
    BrowsingCatalog,
    BookDetail,
    Downloading(f32),
    Error(String),
}
```

**2.2 Navigation**

| State | Up/Down | Confirm | Back |
|-------|---------|---------|------|
| SourceList | Select source | Open catalog/feed | Exit |
| Loading | — | — | Cancel |
| BrowsingCatalog | Select entry | Show detail | Source list |
| BookDetail | — | Download/Read | Catalog |
| Downloading | — | Cancel | (blocked) |

### Phase 3: Preloaded Sources ✓

**OPDS Catalogs (Books)**

| Source | URL | Content |
|--------|-----|---------|
| Project Gutenberg | `https://m.gutenberg.org/ebooks.opds/` | 70K+ public domain ebooks |
| Standard Ebooks | `https://standardebooks.org/feeds/opds` | Curated high-quality public domain |
| Feedbooks | `https://catalog.feedbooks.com/catalog/public_domain.atom` | Classics |

**RSS Feeds (Articles)**

| Source | URL | Content |
|--------|-----|---------|
| Hacker News | `https://news.ycombinator.com/rss` | Tech news |
| Hacker News (Front Page) | `https://hnrss.org/frontpage` | Curated HN |
| Longform | `https://longform.org/rss/` | Long-form journalism |

### Phase 4: Menu Integration (Pending)

Add to SystemMenuActivity:

```rust
enum MenuItem {
    Library,
    Files,
    OnlineCatalogs,  // New!
    Settings,
}
```

## Jina.ai Reader Integration

[Jina.ai Reader](https://jina.ai/reader/) provides free content extraction:

- Input: Any article URL
- Output: Clean markdown/text
- Usage: `https://r.jina.ai/{article_url}`

Example:
```rust
let article_url = "https://example.com/article";
let reader_url = get_reader_url(article_url);
// reader_url = "https://r.jina.ai/https://example.com/article"
let clean_text = http_get(&reader_url)?;
// Display in TextViewer
```

Benefits:
- Removes ads, navigation, sidebars
- Extracts main article content
- Returns clean, readable text
- Free to use (no API key required)

## File Structure

```
crates/xteink-ui/src/
├── feed.rs                     # Data structures, FeedType, Jina.ai
├── feed_browser_activity.rs    # UI for browsing
└── lib.rs                      # Exports

crates/xteink-firmware/src/
├── feed_service.rs             # HTTP + parsing
├── main.rs                     # Integration
└── Cargo.toml                  # feed-rs dependency
```

## Status

| Component | Status |
|-----------|--------|
| feed-rs dependency | ✓ Done |
| Feed data structures | ✓ Done |
| FeedType enum | ✓ Done |
| Jina.ai Reader integration | ✓ Done |
| FeedBrowserActivity | ✓ Done |
| FeedService (firmware) | ✓ Done |
| Unit tests | ✓ Done (12 tests) |
| Menu integration | Pending |
| Article viewer | Pending |
| Integration tests | Pending |

## Future Enhancements

### v2 Features
- Custom sources (add via web UI)
- Cover image thumbnails
- Authentication (Calibre, private libraries)
- OPDS search
- More RSS sources

### v3 Features  
- Source management (edit, delete, reorder)
- Recent downloads list
- Download queue (multiple books)
- Background downloading
- Article offline caching

## Open Questions

1. **Cover images**: Download and display, or skip for v1?
   
2. **Download location**: `/sd/downloads/` or `/sd/books/`?
   
3. **Article cache**: Cache extracted articles to SD for offline reading?

4. **Rate limiting**: Jina.ai may have rate limits - implement local caching?
