# Complete Firmware UI Mockups - 5 Paradigms

Complete ASCII wireframes for all firmware screens: boot, home, library, reading, settings.

---

## 1. ZEN MINIMAL — "Invisible UI"

**Core Principle:** Full-screen content, 1-pixel status bar. No menus visible until invoked.

### Boot → Home → Library → Reading Flow

```
BOOT SCREEN                      HOME (Library)
+------------------+             +------------------+
|                  |             | ┌─ War and Peace │
|     XTEINK       |             | │  Moby Dick     │
|       X4         |  ────────>  | │  Pride &...    │
|                  |             | │  1984          │
|   Loading...     |             | │  [Dune]        │
|                  |             | │  Hamlet        │
|  v1.0.2          |             | └─ Frankenstein  │
|                  |             |                  │
|                  |             | Page 1 of 3  ·▓░░│
+------------------+             +------------------+
         │                              │
         │  Press CONFIRM               │  L/R: Navigate
         │  (or auto after 2s)          │  CONFIRM: Read
         │                              │  BACK: Power off
         v                              v

READING (Text)                   READING (Status Shown)
+------------------+             +------------------+
| It was the best  │             | It was the best  │
| of times, it was │             | of times, it was │
| the worst of     │             | the worst of     │
| times, it was    │             | times, it was    │
| the age of       │             | age of wisdom... │
| wisdom, it was   │             |                  │
| the age of       │             | 47% · Chap 3 ·   │
| foolishness...   │             │ 12:47 · Bat: 85% │
|                  │             +------------------+
| ·············▓░░ │              ^ Status shown on
+------------------+                CONFIRM press

L/R: Page turn
VOL: Skip 10 pages
CONFIRM: Toggle status
BACK (long): Menu

MENU OVERLAY (Long-press BACK)
+------------------+
| It was the best  │
| of times...      │
| ┌──────────────┐ │
| │ ▓ Go to Page │ │
| │   Bookmarks  │ │
| │   Font Size  │ │
| │   Settings   │ │
| └──────────────┘ │
| ·············▓░░ │
+------------------+
```

### Settings Screens

```
SETTINGS MENU                    FONT SETTINGS
+------------------+             +------------------+
| ┌──────────────┐ │             | ┌──────────────┐ │
| │ ▓ Display    │ │             | │ Font: Serif  │ │
| │   Fonts      │ │  ────────>  │ │ Size: [14]   │ │
| │   Storage    │ │             │ │ Line: 1.5    │ │
| │   WiFi       │ │             │ │ Margin: Nrm  │ │
| │   About      │ │             │ └──────────────┘ │
| └──────────────┘ │             |                  │
|                  │             |    Preview:      │
| ·············▓░░ │             |    The quick     │
+------------------+             |    brown fox...  │
                                 +------------------+

GO TO PAGE DIALOG                STORAGE INFO
+------------------+             +------------------+
| ┌──────────────┐ │             | ┌──────────────┐ │
| │ Go to Page:  │ │             │ │ Storage      │ │
| │              │ │             │ │ ████████░░░░ │ │
| │     147      │ │             │ │ 12.4 / 16 GB │ │
| │              │ │             │ │              │ │
| │  [Cancel]    │ │             │ │ Books: 47    │ │
| └──────────────┘ │             │ │ Free: 3.6 GB │ │
|                  │             │ └──────────────┘ │
|                  │             |                  │
| ·············▓░░ │             | ·············▓░░ │
+------------------+             +------------------+
```

---

## 2. LINE READER — "Teleprompter"

**Core Principle:** One line at a time, centered. Words flow like a ticker.

### Boot → Home → Reading Flow

