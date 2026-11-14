# Missal

Rust 1.100.0 (`rust-toolchain.toml`), edition 2024, `unsafe` forbidden. No code comments. Tests only where behaviour cannot be checked by running the app.

Crates:

- `missal-core`: domain types, fields, dates, repository traits.
- `missal-store`: SQLite storage, migrations, embedded templates, seed data.
- `missal-text`: text layout and shaping on parley, embedded fonts, input widgets.
- `missal-docx`: `.docx` import.
- `missal-page`: paged layout and PDF output.
- `missal-editor`: rich-text editor view and model.
- `missal-app`: the `missal` binary: shell, screens, preferences, printing, updates.

Commands:

- Run: `cargo run -p missal-app -- --data-dir /tmp/missal-scratch [--seed 200]`. Always use a scratch `--data-dir`; `--seed` refuses the real one.
- Check: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Screenshots: `cargo run -p missal-app --features headless --example shots -- --out DIR` (also `editor_shots`, `perf`).
- Regenerate bundled templates from `templates/`: `cargo run -p missal-docx --example bundle` (writes `crates/missal-store/bundle/templates.bin`).
- Release binary: `cargo build --release -p missal-app`, then `packaging/smoke.sh target/release/missal`.
- Set `CARGO_INCREMENTAL=0` and use the shared `target` directory only.

Releases (CI in `.github/workflows/ci.yml`, Velopack `vpk` version pinned there):

- Tags are `native-v*` and must equal the `missal-app` version. Old `v*` tags and the latest release belong to retired Electron installs, so never mark a native release as latest.
- Installers and updaters must never delete or move the data directory (`%APPDATA%\Missal`, `~/.config/Missal`).
- Restart for an update is allowed only when `Navigator::has_unsaved_changes` is false.
