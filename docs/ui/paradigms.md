# Xteink X4 UI Paradigms

Minimal UI concepts for e-ink reader with 6 buttons, 400KB RAM, 800x480 display.

---

## 1. The Line Reader

**Philosophy:** Display exactly one line of text at a time, like a teleprompter.

**Why it works:**
- Memory: ~1KB for current line buffer
- No layout calculations needed
- Zero navigation UI chrome
- Each button press = one line advance
- Refresh only the text area (partial updates possible)

**Button mapping:**
- Left/Right: Previous/Next line
- Confirm: Toggle word highlighting mode
- Back: Exit to library
- Vol Up/Down: Font size

```
+------------------+
|                  |
|                  |
|  The quick brown |
|      fox...      |
|                  |
|                  |
|      47/2847     |
+------------------+
```

---

## 2. Card Stack

**Philosophy:** Each "screen" is a card. Flip through cards like a rolodex.

**Why it works:**
- Fixed memory per card (pre-rendered bitmaps or simple text)
- No scrolling, no complex hit-testing
- Clear spatial mental model
- Cards can be: Chapter, Page, Settings, etc.
- Cards can stack (sub-menus)

**Button mapping:**
- Left/Right: Previous/Next card
- Confirm: Enter card / select option
- Back: Exit to parent card / close
- Vol Up/Down: Move card up/down in stack

```
+------------------+     +------------------+
|  [CARD 3 of 12]  |     |                  |
|                  |     |  The quick brown |
|    Chapter 3     |     |  fox jumps over  |
|   -------------  | --> |  the lazy dog    |
|   The Adventure  |     |  while the sun   |
|    Begins...     |     |  slowly sets...  |
|                  |     |                  |
|                  |     |  [2 of 47 lines] |
+------------------+     +------------------+
```

---

## 3. Vim Modal Interface

**Philosophy:** Two distinct modes - NAVIGATE mode and READ mode.

**Why it works:**
- Buttons have different meanings per mode (contextual)
- No on-screen controls needed (mode indicator only)
- Familiar to power users
- 4 buttons sufficient for each mode
- Mode indicator: small corner pixel block

**NAVIGATE Mode:**
- Left/Right: Previous/Next chapter
- Confirm: Enter READ mode
- Back: Exit app
- Vol Up/Down: Library up/down

**READ Mode:**
- Left/Right: Page turn
- Confirm: Toggle bookmark
- Back: Return to NAVIGATE mode
- Vol Up/Down: Line scroll (granular)

```
NAVIGATE Mode:                    READ Mode:
+------------------+              +------------------+
| ▓ Library        |              | The quick brown  |
|   Chapter 3      |              | fox jumps over   |
|   Chapter 4  ◄───┼── Selected   | the lazy dog.    |
|   Chapter 5      |              | It was a bright  |
|   Chapter 6      |              | cold day in      |
|                  |              | April, and the   |
| [NAV]  Vol:3/10  |              | clocks were      |
+------------------+              | striking thir... |
                                  |           [READ] |
                                  +------------------+
```

---

## 4. Zen Minimal (Status Line)

**Philosophy:** Single status line at bottom, full content above.

**Why it works:**
- ~20 lines of content visible
- One-pixel status bar
- Progress shown as dots or percentage
- No menus, no overlays
- All navigation is "invisible" (buttons only)

**Button mapping:**
- Left/Right: Previous/Next page
- Confirm: Toggle progress display style
- Back: Return to library (long-press: menu)
- Vol Up/Down: Skip 10 pages

```
+------------------+
| The quick brown  |
| fox jumps over   |
| the lazy dog.    |
|                  |
| It was a bright  |
| cold day in      |
| April, and the   |
| clocks were      |
| striking thir... |
|                  |
|                  |
| ·············▓░░ │  <- progress: 40%
+------------------+
```

---

## 5. Dual-Pane (Sidebar + Content)

**Philosophy:** Fixed-width navigation sidebar (120px), content fills rest.

