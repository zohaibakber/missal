# Missal native UI: audit

Screenshots were regenerated on 2026-11-14 from the current tree into `design/before/app/` (`shots` example, 1280x800 at 2x, 24 states, light and dark) and `design/before/editor/` (`editor_shots`, real editor, 6 states). Paths below are relative to `design/before/`. Regions are given at 1x (multiply by 2 for the PNG pixel).

Two harness caveats:

- `shots` mounts a stand-in for the editor ("Document editor mounts here", `app/10`, `app/14`). The real editor chrome is only in `editor/05`, `editor/06`, `editor/01`.
- `app/09`, `app/12` (sheets) and `app/16` (shortcuts dialog) may have been captured while their enter animation was still running. In `app/09` and `app/12` the sheet content runs off the right edge of the window; in `app/16` only the scrim is visible. Either way, these surfaces need checking on a real Windows machine. The redesign removes both sheets and rebuilds the cheatsheet, so this does not block anything.

## Priority problems

1. **Placeholders look like values.** In the New FIR form the placeholders `412/2026` and `چوری ۳۷۹ ت پ` are drawn almost in the foreground colour. Then validation reports "Required" under fields that look filled (`app/08-new-fir-validation-*`, x 760–1000, y 80–180). An officer cannot tell an example from their own input. The placeholder colour must be clearly lighter, and example values do not belong in placeholders.
2. **Empty fields are not visible before printing.** In the editor, a filled value and an empty field are the same blue (`editor/05`: `تھانہ نام` and `ضلع نام` at the top of the page are empty; `محمد اسلم…` is filled; both read as blue on a tint, and only a faint dashed underline tells them apart). Print preview then prints the raw field names (`app/19`, `تھانہ نام` / `ضلع نام` in the page header). Nothing in the app ever says "3 fields are empty". This is the most costly real-world failure: a wrong court document.
3. **There is no clear direction model.** The shell is LTR, but tables, rails and forms are RTL. Each component improvises:
   - The keyboard-focus bar on the RTL table is on the left edge (`fir_list.rs` `border_l_2`), the far end of the row.
   - Menus left-align Urdu (`app/05`, `app/06`), while the details rail right-aligns it.
   - Global values put the name box on the left and the value on the right, which reads backwards in RTL (`app/15`, x 445–890, y 285–440).
   - The template search uses an English LTR placeholder above an RTL list (`app/11`).
   - The editor alignment buttons use start/end icons on an RTL document.
4. **The workspace does not say where you are.** Its header shows only the offence (`app/10`, x 690–760, y 40–60, `راہزنی ۳۹۴ ت پ`): no FIR number and no document name. The actions are three unlabelled icons: `…`, `{}` and the printer (x 180–280). `{}` means "custom field values", which no officer will guess. Save is a disabled outline pill that reads "Saved".
5. **Chrome is duplicated, and space goes to it rather than to the document.**
   - "New FIR" appears in the sidebar, in the list header and in the palette.
   - Search appears in the title bar and in the list.
   - Breadcrumbs repeat the sidebar selection.
   - Back/forward sit in the title bar.
   - At 1280 wide the editor page gets 560 px of 1280 (`editor/05`), because the sidebar (176) and the details rail (225) are both always open.
6. **"Recent" is not recent.** The sidebar list is `firs.recent(5)`, which is just the first five rows in the default sort (`state/firs.rs:88`). `app/01` shows 12/2020, 12/2019, 11/2026, 11/2025, 11/2024: the highest FIR numbers, not the ones last opened.
7. **Sheets and dialogs break.** The edit sheet and the custom values sheet overflow the window and hide their labels and values (`app/09`, `app/12`). The FIR number field in the edit sheet looks empty. The status select shows nothing. The cheatsheet dialog is not visible (`app/16`).
8. **Nastaleeq is squeezed, and Latin and Urdu do not share a baseline.**
   - Urdu labels run at 12–13 px in the details rail (`app/10`, `حالت`, `مدعی مقدمہ`, about 9 px tall glyph bodies) and in table headers. That is below what Nastaleeq can carry.
   - Rows in the add-templates popover are 34 px for 15 px Nastaleeq, and the last row is cut through its glyphs (`app/11`, y 315).
   - In table rows, Latin numbers sit about 3 px above the Urdu baseline (`app/01`, the FIR number column against the offence column).
