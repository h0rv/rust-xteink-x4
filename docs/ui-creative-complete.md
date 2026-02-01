# Creative E-Reader UI Concepts — Complete Mockups

**Philosophy:** Leverage 6 buttons + 480x800 screen to create UNIQUE, delightful UX that feels designed (not adapted).

---

## 1. **RADIAL COMMAND PALETTE**

**The Concept:** Pie menu triggered by chords. Directional buttons navigate slices like a D-pad.

### User Flow

```
READING                          CHORD: [CONFIRM + LEFT]
┌─────────────────┐              ┌─────────────────┐
│                 │              │                 │
│   Chapter 7     │              │   Chapter 7     │
│                 │              │                 │
│   The sun rose  │              │   The sun rose  │
│   slowly over   │              │   slowly over   │
│   the eastern   │              │   the eastern   │
│   hills, casting│              │   hills...      │
│                 │              │                 │
│                 │              │    ┌─────┐      │
│                 │              │   ╱  TOC  ╲     │
│   ─── 45% ───   │              │  ╱─────────╲    │
│                 │              │ │ SEARCH    │   │
└─────────────────┘              │ │   ▓▓▓     │◄──┤ ← Highlight
                                 │ │ BOOKMARK  │   │
                                 │  ╲─────────╱    │
                                 │   ╲DICT  /     │
                                 │    └─────┘      │
                                 │   [SHARE]       │
                                 │                 │
                                 │   ─── 45% ───   │
                                 └─────────────────┘

RADIAL NAVIGATION:
[UP/VOL_UP]    = TOC
[RIGHT]        = BOOKMARK ▓ (highlighted)
[DOWN/VOL_DOWN]= DICTIONARY
[LEFT]         = SEARCH
[CONFIRM]      = Select highlighted
[BACK]         = Close menu
```

### Library & Settings with Radial

```
LIBRARY (List View)              RADIAL ON BOOK
┌─────────────────┐              ┌─────────────────┐
│  My Library     │              │  War and Peace  │
│  ───────────────│              │  by Tolstoy     │
│                 │              │                 │
│  War and Peace  │              │  1225 pages     │
│  Moby Dick      │              │  Historical     │
│  Pride &... ◄───┤  ────────>   │                 │
│  1984           │              │    ┌─────┐      │
│  Dune           │              │   ╱READ ╲       │
│                 │              │  ╱INFO   ╲      │
│  Page 1 of 3    │              │ │ DELETE   │     │
│                 │              │ │   ▓▓▓    │◄────┤
└─────────────────┘              │ │ EXPORT   │     │
                                 │  ╲ADD BM  /      │
                                 │   ╲───── /       │
                                 │                 │
                                 │                 │
                                 └─────────────────┘

SETTINGS RADIAL
┌─────────────────┐
│                 │
│   Settings      │
│                 │
│    ┌─────┐      │
│   ╱DISPLAY╲     │
│  ╱─────────╲    │
│ │ FONTS     │   │
│ │   ▓▓▓     │◄──┤
│ │ STORAGE   │   │
│  ╲─────────╱    │
│   ╲WIFI  /      │
│    └─────┘      │
│   [ABOUT]       │
│                 │
└─────────────────┘
```

### Menu States

```
DICTIONARY RADIAL                  BOOKMARK RADIAL
┌─────────────────┐                ┌─────────────────┐
│  "Serendipity"  │                │  ★ Bookmarked!  │
│                 │                │                 │
│  The occurrence │                │  Page 247       │
│  of events...   │                │  Chapter 7      │
│                 │                │                 │
│    ┌─────┐      │                │    ┌─────┐      │
│   ╱PREV  ╲      │                │   ╱EDIT  ╲      │
│  ╱─────────╲    │                │  ╱─────────╲    │
│ │ EXPAND    │   │                │ │ VIEW ALL  │   │
│ │   ▓▓▓     │◄──┤                │ │   ▓▓▓     │◄──┤
│ │ NEXT DEF  │   │                │ │ DELETE    │   │
│  ╲─────────╱    │                │  ╲─────────╱    │
│   ╲CLOSE /      │                │   ╲SHARE /      │
│    └─────┘      │                │    └─────┘      │
│   [PRONOUNCE]   │                │   [NOTE]        │
│                 │                │                 │
└─────────────────┘                └─────────────────┘
```

---

