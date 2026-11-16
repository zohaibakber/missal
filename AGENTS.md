# Missal

Rust 1.100.0 (`rust-toolchain.toml`), edition 2024, `unsafe` forbidden. No code comments. Tests only where behaviour cannot be checked by running the app.

Crates:

- `missal-core`: domain types, fields, dates, repository traits.
- `missal-store`: SQLite storage, migrations, embedded templates, seed data.
- `missal-text`: text layout and shaping on parley, embedded fonts, input widgets.
- `missal-docx`: `.docx` import.
- `missal-page`: paged layout and PDF output.
- `missal-editor`: rich-text editor view and model.
- `missal-auth`: WorkOS AuthKit sign-in (PKCE on `http://127.0.0.1:42155/callback`, which must be in the WorkOS Redirects list; no API key), session cached in the OS keyring, 30 days offline.
- `missal-library`: client for the shared template library (`MISSAL_LIBRARY_URL`, compile time or run time; unset hides the library). Calls carry the WorkOS access token.
- `missal-app`: the `missal` binary: shell, screens, preferences, printing, updates.

`cloud/`: the template library service, deployed with Alchemy v2 (exact beta pinned in `cloud/package.json`): a Worker in `cloud/src/worker.ts`, D1 catalog (`cloud/migrations`), R2 template blobs. It verifies WorkOS access tokens against the client's JWKS. Publishers are WorkOS users listed in `MISSAL_PUBLISHERS`, or with role `admin` or permission `templates:publish`; everyone signed in can list and download. A published template is a one-template `TemplateBundle`; installs are tracked in `library_links`.

Commands:

- Run: `cargo run -p missal-app -- --data-dir /tmp/missal-scratch [--seed 200]`. Always use a scratch `--data-dir`; `--seed` refuses the real one.
- Check: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Screenshots: `cargo run -p missal-app --features headless --example shots -- --out DIR` (also `editor_shots`, `perf`).
- Regenerate bundled templates from `templates/`: `cargo run -p missal-docx --example bundle` (writes `crates/missal-store/bundle/templates.bin`).
- Sign-in: set `WORKOS_CLIENT_ID` (compile time or run time) to gate the app; debug builds without it skip sign-in. Release builds refuse to compile without it.
- Release binary: `WORKOS_CLIENT_ID=client_… cargo build --release -p missal-app`, then `packaging/smoke.sh target/release/missal`.
- Library service: in `cloud/`, `npm ci`, `npm run typecheck`, `npm run deploy` (needs `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`, `WORKOS_CLIENT_ID`, `MISSAL_PUBLISHERS` in the environment or `cloud/.env`; see `cloud/.env.example`). Its URL goes in `MISSAL_LIBRARY_URL`.
- Set `CARGO_INCREMENTAL=0` and use the shared `target` directory only.

Releases (CI in `.github/workflows/ci.yml`, Velopack `vpk` version pinned there):

- Tags are `native-v*` and must equal the `missal-app` version. Old `v*` tags and the latest release belong to retired Electron installs, so never mark a native release as latest.
- Installers and updaters must never delete or move the data directory (`%APPDATA%\Missal`, `~/.config/Missal`).
- Restart for an update is allowed only when `Navigator::has_unsaved_changes` is false.