```
BOOT                             HOME (Book Grid)
+------------------+             +------------------+
|                  │             |                  │
|                  │             |   [War&Peace]    │
|                  │             |    Moby Dick     │
|    XTEINK X4     │  ────────>  |    Dune          │
|                  │             |                  │
|    Loading...    │             |    3 of 12       │
|                  │             |                  │
|                  │             |                  │
+------------------+             +------------------+
                                        │
        L/R: Select book               │
        CONFIRM: Open                  │
        VOL: Scroll list               v

READING (Line Mode)              READING (Word Mode)
+------------------+             +------------------+
|                  │             |                  │
|                  │             |                  │
|                  │             |                  │
| The quick brown  │             |     brown        │  <- Highlight
|     fox...       │             |                  │
|                  │             |                  │
|                  │             |                  │
|    47/2847       │             |   128/15600      │
+------------------+             +------------------+

L/R: Prev/Next line         L/R: Prev/Next word
VOL: Font size              CONFIRM: Exit word mode
CONFIRM: Toggle word mode   BACK: Exit book
BACK: Exit to library

BOOK STATISTICS
+------------------+
|                  │
|   War and Peace  │
|                  │
|   Progress: 2%   │
|   Line: 47       │
|   Words: 128     │
|   Time: 3m       │
|                  │
|    [Continue]    │
+------------------+
```

### Library & Settings

```
LIBRARY LIST                     BOOK INFO (CONFIRM on book)
+------------------+             +------------------+
|                  │             |                  │
|   War and Peace  │             |  War and Peace   │
|   by Tolstoy     │  ────────>  │  by Leo Tolstoy  │
|                  │             │                  │
|   1225 pages     │             │  1225 pages      │
|   2% complete    │             │  Historical      │
|                  │             │  Added: Jan 12   │
|   [Read] [Info]  │             │                  │
|                  │             │  [Read] [Delete] │
+------------------+             +------------------+

SETTINGS MENU                    DISPLAY SETTINGS
+------------------+             +------------------+
|                  │             |                  │
|   Settings       │             |   Display        │
|                  │             │                  │
|   Display        │             │  Refresh: Half   │
|   Font           │  ────────>  │  Invert: Off     │
|   Navigation     │             │  Sleep: 5 min    │
|   Storage        │             │                  │
|   About          │             │  [Save] [Cancel] │
|                  │             |                  │
+------------------+             +------------------+
```

---

## 3. SPATIAL GRID — "Focus Box"

**Core Principle:** 2D grid navigation with inverted focus. Everything visible at once.

### Boot → Home Grid → Library Grid Flow

```
BOOT                             HOME GRID (2x2)
+------------------+             +------------------+
|  XTEINK X4       │             |                  │
|  ─────────────   │             | ┌────┐ ┌────┐    │
|  Booting...      │  ────────>  │ │READ│ │LIB │    │
|                  │             │ └────┘ └────┘    │
|                  │             │ ┌────┐ ┌────┐    │
|                  │             │ │SET │ │PWR │    │
|                  │             │ └────┘ └────┘    │
|                  │             │                  │
|                  │             │  [←][→] [↑][↓]   │
+------------------+             +------------------+
                                        │
        Grid: L/R/VOL U/D              │
        CONFIRM: Select                │
        BACK: Sleep                    v

LIBRARY GRID (3x3)               READING (Text + Grid Overlay)
+------------------+             +------------------+
| ███████ Book2  B3│             │ The quick brown  │
|  Book4   B5    B6│  ────────>  │ fox jumps over   │
|  Book7   B8    B9│             │ the lazy dog...  │
|                  │             │                  │
| Page 1 of 5      │             │ ┌───┐ ┌───┐ ┌──┐ │
+------------------+             │ │<P │ │TOC│ │>P│ │
                                 │ └───┘ └───┘ └──┘ │
L/R/VOL U/D: Move selection      │ ┌───┐ ┌───┐ ┌──┐ │
CONFIRM: Open book               │ │BMK│ │SET│ │X │ │
BACK: Previous screen            │ └───┘ └───┘ └──┘ │
                                 +------------------+

BOOK DETAIL (CONFIRM on book)    NOW READING OVERLAY
+------------------+             +------------------+
| ████████████████ │             │ It was the best  │
| █              █ │             │ of times, it was │
| █   [Cover]    █ │             │ the worst of     │
| █              █ │             │ times...         │
| ████████████████ │             │                  │
|                  │             │ ┌──────────────┐ │
| Dune             │             │ │ Now Reading: │ │
| Frank Herbert    │             │ │ 47% · 12 min │ │
|                  │             │ └──────────────┘ │
| [Read] [Info]    │             +------------------+
+------------------+
```

### Settings as Grid

