# Embedded EPUB Memory Anti-Patterns

Date: 2026-03-06

Scope:
- `einked/`
- `epub-stream/`
- `xteink-firmware/`
- lessons from recent device crashes, host harness profiling, and CrossPoint comparison

## Purpose

This document captures the main memory anti-patterns we have been finding in the EPUB stack and the corresponding design direction that avoids them.

The goal is not "zero allocation". The goal is:
- stable heap shape
- bounded transient allocations
- minimal retained session state
- no large late allocations in hot paths
- no repeated chapter re-materialization just to turn a page

## Core Rule

On embedded devices, memory bugs are usually not "we used too much total RAM".

They are usually one of:
- one large contiguous allocation at the wrong time
- repeated medium allocations that fragment the heap
- stack growth in a deep path that looks harmless on host
- caching that moves data to disk but still buffers the whole thing in RAM first

## Anti-Patterns

### 1. Whole-chapter materialization in the hot path

Pattern:
- decompress or read the full chapter into one `Vec<u8>`
- require `capacity >= uncompressed chapter size`
- then parse/layout from that monolithic buffer

Why it is bad:
- requires one large contiguous heap block
- fails late on fragmented heaps
- makes first-open and cache-miss behavior fragile

Examples seen:
- `chapter_events_with_scratch(...)` requiring a full chapter buffer
- first-open chapter prepare paths that still materialize chapter HTML broadly

Preferred direction:
- stream chapter bytes
- or stage them to temp storage and consume incrementally
- keep caller-owned scratch, but do not require full-chapter contiguous capacity

### 2. "SD-backed cache" that still buffers the whole artifact before writing

Pattern:
- claim to have a disk-backed artifact/cache
- but retain `Vec<RenderPage>` or `Vec<Vec<u8>>` for the whole chapter before writing the file

Why it is bad:
- persistence exists, but runtime memory shape is still chapter-wide
- first-build path remains spike-heavy
- gives a false sense that the caching refactor solved the real problem

Examples seen:
- artifact writer retaining all pages in memory before final write
- payload serialization staged as a full in-memory collection

Preferred direction:
- stream page payloads directly to a temp file
- keep only a small LUT / offsets vector in RAM
- finalize by writing header + LUT + streaming payload bytes into final artifact

### 3. Page retrieval by rerender instead of artifact lookup

Pattern:
- page miss causes chapter reload + re-layout + page extraction
- page windows are treated as the primary navigation model

Why it is bad:
- page turns stay render-centric
- small in-memory windows still trigger expensive churn
- hot path remains CPU and allocator heavy even if retained state is small

Preferred direction:
- chapter artifact on SD
- page count from artifact header
- page lookup by LUT + seek
- page window only as a UI convenience, not the source of truth

### 4. App-layer ownership of cache and resource policy

Pattern:
- UI/activity layer decides:
  - temp directory policy
  - worker/thread policy
  - cache identity
  - buffer sizing
  - artifact strategy

Why it is bad:
- policy leaks across layers
- memory reasoning becomes impossible
- runtime changes require touching UI code

Preferred direction:
- one EPUB session/runtime boundary owns:
  - open policy
  - temp storage
  - artifact lifecycle
  - navigation strategy
  - fallback policy

### 5. Reopening files and rebuilding metadata on every small lookup

Pattern:
- page-count lookup reopens artifact file
- page lookup reopens artifact file again
- LUT is reread repeatedly

Why it is bad:
- extra file I/O churn
- avoidable transient buffers
- hides steady-state inefficiency behind "small operations"

Preferred direction:
- keep lookup paths minimal
- read only the needed header/LUT once when possible
- make page access truly page-granular, not "mini re-open" by habit

### 6. JSON for embedded hot-path caches

Pattern:
- per-page JSON cache files
- JSON envelopes in device cache paths
- `String`-heavy serialization for runtime artifacts

Why it is bad:
- more transient buffers
- more parsing overhead
- more filesystem churn
- poor fit for bounded embedded page/artifact access

Preferred direction:
- compact binary cache/artifact format
- versioned header
- deterministic page LUT
- optional JSON only for host/debug/export paths

### 7. Preallocating the wrong things too early

Pattern:
- allocate large chapter buffers or large optional render state during startup
- do it before open-time need is proven

Why it is bad:
- burns the cleanest heap too early
- can turn a survivable runtime into a boot failure
- creates pressure before the user even opens a book

Preferred direction:
- preallocate only stable fixed-cost state
- keep optional acceleration state lazy
- allocate expensive render-only state as late as possible
- degrade instead of crashing if it cannot be allocated

### 8. Using worker threads as a memory fix instead of a stack-aware design

Pattern:
- move EPUB open to a worker thread
- then tune stack sizes up and down until it stops crashing

Why it is bad:
- can trade one failure mode for another:
  - main-task stack overflow
  - `pthread` stack overflow
  - `ENOMEM` creating the thread
- masks stack-heavy open paths instead of flattening them

Preferred direction:
- reduce call-stack depth in open/layout paths
- use worker stacks only when the path truly needs isolation
- keep explicit stack diagnostics on device

### 9. Hidden image and font retention across the whole session

Pattern:
- retain decoded image/font metadata broadly in session state
- probe/resolve resources eagerly and keep them around "just in case"

Why it is bad:
- shifts memory from transient to retained
- broadens the live heap footprint of every open book

Preferred direction:
- open images/CSS/fonts on demand
- keep only compact reusable indexes
- use bounded probe scratch
- cache only what materially reduces rework

### 10. Mixing stable session state with optional acceleration state

