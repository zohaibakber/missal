# Missal

Desktop writing app built with Electron Forge, Vite+, React, and TanStack Router.

This follows the official [Electron Forge Vite + TypeScript](https://www.electronforge.io/templates/vite-+-typescript) template: `electron-forge start` for development, and Forge `package` / `make` for distributables.

## Development

```bash
vp install
vp run dev
```

`dev` (and `start`) run `electron-forge start`, which compiles main, preload, and renderer through `@electron-forge/plugin-vite` and opens the desktop app. That is the only way to run Missal.

Vite+ still owns `vp check`, `vp test`, and `vp fmt`. Do not use `vp dev` — that is Vite+'s built-in web server, not the product.

## Production

```bash
vp run package
vp run make
```

## Testing

```bash
vp run test
```
