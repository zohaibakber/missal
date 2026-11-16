# Missal

Rust 1.100.0 (`rust-toolchain.toml`), edition 2024, `unsafe` forbidden. No code comments. Tests only where behaviour cannot be checked by running the app.

Crates:

- `missal-core`: domain types, fields, dates, repository traits.
- `missal-store`: SQLite storage, migrations, embedded templates, seed data.
- `missal-text`: text layout and shaping on parley, embedded fonts, input widgets.
- `missal-docx`: `.docx` import.
- `missal-page`: paged layout and PDF output.
- `missal-editor`: rich-text editor view and model.
- `missal-auth`: self-serve email and password accounts against the Missal API (`MISSAL_API_URL`, compile time or run time): sign-up, emailed codes to verify an address or reset a password, sign-in, rotating refresh sessions through `SessionKeeper`, password changes, the account's licence (trial, licensed or expired) and the admin calls (list, update, grant a year). `MISSAL_PURCHASE_URL` (optional, compile time or run time) is the buy-a-licence link. The session is cached in the OS keyring, 30 days offline while the licence is active.
- `missal-library`: client for the shared template library on the same API. Calls carry the access token from `SessionKeeper`.
- `missal-app`: the `missal` binary: shell, screens, preferences, printing, updates.

`cloud/`: the Missal API, deployed with Alchemy v2 (exact beta pinned in `cloud/package.json`) as an Effect `HttpApi` Worker. `cloud/src`: `Api.ts` contract (served as OpenAPI at `/openapi.json`), `Models.ts` `Model.Class` rows (password fields are `Model.Sensitive`, so they never reach JSON), `Accounts.ts` users, sessions and email codes, `Library.ts` templates, `Passwords.ts` PBKDF2-SHA256 at 100,000 iterations (the Workers WebCrypto ceiling), `Tokens.ts` 15-minute HS256 access tokens, `Auth.ts` middleware, `Handlers.ts`, `App.ts` wiring, and ports with Cloudflare and in-memory adapters: `Database.ts` (`Atomic` multi-statement writes: D1 `batch` live, a transaction in tests), `Blobs.ts` (R2), `Throttle.ts` (Workers Rate Limit binding), `Mailer.ts` (Resend over `HttpClient`, a log adapter, and an in-memory outbox for tests). `Worker.ts` binds D1, R2, the rate limit (10 public auth calls per minute per IP, namespace 4721), the mailer and a nightly cron that purges expired sessions and codes and unverified sign-ups older than 7 days. Anyone signs up with name, email and password; a 6-digit code (10 minutes, 60 s resend cooldown, 5 tries) confirms the email, signs them in and starts a 7-day trial. Sign-up and code requests answer 202 whether or not the email has an account. Forgot password mails a reset code; resetting signs in and ends every other session. The licence is derived from `trial_ends_at` and `paid_until` by `licenceOf` in `Models.ts`, never stored; once it ends, sign-in and refresh still work but the catalog and publishing groups fail `LicenceRequired`. Payment is manual: an admin grants a year (`POST /v1/users/:id/licence`), extending from the later of now and the current paid end. Roles are member, publisher (may publish templates) and admin (also manages people, always licensed). Admins are made by hand in D1: sign up, then `UPDATE users SET role = 'admin' WHERE email = '…'`. Five wrong passwords lock an email for 15 minutes. Refresh tokens rotate, and reusing an old one ends that session. A published template is a one-template `TemplateBundle`; installs are tracked in `library_links`.

Commands:

- Run: `cargo run -p missal-app -- --data-dir /tmp/missal-scratch [--seed 200]`. Always use a scratch `--data-dir`; `--seed` refuses the real one.
- Check: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Screenshots: `cargo run -p missal-app --features headless --example shots -- --out DIR` (also `editor_shots`, `perf`).
- Regenerate bundled templates from `templates/`: `cargo run -p missal-docx --example bundle` (writes `crates/missal-store/bundle/templates.bin`).
- Sign-in: set `MISSAL_API_URL` (compile time, run time or the root `.env`) to gate the app; debug builds without it skip sign-in and hide the library. Release builds refuse to compile without it. Only `https://` URLs are accepted, plus `http://127.0.0.1` and `http://localhost` for a local `vp run dev`.
- Release binary: `MISSAL_API_URL=https://… cargo build --release -p missal-app`, then `packaging/smoke.sh target/release/missal`.
- API service: in `cloud/` (Node 24.11.1 or later), TypeScript tooling is Vite+ (`vp`, config in `vite.config.ts`) on pnpm (pinned in `devEngines`, catalog and `allowBuilds` in `pnpm-workspace.yaml`): `vp install`, `vp check` (Oxfmt, Oxlint and type checks), `vp test` (HttpApiTest over in-memory SQLite with the real migrations), then `vp run dev` (local workerd, D1, R2, rate limit and cron) or `vp run deploy`. Sign in to Cloudflare once with `vp exec alchemy profile edit --add Cloudflare` (saved to `~/.alchemy/profiles.json`; state lives in Cloudflare); CI uses `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` instead. Email goes through Resend: set `RESEND_API_KEY` and `MISSAL_MAIL_FROM` (an address on a domain verified in Resend) in the environment or `cloud/.env` (see `cloud/.env.example`). Without either one the Worker logs each email, codes included, as a warning instead of sending it, so dev and early deploys still work. The token signing secret is generated by `Alchemy.makeRandom` on first deploy and kept in state; destroying and recreating the stack signs everyone out. Sign-in hashes a password with 100,000 PBKDF2 rounds, which may exceed the Workers free plan's per-request CPU limit; Workers Paid avoids that.
- Set `CARGO_INCREMENTAL=0` and use the shared `target` directory only.

Releases (CI in `.github/workflows/ci.yml`, Velopack `vpk` version pinned there):

- Tags are `native-v*` and must equal the `missal-app` version. Old `v*` tags and the latest release belong to retired Electron installs, so never mark a native release as latest.
- Installers and updaters must never delete or move the data directory (`%APPDATA%\Missal`, `~/.config/Missal`).
- Restart for an update is allowed only when `Navigator::has_unsaved_changes` is false.