## 2. **DUAL-PANE "TANDEM" MODE**

**The Concept:** Split the tall screen. Top = main content, bottom = reference/context. CONFIRM toggles focus.

### Reading Flow

```
READING (Focus: Top)             TOGGLE FOCUS
┌─────────────────┐              ┌─────────────────┐
│▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│◄──Active     │  Chapter 7      │
│                 │   (glow)     │                 │
│   Chapter 7     │              │   The sun rose  │
│                 │              │   slowly over   │
│   The sun rose  │              │   the eastern   │
│   slowly over   │              │   hills...      │
│   the eastern   │              │                 │
│   hills...      │              │ ────────────────│
│                 │              │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│◄──Active
├─────────────────┤              │▓                ▓│
│                 │              │▓  Notes:        ▓│
│   Notes:        │              │▓                ▓│
│                 │              │▓  - Eastern =   ▓│
│   - Eastern =   │              │▓    hope/new    ▓│
│   hope/new      │              │▓  - Contrast    ▓│
│   - Contrast    │              │▓    with prev   ▓│
│   with prev     │              │▓                ▓│
│                 │              │▓  3 notes       ▓│
│   3 notes       │              │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│
└─────────────────┘              └─────────────────┘

BUTTONS (Reading Mode):
←/→        = Navigate in focused pane only
CONFIRM    = Toggle focus (Top ↔ Bottom)
VOL UP/DN  = Fast scroll in focused pane
BACK       = Sync panes / Return to top
```

### Different Tandem Configurations

```
TANDEM: TEXT + DICTIONARY        TANDEM: TEXT + TOC
┌─────────────────┐              ┌─────────────────┐
│▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│              │  Chapter 7      │
│                 │              │                 │
│   ...serendipity│              │   The sun rose  │
│   and fate...   │              │   slowly...     │
│                 │              │                 │
│                 │              │                 │
│                 │              │                 │
├─────────────────┤              ├─────────────────┤
│ ser·en·dip·i·ty │              │▓ TOC            ▓│
│ /ˌserənˈdipədē/ │              │▓ ├─ Chap 1      ▓│
│                 │              │▓ ├─ Chap 2      ▓│
│ n. the occurrence│             │▓ ├─ Chap 3      ▓│
│ of events by    │              │▓ ├─ Chap 4      ▓│
│ chance...       │              │▓ ├─ Chap 5      ▓│
│                 │              │▓ ├─ Chap 6      ▓│
│ [Prev] [Next]   │              │▓ ├► Chap 7  ◄───▓│
└─────────────────┘              │▓ └─ Chap 8      ▓│
                                 └─────────────────┘

TANDEM: TRANSLATION MODE         TANDEM: STATS MODE
┌─────────────────┐              ┌─────────────────┐
│▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│              │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│
│                 │              │                 │
│   Call me       │              │   "Is this a    │
│   Ishmael...    │              │   dagger I see  │
│                 │              │   before me..." │
│                 │              │                 │
│                 │              │                 │
├─────────────────┤              ├─────────────────┤
│ Llámame         │              │ Reading Stats   │
│ Ishmael...      │              │ ────────────────│
│                 │              │                 │
│                 │              │ Session: 23 min │
│ Spanish         │              │ Pages: 47       │
│ [Original]      │              │ WPM: 245        │
│                 │              │                 │
│                 │              │ ████████░░ 67%  │
└─────────────────┘              └─────────────────┘
```

---

## 3. **CONTEXTUAL HUD INTERFACE**

**The Concept:** Dynamic button labels at bottom - like a game controller overlay. Changes based on context.

### Context Transitions

```
READING CONTEXT                  MENU CONTEXT
┌─────────────────┐              ┌─────────────────┐
│                 │              │  > Settings     │
│   Chapter 7     │              │    Display      │
│                 │              │    Network      │
│   The sun rose  │              │    About   ◄──  │
│   slowly over   │              │                 │
│   the eastern   │              │                 │
│   hills...      │              │                 │
│                 │              │                 │
│                 │              │                 │
│                 │              │                 │
├─────────────────┤              ├─────────────────┤
│◀Prev │▶Next│Menu│              │◀Back │▶Sel│Home │
│Vol:░░▓▓▓▓░░│ 23%│              │     │   │     │
└─────────────────┘              └─────────────────┘

DICTIONARY CONTEXT               LIBRARY CONTEXT
┌─────────────────┐              ┌─────────────────┐
│  "Serendipity"  │              │  War and Peace  │
│                 │              │  Moby Dick      │
│  The occurrence │              │  Pride & Prej...│
│  of events...   │              │  1984      ◄──  │
│                 │              │  Dune           │
│                 │              │                 │
│  [1/3 defs]     │              │                 │
│                 │              │                 │
│                 │              │                 │
│                 │              │                 │
├─────────────────┤              ├─────────────────┤
│◀Prev│▶Next│More│              │◀Prev │▶Next│Open │
│Def:  │Def: │Defs│              │Bk:   │Bk:  │Info │
└─────────────────┘              └─────────────────┘
```