9. **Print preview "Fit" does not fit.** "Fit · 100%" shows a page cut off at the bottom (`app/19`). The preview is a modal over the workspace, so the user cannot fix what it shows. Choosing which documents print lives elsewhere: as checkboxes in the workspace rail, plus an "All will print" status bar.
10. **Dark mode is half done.**
    - The editor page stays pure white with blue chips on a near-black UI (`editor/05-dark`). The palette has a `paper` token that the editor never uses.
    - The selected row (`muted`) and the hovered row (`accent` at 70%) differ by about 1% lightness, in both themes.
    - Scrollbar thumbs at 35% muted are invisible on the dark canvas.

## Surface by surface

### Title bar (`app/01` y 0–34)
- Six things share 34 px: sidebar toggle, back, forward, breadcrumbs, a 448 px search field and empty space. The search field is not centred in the window; it sits in the middle of the remaining flex space, so it moves when the breadcrumb changes.
- The search field looks like a text input but opens the palette. Its placeholder ("Search FIRs, templates and commands") is 13 px and sits beside a two-key `Ctrl` `K` hint.
- Breadcrumbs repeat the sidebar. `FIRs > 1/2021` gives a template name in Urdu next to an LTR chevron (`app/14`: `Templates > مکمل چالان 489F`, which reads backwards).
- The disabled forward arrow has no visible disabled treatment beyond a slightly lighter grey.
- On Windows the native caption buttons are drawn by gpui-kit's `TitleBar`. Nothing reserves space for them in the layout, so the search field can collide with them below about 1000 px wide.

### Sidebar (`app/01` x 0–176)
- An outlined "New FIR N" button duplicates the list header's primary "New FIR".
- FIRs and Templates carry counts (90, 36) that change nothing about what the user does.
- "Recent" is wrong (see priority 6). Recent rows mix a 13 px Latin number with Urdu truncated at about 80 px (`…دھوکہ دہی اور جعل سازی ۴۶`), which drops the offence's section number, the most useful part.
- Shortcuts and Settings are pinned to the bottom. Settings is a primary destination but is styled as a footer link.
- Collapsed mode (`app/02`) is a 48 px icon rail with an outlined `+`. Icon-only navigation is the wrong default for this audience.

### Command palette (`app/17`)
- There is no visible elevation and no scrim. The palette floats over the table, and table rows show through its edges (x 430–560).
- The input box is bordered, inside a bordered panel, with a separate search icon outside the input.
- Row heights are uneven: Go to and Create rows are 24 px, Preferences rows 21 px, FIR rows 24 px (y 195–250 against y 385–455).
- FIR rows put "FIR 12/2020" on the left and the offence right-aligned at the far edge, with a 300 px gap in between.
- The last visible row is cut mid-glyph (y 605).
- The three theme commands are separate rows; one "Theme: Light / Dark / System" command would do.
- `G then H` key hints use the word "then" in muted text between keycaps, which reads like a label.

### FIR list
- Header (`app/01` y 70–110):
  - The 290 px search box is bordered, and its `/` hint sits *outside* the box as a separate keycap.
  - The placeholder "Search FIRs" is 17 px, larger than the row text.
  - Status, Columns and New FIR sit at the far right, 1000 px away from the search box.
  - There is no title and no count. The user only knows how many FIRs match from the status bar that appears when filtering.
- Filters (`app/04`): the status popover is 200 px wide with Urdu labels left-aligned and empty space to the right. There is no tick until something is selected, and no counts. "Clear filter" appears only once a filter is set.
- Columns menu (`app/05`): the FIR number can be hidden, which leaves rows with nothing to identify them. Urdu column names are left-aligned in the menu while the table headers are right-aligned.
- Table:
  - Headers are 12–13 px Urdu with a sort icon on every column (`⇅` x 6), which is visual noise.
  - The FIR date and incident date columns are identical for most rows; both are shown by default.
  - The arrest date shows `—` for most rows.
  - The offence column is flex and the rest are fixed, so at 1280 there is a 160 px dead band between status and offence.
- Rows:
  - 44 px.
  - The hover tint (`accent` at 70%) is barely visible.
  - The focus treatment is a left border, on the wrong edge for RTL.
  - Checkboxes are invisible until hover, so selection cannot be discovered.
  - The row action `…` appears on hover at the far left, opposite the FIR number the eye is on.
  - In `app/03` the checked checkbox renders as a solid square with no visible tick at 1x.
