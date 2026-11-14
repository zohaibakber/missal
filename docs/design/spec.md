# Missal native UI: design direction and build spec

Reference mockups (1280x800 at 2x) are in `design/mockups/`. Each PNG has an HTML source next to it, built from `missal.css` and `chrome.js`. The audit is in `design/audit.md`.

All sizes are logical pixels at 1x. "Urdu" means text rendered with the Jameel Noori Nastaleeq face through `missal_text::label` or `missal_text::TextInput`. GPUI's own text is used only for Latin chrome.

## 0. Direction in one paragraph

Missal should feel like a quiet, fast writing desk.

- **One header bar, nothing else.** The whole window has a single 44 px header holding:
  - a menu icon (all navigation lives in its dropdown);
  - back and forward;
  - the name of what you are looking at;
  - the two or three actions that matter on that screen, as icons;
  - a search icon.

  There is no sidebar, no wordmark, no breadcrumbs and no second page header.
- **The FIR is the centre of gravity.** The list exists to get you into one. The workspace puts the document page in the middle, with the FIR's documents on the left. On the right is a panel that answers "what still needs filling in", so the next action is always visible.
- **Type and spacing make the hierarchy.** Surfaces are flat and separated by single hairlines. Shadows are used only for things that float. Primary actions are near-black ink. There is one accent colour (blue), and it appears only on:
  - focus;
  - selection;
  - links;
  - filled-field tint.
- **Amber is reserved for "empty field".** That is the one warning that matters before printing.
- **Interactions respond on the same frame.** Only floating layers animate, for 120–180 ms with ease-out, and nothing animates under reduced motion.
- **Chrome is English and LTR; content regions are Urdu and RTL.** The rules in section 3 decide every boundary between them, so no component improvises.

## 1. Design system

### 1.1 Colour tokens

This replaces the whole `Palette` in `crates/missal-app/src/theme.rs`; nothing of the old palette is kept. Values are sRGB hex, derived in OKLCH by `design/tools/tokens.py`, which also prints every contrast ratio below. Rerun it after any change.

**What I took from coss ui, and what I did differently.** coss.com was not reachable from this session: the proxy refused the host and the repository was not attachable. I studied coss's token layer through `@coss-svelte/theme` (npm), which describes itself as the translation of coss's product UI language into CSS variables. I only read its `tokens.css` and component CSS.

Taken from it:

1. **Neutral-first.** coss's primary is near-black ink, not a brand colour. Colour is kept for meaning. Missal does the same: the primary button is `action` (ink), and the blue `accent` only marks focus, selection, links and filled fields.
2. **Hover, pressed, borders and inputs are all one ink at small opacities** (coss uses 4%, 8% and 10% black, or white in dark). Every grey therefore belongs to the same family and layers predictably. Missal's `line`, `hover`, `pressed` and `line_strong` are `ink` at 10%, 4.5%, 8% and 47% (white at 8.5%, 5%, 9% and 34.5% in dark). They are flattened to opaque hex because GPUI blends per element, and opaque values keep contrast predictable.
3. **Semantic pairs.** A mid-tone base fills or underlines, a darker (light theme) or lighter (dark theme) `_text` shade carries words, and a faint `_wash` holds tinted surfaces. Missal has `danger`, `danger_text`, `danger_wash` and `notice`, `notice_text`, `notice_wash`.
4. **Dark surfaces layer by lightness, not by shadow** (coss lifts card and popover by mixing in 2–4% white). Missal's dark `raised` is 4 L-points above `surface`, and `sunken` is 4 below.
5. **Quiet shadows:** a 1 px, 5% contact shadow on buttons and inputs, and one large soft shadow for overlays. The focus ring is a 3 px halo at about 22% plus a solid ring colour on the edge.
6. **A strong ease-out curve for entrances** (coss uses `cubic-bezier(0.23, 1, 0.32, 1)`). Missal uses `ease_out_quint` from gpui, which is close to it.

Done differently, on purpose:

1. **Warm neutrals instead of pure grey** (OKLCH hue 70–88, chroma ≤ 0.01). Officers read long Nastaleeq documents; a faintly warm surface and an off-white `paper` (`#fefdf8`) read as paper and tire the eye less. coss's neutrals are zero-chroma.
2. **Stronger input edges.** coss's 10% input border is about 1.3:1. Missal's `line_strong` is at least 3.0:1 on `surface` (WCAG 1.4.11), because the users are not tech-savvy and must see where to type.
3. **A coloured focus ring.** coss's ring is neutral grey. Missal's is `accent` (6.3:1 / 7.3:1), because the app is keyboard-first and the ring is the only blue on most screens, so it is unmistakable.
4. **FIR status colours designed for colour-blind users.**
   - The hues follow the Okabe–Ito idea (orange, blue, magenta), but they are separated mainly by **lightness**.
   - "Closed" is **neutral grey**: the case is finished, and a grey dot reads as "done" without competing.
   - The pairwise OKLab distance is at least 12 for normal, deuteranope and protanope vision in both themes (Machado 2009 simulation in `tokens.py`).
   - The only weak pair is open against challan under tritanopia in dark (3.9). Every status is always shown with its Urdu label, so colour is never the only cue.