### Settings with Contextual HUD

```
SETTINGS HOME                    SETTINGS DISPLAY
┌─────────────────┐              ┌─────────────────┐
│  Settings       │              │  Display        │
│  ───────────────│              │  ───────────────│
│                 │              │                 │
│  > Display      │              │  Brightness     │
│    Fonts        │              │  [▓▓▓▓▓▓░░░] 5 │
│    Network      │              │                 │
│    Storage      │              │  Contrast       │
│    About   ◄──  │              │  [▓▓▓▓░░░░░] 4  │
│                 │              │                 │
│                 │              │  Sleep Timer    │
│                 │              │  [5 min]        │
│                 │              │                 │
├─────────────────┤              ├─────────────────┤
│◀Back │▶Sel│Home │              │◀Back│▶Edit│Save │
│     │   │     │              │     │    │     │
└─────────────────┘              └─────────────────┘

FONT SETTINGS                    WIFI SETTINGS
┌─────────────────┐              ┌─────────────────┐
│  Fonts          │              │  Network        │
│  ───────────────│              │  ───────────────│
│                 │              │                 │
│  Font Family    │              │  WiFi: ON       │
│  [Serif]        │              │                 │
│                 │              │  Available:     │
│  Size: [14] pt  │              │  █ HomeNetwork  │
│                 │              │  ░ GuestWiFi    │
│  Line: 1.5      │              │  ░ xfinitywifi  │
│                 │              │                 │
│  Margin: Normal │              │  [Scan] [Saved] │
│                 │              │                 │
├─────────────────┤              ├─────────────────┤
│◀Back│▶Adj│Save │              │◀Back│▶Sel│Cnct │
│     │ust:│     │              │     │   │     │
└─────────────────┘              └─────────────────┘
```

### Popup Overlays with HUD

```
BOOKMARK POPUP                   GOTO PAGE POPUP
┌─────────────────┐              ┌─────────────────┐
│                 │              │                 │
│   The sun rose  │              │   The sun rose  │
│   slowly over   │              │   slowly over   │
│   the eastern   │              │   the eastern   │
│   hills...      │              │   hills...      │
│                 │              │                 │
│ ┌─────────────┐ │              │ ┌─────────────┐ │
│ │ ★ Bookmark  │ │              │ │ Go to Page: │ │
│ │  added!     │ │              │ │             │ │
│ │  Page 247   │ │              │ │    147      │ │
│ │             │ │              │ │             │ │
│ │ [Note] [OK] │ │              │ │  [Cancel]   │ │
│ └─────────────┘ │              │ └─────────────┘ │
├─────────────────┤              ├─────────────────┤
│◀Note │▶OK│Close│              │◀Cncl│▶Go │Edit │
│     │   │     │              │     │Pg  │Num: │
└─────────────────┘              └─────────────────┘

DELETE CONFIRM                   SYNC PROGRESS
┌─────────────────┐              ┌─────────────────┐
│                 │              │                 │
│   The sun rose  │              │   The sun rose  │
│   slowly over   │              │   slowly over   │
│   the eastern   │              │   the eastern   │
│   hills...      │              │   hills...      │
│                 │              │                 │
│ ┌─────────────┐ │              │ ┌─────────────┐ │
│ │ Delete      │ │              │ │ Syncing...  │ │
│ │ "War and    │ │              │ │             │ │
│ │  Peace"?    │ │              │ │ ████████░░░ │ │
│ │             │ │              │ │ 80%         │ │
│ │[No]    [Yes]│ │              │ │             │ │
│ └─────────────┘ │              │ └─────────────┘ │
├─────────────────┤              ├─────────────────┤
│◀No  │▶Yes│Cncl │              │◀Cncl│▶OK│Hide │
│     │   │     │              │     │   │     │
└─────────────────┘              └─────────────────┘
```

