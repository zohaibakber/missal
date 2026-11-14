<!--VITE PLUS START-->

# Using Vite+, the Unified Toolchain for the Web

This project is using Vite+, a unified toolchain built on top of Vite, Rolldown, Vitest, tsdown, Oxlint, Oxfmt, and Vite Task. Vite+ wraps runtime management, package management, and frontend tooling in a single global CLI called `vp`. Vite+ is distinct from Vite, and it invokes Vite through `vp dev` and `vp build`. Run `vp help` to print a list of commands and `vp <command> --help` for information about a specific command.

Docs are local at `node_modules/vite-plus/docs` or online at https://viteplus.dev/guide/.

## Built-in Commands vs Scripts

`vp <name>` runs a built-in command. `vp run <name>` runs a `package.json` script or a `vite.config.ts` task. Scripts cannot overwrite built-ins, so `vp dev` and `vp run dev` may do different things. Check `package.json` and `vite.config.ts` first, and run `vp run <name>` when the project defines a script or task with that name.

## Running Scripts

Run TypeScript and JavaScript files with `vpx <file>`, for example `vpx ./scripts/seed.ts --dry-run`. It handles enums, decorators, JSX, and tsconfig `paths`, so do not add `tsx` or `ts-node` to run scripts. `vpx --watch <file>` reruns the file when a file it uses changes, in place of `tsx watch`. `vp node <file>` runs plain Node.js.

## Tool Versions

Run `vp toolchain` to show versions and relationships in the active Vite+
release. Add a tool name to select part of the graph. For example, run
`vp toolchain vite`. Use `--global` to ignore the local `vite-plus` package. Use
`vp why <package>` to show the package-manager dependency graph.

## Review Checklist

- [ ] Run `vp install` after pulling remote changes and before getting started.
- [ ] Run `vp check` and `vp test` to format, lint, type check and test changes.
- [ ] Check if there are `vite.config.ts` tasks or `package.json` scripts necessary for validation, run via `vp run <script>`.
- [ ] If setup, runtime, or package-manager behavior looks wrong, run `vp env doctor` and include its output when asking for help.

<!--VITE PLUS END-->

# Missal

Missal is an Electron-only desktop app. `vp dev` builds main, preload and the storage worker through `vite-plugin-electron` and launches Electron with the renderer on the Vite dev server. `vp build` writes `dist` and `dist-electron`. `vp run package`, `vp run make` and `vp run release` hand those to `electron-builder` (`electron-builder.yml`).

The renderer talks to the SQLite storage worker through Effect RPC (`StorageRpcs` in `src/electron/storage-rpc.ts`) over a MessagePort that main relays. Add a storage operation by adding a repository method, an `Rpc` with that method's argument as payload, a handler in `storage-handlers.ts`, and a client mapping in `src/repositories/rpc.ts`.

Code has no comments. Tests exist only where behavior cannot be checked by running the app.

# Native app (Rust and GPUI)

The Rust rewrite lives in `crates/` and is the future of the app. The Electron app above is still shipped and must keep working: do not delete or change it, `ci.yml`, `electron-builder.yml` or `package.json` as part of native work. Retiring it is the owner's decision.

Rust 1.100.0 (`rust-toolchain.toml`), edition 2024, `unsafe` forbidden. House rules are the same as above: no comments, tests only where behaviour cannot be checked by running the app.

Crate map:

- `missal-core`: domain types, field definitions, dates, repository traits.
- `missal-store`: SQLite storage, migrations, embedded templates and seed data.
- `missal-text`: text layout and shaping on parley, embedded fonts, input widgets.
- `missal-docx`: `.docx` import.
- `missal-page`: paged layout and PDF output.
- `missal-editor`: rich-text editor view and model.
- `missal-app`: the `missal` binary: shell, screens, preferences, printing, updates.

Commands:

- Run: `cargo run -p missal-app -- --data-dir /tmp/missal-scratch [--seed 200]`. Always use a scratch `--data-dir`; `--seed` refuses the real directory.
- Check: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Screenshots: `cargo run -p missal-app --features headless --example shots -- --out DIR` (also `editor_shots`, `perf`).
- Release binary: `cargo build --release -p missal-app`, then `packaging/smoke.sh target/release/missal`.
- Set `CARGO_INCREMENTAL=0` and use the shared `target` directory only; GPUI builds are large.

Packaging and updates:

- Installer and updater are Velopack (`vpk`), pinned in `.github/workflows/native.yml`. Pack scripts are in `packaging/`. Details, licences and release steps are in `docs/packaging.md`.
- Native releases use `native-v*` tags (the Electron ones use `v*`). The tag must equal the `missal-app` version.
- Update logic: `crates/missal-app/src/update/`. `release.rs` is pure and unit-tested. `installer.rs` is the only file that touches Velopack. `controller.rs` shows the toast. Restart is allowed only when `Navigator::has_unsaved_changes` is false.
- Windows resources (icon, version info) are embedded by `crates/missal-app/build.rs`, Windows targets only.
- The data directory (`%APPDATA%\Missal`) is shared with the Electron app. Installers and updaters must never delete or move it.
- Windows behaviour is unverified until `docs/windows-checklist.md` has been worked through on a real machine.
