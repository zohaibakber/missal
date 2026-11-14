# Missal

Desktop app for writing First Information Reports in Urdu. Two implementations live in this repository while the move to native happens:

- **Native (Rust and GPUI)**: the Cargo workspace under `crates/`. This is the app that is being made shippable. Windows first, Linux second.
- **Electron (TypeScript)**: the original app under `src/`, built with Vite+, React, TanStack Router, Lexical and Effect. It keeps its own CI (`.github/workflows/ci.yml`), installer config (`electron-builder.yml`) and `v*` release tags until the owner retires it.

Both read the same SQLite database, `missal.sqlite` in `%APPDATA%\Missal` on Windows (`~/.config/Missal` on Linux).

## Native app

Rust 1.100.0 is pinned by `rust-toolchain.toml` (edition 2024, `unsafe` forbidden).

```bash
cargo run -p missal-app
cargo run -p missal-app -- --data-dir /tmp/missal-scratch
cargo run -p missal-app -- --data-dir /tmp/missal-scratch --seed 200
```

- `--data-dir DIR` (or `MISSAL_DATA_DIR`) keeps the database and preferences in `DIR` instead of the real data directory. Use it for every experiment.
- `--seed COUNT` fills the database with sample FIRs and exits. It refuses to touch the real data directory.
- Release builds check GitHub Releases once at startup for updates. This is skipped in debug builds, when `--data-dir` is used, and when `MISSAL_NO_UPDATE_CHECK` is set.

Linux build dependencies (Debian and Ubuntu):

```bash
sudo apt-get install libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev \
  libx11-xcb-dev libxcb1-dev libfontconfig-dev libfreetype-dev libssl-dev libzstd-dev \
  clang cmake pkg-config mesa-vulkan-drivers
```

Running on Linux needs a Vulkan driver (`mesa-vulkan-drivers`, or the vendor driver). Without one the window cannot be created.

### Checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### Headless screenshots

The `headless` feature renders the real screens without a window:

```bash
cargo run -p missal-app --features headless --example shots -- --out OUTPUT_DIR [--theme light|dark]
cargo run -p missal-app --features headless --example editor_shots -- --out OUTPUT_DIR
cargo run -p missal-app --features headless --release --example perf -- 50000
```

### Release build and smoke test

```bash
cargo build --release -p missal-app
packaging/smoke.sh target/release/missal
```

`smoke.sh` copies the binary into an empty directory, seeds a scratch data directory and launches it under Xvfb, which proves nothing is read from the repository at run time. Fonts, icons, templates and gpui-kit assets are compiled in.

### Packaging and releasing

See [docs/packaging.md](docs/packaging.md): Velopack installer and updater, the Windows installer, the Linux AppImage and deb, SumatraPDF, licences, CI and the release steps. Native releases use `native-v*` tags so they never collide with the Electron `v*` tags.

Windows behaviour has not been verified on a real machine yet. [docs/windows-checklist.md](docs/windows-checklist.md) is the list to work through.

### Crates

- `missal-core`: domain types, field definitions, dates and the repository traits.
- `missal-store`: SQLite storage, migrations, embedded template bundle and seed data.
- `missal-text`: text layout and shaping on parley, embedded fonts, input widgets and labels.
- `missal-docx`: Word `.docx` import into editor documents.
- `missal-page`: paged layout and PDF output for printing.
- `missal-editor`: the rich-text document editor view and model.
- `missal-app`: the `missal` binary: shell, screens, preferences, printing and updates.

### House rules

- Code has no comments.
- Tests exist only where behaviour cannot be checked by running the app.
- Rust 1.100.0, edition 2024, no `unsafe`.

## Electron app

```bash
vp install
vp dev
vp check
vp test
vp run package
vp run make
vp run release
```

`package` writes an unpacked app, `make` writes the Windows NSIS installer to `release/<version>/`, and `release` publishes a draft GitHub release that `electron-updater` reads.
