# Easy Tweaks

A small Windows desktop app that applies useful Windows settings with one click each, and can undo every one of them. Built with **Tauri 2**, **Rust** and **React**.

**[Download the latest version](https://github.com/AliElKassem0/Easy-Tweaks/releases/latest)** (Windows 10 / 11)

## What it does

Lots of "Windows tweak" tools change dozens of registry values at once, some of them myths, some of them hard to undo. Easy Tweaks takes the opposite approach:

- **33 tweaks**, each a single toggle, across four pages:
  - **Input:** mouse acceleration, hover time, Sticky / Filter / Toggle Keys shortcuts, …
  - **Visual:** window animations, transparency, Aero Peek, menu delay, …
  - **Windows:** taskbar search box, Task View button, file extensions, hidden files, …
  - **Debloat:** suggested apps, tips, web results in Start search, advertising ID, Game DVR / Game Bar, …
- **Everything can be undone.** Before a value is changed, the original is saved to `backups.json`. Revert puts back exactly what was there.
- **No admin rights needed.** All tweaks only touch your own user's settings (`HKEY_CURRENT_USER`).
- **Takes effect right away where possible.** 8 tweaks are pushed to Windows live, the rest tell you whether they need a sign-out or an Explorer restart (with a button for the restart).
- **Apply all / Revert all** per page, with a progress toast.
- **Dashboard** with live CPU and RAM usage and system info.
- Tweaks that don't exist on your Windows version are hidden.

Which tweaks made it in, and which were dropped as myths or harmful, is documented in [ROADMAP.md](ROADMAP.md).

## Installing

1. Download `Easy Tweaks_x.y.z_x64-setup.exe` from the [latest release](https://github.com/AliElKassem0/Easy-Tweaks/releases/latest).
2. Run it. The app isn't code-signed, so Windows SmartScreen will warn you. Click **More info → Run anyway**.

## Building from source

You need [Node.js](https://nodejs.org/) 22+, [Rust](https://rustup.rs/) and the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

```bash
npm install
npm run tauri dev     # run the app in development mode
npm run tauri build   # build the installer
cargo test --manifest-path src-tauri/Cargo.toml   # Rust tests
```

On macOS and Linux the app runs against a fake in-memory registry, so you can work on the UI without Windows.

## Project structure

| Path | What it does |
|---|---|
| `src/App.tsx` | The React UI: pages, toggles, dashboard, toast |
| `src-tauri/src/lib.rs` | The tweak list and the commands the UI calls (apply, revert, list) |
| `src-tauri/src/windows_registry.rs` | Reads and writes the real Windows registry |
| `src-tauri/src/backups.rs` | Saves original values to `backups.json` (written atomically) |
| `src-tauri/src/live.rs` | Applies supported tweaks immediately via `SystemParametersInfo` |
| `src-tauri/src/explorer.rs` | Restarts Windows Explorer |
| `src-tauri/src/stats.rs` | CPU, RAM and system info for the dashboard |

Every push to `main` builds and tests the app on a Windows machine with GitHub Actions. Pushing a version tag (e.g. `v0.1.0`) also publishes a release with the installer.

## License

[MIT](LICENSE)