---

## 4. **MOMENTUM SCROLLING + PHYSICS**

**The Concept:** Buttons control virtual physics. Short = precise. Long-press = build momentum. Visual "charge" indicator.

### Physics States

```
IDLE                             CHARGING (Hold RIGHT)
┌─────────────────┐              ┌─────────────────┐
│                 │              │                 │
│   Chapter 7     │              │   Chapter 7     │
│                 │              │                 │
│   The sun rose  │              │   The sun rose  │
│   slowly over   │              │   slowly over   │
│   the eastern   │              │   the eastern   │
│   hills...      │              │   hills...      │
│                 │              │                 │
│                 │              │        ▓▓▓▓▓▓▓  │
│                 │              │        ▓▓▓▓▓░░░ │
│   ─── 45% ───   │              │   ─── 45% ───   │
│                 │              │    ░░░░░░░░░▓▓▓ │◄── Charge meter
└─────────────────┘              └─────────────────┘
                                  Hold → to charge
                                  Release to glide

GLIDING (Momentum)               CAUGHT (CONFIRM pressed)
┌─────────────────┐              ┌─────────────────┐
│                 │              │                 │
│   ...eastern    │              │   Chapter 8     │◄── Landed here
│   hills, the    │              │                 │
│   valley        │              │   The market    │
│   below...      │              │   square was    │
│ ▓▓▓▓▓▓▓▓▓▓▓▓▓▓░ │◄──Ghost    │   bustling...   │
│ ▓▓▓▓awoke▓▓▓▓▓▓ │    preview │                 │
│ ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓ │              │                 │
│                 │              │                 │
│   ─── 46% ───   │              │   ─── 52% ───   │
│   Gliding...    │              │   [Caught!]     │
└─────────────────┘              └─────────────────┘

BUTTONS:
Short ←/→  = Line by line (precise)
Hold ←/→   = Charge momentum (visual feedback)
Release    = Glide (decelerates smoothly)
CONFIRM    = Catch/stop immediately
VOL UP/DN  = Adjust "friction" (slow/medium/fast glide)
```

### Speed Visualization

```
SLOW FRICTION                    FAST FRICTION
┌─────────────────┐              ┌─────────────────┐
│                 │              │                 │
│   Chapter 7     │              │   Chapter 7     │
│                 │              │                 │
│   The sun rose  │              │   The sun rose  │
│   slowly over   │              │   slowly over   │
│   the eastern   │              │   the eastern   │
│   hills...      │              │   hills...      │
│                 │              │                 │
│                 │              │                 │
│                 │              │                 │
│   ─── 45% ───   │              │   ─── 45% ───   │
│  ◀─── SLOW ───▶ │              │  ◀── FAST ──▶   │
└─────────────────┘              └─────────────────┘
    Decay: 3 pages                 Decay: 20 pages
    Smooth stop                    Long glide
```

### Settings

```
PHYSICS SETTINGS
┌─────────────────┐
│  Scrolling      │
│  ───────────────│
│                 │
│  Friction:      │
│  [▓▓▓▓▓░░░░░]  │
│   Low  [Med] High│
│                 │
│  Acceleration:  │
│  [▓▓▓▓▓▓▓░░░]  │
│   Slow  [Med] Fast│
│                 │
│  Preview: ON    │
│  Show ghost text│
│                 │
├─────────────────┤
│◀Back│▶Adj│Save │
└─────────────────┘
```

---

## 5. **TIME-TRAVEL HISTORY STACK**

**The Concept:** 3D stack visualization of reading history. Navigate time, not just pages.

### History Views

