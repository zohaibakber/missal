# Missal

Rust 1.100.0 (`rust-toolchain.toml`), edition 2024, `unsafe` forbidden. No code comments. Tests only where behaviour cannot be checked by running the app.

Crates:

- `missal-core`: domain types, fields, dates, repository traits.
- `missal-store`: SQLite storage, migrations, embedded templates, seed data.
- `missal-text`: text layout and shaping on parley, embedded fonts, input widgets.
- `missal-docx`: `.docx` import.
- `missal-page`: paged layout and PDF output.
- `missal-editor`: rich-text editor view and model.
- `missal-auth`: email and password accounts against the Missal API (`MISSAL_API_URL`, compile time or run time): sign-in, rotating refresh sessions through `SessionKeeper`, password changes and the admin user calls. The session is cached in the OS keyring, 30 days offline.
- `missal-library`: client for the shared template library on the same API. Calls carry the access token from `SessionKeeper`.
- `missal-app`: the `missal` binary: shell, screens, preferences, printing, updates.

`cloud/`: the Missal API, deployed with Alchemy v2 (exact beta pinned in `cloud/package.json`) as an Effect `HttpApi` Worker in `cloud/src`: `Api.ts` contract, `Accounts.ts` users and sessions, `Passwords.ts` PBKDF2-SHA256 at 100,000 iterations (the Workers WebCrypto ceiling), `Tokens.ts` 15-minute HS256 access tokens, `Auth.ts` middleware, `Library.ts` templates, `Handlers.ts`, `Worker.ts` bindings. D1 holds users, sessions, sign-in lockouts and the template catalog (`cloud/migrations`); R2 holds template blobs. Accounts are created by administrators with a temporary password that must be changed at first sign-in; there is no public sign-up. Roles are member, publisher (may publish templates) and admin (also manages people). The first sign-in that matches `MISSAL_ADMIN_EMAIL` and `MISSAL_ADMIN_PASSWORD` while no user exists creates the first admin. Refresh tokens rotate, and reusing an old one ends that session. A published template is a one-template `TemplateBundle`; installs are tracked in `library_links`.

Commands:

- Run: `cargo run -p missal-app -- --data-dir /tmp/missal-scratch [--seed 200]`. Always use a scratch `--data-dir`; `--seed` refuses the real one.
- Check: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Screenshots: `cargo run -p missal-app --features headless --example shots -- --out DIR` (also `editor_shots`, `perf`).
- Regenerate bundled templates from `templates/`: `cargo run -p missal-docx --example bundle` (writes `crates/missal-store/bundle/templates.bin`).
- Sign-in: set `MISSAL_API_URL` (compile time, run time or the root `.env`) to gate the app; debug builds without it skip sign-in and hide the library. Release builds refuse to compile without it. Only `https://` URLs are accepted, plus `http://127.0.0.1` and `http://localhost` for a local `npm run dev`.
- Release binary: `MISSAL_API_URL=https://… cargo build --release -p missal-app`, then `packaging/smoke.sh target/release/missal`.
- API service: in `cloud/`, `npm ci`, `npm run typecheck`, then `npm run dev` (local workerd, D1 and R2) or `npm run deploy`. Both need `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` (state lives in Cloudflare), `MISSAL_TOKEN_SECRET` (32 random bytes, base64url: `node -e "console.log(crypto.randomBytes(32).toString('base64url'))"`), `MISSAL_ADMIN_EMAIL` and `MISSAL_ADMIN_PASSWORD` in the environment or `cloud/.env` (see `cloud/.env.example`). Changing `MISSAL_TOKEN_SECRET` signs everyone out within 15 minutes. Sign-in hashes a password with 100,000 PBKDF2 rounds, which may exceed the Workers free plan's per-request CPU limit; Workers Paid avoids that.
- Set `CARGO_INCREMENTAL=0` and use the shared `target` directory only.

Releases (CI in `.github/workflows/ci.yml`, Velopack `vpk` version pinned there):

- Tags are `native-v*` and must equal the `missal-app` version. Old `v*` tags and the latest release belong to retired Electron installs, so never mark a native release as latest.
- Installers and updaters must never delete or move the data directory (`%APPDATA%\Missal`, `~/.config/Missal`).
- Restart for an update is allowed only when `Navigator::has_unsaved_changes` is false.