```
SETTINGS GRID                    DISPLAY SUBGRID
+------------------+             +------------------+
| [WiFi]  Fonts    │             │ [Bright] Contrast│
| Storage [About]  │  ────────>  │ Sleep  [Invert]  │
| Sleep   Date     │             │                  │
|                  │             │ Page 1 of 2      │
| Page 1 of 2      │             +------------------+
+------------------+

FONT GRID                        WIFI GRID
+------------------+             +------------------+
│ [Serif] Sans     │             │ [Scan] Saved     │
│ Mono   [Size+]   │             │                  │
│ [Size-] Line++   │             │ Home_WiFi ████   │
│                  │             │ Guest_    ░░░░   │
│ Page 1 of 3      │             │                  │
+------------------+             │ Page 1 of 1      │
                                 +------------------+
```

---

## 4. VIM MODAL — "Contextual Mode"

**Core Principle:** Two modes (NAV vs READ). Mode indicator in corner. Buttons remap.

### Mode Transitions Flow

```
BOOT                             NAV MODE (Library)
+------------------+             +------------------+
|  XTEINK X4       │             │ ▓ Library       │ <- NAV indicator
|  Loading...      │  ────────>  │   Chapter 3     │
|                  │             │   Chapter 4  ◄──┤ <- Selection
|                  │             │   Chapter 5     │
|                  │             │   Chapter 6     │
|                  │             │                 │
|                  │             │ [NAV]  Vol:3/10 │
+------------------+             +------------------+
                                        │
        NAV Mode Buttons:              │
        L/R: Move selection            │  CONFIRM: Enter
        VOL: Scroll list               │  READ mode
        CONFIRM: → READ mode           v
        BACK: Exit app

READ MODE (Minimal)              READ MODE (Status)
+------------------+             +------------------+
│ The quick brown  │             │ The quick brown  │
│ fox jumps over   │             │ fox jumps over   │
│ the lazy dog.    │             │ the lazy dog.    │
│                  │             │                  │
│ It was a bright  │             │ It was a bright  │
│ cold day in      │             │ cold day in      │
│ April, and the   │             │ April, and the   │
│ clocks were      │             │ clocks were      │
│ striking thir... │             │ striking thir... │
│         [READ]   │             │ 47% [READ] 12:47 │
+------------------+             +------------------+

READ Mode Buttons:
L/R: Page turn
VOL: Line scroll
CONFIRM: Bookmark toggle
BACK: → NAV mode
DOUBLE-BACK: Exit book
```

### NAV Mode Deep Dive

```
NAV: Library                     NAV: TOC Expanded
+------------------+             +------------------+
│ ▓ Library       │             │ ▓ Pride & Prej..│
│   War and Peace │             │   ▼ Chapter 1   │
│   Moby Dick     │  ────────>  │     Chapter 2   │
│   Pride &... ◄──┤             │     Chapter 3  ◄┤
│   1984          │             │     Chapter 4   │
│   Dune          │             │   Chapter 5-10  │
│                 │             │                 │
│ [NAV] Bk 3/47   │             │ [NAV] Ch 3/61   │
+------------------+             +------------------+

NAV: Settings                    NAV: Storage
+------------------+             +------------------+
│ ▓ Settings      │             │ ▓ Storage       │
│   Display       │             │   ████████░░░░  │
│   Fonts         │             │   12.4 / 16 GB  │
│   Storage  ◄────┤             │                 │
│   WiFi          │             │   Books: 47     │
│   About         │             │   Free: 3.6 GB  │
│                 │             │                 │
│ [NAV] Itm 3/5   │             │ [NAV] Info      │
+------------------+             +------------------+
```

### Mode Overlays & Popups