**Why it works:**
- Sidebar never changes (reduced refresh area)
- Always know where you are
- Content area is pure text
- Sidebar can show: TOC, bookmarks, search results
- Split reduces cognitive load

**Button mapping:**
- Left/Right: Move focus between sidebar/content OR navigate within pane
- Confirm: Select item in focused pane
- Back: Go back / collapse
- Vol Up/Down: Scroll within focused pane

```
+------------------+--------+
| TOC              │ The    │
| ├─ Chap 1        │ quick  │
| ├─ Chap 2        │ brown  │
| ├► Chap 3        │ fox    │
| │  ├─ Sec A      │ jumps  │
│ │  └─ Sec B      │ over   │
| ├─ Chap 4        │ the    │
| ├─ Chap 5        │ lazy   │
|                  │ dog.   │
| [Page 47/200]    │        │
+------------------+--------+
        ^
    120px sidebar
```

---

## 6. Spatial Grid (2D Navigation)

**Philosophy:** Content arranged in a 2D grid. Arrow keys move focus.

**Why it works:**
- No hidden states - everything visible
- Excellent for: library view, settings, TOC
- Focus indicator is just an inverted box
- Predictable navigation
- Minimal redraw: only move focus indicator

**Button mapping:**
- Left/Right/Up/Down: Move selection (Vol buttons = Up/Down)
- Confirm: Open selected item
- Back: Go back / exit

```
Library Grid (3x3 books shown):
+------------------+
| [Book1] Book2  B3│
|  Book4   B5   [B6]◄─── Focus here
|  Book7   B8    B9│
|                  │
|  Page 1 of 5     │
+------------------+

Or Settings Grid:
+------------------+
| [WiFi]  Fonts    │
| Storage [About]◄─┤
|  Sleep   Date    │
+------------------+
```

---

## 7. Progressive Disclosure (Accordion)

**Philosophy:** Start minimal. Expand sections on demand.

**Why it works:**
- Initial screen: Just the book title and "Start Reading"
- Expands to show: TOC, Bookmarks, Progress
- Collapses back to minimal
- Perfect for slow e-ink (don't show what you don't need)
- Memory: only render expanded sections

**Button mapping:**
- Left/Right: Select section (collapsed) or navigate within expanded section
- Confirm: Expand/collapse current section
- Back: Collapse all / exit
- Vol Up/Down: Scroll within expanded content

```
Collapsed:                        Expanded (TOC):
+------------------+              +------------------+
|                  |              | ▼ Pride & Prej...│
|  Pride &         |              |                  │
|  Prejudice       |              |   Chapter 1      │
|                  |              | ► Chapter 2      │
|  [Start Reading] │              |   Chapter 3      │
|                  |              |   Chapter 4      │
|  ▶ Contents      │              |   ...            │
|  ▶ Bookmarks     │              |                  │
|  ▶ Progress      │              |  [Start Reading] │
+------------------+              +------------------+
```

---

## Comparison Matrix

| Paradigm | RAM | Buttons Used | Best For | Complexity |
|----------|-----|--------------|----------|------------|
| Line Reader | 1KB | 6 | Novels, poetry | Very Low |
| Card Stack | 10KB | 6 | Mixed content | Low |
| Vim Modal | 5KB | 6 | Power users | Medium |
| Zen Minimal | 2KB | 6 | Pure reading | Very Low |
| Dual-Pane | 8KB | 6 | Navigation-heavy | Medium |
| Spatial Grid | 5KB | 6 (as 4-way) | Library, settings | Low |
| Progressive | 3KB | 6 | General purpose | Low |

---

## Recommendations

1. **For v1.0 (MVP):** Zen Minimal or Line Reader
   - Fastest to implement
   - Lowest bug surface
   - Users get pure reading experience

2. **For library navigation:** Spatial Grid
   - Works with just 4 directional inputs
   - Clear visual hierarchy

3. **For advanced users:** Vim Modal
   - Efficient once learned
   - No screen real estate wasted on controls

4. **Hybrid approach:** Progressive Disclosure
   - Can adapt all other paradigms
   - Scales from simple to complex