- Selection (`app/03`): the status bar reads "90 selected · Clear" and puts a solid red "Delete 90 FIRs" at the bottom right. That is the strongest-coloured element on screen for a destructive action. There is no keyboard path to delete (no `Delete` binding).
- Context menu (`app/06`): it opens flush against the window's left edge and is clipped (x 0–105). It has only Open, Edit details and Delete; Print is missing.
- States: the loading skeleton is three grey bars per row that do not match the column layout. The failure state appends the raw error to the copy ("Restart Missal. (message)"). Empty and no-match states are fine but generic.

### New FIR form (`app/07`, `app/08`)
- The header reads "New FIR • Saved" for a record that does not exist yet.
- The layout is a 3-column grid: a section title column at the far right, then two field columns. The section titles (`مقدمہ`, `تاریخیں`, `دیگر تفصیلات`) are 14 px muted and float at the far right, unrelated to their fields.
- Field order inside a section is not the order of the paper FIR: the complainant comes before the offence in visual reading order.
- Date inputs: the calendar button sits outside the input box, so each date takes an input plus an orphan icon. The arrest-date input is 4 px higher than its neighbours because it has no required marker (`app/08` x 405–560).
- Placeholders look like values (priority 1).
- "Required" appears under the field in danger red. That is fine, but it stacks with a red asterisk in the label: two signals.
- Accused, witnesses and zimni are lists, but the UI for adding and removing entries is not visible above the fold. The form scrolls inside the pane with no sticky action bar, so "Create FIR" is in the header, far from the last field.
- The body (`متن`) is a 6-row text area of 15 px Nastaleeq with 1.4 line height. Glyphs touch between lines.

### Edit sheet (`app/09`)
- It overflows (see priority 7).
- It is a second, different layout of the same fields: a single column, a different order, and none of the form's sections. Maintaining two forms will drift further.
- Footer: "No changes · Reset · Save" with Save cut off.

### FIR workspace (`app/10`, `editor/05`, `editor/06`)
- Header: see priority 4. The offence is 18 px, right-aligned against the right rail, while the actions are at the far left.
- Document rail (right, 288 px):
  - Each row puts the document name next to an "include when printing" checkbox. Every document is ticked, so the checkboxes say nothing until someone unticks one.
  - "Documents 3 +" is the only way to add a document, through a 24 px `+`.
  - Reorder and remove are hidden in a context menu.
  - The active row is a light grey fill with no other cue.
- Details rail:
  - Label-over-value pairs with 12 px Urdu labels and 16–17 px values. Each pair takes about 50 px, so the rail scrolls almost immediately.
  - Dates are 17 px Latin, larger than the Urdu values beside them.
  - A pencil button edits all of the details.
  - "All will print" sits at the bottom as a status bar.
- Custom values sheet (`app/12`): a 35-input sheet with field names above each input. Opening it hides the document, so the user cannot see where a value goes. Most of the 35 fields are irrelevant to the open document.
- Add templates popover (`app/11`):
  - 36 rows of 15 px Nastaleeq in 27 px rows, so stacked glyphs collide with the row above.
  - The "Added" tag sits on the far left, away from the name.
  - The search placeholder is LTR English in an Auto input. Typing the first Urdu letter flips the caret to the right side.
  - Footer: "Select templates to add" plus a disabled "Add".
- Editor toolbar (`editor/05` y 120–150):
  - LTR icons with an always-visible "{} Insert field" label.
  - The alignment group uses start/centre/end/justify icons; on an RTL page "start" draws lines flush left while aligning text right.
  - The active-state pill is a filled dark circle in light mode and grey in dark mode.
  - Zoom is a 90% label between two magnifier icons, and there is no "fit".
- Page: in dark mode the page is pure white (`editor/05-dark`). Filled and empty field chips use the same blue (priority 2). The labels toggle (`Mod+Shift+L`) is buried in the `…` menu as "Show field names".

### Templates (`app/13`, `app/14`, `editor/01`, `editor/02`)
- The list is on the right, 288 px. Every item shows the same preview line, `سرکاربذریعہ مقدمہ نمبر : مورخہ: جرم :بنام…`, because templates share their header block, so the preview adds 30 px per row and tells nothing.
- The template title is a bordered text input in the header, styled like a search box.
- Header actions are `…`, "Saved", print and "Upload Word file". "Upload" is the wrong verb: nothing is uploaded.
- With no template selected, the empty centre shows "Choose a template from the list, or start a new one." in a full-height pane.
- Import review state (NewTemplate route): the copy "Review the layout and placeholders, then save the template." uses "placeholders" while the rest of the app says "fields".