```
READ: Bookmark Toggle            READ: Go To Page
+------------------+             +------------------+
│ The quick brown  │             │ The quick brown  │
│ fox jumps over   │             │ fox jumps over   │
│ the lazy dog.    │             │ the lazy dog.    │
│                  │             │                  │
│ ┌──────────────┐ │             │ ┌──────────────┐ │
│ │ ★ Bookmarked!│ │             │ │ Go to: 147   │ │
│ │ Page 47      │ │             │ │      [Go]    │ │
│ └──────────────┘ │             │ └──────────────┘ │
│                  │             │                  │
│         [READ]   │             │ 23% [READ] 12:47 │
+------------------+             +------------------+

MODE SWITCH ANIMATION            CONFIRM DIALOG
+------------------+             +------------------+
│                  │             │ The quick brown  │
│    [====NAV====] │             │ fox jumps over   │
│                  │             │                  │
│  Switching to    │             │ ┌──────────────┐ │
│  Library view... │             │ │ Exit book?   │ │
│                  │             │ │ [Yes]  [No]  │ │
│    (no refresh   │             │ └──────────────┘ │
│     needed)      │             │                  │
+------------------+             │         [READ]   │
                                 +------------------+
```

---

## 5. CARD STACK — "Rolodex"

**Core Principle:** Discrete cards stack. Flip through them. Cards have depth.

### Boot → Home Stack → Reading Stack Flow

```
BOOT                             HOME CARD
+------------------+             +------------------+
|  XTEINK X4       │             │  ┌───────────┐   │
|  Loading...      │  ────────>  │  │           │   │
|                  │             │  │  XTEINK   │   │
|                  │             │  │    X4     │   │
|                  │             │  │           │   │
|                  │             │  │  [READ]   │   │
|                  │             │  │  [LIB]    │   │
|                  │             │  └───────────┘   │
|                  │             │  Card 1 of 3     │
+------------------+             +------------------+
                                        │
        Card buttons:                  │
        L/R: Flip card                 │  CONFIRM: Enter
        VOL: Jump 5 cards              │  selected option
        CONFIRM: Enter                 v
        BACK: Exit

LIBRARY CARD (List)              LIBRARY CARD (Grid)
+------------------+             +------------------+
│  ┌───────────┐   │             │  ┌───────────┐   │
│  │ Library   │   │             │  │  [B1] [B2]│   │
│  │ ├─ War&P  │   │  ────────>  │  │  [B3] [B4]│   │
│  │ ├─ Moby   │   │             │  │  [B5] [B6]│   │
│  │ ├► Dune   │   │             │  │           │   │
│  │ ├─ 1984   │   │             │  │  Page 1/8 │   │
│  │ └─ ...    │   │             │  └───────────┘   │
│  └───────────┘   │             │  Card 2 of 8     │
│  Card 1 of 8     │             +------------------+
+------------------+
         │
         │ CONFIRM on book
         v

BOOK CARD (Info)                 READING CARD (Content)
+------------------+             +------------------+
│  ┌───────────┐   │             │  ┌───────────┐   │
│  │  [Cover]  │   │             │  │ The quick │   │
│  │  Dune     │   │             │  │ brown fox │   │
│  │  Herbert  │   │  ────────>  │  │ jumps...  │   │
│  │           │   │             │  │           │   │
│  │  412 pp   │   │             │  │ It was... │   │
│  │  Sci-Fi   │   │             │  │           │   │
│  └───────────┘   │             │  └───────────┘   │
│  Card 3 of 47    │             │  Page 3 of 412   │
+------------------+             +------------------+
```

### Card Types & Menu Cards

