# Missal

Desktop app for writing First Information Reports in Urdu, built in Rust with GPUI. Windows first, Linux second.

```bash
cargo run -p missal-app -- --data-dir /tmp/missal-scratch [--seed 200]
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo build --release -p missal-app && packaging/smoke.sh target/release/missal
```

Linux needs `libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev libx11-xcb-dev libxcb1-dev libfontconfig-dev libfreetype-dev libssl-dev libzstd-dev clang cmake pkg-config mesa-vulkan-drivers`.

Packaging is Velopack: `packaging/windows/pack.ps1` and `packaging/linux/pack.sh`, run by CI on `native-v*` tags.

Data lives in `missal.db` and `missal-preferences.json` under `%APPDATA%\Missal` (`~/.config/Missal` on Linux).
