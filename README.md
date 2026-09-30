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

## Bundled templates

Word files in `templates/` ship with the app. The file name is the template name. Use the original Word mail-merge documents: each `MERGEFIELD` becomes a field and its sample value is dropped. Fields typed as text between `«` and `»` (`«تھانہ_نام_»`) work too. Don't use copies re-saved through Google Docs or other converters; they lose page breaks, page size and fonts. After adding or changing a file, run:

```bash
vp run templates:build
```

This converts each file with the same importer as "Upload Word" and writes `bundled-templates/`, which is committed. It lists any field name it doesn't know (it becomes a custom field) and any token it could not convert. A test fails if a Word file changed without a rebuild.

On launch, Missal installs new bundled templates and updates ones the user hasn't edited. Templates the user edited or deleted are left alone. Renaming a Word file makes it a new template.

## Production

```bash
vp run package
vp run make
```

## Testing

```bash
vp run test
```