```
SETTINGS CARD (Main)             SETTINGS CARD (Display)
+------------------+             +------------------+
│  ┌───────────┐   │             │  ┌───────────┐   │
│  │ Settings  │   │             │  │ Display   │   │
│  │ Display   │   │  ────────>  │  │ Bright: 5 │   │
│  │ Fonts     │   │             │  │ Contrast:3│   │
│  │ Storage ◄─┤   │             │  │ Sleep: 5m │   │
│  │ WiFi      │   │             │  │ Invert: N │   │
│  │ About     │   │             │  │           │   │
│  └───────────┘   │             │  └───────────┘   │
│  Card 1 of 5     │             │  Card 1 of 3     │
+------------------+             +------------------+

MENU CARD (Overlay)              CONFIRM CARD
+------------------+             +------------------+
│  ┌───────────┐   │             │  ┌───────────┐   │
│  │ ┌───────┐ │   │             │  │           │   │
│  │ │ GoTo  │ │   │             │  │  Delete   │   │
│  │ │ BMark │ │   │             │  │  "Dune"?  │   │
│  │ │ Font  │ │   │             │  │           │   │
│  │ │ Sleep │ │   │             │  │  [YES]    │   │
│  │ │ Exit  │ │   │             │  │  [NO ]    │   │
│  │ └───────┘ │   │             │  │           │   │
│  └───────────┘   │             │  └───────────┘   │
│  Overlay         │             │  Card 47 of 47   │
+------------------+             +------------------+

PROGRESS CARD                    STATISTICS CARD
+------------------+             +------------------+
│  ┌───────────┐   │             │  ┌───────────┐   │
│  │ Progress  │   │             │  │ Reading   │   │
│  │           │   │             │  │ Stats:    │   │
│  │ █████░░░░ │   │             │  │           │   │
│  │ 47%       │   │             │  │ 12 books  │   │
│  │ Page 147  │   │             │  │ 4.2k pages│   │
│  │ Chapter 3 │   │             │  │ 23 hours  │   │
│  │           │   │             │  │ This week │   │
│  └───────────┘   │             │  └───────────┘   │
│  Card 47/412     │             │  Card 1 of 1     │
+------------------+             +------------------+
```

### Card Navigation Visualization

```
CARD STACK VISUALIZATION

Top View (Stack of cards):
    ┌─────────┐
    │ CARD 3  │ <- Current (Reading)
    ├─────────┤
    │ CARD 2  │ <- Book Info
    ├─────────┤
    │ CARD 1  │ <- Library
    └─────────┘

L/R: Flip to prev/next card in stack
VOL: Jump multiple cards
CONFIRM: Drill deeper (pushes new card)
BACK: Pop current card (go back)

DEEP STACK EXAMPLE:
    ┌─────────┐
    │Display  │ <- Current (Settings submenu)
    ├─────────┤
    │Settings │ <- Parent
    ├─────────┤
    │Home     │ <- Root
    ├─────────┤
    │Library  │
    ├─────────┤
    │Book Info│
    ├─────────┤
    │Reading  │
    └─────────┘

BACK button pops the stack one level at a time.
```

---

## Comparison: Full UI Flows

| Screen | Zen | Line | Grid | Vim | Cards |
|--------|-----|------|------|-----|-------|
| **Home** | List w/ progress | List | 2x2 grid | NAV: Library list | Home card |
| **Library** | Same as home | Book list + info | 3x3 book grid | NAV: Expandable list | Stack of book cards |
| **Reading** | Full text | 1 line | Text + grid overlay | READ: Full text | Content card |
| **Menu** | Long-press overlay | Full-screen menu | Grid overlay | Mode switch | Menu overlay card |
| **Settings** | List overlay | List | Settings grid | NAV: Settings list | Stack of setting cards |

---

## Button Mapping Summary

```
┌─────────────────────────────────────────────────────────────────┐
│                    BUTTON MAPPING COMPARISON                    │
├─────────────────────────────────────────────────────────────────┤
│           │ LEFT    │ RIGHT   │ CONFIRM  │ BACK    │ VOL U/D  │
├───────────┼─────────┼─────────┼──────────┼─────────┼──────────┤
│ Zen       │ PrevPg  │ NextPg  │ Status   │ Menu    │ Skip 10  │
│           │         │         │          │ (long)  │          │
├───────────┼─────────┼─────────┼──────────┼─────────┼──────────┤
│ Line      │ PrevLine│ NextLine│ WordMode │ Exit    │ FontSize │
├───────────┼─────────┼─────────┼──────────┼─────────┼──────────┤
│ Grid      │ Left    │ Right   │ Select   │ Back    │ Up/Down  │
├───────────┼─────────┼─────────┼──────────┼─────────┼──────────┤
│ Vim NAV   │ PrevItem│ NextItem│ →READ    │ Exit    │ Scroll   │
│ Vim READ  │ PrevPg  │ NextPg  │ Bookmark │ →NAV    │ LineScroll│
├───────────┼─────────┼─────────┼──────────┼─────────┼──────────┤
│ Cards     │ PrevCard│ NextCard│ Enter    │ PopCard │ Jump5    │
└───────────┴─────────┴─────────┴──────────┴─────────┴──────────┘
```
