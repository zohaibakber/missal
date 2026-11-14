# Packaging and releasing the native app

## Choices

**Installer and updater: Velopack** (`vpk` 1.3.39-g8d2844f, Rust crate `velopack` 1.3.39-g8d2844f, pinned in `Cargo.toml` and `.github/workflows/native.yml`; keep the two in step).

- Windows: `Missal-win-Setup.exe` is a one-click, per-user installer (no elevation, installs to `%LOCALAPPDATA%\Missal`), the closest match to the electron-builder NSIS one-click per-user installer. Uninstall removes only that folder, so `%APPDATA%\Missal` (the database shared with the Electron app) is never touched. The updater, delta packages and an update API come in the same box.
- Linux: the same tool produces an AppImage that can update itself.
- Rejected: cargo-packager (builds NSIS/AppImage/deb, but its updater is a separate component with signature keys to manage and no delta support), cargo-wix and Inno Setup (installers only, no updater, so a second system would be needed for updates).
- Cost: `vpk` is a .NET tool, so CI needs the .NET SDK (preinstalled on GitHub runners). The runtime has no .NET dependency.

**Updates** (`crates/missal-app/src/update/`): in release builds, once at startup, a background thread asks the GitHub API for the releases of `zohaibakber/missal`, keeps only published, non-prerelease releases tagged `native-vX.Y.Z` (Electron `v*` releases in the same repository are ignored), picks the highest version above the running one that has the channel feed (`releases.win.json` or `releases.linux.json`), and hands that release to Velopack. Velopack downloads the package and verifies its SHA-256 (and SHA-1) from the feed before it is kept. The UI is never blocked. When the package is ready a toast says "Update ready" with a Restart now button. Restart is refused while the navigator reports unsaved changes. If the user ignores the toast, the downloaded update is applied at the next launch, before any work is open.

Updates are skipped for debug builds, when `--data-dir` is given, when `MISSAL_NO_UPDATE_CHECK` is set, and when the app was not installed by Velopack (cargo run, plain binary, deb).

The GitHub API is called unauthenticated (60 requests per hour per IP). One call per launch is fine, but many stations behind one NAT address could hit it; the failure is silent and retried at next launch.

Signature note: Velopack verifies checksums from the release feed, which is served over HTTPS from the repository's own releases. It does not verify a publisher signature. Authenticode signing of `Setup.exe` and `missal.exe` (see below) is what protects against a tampered build on Windows. Without it SmartScreen warns on first install.

## Tags and releases

- Native: `native-vX.Y.Z`, where `X.Y.Z` equals the `missal-app` crate version (the workflow checks). Electron: `v*` as before. `v*` does not match `native-v*`, so neither workflow reacts to the other's tags.
- `.github/workflows/native.yml` runs fmt, clippy and tests on Linux, builds on Windows, and on a `native-v*` tag builds the installer, AppImage and deb and creates a draft release.
- The draft is created with `--latest=false`. After publishing it, confirm it is not marked "Latest": electron-updater reads the latest release and would break if it pointed at a native release. If GitHub marks it, run `gh release edit native-vX.Y.Z --latest=false`, or mark the older Electron release as latest again.
- Do not mark native releases as pre-release: the updater ignores those.

Release steps:

1. Bump `version` in `[workspace.package]` of `Cargo.toml`, commit.
2. `git tag native-vX.Y.Z && git push origin native-vX.Y.Z`.
3. Wait for the workflow, test the artefacts from the draft on Windows with the checklist in `docs/windows-checklist.md`.
4. Publish the draft and fix the latest flag as above.

Release assets per platform: `Missal-win-Setup.exe`, `Missal-win-Portable.zip`, `Missal-<version>-win-full.nupkg`, `releases.win.json`, `assets.win.json`, `RELEASES-win`, and the Linux equivalents plus `Missal-linux.AppImage` and `missal_<version>_amd64.deb`. The updater needs the `.nupkg` and `releases.*.json` files, so upload everything.

Delta packages are not produced: CI packs each tag from scratch. To enable them, download the previous native release into the output directory with `vpk download github` before packing (and mind that it must pick a `native-v*` release).

The package id `Missal` must never change, or installed copies stop updating.

## Windows

- `crates/missal-app/build.rs` embeds `assets/icon.ico` and version info through `winresource`, for Windows targets only.
- Release builds use the GUI subsystem (`windows_subsystem = "windows"`), so no console opens. Debug builds keep the console.
- `packaging/windows/pack.ps1 -Version X.Y.Z` stages `missal.exe`, notices and (optionally) SumatraPDF, and runs `vpk pack`.
- Velopack launches the app with install and update hook arguments; `VelopackApp::run()` runs first in `main`.

### SumatraPDF

Printing uses `SumatraPDF.exe` found next to `missal.exe` (see `printing.rs`). It is **not bundled yet**: the build host used for this work could not reach the official download, so no version and checksum could be pinned honestly. To bundle it:

1. Pick a release from https://www.sumatrapdfreader.org/download-free-pdf-viewer (64-bit portable zip).
2. Download it once on a trusted machine, compute `sha256sum`, and fill `version`, `url` and `sha256` in `packaging/windows/sumatra.lock`.
3. Commit. `pack.ps1` runs `fetch-sumatra.ps1`, which downloads that URL, aborts on a checksum mismatch and copies the executable to `SumatraPDF.exe` in the install folder. With an empty lock file nothing is bundled and printing falls back to the default PDF viewer.

Licence obligations when shipping it: SumatraPDF is GPLv3. It is a separate executable started as a child process, not linked, so Missal itself is not subject to the GPL. When distributing the binary you must (a) include the GPLv3 text and copyright notice (`THIRD-PARTY-NOTICES.txt` is already staged into the installer and names the licence and source), (b) make the corresponding source available or provide a written offer (link https://github.com/sumatrapdfreader/sumatrapdf for the exact version bundled), and (c) not remove its own notices. Have the owner confirm this is acceptable.

## Linux

- `packaging/linux/pack.sh X.Y.Z` stages the binary and runs `vpk [linux] pack`, producing `Missal-linux.AppImage` plus update packages.
- `packaging/linux/build-deb.sh BINARY X.Y.Z OUTDIR` builds a `.deb` with the desktop file (`packaging/linux/missal.desktop`) and icon. A deb does not self-update.
- The window `app_id` is `com.zohaibakber.missal`, matching the desktop file's `StartupWMClass`.
- Runtime requirement: a working Vulkan driver and ICD. Intel and AMD: `mesa-vulkan-drivers`. NVIDIA: the vendor driver. With no GPU: `mesa-vulkan-drivers` provides the software renderer (lavapipe), which works but is slow. Also needs `libxkbcommon`, `libxkbcommon-x11`, `libwayland-client`, `libfontconfig`, `libfreetype` and X11 or Wayland session libraries, which any desktop install has. AppImages do not bundle GPU drivers.

## Code signing and other owner tasks

- Authenticode certificate for Windows (`signtool` or Azure Trusted Signing), then pass `--signParams` to `vpk pack`, which needs a repository secret. Not done.
- Confirm redistribution terms of the embedded Jameel Noori Nastaleeq font.
- Decide on SumatraPDF bundling (above).
- Work through `docs/windows-checklist.md`.
- Decide when to retire the Electron app and its workflows.

## Licences

See `packaging/THIRD-PARTY-NOTICES.txt`, shipped inside every installer and deb. Inter is OFL (licence file included), GPUI crates are Apache-2.0, Velopack is MIT, SumatraPDF is GPLv3 (separate process).
