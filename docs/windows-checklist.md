# Windows verification checklist

Everything here was written and tested on Linux. Windows behaviour is unverified until each item below has been checked on a real Windows 10 or 11 machine. Record the Windows build, display scale and keyboard layouts used, and the date, next to anything that fails.

Test machine matrix, at minimum: one single-monitor 100% display, one 125% or 150% laptop panel, one two-monitor setup with different scales, a Urdu phonetic keyboard layout, the standard Urdu layout, and an English layout. Use the installed release build, not `cargo run`, unless an item says otherwise.

## Installer and updater

- [ ] `Missal-win-Setup.exe` installs per user without an elevation prompt, then starts Missal.
- [ ] The install folder is `%LOCALAPPDATA%\Missal`, with a Start menu shortcut named Missal and the app icon on the shortcut, the taskbar and the title bar.
- [ ] Right-click the exe, Properties, Details: product name, version and copyright are filled in.
- [ ] No console window appears when launching, from the shortcut or from Explorer.
- [ ] With the Electron version installed, existing data in `%APPDATA%\Missal` (missal.sqlite and preferences.json) opens untouched in the native app, and the Electron app still works afterwards.
- [ ] Uninstalling from Settings, Apps removes the app folder and shortcut and leaves `%APPDATA%\Missal` intact.
- [ ] Windows SmartScreen behaviour for an unsigned installer is understood, and the signing decision is recorded (see docs/packaging.md).
- [ ] Update path: install version N, publish N+1 as a native release, start N. Within a minute after startup a toast reads "Update ready" with a Restart now button. Nothing blocks typing while it downloads.
- [ ] With unsaved edits open, Restart now does not restart and shows the unsaved-changes warning. After saving, Restart now relaunches into N+1 with data intact.
- [ ] Ignoring the toast and quitting, then launching again, applies the update at launch.
- [ ] Offline start, and a start behind a blocking proxy, show no error and no delay.
- [ ] `SumatraPDF.exe` sits beside `missal.exe` in `%LOCALAPPDATA%\Missal\current` if it was bundled for this build.

## Text engine

- [ ] Font resolution with Jameel Noori Nastaleeq installed system-wide: Urdu text renders with the embedded face and does not change when the system copy is installed or removed.
- [ ] RTL glyphs painted through `paint_glyph` are positioned correctly: joining forms, diacritics, and mixed Urdu and Latin or digit runs.
- [ ] Synthetic bold blends without halos or doubled edges on light and dark themes.
- [ ] Crispness at 125% and 150% display scale: no blur on Inter UI text, no shimmering on Nastaleeq strokes.
- [ ] Urdu IME (phonetic and standard layouts, and the Windows touch keyboard): composition text shows inline, commits cleanly, Backspace during composition behaves.
- [ ] Clipboard: copy and paste of plain Urdu and Latin text to and from Notepad, Word and a browser.

## App shell

- [ ] Fresh launch on a profile that has `%APPDATA%\Missal` shows the existing FIRs and templates.
- [ ] Title-bar controls (minimise, maximise, close) work, hover and press states are right, double-click on the title maximises, dragging to a screen edge or corner snaps, and Win+Arrow works.
- [ ] Window state persists across restarts: position, size and maximised, on a second monitor, after unplugging that monitor, and across monitors with different scales.
- [ ] Theme follows Windows light and dark live while the app runs, with the preference set to system.
- [ ] Shortcuts work under the Urdu keyboard layouts and the English layout: `?` for the cheat sheet, `/` for search, g-chords (g then the next key), Alt+Left and Alt+Right for back and forward, the command palette, Ctrl+P.
- [ ] Shortcuts do not fire while composing with an IME.

## Editor

- [ ] Urdu IMEs: the candidate window opens at the caret, follows it while typing across lines, and stays on the right monitor at high DPI.
- [ ] AltGr arrives from Windows as Ctrl+Alt. Confirm layouts that use AltGr can still type their characters, and that Ctrl+Alt+E, Ctrl+Alt+R, Ctrl+Alt+L and Ctrl+Alt+J run the editor commands only on layouts without AltGr, without colliding with Ctrl shortcuts or typing stray characters.
- [ ] The « and » field markers: record which key sequence types them on each Urdu layout, and check that the field-marker setting accepts them.
- [ ] Shift+Insert pastes, Ctrl+Insert copies.
- [ ] Rich clipboard round trip: copy formatted Urdu from the editor into Word and back, and between two Missal documents, keeping bold, italic, underline, alignment, lists and tables.
- [ ] Drag-selecting past the edge of the viewport autoscrolls smoothly at 125% and 150% scale and across monitors, and stops when the button is released.
- [ ] Undo and redo across IME commits.

## Printing and files

- [ ] With `SumatraPDF.exe` beside `missal.exe` (or in `%LOCALAPPDATA%\SumatraPDF` or Program Files), Print opens SumatraPDF with its print dialog (`-print-dialog -exit-when-done`), prints, and SumatraPDF exits afterwards.
- [ ] Without SumatraPDF, Print opens the PDF in the default viewer (`open_with_system`), and the toast explains how to print from there.
- [ ] Printed output matches the on-screen preview: page size A4 and legal, margins, Nastaleeq line heights, tables and borders.
- [ ] The native Save dialog suggests a safe file name, handles Urdu names, existing files and cancelling.
- [ ] The native Open dialog imports a `.docx` template, including one with Urdu in the file name, and from a network share.
- [ ] Temporary print files in `%TEMP%\missal-print` are cleaned after a day.
