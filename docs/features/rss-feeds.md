# RSS Feed Loading and Offline Reading

Plan for adding RSS/Atom feed support with SD-card caching on Xteink X4.

## Why

- **Fresh content on e-ink** - Read news and blogs in the same interface as books.
- **Offline-first** - Sync once over WiFi, read later without connectivity.
- **Fits device model** - Incremental downloads and plain-text article views work well on constrained hardware.

## Constraints

- **MCU/RAM limits** - ESP32-C3 with tight RAM budget; avoid full-DOM parsers and large in-memory lists.
- **Storage is available** - SD card is the primary cache and content store.
- **Display/update model** - E-ink favors paginated, text-first article rendering.
- **Input model** - 6 navigation buttons + power, no touchscreen.

## Scope (v1)

- Add/remove RSS or Atom feed URLs.
- Manual sync from a menu action (no background daemon required for v1).
- Parse feed metadata and entries.
- Download and cache article content to SD card.
- Read cached articles offline in a text-focused reader.
- Mark read/unread and star/unstar.

Non-goals (v1):
- Rich web rendering (full CSS/JS).
- Podcasts/audio enclosure playback.
- Account sync across devices.

## High-Level Architecture

```
WiFi HTTP Client
    -> Feed Fetcher (ETag/Last-Modified)
    -> Feed Parser (streaming XML)
    -> Entry Queue
    -> Article Fetch + Extract (readable text)
    -> SD Cache Store
    -> UI Library/Reader
```

## SD Card Layout

Use a hidden app directory to avoid cluttering user files.

```
/.xteink/rss/
  feeds.json                 # list of subscribed feeds + sync metadata
  index.bin                  # compact entry index (global)
  feeds/
    <feed_id>.json           # feed metadata + per-feed cursors
  entries/
    <entry_id>.txt           # extracted article text (UTF-8)
    <entry_id>.meta.json     # title, author, date, source URL, flags
  images/
    <entry_id>_<n>.bmp       # optional v2 thumbnails (not required in v1)
```

## Data Model

Suggested minimal structs:

```rust
pub struct FeedConfig {
    pub id: u64,
    pub title: String,
    pub url: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub last_sync_unix: u64,
    pub enabled: bool,
}

pub struct EntrySummary {
    pub id: u64,
    pub feed_id: u64,
    pub title: String,
    pub link: String,
    pub published_unix: u64,
    pub read: bool,
    pub starred: bool,
    pub cached: bool,
}
```

Implementation notes:
- Use stable `u64` IDs from a hash of normalized URL (+ guid for entries).
- Keep index compact and append-friendly.
- Keep article bodies in separate files for lazy loading.

## Network and Sync Behavior

Per feed sync flow:

1. `GET <feed_url>` with `If-None-Match` and `If-Modified-Since`.
2. On `304 Not Modified`, update sync timestamp and stop.
3. On `200 OK`, stream-parse items/entries and upsert index.
4. For new entries, fetch article URLs with limits:
   - max articles per sync (example: 10)
   - max bytes per article (example: 128 KB response cap)
   - timeout per request
5. Extract readable text, store `entries/<id>.txt`, set `cached=true`.
6. Run retention cleanup.

## Parsing Strategy

- Feed parse: streaming XML parser (`quick-xml` style) to avoid heap spikes.
- Content extraction: simple HTML-to-text pipeline:
  - strip scripts/styles/nav/footer blocks heuristically
  - keep heading/paragraph/list/basic emphasis markers
  - decode HTML entities
  - normalize whitespace for pagination
- Fallback: if extraction fails, store link + summary and mark as "web-only".

## Cache and Retention Policy

Default policy (user-configurable later):
- Keep newest **500** entries in index.
- Keep cached bodies for newest **200** entries.
- Evict oldest unread only after hard cap is reached (prefer evicting read entries first).
- Per-feed backoff on repeated failures (e.g., 1m, 5m, 30m).

Cleanup pass runs after each sync:
- remove orphaned article/meta files
- rewrite compacted index when tombstones exceed threshold

## UI Flow

Menu additions:
- `RSS` in main menu.

RSS screens:
1. **Feeds List**
   - feed title, unread count, last sync age
   - actions: open feed, sync now, add feed, remove feed, toggle enable
2. **Feed Entries**
   - reverse-chronological entries
   - badges: unread/read, cached
   - actions: open, mark read/unread, star
3. **Article Reader**
   - same paginator model as EPUB/TXT where possible
   - header shows source + published date
   - action: open original URL info (for later web handoff)

Button mapping should mirror existing reader conventions to reduce cognitive load.

## Error Handling

Show clear per-feed errors:
- DNS/connect timeout
- HTTP error status
- invalid feed format
- article fetch blocked/unsupported
- SD write failure

Design principles:
- Never block boot because of RSS state corruption.
- If index is damaged, recover by rebuilding from feed configs and entry metadata files.

## Implementation Plan

## Phase 1: Storage + Models

- Add `rss` module in firmware for SD-backed store.
- Implement feed list persistence and entry index read/write.
- Add CLI/debug commands for listing feeds and entries.

Done when:
- Can add/remove feeds and persist across reboot.

## Phase 2: Fetch + Parse

- Add HTTP fetcher for feed URLs.
- Implement conditional requests (ETag/Last-Modified).
- Parse RSS/Atom entries and update local index.

Done when:
- Manual sync updates entry lists without article bodies.

## Phase 3: Article Cache

- Fetch entry links with bounded concurrency (start with sequential).
- Convert HTML to readable text and cache to `entries/*.txt`.
- Implement retention cleanup.

Done when:
- Can open cached articles offline.

## Phase 4: UI Integration

- Add RSS activities/screens in `xteink-ui`.
- Wire firmware backend to UI actions (sync, open feed, open article).
- Add status/progress indicators during sync.

Done when:
- End-to-end flow works on device: subscribe -> sync -> open offline article.

## Testing Strategy

Desktop-first:
- Add simulator scenario with mock feed JSON/XML payloads.
- Snapshot tests for parser outputs and text extraction normalization.
- Failure-case tests: malformed XML, large entries, missing dates, duplicate GUIDs.

On device:
- Measure sync heap usage and max latency.
- Validate SD growth and retention behavior over repeated sync cycles.

## Candidate Dependencies

Firmware/backend:
- `quick-xml` (streaming feed parse)
- `ureq` or ESP-IDF HTTP client wrapper already available through `esp-idf-svc`
- optional small HTML-to-text helper (or custom extractor to stay lean)

Prefer existing workspace dependencies first.

## Open Questions

- Should RSS sync run only on demand in v1, or also on wake when WiFi is connected?
- Do we need OPML import/export in v1.1 for easier feed onboarding?
- Should starred items be exempt from retention eviction by default?