### Settings (`app/15`)
- Two columns, with titles and descriptions on the left (220 px) and controls on the right. The descriptions wrap to 3–4 lines.
- Theme uses three 70x52 preview cards, which is a lot of space for a setting changed once.
- Field markers: two 56 px inputs around a muted `نام`, then "Example «جرم»". It works, but the inputs are oversized for one character.
- Global values: see priority 3. The value placeholder is `قدر` (literally "value") in every row. "Add value" is a small outline button.
- FIR fields: a wall of 27 accent chips (`«مقدمہ نمبر»`, `«گواہان 1»`…). They are read-only reference data, yet they are the most colourful thing on the page. Inside the RTL chips, the bidi order puts the number on the wrong side of the guillemets (`«1 گواہان»`).
- Custom fields: below the fold. Renaming and deleting are unclear.
- The page has no section navigation; it is one long scroll.

### Print preview (`app/19`–`app/24`)
- A modal 1150 px wide over the app.
- Header: "Print preview / FIR 1/2021 · 3 documents · 47 pages", with Cancel, Export PDF and Print.
- The footer splits page navigation (left) from zoom (right). Zoom is a dropdown labelled "Fit · 100%".
- There is no document list, so the user cannot see which document page 23 belongs to.
- There is no warning about empty fields.
- "Cancel" closes a preview, which is not a cancellation.
- The error state (`app/24`) is fine.

### Dialogs, toasts, menus, tooltips, focus, icons, scrollbars
- Confirm dialogs (`confirm_delete`): the copy is good. Both buttons are the same size, with the danger button filled. Initial focus is not specified.
- Toasts: gpui-kit's default (top right, with a slide). "Document saved" fires on every Ctrl+S, which is noisy.
- Menus: gpui-kit popup. Items are 28 px. Urdu items are left-aligned in some menus and right-aligned in others.
- Tooltips:
  - Two formats are in use: "Toggle sidebar" (no keys) and "Command menu  (Ctrl+K)", with two spaces and parentheses.
  - The editor tooltips use their own copy of the same helper (`missal-editor/src/view/toolbar.rs` `hint`).
- Focus: inputs show a 3 px ring. Rows, rail items and tabs show nothing on keyboard focus, apart from the list's left bar.
- Icons: Lucide at 16 px everywhere, at 14 px for status. The custom status glyphs (half-filled circles) are hard to tell apart at 14 px (`app/01`, blue and violet).
- Scrollbars: thumbs at 35% muted are fine in light, invisible in dark.
- Radii:
  - The theme sets 8 and 11.
  - Components hardcode 5, 6, 7, 8 and 10 (`ui/mod.rs`, `sidebar.rs`, `fir_list.rs`).
  - Heights vary: 28 (gpui buttons), 30 (palette trigger), 32 (nav), 34 (status option), 36 (recent), 38, 40, 44 and 48.
  - There is no scale.

### Code-level observations that shape the spec
- `missal-text` `TextInput`:
  - It always draws its own bordered box (`input/mod.rs` `render`), so a search field cannot hold an icon or a clear button *inside* the box. The current code puts them outside.
  - Direction `Auto` resolves from the first strong character of the text, or of the placeholder while the box is empty. An English placeholder therefore makes an empty box LTR, and the first Urdu letter flips it.
  - Arrow keys move visually (`move_visual`).
  - Its line height is the font's natural height (`row_height` probes "Aب").
- `Label` supports `rtl()`, `ltr()`, `direction(Auto|AutoLtr)`, `align`, `leading` (line height) and `truncate`. Labels are the only route for Urdu in chrome.
- gpui-kit:
  - It has `Animation` with `with_easing`, springs, `ease_out_quint` and `ease_in_out`.
  - Every `with_animation` already respects `cx.reduce_motion()`.
  - Popovers animate 150 ms with ease-out cubic; sheets 150 ms; tabs 200 ms.
  - Box shadows are supported (`popover_style` uses a ring plus a shadow), but `theme.shadow = false` in `theme.rs` turns elevation tokens off.
  - `items_baseline()` exists in gpui, but missal-text labels do not report a baseline, so mixed-script rows cannot baseline-align by flex alone.
- `missal-editor` paints field chips with `field_text`, `field_background` and `unresolved_underline` (`view/element.rs:212`). Three colours are all it has, so an "empty" style needs one more slot.
- Search already folds Urdu-Indic and Arabic-Indic digits to Western digits (`state/rows.rs` `fold_for_search`). That is good, and it is kept.
