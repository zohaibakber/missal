# Missal

Electron desktop app for writing First Information Reports, built with Vite+, React, TanStack Router, Lexical and Effect.

## Development

```bash
vp install
vp dev
```

## Distribution

```bash
vp run package
vp run make
vp run release
```

`package` writes an unpacked app, `make` writes the Windows NSIS installer to `release/<version>/`, and `release` publishes a draft GitHub release that `electron-updater` reads.

## Checks

```bash
vp check
vp test
```