Pattern:
- one struct owns:
  - required reader state
  - optional bitmap caches
  - transient open/layout scratch
  - navigation window state

Why it is bad:
- lifetimes blur together
- temporary state becomes accidentally retained
- fallback behavior becomes harder to reason about

Preferred direction:
- split clearly into:
  - persistent session state
  - optional acceleration state
  - transient open/layout scratch

### 11. Non-local ownership tricks for buffers

Pattern:
- leaked allocations
- raw pointers to stable buffers
- `'static` slices created by ownership hacks

Why it is bad:
- obscures lifetime rules
- complicates mutation guarantees
- makes later refactors dangerous

Preferred direction:
- normal owned buffer types
- local mutability and explicit ownership
- no leaked backing memory unless there is an unavoidable platform boundary

### 12. Feature richness before failure containment

Pattern:
- prioritize full image/font/style fidelity before ensuring:
  - bad book does not crash the device
  - low-memory fallback survives
  - failed opens clean up temp files

Why it is bad:
- nice-path features dominate engineering effort
- device remains unusable on real books

Preferred direction:
- first guarantee:
  - open does not crash
  - page turn does not crash
  - failure paths clean up
  - degraded rendering is still readable
- then add richer fidelity

## Current Concrete Findings

These are the currently observed manifestations of the anti-patterns above in this repo.

### A. Monolithic reader session objects are still dangerous even after temp-backed open succeeds

Pattern:
- one reader session object owns:
  - parser/open state
  - render engine state
  - cache policy
  - bitmap resources
  - reader cursor

Why it is bad:
- even if temp-backed open succeeds, the next post-open allocation can still fail
- it pushes multiple lifetimes into one composite allocation boundary

Preferred direction:
- persistent session should hold only:
  - source identity/path
  - layout/profile settings
  - chapter/page cursor
  - optional bitmap resource
  - cache root
- `EpubBook` and `RenderEngine` should be transient workers, not persistent session members

### B. Cache hits are already sessionless, so persistent parser ownership is usually unjustified

Pattern:
- keep `EpubBook` and `RenderEngine` alive in session state even though page-count and page lookup can already come from artifact files alone

Why it is bad:
- retained state is broader than the actual hot-path requirement
- it confuses “needed on cache miss” with “must stay alive all session”

Preferred direction:
- cache hit path should require:
  - cache root
  - pagination profile id
  - chapter/page cursor
- cache miss path may reopen transient book/engine and then drop them immediately

### C. Rerender-on-probe is still a real anti-pattern even after session slimming

Pattern:
- finding the first readable chapter or probing adjacent chapters reopens the book and rerenders a chapter on cache miss

Why it is bad:
- chapter probes become allocator-heavy
- small navigation questions still trigger heavyweight work

Preferred direction:
- first try artifact/header metadata
- if chapter readability or page count has already been discovered, persist that compactly
- keep chapter probes as exceptional work, not a normal steady-state path

### D. `Box::new(LargeType::new())` is a hidden stack-allocation bug

Pattern:
- construct a large state inline and immediately box it, for example:
  - `Box::new(InflateState::new(...))`

Why it is bad:
- the value is first created on the current stack frame and only then moved to the heap
- on ESP this can fail even when heap is healthy
- the failure often looks like a mysterious stack fault in an otherwise heap-oriented path

Confirmed manifestation:
- the EPUB temp-open path crashed on device immediately after `[EPUB-TEMP] zip_ready`
- the best current explanation is ZIP inflate-state initialization in `epub-stream/src/zip.rs`

Preferred direction:
- use heap-first constructors when the dependency provides them
- if not, redesign the allocation path instead of boxing a large stack temporary

## Positive Patterns We Want Instead

### Artifact-first navigation

- book metadata/index on SD
- chapter artifacts on SD
- page lookup by LUT
- current page only in RAM

### Small fixed-cost resident set

- framebuffer in RAM
- current page / optional lazy bitmap
- bounded scratch only

### On-demand resources

- CSS opened when needed
- image data opened when needed
- fonts resolved/loaded with strict limits

### Explicit fallback behavior

- if bitmap alloc fails, render simpler
- if image decode is too expensive, skip or outline
- if cache write fails, keep reading without crashing

### Measured guardrails

- first-open budget
- page-turn budget
- temp-open budget
- max single allocation budget
- stack watermark logging on device

## Working Heuristic

When reviewing any EPUB code path, ask:

1. Does this hold chapter-wide state in RAM longer than necessary?
2. Does this allocate proportionally to chapter size?
3. Does this reopen/rebuild work that should be an artifact lookup?
4. Is this data really session state, or just transient scratch?
5. If memory is tight, does this degrade, or does it fail hard?
6. Does disk-backed caching still secretly stage the whole thing in memory first?

If the answer to any of those is "yes", it is probably an embedded memory anti-pattern.

## Current Biggest Remaining Risks

At time of writing, the biggest remaining practical risks are:
- first-open cache-miss chapter prepare still having a large transient allocation shape
- full-chapter contiguous processing still existing in some upstream paths
- stack-heavy open/layout paths on device
- remaining policy leakage between UI/session/render layers

## Bottom Line

The paradigm shift is:

- stop thinking in terms of "objects we keep around"
- start thinking in terms of:
  - what must stay resident
  - what can be streamed
  - what can be rebuilt from SD
  - what must never allocate late

For this reader, the correct memory shape is:
- stable framebuffer
- compact session/index state
- SD-backed chapter artifacts
- page-at-a-time lookup
- bounded scratch
- graceful degradation under pressure