5. **No translucency at runtime and no gradients, glass or noise.** These are not portable to GPUI's per-element blending, and they add nothing here.
6. **Tighter radii** (6/8/12, not coss's 10 px base). Missal is a dense desktop tool with 28 px controls, where a 10 px radius makes buttons look like pills.

| Token | Light | Dark | Use | gpui-kit `ThemeColor` fields it feeds |
|---|---|---|---|---|
| `surface` | `#fefdfc` | `#161513` | Window, panels, inputs, menus in light | `background`, `table`, `table_even`, `table_head`, `list`, `list_even`, `list_head`, `title_bar`, `status_bar`, `tab_active`, `sidebar` |
| `raised` | `#ffffff` | `#201e1c` | Popovers, menus, dialogs, palette, toasts (dark lifts by lightness) | `popover` |
| `sunken` | `#f0efec` | `#0d0c0a` | Canvas behind document pages | (app-only, `Palette::sunken`) |
| `paper` | `#fefdf8` | `#201e1b` | Editor page (warm paper; dark paper in dark) | (missal-editor reads it via the app `FieldContext`, not gpui-kit) |
| `ink` | `#1e1b18` | `#edebe7` | Primary text, active icons, editor ink | `foreground`, `popover_foreground`, `accent_foreground`, `secondary_foreground`, `caret` |
| `ink_muted` | `#5f5a55` | `#aeaaa5` | Secondary text, idle icons | `muted_foreground`, `table_head_foreground`, `tab_foreground` |
| `ink_faint` | `#77726d` | `#8c8884` | Placeholders, counts, meta, disabled text | (app-only; `TextInput` placeholder colour) |
| `line` | `#e7e7e5` | `#2a2927` | Hairlines between regions and rows (decorative) | `border`, `table_row_border`, `title_bar_border`, `window_border` |
| `line_strong` | `#959391` | `#666564` | Input, checkbox and outline-button edges | `input` |
| `hover` | `#f4f3f2` | `#22201f` | Hover fill; keycap fill; segmented track | `accent`, `list_hover`, `table_hover`, `secondary`, `muted`, `skeleton` |
| `pressed` | `#ecebea` | `#2b2a28` | Pressed fill; toggled icon button; active tab item | `secondary_active`, `list_active`, `table_active` |
| `action` | `#1e1b18` | `#edebe7` | Primary button fill, checkbox on | `primary`, `button_primary`, `switch` (on) |
| `action_hover` | `#36322e` | `#d3d1cd` | Primary hover | `primary_hover`, `button_primary_hover` |
| `on_action` | `#fbfaf8` | `#181613` | Text and icons on `action` | `primary_foreground`, `button_primary_foreground` |
| `accent` | `#2b5bb7` | `#80a3f0` | Focus ring, links, match highlight text, field-name chips | `ring`, `link`, `drag_border`, `list_active_border`, `table_active_border` |
| `accent_wash` | `#e9edf5` | `#242b3d` | Selected row and item fill; text selection uses accent at 28% / 40% | `selection` (accent at 28% light / 40% dark), `drop_target` |
| `field_wash` | `#eceff3` | `#2f333d` | Filled-field tint in the editor (on `paper`) | (missal-editor `field_background`) |
| `danger` | `#c32a28` | `#c44039` | Destructive button fill | `danger`, `button_danger` |
| `danger_text` | `#b32322` | `#f47b74` | Destructive text, error text, danger-outline edge | (app-only) |
| `danger_wash` | `#f9eceb` | `#351d1a` | Error banner fill | (app-only) |
| `notice` | `#c47a1c` | `#eba941` | Empty-field dashed underline, warning icon | `warning` |
| `notice_text` | `#88520c` | `#edbb64` | Empty-field text, warning copy | (app-only) |
| `notice_wash` | `#fff2d6` | `#382c1a` | Empty-field chip and warning box fill | (app-only) |
| `st_open` | `#d37812` | `#f2a635` | Status dot: زیر التوا | (app-only) |
| `st_investigating` | `#0077bd` | `#3883d1` | Status dot: زیر تفتیش | `info` |
| `st_challan` | `#732679` | `#f5a0c5` | Status dot: چالان جمع | (app-only) |
| `st_closed` | `#888682` | `#6e6b68` | Status dot: بند (neutral: the case is finished) | (app-only) |

Not tokens (fixed): print preview paper `#ffffff` with ink `#111111` in both themes; scrim `rgba(20,17,14,0.28)` light, `rgba(0,0,0,0.55)` dark.

**Contrast** (WCAG 2.x ratios, from `tokens.py`; AA needs 4.5 for text, 3.0 for UI edges and graphics):

| Foreground | on surface | raised | sunken | hover | pressed | accent_wash | Light / dark |
|---|---|---|---|---|---|---|---|
| `ink` | 16.87 / 15.34 | 17.12 / 13.99 | 14.88 / 16.39 | 15.45 / 13.58 | 14.39 / 12.06 | 14.59 / 11.78 | AA everywhere |
| `ink_muted` | 6.74 / 7.93 | 6.84 / 7.23 | 5.94 / 8.47 | 6.17 / 7.02 | 5.75 / 6.23 | 5.83 / 6.09 | AA everywhere |
| `ink_faint` | 4.69 / 5.22 | 4.76 / 4.76 | 4.14 / 5.58 | 4.29 / 4.62 | 4.00 / 4.11 | 4.06 / 4.01 | AA on surface and raised. **Only on those two:** never put `ink_faint` text on `sunken`, `hover`, `pressed` or `accent_wash` (use `ink_muted` there) |
| `accent` (as text) | 6.33 / 7.30 | 6.42 / 6.66 | 5.58 / 7.80 | 5.79 / 6.46 | 5.40 / 5.74 | 5.47 / 5.61 | AA |
| `danger_text` | 6.49 / 6.91 | 6.59 / 6.30 | 5.72 / 7.39 | 5.94 / 6.12 | 5.54 / 5.43 | 5.61 / 5.31 | AA |
| `notice_text` | 6.35 / 10.37 | 6.44 / 9.46 | 5.60 / 11.08 | 5.81 / 9.18 | 5.42 / 8.15 | 5.49 / 7.97 | AA |

Pairs:

- `on_action` on `action`: 16.39 / 15.19 (on `action_hover` 12.16 / 11.83).
- White on `danger`: 5.70 / 5.06.
- `ink` on `paper`: 16.78 / 13.99; on `field_wash`: 14.83 / 10.60.
- `notice_text` on `notice_wash`: 5.81 / 7.68.
- `danger_text` on `danger_wash`: 5.73 / 5.92.

Non-text (needs 3.0 on `surface`):

| Token | Light | Dark |
|---|---|---|
| `line_strong` | 3.02 | 3.17 |
| `accent` and `ring` | 6.33 | 7.30 |
| `notice` | 3.36 | 8.97 |
| `st_open` | 3.20 | 8.92 |
| `st_investigating` | 4.74 | 4.63 |
| `st_challan` | 9.06 | 9.32 |
| `st_closed` | 3.59 | 3.46 |
| `line` (decorative only; separates regions alongside spacing, never the only edge of a control) | 1.22 | 1.25 |

Status dots on `hover` and `accent_wash` stay above 2.6, and each dot always sits next to its label.

Rules:

- **Status is an 8 px dot plus the Urdu label in `ink`.** Never colour the text. Drop the custom half-filled status glyphs (`ui/status.rs`).
- **Hover, selected and keyboard-focused rows are visibly different.** Hover is `hover`. Selected is `accent_wash`. Focus is `hover` plus a 2 px `accent` bar on the row's start edge.
- **Scrollbar thumb:** `ink_muted` at 40% idle and 70% on hover; 6 px wide, 3 px inset.
- **gpui-kit mapping:** `apply_palette` sets every field in the last column of the token table. The rest:
  - `title_bar` is `surface`.
  - `overlay` is the scrim.
  - `scrollbar` is transparent; `scrollbar_thumb` and `scrollbar_thumb_hover` are as above.
  - `switch_thumb` and `slider_thumb` are `surface`.
  - `progress_bar` and `slider_bar` are `action`.
  - `success` is `st_closed`'s light shade for icons only; prefer `notice` and `danger`.
  - `theme.radius = 6`, `theme.radius_lg = 8`, `theme.shadow = true` (so `popover_style` gets its ring plus shadow), `theme.font_size = 14`.
  - `Palette` keeps the app-only tokens (`sunken`, `paper`, `ink_faint`, the `_text` and `_wash` shades, the four statuses) as fields for screens to use.

**Shadows:**

| Name | Light | Dark | Used on |
|---|---|---|---|
| `shadow_sm` | `0 1px 2px` ink at 5% | black at 35% | Primary, outline and danger buttons; inputs |
| `shadow_md` | 1 px ring of ink at 8%, plus `0 8px 24px` ink at 10% | ring of white at 8%, plus `0 8px 24px` black at 45% | Menus, popovers, toasts |
| `shadow_lg` | 1 px ring of ink at 8%, plus `0 16px 48px` ink at 14% | ring of white at 8%, plus `0 16px 48px` black at 55% | Dialogs, palette |
| Paper | 1 px `line` ring plus `shadow_sm` | | Document page |

GPUI's `BoxShadow` supports offset, blur, spread and colour; a ring is a zero-blur shadow with 1 px spread.

### 1.2 Type

**Latin (Inter, UI chrome).** Tabular figures for every number (Inter `tnum`; see risks).

| Role | Size / line | Weight | Where |
|---|---|---|---|
| `caption` | 11 / 16 | 500 | Keycaps, uppercase-free micro labels |
| `meta` | 12 / 16 | 400–500 | Counts, hints, panel section labels, "Saved" |
| `small` | 13 / 20 | 400–500 | Buttons, menus, tabs, table dates, toolbar text |
| `body` | 14 / 20 | 400 | Default text, dialog body, FIR number in rows (500) |
| `title` | 15 / 20 | 600 | Header bar title |
| `section` | 16 / 24 | 600 | Settings section titles, dialog titles, empty-state titles at 14/500 |

There is no 20 px page title any more: the header bar carries the title.

**Urdu (Jameel Noori Nastaleeq).** The face's real extent is about 2.04 em: yMax 2718 and yMin −1460 at 2048 upem. `hhea` covers only 1.40 em, so the normal line height clips stacked letters. These sizes and line heights keep every glyph inside its box:

| Role | Size | Line height | Minimum box for a single line | Where |
|---|---|---|---|---|
| `ur_label` | 14 | 26 (`LineHeight::Px(26)`) | 26 | Field labels in panels, table headers, chips |
| `ur_body` | 16 | 30 | 28 | Table cells, lists, inputs, values |
| `ur_input_label` | 15 | 28 | 28 | Form field labels |
| `ur_title` | 18 | 34 | 32 | Header bar offence, form section headings |
| `ur_display` | 22 | 40 | 38 | Unused in chrome; document titles come from templates |

Rules:

- **Nothing in Urdu goes below 14 px.** Today's 12 and 13 px Urdu labels (details rail, table head, menus) all move up.
- **Never set a weight on Urdu.** The face has one weight, and synthetic bold (`missal-text/src/synth.rs`) smears Nastaleeq. Use size and colour for hierarchy.
- **Never clip Urdu vertically.** Containers holding Urdu must not use `overflow_hidden` on the vertical axis. Truncation is horizontal only (`label(..).truncate()`).
- **Baseline match.** In a row mixing Latin 14 px and Urdu 16 px, centring both boxes puts the Urdu baseline about 3 px lower. Give the Latin cell `pt(px(3.))`, or wrap the Urdu label with `mt(px(-2.))` as the mockups do. Verify each table and list in the screenshot harness at 2x: the baselines must match within 1 px.
- **Multi-line Urdu inputs** (the body text) use line height 30 at 16 px. `TextInput` currently derives line height from the font's natural height (`row_height`), which is about 22 at 16 px. Add `TextInput::line_height(LineHeight)` in missal-text and use 30 for Urdu inputs. This is foundation work (WP0).

### 1.3 Spacing, radii, borders, elevation

- **Spacing scale:** 2, 4, 6, 8, 12, 16, 20, 24, 32, 40. Panel inner padding is 16. Page gutter is 24. Gap between controls in a group is 4; between groups, 12.
- **Radii:**
  - 4: keycaps, checkboxes, menu items, field chips (3).
  - 6: buttons, inputs, list items, segmented items.
  - 8: popovers, menus, toasts, segmented track, warning boxes.
  - 12: dialogs, command palette.
  - 2: document paper.
  - 12 (pill): the status pill in the header.

  Nothing else. Delete the hardcoded 5, 7 and 10.
- **Borders:** a 1 px `line` hairline separates regions: header from body, rail from canvas, and table rows. Do not nest bordered boxes; no card inside a card. Inputs use a 1 px `line_strong` outline. Selected or active items use fill, never a border.
- **Elevation:** shadows only on floating layers and controls, using the three shadow tokens in 1.1 (`shadow_sm` on buttons and inputs, `shadow_md` on popovers, menus and toasts, `shadow_lg` on dialogs and palette). In dark mode, `raised` surfaces are also lighter, so layering reads even where a shadow is invisible.

### 1.4 Controls

| Control | Height | Padding | Notes |
|---|---|---|---|
| Header bar | 44 | 8 left | Bottom hairline |
| Icon button | 28 (24 in dense rows and inputs) | — | 16 px icon (14 in 24 px buttons). Idle `ink_muted`, hover `hover` fill and `ink`, toggled-on `pressed` fill and `ink`, disabled 40% opacity |
| Button `sm` | 28 | 10 | Used in the header and panels |
| Button `md` | 32 | 12 | Dialogs and form footer |
| Primary | `action` fill, `on_action` text, `shadow_sm` | | One per screen at most |
| Outline | 1 px `line_strong` at 70%, `surface` fill | | Secondary in dialogs |
| Ghost | No fill; hover `hover` | | Cancel, Clear |
| Danger | `danger` fill (dialogs only) or `danger` outline (in-page) | | |
| Latin input | 32 | 10 | Single line |
| Urdu input | 36 | 10 | Every input that can hold Urdu, even when it also takes digits, so form rows line up |
| Checkbox | 16 | — | Radius 4. On: `action` fill, `on_action` `check` icon at 12 px |
| Segmented control | Track 28 (2 px inset), items 24 | 8–10 | Track `hover`. On item: `surface` plus `shadow_sm`, `ink` |
| Tab (panel) | 40 | — | 13/500. On: `ink` with a 2 px `ink` underline. Off: `ink_muted` |
| Status filter item | 28 | 10 | Dot, Urdu label, count in `ink_faint`. On: `pressed` fill. Zero counts at 50% opacity |
| Table header row | 36 | — | Urdu 14 `ink_muted`. Only the sorted column shows an arrow |
| Table row | 44 | Cells 8 px horizontal | |
| List item (rails) | 40 (44 with a meta line) | 10 | |
| Menu item | 32 | 8 | 14 px icon, 13 px text, shortcut keycaps at the end |
| Keycap | 18 | 4 | Radius 4, 1 px `line`, 11/500 `ink_muted`. Transparent fill inside buttons |

Icons: Lucide, as shipped by gpui-kit (`gpui_kit::assets::IconName`; all 1830 Lucide glyphs are compiled in). Sizes are 16 in buttons, 14 in menus, inputs and 24 px buttons, and 12 inside checkboxes and keycaps. Stroke is the Lucide default (2 on a 24 grid, which renders 1.33 px at 16). Use one icon per concept, from the table in section 4.

### 1.5 Focus

- **Keyboard focus** shows a 2 px `ring` outside the element with a 2 px `surface` gap: `focus_ring_style` from gpui-kit, already used by `TextInput`.
- **Inputs** show the ring on any focus. Everything else shows it on keyboard focus only.
- **Rows and list items do not draw a ring.** They show the 2 px start-edge bar plus `hover` fill, because a ring around a full-width row is noisy.
- **Leave room for rings.** No container with `overflow_hidden` may sit within 4 px of a focusable child; the ring would be clipped.
- **Focus order:** header (menu, back, forward, title actions, search), then the screen's main region in reading order. In RTL regions, reading order runs right to left.

### 1.6 Motion

Tokens (gpui `Animation`):

- `motion_fast`: 120 ms, `ease_out_cubic` (`1 - (1 - t)^3`, as gpui-kit's popover uses).
- `motion_base`: 180 ms, the same easing.
- `motion_exit`: 100 ms, linear opacity only.

| What | Animates | How |
|---|---|---|
| Menus, popovers, dropdowns, tooltips' first show | Yes | `motion_fast`: opacity 0→1 and translate-y −4→0 (popovers keep gpui-kit's 150 ms; do not fight it) |
| Command palette, dialogs | Yes | `motion_base`: opacity plus scale 0.98→1, scrim opacity 0→1. Exit: `motion_exit` |
| Toasts | Yes | `motion_base`: translate-y 8→0 plus opacity. Exit: `motion_exit`. gpui-kit's notification uses 400 ms with no hook to change it, so WP0 builds a small toast host (see risks) |
| Insert-field and add-document popovers | Yes | As menus |
| Fields panel open and close | **No** | Instant. Animating width reflows the page and Urdu shaping every frame |
| Navigation between screens, document switching, tab switching, filtering, sorting, selection, hover, focus, zoom, theme change | **No** | Must feel instant |
| Save state | **No** | Text swap "Saving…" → "Saved" with no spinner unless a save takes over 400 ms; then show `loader-circle` spinning (gpui-kit `Spinner`) |
| Skeletons | **No shimmer** | Static `hover`-coloured bars, shown only if loading takes over 150 ms |

Reduced motion:

- gpui's `with_animation` already renders end states when `cx.reduce_motion()` is set.
- The app never sets it. WP0 reads the OS preference at startup and when the window regains focus, then calls `cx.set_reduce_motion(..)`:
  - Windows: `SPI_GETCLIENTAREAANIMATION`, through the `windows` crate already in the tree for packaging, or `platform.rs`.
  - Linux: `gsettings get org.gnome.desktop.interface enable-animations`.
- With reduced motion on, nothing moves. Opacity changes are instant too.

## 2. Information architecture

### 2.1 Shell

- **Persistent elements are removed:**
  - the sidebar (FIRs, Templates, Recent, Shortcuts, Settings, New FIR);
  - breadcrumbs;
  - the title-bar search field;
  - the "Missal" name.
- **The header bar (44 px) is the only chrome.** Left to right:
  1. `menu` icon button: the navigation dropdown (2.2).
  2. `arrow-left` and `arrow-right`: history (Alt+Left, Alt+Right).
  3. Title region: the current object's identity (2.3).
  4. Flexible space. The header is the window drag region; buttons are not.
  5. Screen actions: icon buttons, plus at most one text primary button.
  6. `search` icon button: opens the command palette (Ctrl+K, and `/` outside the FIR list).
  7. On Windows, the caption buttons (gpui-kit `TitleBar`, 3 x 46 px). Reserve 138 px on the right; the actions must never run under them.
- **Nothing in the shell persists on screen.** Each screen owns its own panels. The only persistent panels are in context: the document rail inside a FIR, and the template list in Templates. Each is the list of things you switch between *within* that object, which justifies keeping it on screen.
- **Recent FIRs** move to the command palette's empty state (2.4). They are recorded properly: add `opened_at` to a small recent list in `prefs`, 10 entries, updated on `navigate(Route::Fir)`.

### 2.2 Navigation menu (mockup `fir-list-menu-light.png`)

The `menu` icon opens a 248 px dropdown anchored under it. Items, with shortcuts shown as keycaps:

1. FIRs `G H` (`folder`)
2. Templates `G T` (`file-text`)
3. Settings `G S` (`settings`)
4. separator
5. New FIR `N` (`plus`)
6. Import Word file `Ctrl O` (`file-up`), which goes to Templates and opens the picker
7. separator
8. Keyboard shortcuts `?` (`keyboard`)
9. Theme, with the current value at the end (`sun-moon`). It opens a submenu of System, Light and Dark, each with a check on the current one.
10. About Missal (`info`): version, data folder path, "Check for updates".

The current destination's row has `hover` fill. The menu works with arrow keys, Enter and Esc. It opens on click, or on `Alt` + `Space`… no: on `F10`, the Windows convention for the menu bar.

### 2.3 Header title per screen

| Screen | Title | Meta (after the title, `ink_muted` 13) | Actions (right) |
|---|---|---|---|
| FIR list | `FIRs` | Count, `90` or `4 of 90` when filtered | **New FIR** (text primary, `plus`, keycap N). In selection mode it is replaced by: "2 selected", Clear (ghost), Delete 2 FIRs (`danger` outline, `trash`) |
| FIR workspace | `FIR 1/2021` (LTR) plus a status pill (dot and Urdu label, 24 px, `line` outline) | Offence as `ur_title` in `ink_muted`, truncated | Save state text ("Saved", or "Unsaved", or "Saving…"), `pencil` Edit details, `printer` Print, `panel-right` Fields panel (toggled on), `ellipsis` More |
| New FIR or edit FIR | `New FIR`, or `Edit FIR 1/2021` | "Fields marked * are required" (12, `ink_faint`) | none; actions are in the form footer |
| Templates, nothing selected | `Templates` | — | none |
| Template | Template name, `ur_title` in `ink` | — | Save state, `pencil` Rename, `printer` Preview, `ellipsis` More (Replace from Word file…, Duplicate, Delete) |
| Import review | `Import: <file name>` | "23 fields found, 2 not recognised" | Discard (ghost text), **Save template** (primary text) |
| Settings | `Settings` | — | none |
| Print | `Print FIR 1/2021` | `2 documents · 3 pages` | `file-down` Save as PDF, **Print** (primary text, keycap Ctrl P) |

### 2.4 Command palette (mockup `palette-light.png`)

- 600 px wide, 88 px from the top of the window, radius 12, dialog shadow, scrim.
- **Input row** (52 px): `search` icon at the left, a bare input, and an `Esc` keycap at the right. The input direction is `AutoLtr` (see 3.4). There is no border around the input; the palette is the box.
- **Empty query:** "Recent FIRs" (up to 6), then "Actions": New FIR, New template, Import Word file, Keyboard shortcuts, Theme…, Go to Templates, Go to Settings.
- **With a query:**
  - FIRs whose number, offence or accused match, with the matching substring in `accent` at 600.
  - Then templates.
  - Then commands.
  - Groups with no hits are hidden.
- **FIR row** (40 px): status dot, then the FIR number (LTR, 92 px column, 500), then the offence as `ur_body`, left-aligned (see 3.5). The highlighted row ends with "Open ↵".
- **Template row:** `file-text` icon, then the name as `ur_body`, left-aligned.
- **Footer** (36 px): `↑↓` Move, `↵` Open, `Esc` Close; the result count at the right.
- **Keyboard:**
  - Up and Down move.
  - Enter runs.
  - Esc closes, or clears the query first if it is not empty.
  - Typing always goes to the input.
- Theme becomes one command with a submenu step (Enter, then pick).

### 2.5 Core loop

1. **Find or create an FIR.** Open the FIR list, or press Ctrl+K and type a number or a name. N creates an FIR.
2. **Fill the details.** Use the New FIR form page. Ctrl+S creates the FIR and lands in its workspace.
3. **Add documents.** In an FIR with no documents, the empty workspace has one primary button, "Add document A". The add-document popover lets you tick several templates; Ctrl+Enter adds them.
4. **Write and fill.** The page sits in the centre. The Fields panel on the right lists **empty fields first, with inputs right there**: custom values save to this FIR, global values save to every FIR. When nothing is empty, the panel says so and shows the filled values.
5. **Print.** The printer icon or Ctrl+P opens the Print screen. It shows which documents go out, with checkboxes, a page count per document, and an amber warning listing any empty fields, with "Fill them in" taking you back to the first one.

### 2.6 What moved or was dropped

| Was | Now | Why |
|---|---|---|
| Sidebar: FIRs, Templates, Settings | Navigation menu (header `menu` icon), G-shortcuts, palette | Owner requirement; frees 176–224 px for the page |
| Sidebar: New FIR | List header primary, nav menu, palette, N | It was duplicated in three places |
| Sidebar: Recent | Palette empty state, with real opened-at data | It was not recent (audit, priority 6) |
| Sidebar: Shortcuts | Nav menu, `?`, palette | |
| Sidebar collapse (Ctrl+B, `prefs.sidebar_open`) | **Dropped.** Ctrl+B is freed for the editor's Bold | There is no sidebar. Remove the `sidebar_open` pref on load and ignore it |
| Breadcrumbs | Header title; Back (Alt+Left) | They repeated the location |
| Title-bar search field | `search` icon; Ctrl+K | Owner requirement |
| Multi-select status filter popover | Single-select status filter row with counts | One click, visible state, counts. Multi-select was rarely useful; `prefs.table.statuses` keeps its `Vec` but holds 0 or 1 entries |
| Status "Clear filter" | The "All" item | |
| Sort icon on every column | Arrow on the sorted column only; any header is still clickable | Less noise |
| Column menu could hide the FIR number | FIR number is always shown; menu items are Offence, Accused, Status, FIR date, Incident date, Arrest date | A row needs an identity |
| Edit sheet (`edit_sheet.rs`) | `Edit FIR` page using the same form as New FIR (`Route::EditFir(id)`) | One form; the sheet overflowed |
| Custom values sheet (`custom_values.rs`) | Fields panel (workspace right): custom fields used by the open document. Every custom field is still editable under "Case details → Custom values" | Fill values in context, without hiding the page |
| Details rail (always open) | Fields panel tab "Case details": read-only summary plus Edit | It is still reachable, and it no longer takes space from the page by default |
| "Include when printing" checkboxes in the document rail plus the "All will print" bar | Print screen document list | They decide printing, so they live where printing is decided. The state persists as today |
| Document rail ⋯ menu: Move up, Move down, Remove | The same items in the row's ⋯ menu, plus Alt+Shift+Up/Down to move | |
| "Show field names" in the workspace ⋯ menu | `Values | Names` segmented control in the editor toolbar (Ctrl+Shift+L kept) | It is a primary mode |
| Print preview as a modal | Print as a full screen (`Route::Print(..)`) with a document list | It needs room, and Back returns to where you were |
| Template list preview line | Dropped; the field count is shown instead | Every preview was the same header text |
| "Upload Word file" button | `file-up` icon in the template list header ("Import Word file"), the nav menu, and Ctrl+O | The wording was wrong, and the action was duplicated |
| Settings theme preview cards | Appearance section: a segmented control (System, Light, Dark); also in the nav menu | Too large for a setting changed once |
| Settings "FIR fields" chip wall | "Built-in fields" section: a two-column read-only RTL list, collapsed by default | Reference data should not be the loudest thing on the page |

Nothing the app does today becomes unreachable.

## 3. Direction and language rules

### 3.1 The shell's direction

- **The window is LTR.** Menus, buttons, tooltips, dialogs, toasts, settings copy, keyboard hints and the header are English, left-aligned, in Inter.
- **Content regions are RTL.** These are the FIR table body and headers, the document rail, the Fields panel lists, the template list, the FIR form, global and custom value lists, and the editor page. Inside them, everything is right-aligned and items read right to left.
- **Panels sit in LTR positions,** because the window is LTR: lists you switch between on the left, the inspector on the right, the canvas in the centre.
  - This matches every Windows app these officers already use.
  - It keeps one rule for all three screens.
  - Inside each panel, the content is RTL and hugs the panel's right edge.
- **Coherence at the boundary:**
  1. **Region edges are hairlines, never boxes.** A right-aligned Urdu list next to a left-aligned English header reads as two columns of one page, not as a clash.
  2. **Every RTL region has an English label above it at the left** (`meta` 12/500 `ink_muted`: "Documents", "Empty in this document", "Templates"). Its controls (add, count) sit on the same line. The eye lands on the left label, then on the content's right edge.
  3. **Anything in an RTL region that is an action** (Add accused, Add global value, Add document) is an English chrome button. It is placed at the region's *start* (right) edge for adders under RTL lists, and at the left for the region header's tools.
  4. **Numbers and dates are always LTR runs**, isolated, in Inter with tabular figures, even inside RTL text.

### 3.2 Digits

- **Store and display exactly what the user typed.** Never convert digits in content (offences such as `ڈکیتی ۳۹۲ ت پ` keep Urdu-Indic digits).
- **Chrome digits are Western:** counts, page numbers, zoom, "+1", dates in the table and header.
- **Dates are canonical `DD-MM-YYYY` in Western digits.** The date parser accepts Urdu-Indic (۰–۹) and Arabic-Indic (٠–٩) digits and `/` or `.` separators, and rewrites the field to canonical form on blur. Behaviour change: `CalendarDate` parse folding, done in WP2's form.
- **FIR number, IMEI, SIM, page number and zoom inputs** accept Urdu-Indic digits and fold them to Western on blur. The FIR number is matched folded already (`fold_for_search`).
- **Search folds digits on both sides** (already true). "379" finds `۳۷۹`.

### 3.3 Arrow keys and caret

- **Arrow keys move visually,** as `TextInput` does today (`move_visual`): Left goes left on screen, in both directions. This is how Windows edit controls behave with RTL text. Keep it.
- **Home and End are logical.** Home goes to the start of the line, which is the right edge in RTL.
- **Ctrl+Left and Ctrl+Right move by word, visually.**
- **When an input gains focus by keyboard, the caret goes to the logical end** (the left end in RTL), so the user can keep typing. Clicking places the caret at the click.

### 3.4 Inputs

`TextInput` direction options: `Ltr`, `Rtl`, `Auto` (empty → RTL, otherwise the first strong character decides), `AutoLtr` (empty → LTR). While the box is empty, `Auto` also uses the placeholder's first strong character. Two foundation additions are needed in missal-text (WP0):

- **`TextInput::bare()`.** No border, background or padding, so a wrapper `SearchField` or `Field` component can put an icon and a clear button *inside* the box. Today the box is drawn by `TextInput::render` and cannot hold children.
- **`TextInput::placeholder_direction(Direction)`.** The placeholder is laid out with its own direction, independent of the field's. An English placeholder in an RTL field then sits on the right without flipping the field.

Rules:

- **Use a stable direction per field.** `Auto` and `AutoLtr` resolve only from the first strong character, so the box can only flip when going from empty to its first letter. That is the moment the caret jumps sides, which is acceptable once, but only where the content is genuinely unpredictable.
- **Alignment is set explicitly** (`Align::Right` or `Align::Left`, not `Start` or `End`) for every Urdu-context field. Then an `Auto` field never jumps sides when its content direction changes; only the bidi order inside it changes.

| Input | Direction | Align | Icon side | Clear side | Placeholder (language, side) | Caret starts | Why |
|---|---|---|---|---|---|---|---|
| Command palette | `AutoLtr` | Follows direction (Left for LTR, Right for Urdu) | `search` left, outside the text box | `Esc` keycap right | "Search FIRs, templates, commands" (English, left) | Left | Queries are FIR numbers (most common), English commands or Urdu names; genuinely mixed. Empty stays LTR so the English placeholder and the first digit do not jump |
| FIR list search | `AutoLtr` | As above | `search` left | `x` right, shown when there is text; `/` keycap while empty and unfocused | "FIR no., offence or name" (English, left) | Left; jumps right if the first letter is Urdu | Same mix; number lookups dominate. Mockup `fir-list-search-light.png` shows Urdu typed (right-aligned against the clear button) |
| Template list search | `Rtl` | Right | `search` right (the start of an RTL box) | `x` left | "تلاش" (Urdu, right); the list is all Urdu | Right | Every template name begins with Urdu, so the field never needs to flip. Mockup `templates-light.png` |
| Add-document search (popover) | `Rtl` | Right | `search` right | `Esc` keycap left | "تلاش" (right) | Right | Same as templates. Mockup `components-light.png` |
| Insert-field search (editor popover) | `Rtl` | Right | `search` right | `x` left | "تلاش" (right) | Right | Field names are Urdu, some with Latin words (`SHO نام`, `Date FIR`); RTL base orders those correctly |
| Custom field filter (Fields panel and Case details) | `Rtl` | Right | `search` right | `x` left | "تلاش" (right) | Right | Urdu names |
| FIR form: FIR number | `Ltr` | Right | — | — | "e.g. 412/2026" (English, `ink_faint`, right) | Right edge; text grows leftward | Numeric with a slash; LTR order, aligned under the RTL label. The placeholder starts with "e.g." and is `ink_faint`, so it cannot be mistaken for a value |
| FIR form: dates (FIR, incident, arrest) | `Ltr` | Right | `calendar` icon button *inside* the box, at the left end | — | "DD-MM-YYYY" (Latin, `ink_faint`, right) | Right | Fixed format; Urdu-Indic digits accepted and folded on blur |
| FIR form: IMEI 1 and 2 | `Ltr` | Right | — | — | none | Right | 15 digits; LTR keeps digit order and selection sane. Shown with tabular figures |
| FIR form: SIM 1 and 2 | `Ltr` | Right | — | — | none | Right | Phone numbers `03001234567`, same reasoning |
| FIR form: offence, complainant, officer, writer, recovery, accused[i], witness[i], zimni[i] | `Rtl` | Right | — | `x` remove button for list entries, at the row's left end | none (labels are enough; no example values) | Right | Always Urdu prose; Latin and digits inside order correctly in an RTL base |
| FIR form: body (multiline) | `Rtl` | Right | — | — | none | Right | Long Urdu text; line height 30 |
| FIR form: status | Select, not text | Right | Dot right, chevrons `chevrons-up-down` left | — | — | — | Options are Urdu labels with dots, in an RTL list |
| Global value name | `Rtl` | Right | — | — | — | Right | Names are Urdu, sometimes with a Latin word (`SHO نام`) |
| Global value value | `Auto` | **Right** | — | — | "Empty" (English, `ink_faint`, right, via `placeholder_direction(Ltr)`) | Right | Values are usually Urdu, sometimes a number or English; `Auto` gets bidi order right and fixed Right alignment prevents a jump |
| Custom field name (Settings, add and rename) | `Rtl` | Right | — | — | "Field name" (English, right) | Right | Urdu names |
| Custom field value (Fields panel, Case details) | `Auto` | Right | — | `x` clear left when focused with text | "Empty" (right) | Right | Dates, cheque numbers, bank names: genuinely mixed |
| Template name (rename and new) | `Rtl` | Right | — | — | "Template name" (English, right) | Right | Names start in Urdu (`مکمل چالان 489F`) |
| Field markers (open and close) | `Ltr` | Center | — | — | — | — | 1–3 symbols, 56→48 px boxes |
| Print screen: page number | `Ltr` | Center | — | — | — | — | Number |
| Settings: anything English | `Ltr` | Left | — | — | — | Left | — |

### 3.5 Non-input surfaces

- **FIR table** (mockup `fir-list-*.png`):
  - The whole table is RTL, including the header row.
  - Column order from the right edge:
    1. selection checkbox, 36 px;
    2. FIR number, 108 px, LTR run right-aligned in its cell, 14/500;
    3. offence, flex 1.1;
    4. accused, flex 1: the first accused, plus `+N` in `ink_faint` Latin at its end, as `ur_body` in `ink_muted`;
    5. status, 132 px, dot then label;
    6. FIR date, 112 px, Latin 13 `ink_muted`, right-aligned, tabular;
    7. incident date and arrest date, hidden by default, same style;
    8. row actions `ellipsis`, 36 px, at the far left.
  - The checkbox is first because it belongs to the start of the row in RTL.
  - Actions sit at the end, where they do not interfere with reading.
  - Dates are right-aligned (the cell's start); fixed width plus tabular figures keep them in a straight column.
  - The keyboard-focus bar is on the **right** edge.
- **Workspace header:** LTR chrome. `FIR 1/2021` sits at the left, then the status pill (Urdu inside, left-aligned in LTR flow), then the offence (Urdu, left-aligned, truncated at its end).
- **Document rail:** RTL list.
  - Each row has the name at the right, `ur_body`, truncated at its left end.
  - A position number `1 2 3` in `ink_faint` Latin 12 is on the far right, before the name. It is not a checkbox.
  - The ⋯ menu shows at the far left on hover.
  - The active row has `accent_wash` fill.
  - A dirty dot (6 px `notice_text`) sits at the far left when the active document has unsaved changes.
- **Fields panel:**
  - The section labels are English at the left: "Empty in this document", "Filled".
  - Each empty-field item has the Urdu field name right-aligned, and a scope tag at the left ("Global" or "This FIR", 11/500 `ink_faint`).
  - Its input is below.
  - Filled items are an RTL two-column grid: label (`ur_label` `ink_muted`, 92 px column at the right), then value (`ur_body`, or Latin for numbers and dates, right-aligned, truncated).
- **Case details tab:** the same grid for every FIR field. Lists (accused, witnesses, zimni) are numbered `1 2` in `ink_faint` Latin at the right of each item. An "Edit details" button (text, `pencil`) is at the top.
- **Menus with Urdu items** (status filter options, column chooser, Theme…): menus are LTR chrome. Urdu items are `ur_body` labels left-aligned after the icon or check. Status uses dot then label.
- **Breadcrumbs:** gone.
- **Toasts and dialogs quoting Urdu:** the sentence is English and LTR, and the Urdu name is an isolated RTL run inline. For example: "Added ‹ضمنی حصول ریمانڈ جسمانی›" and "Delete «مکمل چالان 489F»?". Implement as an LTR `label` with the Urdu wrapped in U+2067 RLI … U+2069 PDI. Parley honours isolates, so a trailing `?` or `.` stays on the English side.
- **Tooltips:** always English. Format: `Label` then a gap of 8, then the keycaps (no parentheses). Example: "Print  [Ctrl] [P]".
- **Header title for a template:** the Urdu name as `ur_title`, left-aligned in the LTR header.
- **Editor page:** the page direction comes from the document (RTL for these templates). Field values that are dates or numbers are inserted as LTR isolates (the editor already keeps `14-11-2021` in order; keep it that way and add a regression shot). The page's own content never follows chrome rules.

## 4. Icon-only controls

The rule: an icon alone when the glyph is unambiguous in context. Every icon-only button has:

- a tooltip with its English name and its shortcut (when one exists);
- an accessible name (`accessibility_label` or `.tooltip` text);
- a hover fill.

Tooltip delay is 400 ms; the tooltip appears without animation.

| Control | Lucide icon (`IconName`) | Tooltip / accessible name | Shortcut |
|---|---|---|---|
| Navigation menu | `menu` (`Menu`) | Menu | F10 |
| Back / Forward | `arrow-left` / `arrow-right` | Back / Forward | Alt+Left / Alt+Right |
| Search | `search` | Search | Ctrl+K |
| Table columns | `columns-3` (`Columns3`) | Columns | — |
| Row actions | `ellipsis` | More actions | Shift+F10 / context key |
| Clear search | `x` | Clear search | Esc |
| Edit details (workspace) | `pencil` | Edit details | E |
| Print (workspace, template) | `printer` | Print / Preview | Ctrl+P |
| Fields panel toggle | `panel-right` (`PanelRight`) | Fields panel | Ctrl+. |
| More (workspace, template) | `ellipsis` | More actions | — |
| Undo / Redo | `undo-2` / `redo-2` | Undo / Redo | Ctrl+Z / Ctrl+Y |
| Bold / Italic / Underline | `bold` / `italic` / `underline` | Bold / Italic / Underline | Ctrl+B / Ctrl+I / Ctrl+U |
| Align left / center / right / justify | `text-align-start` / `text-align-center` / `text-align-end` / `text-align-justify` | **Align left / Center / Align right / Justify** (physical names, because the document is RTL and "start" would be wrong); buttons in that physical order | Ctrl+L / E / R / J inside the editor |
| Bulleted / numbered list | `list` / `list-ordered` | Bulleted list / Numbered list | — |
| Insert field | `braces` | Insert field (or type «name») | Ctrl+Shift+F |
| Zoom out / in | `minus` / `plus` | Zoom out / Zoom in | Ctrl+− / Ctrl+= |
| Import Word file | `file-up` (`FileUp`) | Import Word file | Ctrl+O |
| New template | `plus` | New template | Shift+N |
| Rename template | `pencil` | Rename | F2 |
| Save as PDF | `file-down` (`FileDown`) | Save as PDF | Ctrl+Shift+S |
| Remove list entry (form) | `x` | Remove | — |
| Delete global value | `trash` | Delete | — |
| Keyboard shortcuts | (nav menu item) `keyboard` | — | ? |
| Previous / next page (print) | `chevron-left` / `chevron-right` | Previous page / Next page | Page Up / Page Down |
| Date picker | `calendar` | Pick a date | Alt+Down |

Controls that **keep text**, and why:

- **New FIR** (FIR list header): the single primary action on that screen, and the first thing a new user needs.
- **Print** (Print screen): the final, irreversible action.
- **Create FIR, Save changes, Cancel** (form footer), and every dialog button: commitments read better as words.
- **Delete 2 FIRs, Clear** (selection): destructive actions must say what they do and how many.
- **Add document, Add accused, Add witness, Add zimni, Add global value, Add custom field:** a bare `+` next to a list is ambiguous about what it adds. Exception: the template list header's `plus`, whose tooltip and context make it clear.
- **Values | Names** (editor toolbar): no glyph means "show field names instead of values".
- **Status filter items:** they are data labels, not actions.
- **Edit details** inside Case details: a panel-level action next to a long list, which reads better as a word. The header `pencil` covers the icon route.
- **Fill them in** (print warning): a link inside a sentence.
- **Save state** ("Saved", "Unsaved"): it is information, not a control. Ctrl+S saves.
- **Zoom value** ("86%"): a text button that resets to Fit.

## 5. Screens

Each screen lists its layout, states, copy, keyboard behaviour and what to remove. Positions are given at 1280x800. The layout must also hold at 1366x768, the common station laptop, and must not break at 1024x700, the minimum window size (set it in `launch.rs`).

### 5.1 FIR list (mockups `fir-list-light.png`, `fir-list-dark.png`, `fir-list-search-light.png`, `fir-list-menu-light.png`)

- **Header bar:** title `FIRs`, meta count, primary **New FIR** (`sm`, `plus`, keycap N).
- **Toolbar row** (52 px, padding 4 top, gutter 24):
  - Search field, 300 px, `AutoLtr`, icon and clear inside, `/` keycap while empty.
  - Gap 12.
  - Status filter row: All, then the four statuses with counts, 28 px items, 2 px gap. The counts reflect the current search.
  - Flexible space.
  - `columns-3` icon button.
- **Table:**
  - Full width minus 24 px gutters.
  - Header 36 px, then a `uniform_list` of 44 px rows with a hairline after each.
  - Columns as in 3.5.
  - The checkbox is visible on row hover, on keyboard focus, and on all rows once any row is selected.
- **Selection mode:** the header actions change as in 2.3. Esc clears the selection first, then the search, then blurs.
- **States:**
  - **Loading:** after 150 ms, 8 skeleton rows using the real column widths, with bars `hover` coloured, 12 px tall, radius 4.
  - **Empty** (no FIRs): centred in the table area.
    - Icon tile: 36 px, radius 8, `hover` fill, `folder` icon.
    - "No FIRs yet" (14/500).
    - "Create the first FIR to start writing its documents." (13 `ink_muted`).
    - Primary "New FIR N".
  - **No match:** `search` tile, "No FIRs match “…”" with the query in an isolate, then a ghost "Clear search".
  - **Error:** `triangle-alert` tile, "Couldn't read your FIRs", "Missal could not open its database.", outline "Try again". The raw error goes in a `ink_faint` 12 line below, selectable.
- **Keyboard:**
  - `/` focuses search.
  - Down or Enter from search moves into the rows.
  - J/K or Up/Down move focus.
  - Enter opens.
  - X or Space toggles selection.
  - Ctrl+A selects all visible.
  - E edits details.
  - **Delete** asks to delete the focused or selected FIRs (new binding).
  - Shift+F10 or the context-menu key opens the row menu.
  - Row menu items: Open ↵, Edit details E, Print… Ctrl+P, separator, Delete Del.
- **Accused column:** needs `FirRow` to carry the first accused name and the accused count. The store query (`missal-store/src/rows.rs`) and the haystack both gain the accused names. Searching a name must find the FIR. This is a data change, owned by WP1.
- **Remove:**
  - the header search box's external `/` cap;
  - the bottom status bar ("N of M FIRs", selection bar);
  - sort icons on unsorted columns.

### 5.2 FIR workspace (mockups `workspace-light.png`, `workspace-dark.png`)

- **Header bar** as in 2.3.
  - Save state is text in `ink_faint` 12 with a `check` icon ("Saved"). It turns to "Unsaved" in `ink` with a 6 px `notice_text` dot.
  - Ctrl+S saves. **Do not toast "Document saved".**
- **Body:** three columns.
- **Document rail** (left, 248 px, `surface`, right hairline):
  - Label row (40 px): "Documents" and the count.
  - Rows as in 3.5.
  - Then the "Add document A" row (32 px, `plus`, `ink_muted`, keycap A). It opens the add-document popover anchored to that row, 360 px wide (mockup `components-light.png`):
    - an RTL search;
    - rows of 40 px with a checkbox at the right;
    - templates already in this FIR are shown with "Added" in `ink_faint` and cannot be selected;
    - footer: "N selected" and primary "Add N documents" with keycaps Ctrl ↵.
  - Arrow keys move through the rows, Space ticks, Ctrl+Enter adds, Esc closes.
- **Centre:**
  - The editor toolbar (40 px, `surface`, bottom hairline), groups separated by a 1 px x 20 px rule:
    1. undo/redo;
    2. B/I/U;
    3. alignment, in physical order;
    4. lists;
    5. insert field;
    6. flexible space;
    7. `Values | Names` segmented;
    8. rule;
    9. zoom: `minus`, the percentage text button, `plus`.
  - The canvas (`sunken`) has 20 px top padding and the page centred.
  - The default zoom is fit-width with a 24 px margin each side. Remember the last zoom per session, not per document.
- **Fields panel** (right, 296 px, left hairline). Toggle with `panel-right` or Ctrl+. It auto-hides below 1180 px window width; its toggle stays.
  - **Tabs:** "Fields", with an amber count badge (18 px, `notice_wash` / `notice_text`, 11/600) when the document has empty fields; and "Case details".
  - **Fields tab:**
    - "Empty in this document". Each item: name, scope tag, and an input. Global values carry the help line "Used by every FIR. Press Enter to save." in `ink_faint` 12, shown only while focused.
    - Enter or blur saves. Saving fills every occurrence in the page at once, with no animation.
    - FIR-property fields (for example the arrest date) show a text button "Edit in details" instead of an input.
    - Then "Filled N", as a grid.
    - Clicking any item scrolls the page to the field's first occurrence and puts the caret there. Use `reveal_field` in missal-editor.
  - **Fields tab with nothing empty:** a `circle-check` tile in `st_closed`, "Every field in this document is filled.", then the Filled grid.
  - **Case details tab:**
    - An "Edit details" text button with `pencil`.
    - The grid of all FIR fields.
    - Then "Custom values": every custom field with a value, plus a filter input and "Show all 35" to edit any custom value. This replaces the custom values sheet.
- **Editor page chips:**
  - **Filled value:** `ink` text on `field_fill`, radius 3, no underline.
  - **Empty field:** shows the field name in `notice_text` on `notice_wash`, with a 1.5 px dashed `notice` underline.
  - **Names mode:** every chip shows the field name in `accent` on `field_fill`.
  - This needs a fourth paint slot in `missal-editor/src/view/element.rs` (`empty_text`, `empty_background`, `empty_underline`), and the editor needs to know which references are empty. The app already computes the values, so pass a set of empty references with the field context.
- **States:**
  - **Loading:** rail skeleton plus a blank canvas, after 150 ms.
  - **No documents:** the canvas shows the empty state "No documents yet / Add a template to start writing for this FIR. / [+ Add document A]". The Fields panel shows Case details.
  - **Document failed to load:** an empty state in the canvas with "Try again".
  - **FIR not found:** "This FIR doesn't exist or was deleted." with "Back to FIRs".
- **Keyboard:**
  - Alt+Up/Down switch documents. If the current one is unsaved, the existing "Save changes first?" dialog appears.
  - Alt+Shift+Up/Down move the active document.
  - A opens add-document.
  - E edits details.
  - Ctrl+P prints.
  - Ctrl+Shift+L toggles Names.
  - Ctrl+. toggles the panel.
  - F6 cycles focus: rail → page → panel → header.
- **Remove:**
  - the `{}` header button;
  - the details rail;
  - the "All will print" bar;
  - include checkboxes in the rail;
  - "Insert field" text;
  - the "Document saved" toast.

### 5.3 New FIR and Edit FIR form (mockup `fir-form-light.png`)

- **Route:** `NewFir`, plus a new `EditFir(id)`. The edit route reuses the same view with values loaded; its footer says "Save changes".
- **Layout:**
  - The scroll area holds a centred 760 px column. The whole form is RTL.
  - Each section starts with a 20 px gap, then a heading row: the Urdu heading `ur_title` at the right, then a hairline filling to the left. The gap from heading to fields is 12.
- **Sections and fields,** each grid filling right to left:
  1. `مقدمہ`: FIR number*, Status; Offence*, Complainant. 2 columns, column gap 20, row gap 14.
  2. `تاریخیں`: FIR date*, Incident date*, Arrest date. 3 columns.
  3. `ملزمان`: a list. Each entry is a number (Latin 12 `ink_faint`, 18 px, at the right), a full-width input, and a remove `x` at the left. Below it, "Add accused" (ghost `sm`, `plus`) at the right. Enter in the last entry adds a new entry and focuses it.
  4. `گواہان`: a list, as above.
  5. `ضمنی`: a list, as above.
  6. `تفتیش`: Investigation officer, Writer; Recovery (full width).
  7. `موبائل`: IMEI 1, SIM 1; IMEI 2, SIM 2.
  8. `متن`: Body, multiline, at least 6 rows, line height 30.
- **Labels:**
  - `ur_input_label` 15/28 in `ink`, 4 px above the input.
  - Required fields get a `*` in `danger` after the label (on its left side).
  - No placeholders except dates and the FIR number (see 3.4).
- **Footer** (56 px, sticky, top hairline):
  - Left: save hint. "Not saved yet" (`circle-alert`, 12 `ink_muted`) for New; for Edit, "No changes" or "Unsaved changes".
  - Right: Cancel (ghost), then **Create FIR** (primary) with keycaps Ctrl S, or "Save changes".
- **Validation:**
  - It runs on submit, then live per field.
  - A field in error gets a `danger` outline and one line below it: 12 px `danger`, right-aligned, in English (for example "Enter the FIR number, for example 412/2026.").
  - Scroll to the first error and focus it.
  - Do not also tint the label.
- **Keyboard:**
  - Tab follows visual reading order: right column first, then left, row by row.
  - Ctrl+S submits.
  - Esc asks to discard only if the form is dirty.
- **Remove:**
  - the "Saved" indicator on New;
  - the right-hand section-title column;
  - the external calendar buttons;
  - the edit sheet.

### 5.4 Templates (mockup `templates-light.png`)

- **Header** as in 2.3.
- **Template list** (left, 320 px, right hairline):
  - Label row (44 px): "Templates", count, `file-up`, `plus`.
  - The RTL search field (36 px, inset 12).
  - A result count "9 of 36" (12 `ink_faint`) while filtering.
  - Rows of 44 px: name `ur_body` at the right; "18 fields" at the left (12 `ink_faint`). The match is highlighted with a `notice` at 30% fill.
  - The active row has `accent_wash` fill.
  - Up/Down or Alt+Up/Down move; Enter focuses the editor.
- **Centre:** the editor toolbar (as in 5.2, without `Values | Names`; templates always show names) and the page. There is no right panel.
- **Rename:** the `pencil` button or F2 turns the header title into an inline `Rtl` input (36 px, no border until focus). Enter saves; Esc cancels.
- **Import flow:**
  1. Ctrl+O, `file-up`, or the nav menu opens the system file dialog (.docx).
  2. On success, the route goes to the import review.
  3. The header shows the file name and the field summary.
  4. If any markers were not recognised, an amber box sits above the page: "2 markers weren't recognised: «…», «…». They will stay as plain text." Each marker is an Urdu isolate.
  5. Save template (primary) or Discard.
  6. Replacing an existing template's content goes through More → "Replace from Word file…", with the existing confirm dialog.
- **Empty states:**
  - No templates: `file-text` tile, "No templates yet", "Import a Word document to make your first template.", primary "Import Word file", ghost "New blank template".
  - Nothing selected: the canvas says "Choose a template from the list." in 13 `ink_muted`. No tile.
- **Remove:** the preview line, the bordered title input, the "Upload Word file" button.

### 5.5 Settings (mockup `settings-light.png`)

- **Layout:**
  - Left section nav: 220 px, 32 px items, active item with `hover` fill. Clicking an item scrolls to its section.
  - Content: a 680 px column, 24 px left padding.
  - Sections are separated by a hairline and 28 px gaps.
  - Section title `section`, description 13 `ink_muted` on one line, 14 px gap to the content.
- **Global values:**
  - An RTL row per value (36 px controls, 4 px vertical gap): name (150 px, text; click or F2 to rename inline), value input (`Auto`, Right), and `trash` at the far left.
  - "Add global value" at the right under the list.
  - Each value saves on blur or Enter, with no Save button. A quiet "Saved" appears in the section title row for 2 s, then fades in 100 ms.
  - Duplicate or empty names show inline errors.
- **Field markers:**
  - Two 48x36 LTR centred inputs with `نام` between them, a rule, then "Example" and a live chip.
  - Validation: 1–3 symbols, no letters or digits, open differs from close. The error sits inline below.
- **Custom fields:**
  - Title with count.
  - An RTL list of 40 px rows: name at the right, `ellipsis` at the left (Rename, Delete…). Delete warns if templates use the field, with "Used in N templates".
  - "Add custom field" at the right.
  - A filter input appears above the list when there are more than 12 fields.
- **Built-in fields:** collapsed by default ("Show 27 built-in fields"). When open, a two-column RTL grid of names as plain `ur_label` (no chips), each with its marker form on hover.
- **Appearance:** a segmented control System / Light / Dark, applied instantly.
- **About:** version, data folder (a selectable path; ghost "Open folder"), "Check for updates" (outline), the last checked time. This also hosts the update state that `update/controller.rs` toasts today. The toast remains.
- **Remove:** theme cards, the chip wall, the two-column title/description layout.

### 5.6 Print (mockup `print-light.png`)

- **Route:** `Print(PrintRequest)`. Back returns to the previous route. Esc returns too.
- **Header** as in 2.3.
- **Left panel** (280 px):
  - **Warning box** (only if empty fields exist in the included documents):
    - `triangle-alert`, then "**N fields are empty.** They will print as their names, for example ‹first name›."
    - "Fill them in" goes back to the workspace, opens the Fields panel and focuses the first empty field.
  - "Documents to print" label, then RTL rows of 44 px: checkbox at the right, name, and "2 pages" at the left (12 `ink_faint`). Unticked rows are `ink_faint`.
  - Clicking a row's name scrolls the preview to that document's first page.
  - Ticking re-lays out and updates the header meta.
  - Order follows the workspace order; Alt+Shift+Up/Down reorder here too.
  - Footer note: "You choose the printer in the next step." (12 `ink_faint`).
- **Centre:**
  - The `sunken`, with pages (always white) at "Fit page" by default. The whole page is visible, which fixes the fit bug.
  - The page shadow is the paper token.
- **Footer** (44 px):
  - Left: `chevron-left`, "Page", a page input (44 px), "of N", `chevron-right`.
  - Right: `minus`, the zoom dropdown (Fit page, Fit width, 50, 75, 100, 125, 150%), `plus`.
- **States:**
  - **Laying out:** after 150 ms, a skeleton page and "Laying out pages…" (12 `ink_muted`) in the footer.
  - **Error:** the existing empty state with "Try again".
  - **Print failure:** an error toast with Retry.
  - **PDF saved:** toast "Saved FIR 1-2026.pdf" with "Show in folder".
- **Keyboard:**
  - Ctrl+P prints.
  - Ctrl+Shift+S saves the PDF.
  - Page Up/Down change page.
  - Ctrl+=/−/0 zoom.
  - Esc goes back.
- **Remove:** the modal frame, "Cancel" and the "Fit · 100%" label.

### 5.7 Dialogs, toasts, menus, tooltips, shortcuts (mockup `components-light.png`, `components-dark.png`)

- **Confirm dialog:**
  - 420 px wide, padding 20, radius 12, dialog shadow and scrim.
  - Title `section`, body 13 `ink_muted`, 18 px gap, then buttons right-aligned: Cancel (outline) and the action (`danger` for destructive).
  - **Initial focus is on Cancel.** Enter activates the focused button; Esc cancels.
  - Copy pattern: title "Delete FIR 9/2026?"; body "Its 3 documents are deleted too. This can't be undone."
- **Unsaved-changes dialog:** title "Save changes to ‹name›?". Buttons: "Don't save" (ghost, left), Cancel (outline), **Save** (primary). This matches Windows convention.
- **Toasts:**
  - Bottom right, 16 px from the edges, 360 px max width, stacked 8 px apart.
  - Icon 14 (`circle-check` in `st_closed`, `circle-alert` in `danger`, `info` in `ink_muted`), text 13, an optional action as text in `accent`.
  - Auto-dismiss after 4 s (8 s when there is an action). Hover pauses.
  - Only for the results of user actions that are not otherwise visible: FIR created, document added (with Undo), FIRs deleted, PDF saved, errors.
- **Menus:**
  - `popover_style`, radius 8, 4 px padding.
  - Items 32 px, radius 4, 14 px icon in `ink_muted`, 13 text, keycaps right-aligned.
  - Destructive items are `danger` text and icon, last, after a separator.
  - Checked items show `check` at the icon position.
  - Keyboard: arrows, Enter, Esc, and type-to-select.
- **Tooltips:**
  - `ink` background, `surface` text, 12 px, 4/8 padding, radius 6.
  - Keycaps have a transparent fill and a `surface` outline at 35%.
  - 400 ms delay; once one is showing, the next appears instantly while the pointer moves along the toolbar. No animation.
- **Shortcuts cheatsheet:**
  - A 640 px dialog.
  - Two columns of groups: General, FIR list, FIR workspace, Editor, Templates, Print.
  - Rows of 28 px: action name (13) at the left, keycaps at the right.
  - "then" between chord steps is drawn as a thin `chevron-right` 12 in `ink_faint`, not as a word.
  - A filter input at the top (`AutoLtr`).
  - `?` toggles it; Esc closes.

## 6. Implementation notes and risks

| Risk | Spec response |
|---|---|
| `TextInput` draws its own box, so icons cannot go inside | WP0 adds `TextInput::bare()`, `line_height(..)` and `placeholder_direction(..)` in missal-text, and an app-level `Field`/`SearchField` wrapper that draws the box, icon, clear button and focus ring. Until then, mockups that put icons inside fields cannot be matched |
| GPUI text cannot do RTL | Every Urdu string in chrome goes through `missal_text::label`, including menu items (`PopupMenuItem::element`, as `fir_list.rs` already does), tooltips quoting Urdu (avoid them) and toasts (custom toast host) |
| Baselines across Latin and Urdu labels | `label` reports no baseline to taffy. Use fixed nudges (Latin `pt(3)` in 44 px rows, Urdu `mt(-2)` in 28–36 px controls) and verify in the 2x screenshots. Do not use `items_baseline` |
| Tabular figures | GPUI text supports `FontFeatures`. Set `tnum` on the Latin text style for tables, dates and counts. If a run cannot take features, fall back to fixed-width cells with right alignment, which still lines up `DD-MM-YYYY` |
| gpui-kit notification animation is fixed at 400 ms and top-anchored by default | `placement(Anchor::BottomRight)` exists. If the 400 ms enter feels slow, WP0 builds a 100-line toast host on gpui `Animation`. Either is acceptable; the spec's motion tokens are the target |
| Shadows were turned off (`theme.shadow = false`) | Set it to true and use `popover_style` for menus and popovers, and a dialog shadow for the palette and dialogs. GPUI renders box shadows natively. No blur, backdrop filters or gradients are used anywhere in this spec |
| Panel toggle animation | Not animated, by design. GPUI could animate width, but every frame would re-shape Nastaleeq on the page |
| Dashed underline for empty fields in the editor | The editor already paints a dashed `unresolved_underline`. Reuse that painter with the new `empty_*` colours. If dashes cost too much, a solid 1.5 px `notice` underline is acceptable |
| Window caption buttons on Windows | gpui-kit `TitleBar` draws them. Header actions must end at least 138 px from the right edge, so test at 1024 px wide. On Linux, gpui-kit draws its own set; keep the same reserve |
| F10 for the menu | gpui on Windows may deliver F10 as a system key. If it does not arrive, Alt+M is the fallback; record which one works in `docs/windows-checklist.md` |
| Reduced motion is never set | WP0 wires the OS setting to `cx.set_reduce_motion` |
| Removing the sidebar removes Ctrl+B "toggle sidebar" | Rebind Ctrl+B to Bold in the editor only (the editor already binds it in its own context; check for conflicts in `shortcuts.rs`) |

## 7. Work packages

Package ordering: **WP0 first, alone and short.** WP1, WP2 and WP3 can then run in parallel. Each later package owns disjoint files. Shared files are touched only by WP0, or by append-only edits named below.

### WP0: Foundations (1 engineer; lands first, about 2–3 days)

**Owns:**

- `crates/missal-app/src/theme.rs`: tokens from 1.1; constants for heights, radii and spacing from 1.3 and 1.4; `apply_palette` updated; `shadow = true`.
- `crates/missal-app/src/ui/mod.rs`: shared components.
  - `icon_button(id, IconName, tooltip, keys)` with accessible name and tooltip format.
  - `Button` presets (`primary_sm`, `ghost_sm`, `danger_outline`).
  - `keycaps`.
  - `section_label`.
  - `empty_state` (new spec).
  - `skeleton_bar`.
  - `status_dot`.
  - `status_pill`.
  - `segmented`.
  - `count_badge`.
  - `warn_box`.
  - `ur_label(text, UrRole)` helper applying the size and line height from 1.2.
- `crates/missal-app/src/ui/input.rs`: `Field` and `SearchField` wrappers. Field kinds map to the direction table in 3.4 (`Kind::Search`, `SearchRtl`, `Urdu`, `UrduAuto`, `LatinNumeric`, `Date`, `Latin`).
- `crates/missal-app/src/ui/titlebar.rs`: the new header bar. Screens supply `HeaderContent { title, meta, actions }` through a trait on the active screen view.
- `crates/missal-app/src/ui/nav_menu.rs` (new): the navigation dropdown.
- `crates/missal-app/src/ui/status.rs`: dot-based status.
- `crates/missal-app/src/ui/dialogs.rs`: confirm and unsaved dialogs to spec.
- `crates/missal-app/src/ui/toast.rs` (new, if needed): the toast host.
- `crates/missal-app/src/ui/cheatsheet.rs`.
- `crates/missal-app/src/app.rs` (shell layout): remove the sidebar, mount the header, add `Route::EditFir` and `Route::Print` as variants with placeholder views so other packages can fill them, and wire reduced motion.
- `crates/missal-app/src/shortcuts.rs`: new bindings: Delete, F10, Ctrl+., Alt+Shift+Up/Down, Ctrl+Shift+S, F2, Ctrl+O global; remove ToggleSidebar.
- `crates/missal-app/src/prefs.rs`: drop `sidebar_open`, add `recent_firs`.
- `crates/missal-app/src/ui/palette.rs` (command palette to 2.4).
- **Delete** `crates/missal-app/src/ui/sidebar.rs`.
- `crates/missal-text/src/input/mod.rs`: `bare()`, `line_height(..)` and `placeholder_direction(..)`, with unit tests for direction resolution.

**Done when** these screenshots match the mockups within the tolerances below:

- The `shots` harness gains:
  - `00-components` (a test-only route that renders the shared components);
  - `01-fir-list` (header only needs to match: list content can still be old);
  - `16-shortcuts`;
  - `17-command-palette` (matches `palette-light.png` with the query `11/20` typed);
  - `25-nav-menu` (matches `fir-list-menu-light.png`'s header and dropdown).
- Light and dark.
- `cargo test -p missal-text` covers `bare` and `placeholder_direction`.

### WP1: FIR list and Print (1 engineer)

**Owns:**

- `crates/missal-app/src/screens/fir_list.rs`.
- `crates/missal-app/src/state/rows.rs` and `state/firs.rs` (accused fields, haystack, real recents hook).
- `crates/missal-store/src/rows.rs` (accused in the row query; covered by the existing store tests plus one new test).
- `crates/missal-app/src/ui/print_preview.rs`, becoming the Print route view: document list, warning, footer, fit-page default.
- `crates/missal-app/src/printing.rs`, `crates/missal-app/src/packet.rs` (the empty-field count for the print warning).

**Done when:**

- `shots` `01-fir-list`, `03-selection`, a new `04b-search-urdu` (types `ڈکیتی`), `04-status-filter`, `05-columns-menu` and `06-row-context-menu` match `fir-list-light/dark.png` and `fir-list-search-light.png` in light and dark.
- `19`–`24` print shots match `print-light.png` (fit page shows the whole page; the warning is visible for FIR 3, which has empty globals).
- Delete via keyboard works in a manual run.

### WP2: FIR workspace, form and editor chrome (1 engineer, the largest package)

**Owns:**

- `crates/missal-app/src/screens/workspace.rs`.
- `crates/missal-app/src/screens/fir_form.rs` (New and Edit).
- **Delete** `screens/edit_sheet.rs` and `screens/custom_values.rs`, moving their logic into the form and the Fields panel.
- New `screens/fields_panel.rs`.
- `crates/missal-app/src/editor.rs`.
- `crates/missal-editor/src/view/toolbar.rs` and `view/element.rs`: icon-only toolbar, physical alignment names, the segmented Values/Names control, empty-field paint slots, reading `paper` and `ink` from the theme.

**Done when:**

- `shots` `07-new-fir` (filled with the mockup's values, by a harness that types them), `08-new-fir-validation`, `09-edit-fir` (replaces `09-edit-sheet`), `10-workspace`, `11-add-templates` (renamed `11-add-document`, with `ریمانڈ` typed) and `12-fields-panel` (replaces `12-custom-values`) match `fir-form-light.png`, `workspace-light/dark.png` and the add-document card in `components-light.png`.
- `editor_shots` `05`/`06` show amber empty fields and the dark paper.
- Placeholders are visibly lighter than values.

### WP3: Templates and Settings (1 engineer)

**Owns:**

- `crates/missal-app/src/screens/templates.rs`.
- `crates/missal-app/src/word.rs` (import review copy and state).
- `crates/missal-app/src/screens/settings.rs`.
- The About section's use of `crates/missal-app/src/update/controller.rs` (read-only use of its state; `installer.rs` is untouched).

**Done when:**

- `shots` `13-templates-empty`, `14-template-page` (with `چالان` typed in the list search) and `15-settings` match `templates-light.png` and `settings-light.png` in light and dark.
- A new `15b-import-review` shot shows the review header and the unrecognised-markers box.
- Global values save on blur in a manual run with `--data-dir /tmp/missal-scratch`.

**Shared-file rule after WP0:**

- `app.rs` route-to-view wiring and `shortcuts.rs` binding tables are append-only for WP1–3, one line per route or binding. Rebase conflicts there are mechanical.
- `theme.rs` and `ui/*` change only through WP0 or by agreement. A missing shared component is added to `ui/` by whichever package needs it first, in a small separate commit.

**Matching tolerance for "done":**

- Compare 2x screenshots side by side with the mockup.
- Positions within 2 px, sizes within 2 px, colours exactly the tokens.
- Text must be identical, apart from data differences in the seed.
- No Urdu glyph may be clipped.
- Latin and Urdu baselines must match within 1 px in rows.
- Mockups use Chrome's text engine, so Nastaleeq word spacing and kerning will differ slightly from parley's; judge layout, not glyph shapes.
- The Electron app, `ci.yml`, `electron-builder.yml` and `package.json` are untouched, as `AGENTS.md` requires.