```
READING (Present)                ENTER HISTORY MODE
┌─────────────────┐              ┌─────────────────┐
│                 │              │                 │
│   Chapter 7     │              │   ≋≋ History ≋≋ │
│                 │              │                 │
│   The sun rose  │              │  ┌──┐ ┌──┐ ┌──┐ │
│   slowly over   │              │  │45│ │46│ │47│ │
│   the eastern   │              │  └──┘ └──┘ └──┘ │
│   hills...      │              │                 │
│                 │              │  Now at: 14:23  │
│                 │              │                 │
│   ─── 45% ───   │              │  [←] Go back    │
│                 │              │  [→] Forward    │
└─────────────────┘              └─────────────────┘
                                  [BACK] Exit history

HISTORY: SCROLL BACK             HISTORY: ZOOMED IN
┌─────────────────┐              ┌─────────────────┐
│  ≋≋ History ≋≋  │              │  Page 46        │
│                 │              │  @ 14:23        │
│     ┌──┐        │              │                 │
│  ┌──┐│47│┌──┐   │              │  ┌──────────┐   │
│  │45││◄─┤│49│   │◄── Current   │  │ The sun  │   │
│  └──┘└──┘└──┘   │              │  │ rose...  │   │
│                 │              │  │          │   │
│  12:30 14:23    │              │  └──────────┘   │
│   46%   47%     │              │                 │
│                 │              │  Time: 5 min    │
│ [←] Back  [→] Fwd│             │  [←] │ [→] │ [Jump]│
└─────────────────┘              └─────────────────┘
```

### Deep History

```
DAY VIEW (Zoom out)              SESSION TIMELINE
┌─────────────────┐              ┌─────────────────┐
│  ≋≋ Jan 14 ≋≋   │              │  ≋≋ Today ≋≋    │
│                 │              │                 │
│ Morning:        │              │  War & Peace    │
│ ████░░░░░░ 28%  │              │  ████████░░ 67% │
│                 │              │  45 min         │
│ Afternoon:      │              │                 │
│ ░░░░░░░░░░  0%  │              │  Moby Dick      │
│                 │              │  ██░░░░░░░░ 12% │
│ Evening:        │              │  12 min         │
│ ██████░░░░ 54%  │              │                 │
│                 │              │  Articles       │
│ Books: War&Peace│              │  █░░░░░░░░░  5% │
│       Moby Dick │              │  3 min          │
│ [←] Week │ Day [→]│            │ [←] Day │ Week [→]│
└─────────────────┘              └─────────────────┘

JUMP TO BOOKMARK                 COMPARE VERSIONS
┌─────────────────┐              ┌─────────────────┐
│  ≋ Bookmarks ≋  │              │  ≋ Versions ≋   │
│                 │              │                 │
│ ★ Page 247      │              │  [Current]      │
│   "Important    │              │  vs             │
│    passage"     │              │  [Yesterday]    │
│   Jan 12, 14:23 │              │                 │
│                 │              │  Changes: 3     │
│ ★ Chapter 3     │              │  +2 paragraphs  │
│   Opening       │              │  -1 note        │
│   Jan 10, 09:15 │              │                 │
│                 │              │  [View Diff]    │
│ ★ Page 89       │              │                 │
│   Character     │              │ [←] │ [→] │ [Merge]│
└─────────────────┘              └─────────────────┘
```

---

## Button Chord Cheat Sheet

```
┌─────────────────────────────────────────────────────┐
│               UNIVERSAL CHORD GUIDE                 │
├─────────────────────────────────────────────────────┤
│                                                     │
│  [LEFT] + [RIGHT]         = Screenshot              │
│  [CONFIRM] + [BACK]       = Home                    │
│  [VOL UP] + [VOL DOWN]    = Mute / Settings toggle  │
│  [CONFIRM] + [LEFT]       = Radial menu (left side) │
│  [CONFIRM] + [RIGHT]      = Radial menu (right side)│
│  [LEFT] + [VOL UP]        = Font size +             │
│  [RIGHT] + [VOL DOWN]     = Font size -             │
│  [BACK] + [VOL UP]        = Brightness +            │
│  [BACK] + [VOL DOWN]      = Brightness -            │
│  Hold [BACK] 1s           = History mode            │
│  Hold [CONFIRM] 1s        = Tandem mode toggle      │
│                                                     │
└─────────────────────────────────────────────────────┘
```

---

## Implementation Priority

1. **Contextual HUD** - Immediate quality boost, easy to implement
2. **Radial Menu** - Differentiating feature, great for power users
3. **Tandem Mode** - Unique to tall screen form factor
4. **Momentum Scrolling** - Delightful physics feel
5. **Time Travel** - Advanced feature for v2.0

Each paradigm can be a "Reading Mode" users switch between:
- **Focus Mode:** Zen + HUD
- **Power Mode:** Radial + Tandem
- **Casual Mode:** Momentum + Time Travel
