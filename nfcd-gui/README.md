# nfcd-gui

Menu bar app for nfcd (Tauri v2 + React + shadcn/ui). Work in progress.

The app controls the watcher through the `nfcd` CLI, so install that first
(`brew install chanungdev/tap/nfcd`).

```bash
pnpm install
pnpm tauri dev                          # run
pnpm tauri build --debug --bundles app  # build nfcd.app
```

`src-tauri` is excluded from the root Cargo workspace because building it
requires the frontend output in `dist/`.

Menu bar icons are rendered from Lucide (`file-type-corner` / `file-x-corner`).
To change them, edit `scripts/tray-icons.mjs` and run `node scripts/tray-icons.mjs`.
