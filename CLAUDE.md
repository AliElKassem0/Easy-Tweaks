# Easy Tweaks: project context

## What this is
A Windows gaming tweak app ("Easy Tweaks") that flips Windows registry settings
(mouse acceleration, sticky keys, menu delay, etc.) with toggles, and can revert
every change from a backup. I'm a CS student building it to learn, so explain
what you change instead of only writing code.

## Setup
- I code on a **MacBook Pro**, VS Code. I test the real app on my **Windows PC**.
- Stack: **Tauri 2 + React + TypeScript** (frontend in `src/`, Rust backend in `src-tauri/`).
- Run with `npm run tauri dev`.
- Plan: build the .exe with GitHub Actions later, then test it on the PC.

## Key design decision: fake registry (mock)
macOS has no Windows registry, and the `winreg` crate only compiles on Windows.
So the registry is behind a trait:
- `src-tauri/src/registry.rs`: `trait Registry { read, write, delete }` plus
  `FakeRegistry` (in-memory HashMap seeded with Windows default values,
  prints every write/delete to the terminal as `[fake registry] ...`).
- `src-tauri/src/windows_registry.rs`: the real `WindowsRegistry` (`winreg` 0.56, HKCU only),
  compiled only on Windows. `make_registry()` in lib.rs picks it with `#[cfg(windows)]`,
  otherwise `FakeRegistry`. Trait methods return `Result`; `read` returns `Ok(None)` for
  missing values and `Err` for unsupported types (so revert never deletes a value it didn't understand).
- The Windows code can't be fully built on the Mac (Tauri's build script needs `rc.exe`).
  CI (`.github/workflows/build-windows.yml`) runs `cargo test` + `tauri build` on windows-latest
  and uploads `easy-tweaks.exe` and the NSIS installer as artifacts.

## Current state
- `src-tauri/src/lib.rs`:
  - `const TWEAKS: &[Tweak]` with fields id, page, name, description, key, values.
    All current tweaks are HKCU. Values are typed: `Sz("0")` (REG_SZ) or `Dword(0)` (REG_DWORD).
  - `registry.rs` has `enum RegValue { Sz(String), Dword(u32) }` (serde `untagged`, so
    backups.json stores `"400"` / `1` / `null`). The const-friendly `Value` in lib.rs
    converts with `to_reg()`.
  - `AppState { registry: Box<dyn Registry>, backups: HashMap<id, Vec<(name, Option<old value>)>> }`
    in a `Mutex`, managed by Tauri.
  - Commands: `list_tweaks(page)` (returns id, name, description, applied),
    `apply_tweak(id)` (backs up old values only the first time, then writes),
    `revert_tweak(id)` (restores the backup, or deletes the value if it didn't exist before).
- `src/App.tsx`: sidebar (Dashboard, Tweaks, Input, Debloat) using `useState`.
  Each page calls `list_tweaks` and renders `TweakRow` components with a toggle.
- `src/App.css`: "Crimson" theme using CSS variables in `:root`
  (`--bg`, `--panel`, `--line`, `--hover`, `--accent`, `--text`, `--muted`).
- `src-tauri/src/backups.rs`: backups are saved to `backups.json` in Tauri's app data
  folder (written before the registry is touched, atomic temp-file + rename) and loaded
  at startup in `.setup()`. A corrupt file is moved to `backups.json.broken`.
- The apply → backup → revert cycle works and has been tested in the terminal.
- 9 tweaks: mouse accel, Sticky/Filter/Toggle Keys shortcuts, key repeat delay (Input);
  menu delay, show file extensions (Tweaks); Game DVR capture, Bing in Start search (Debloat).
- One `key` per tweak. A tweak that needs values under two keys needs a refactor first.

## Next steps
1. Push to GitHub, run the workflow, test all 9 tweaks on the Windows PC
   (check Bing search on Windows 11; remove it if Windows ignores it).
2. Apply settings immediately (`SystemParametersInfo`) instead of after sign-out.
3. Admin-only (HKLM) tweaks later, with a clear "needs admin" label.
4. Dashboard: live CPU/RAM stats.

## Rules for tweaks
- Only include tweaks with real, verifiable effects. No inflated claims like
  "10–25% FPS" or "10–40 ms ping" without benchmarks.
- Label honestly: HKCU (user-only) vs HKLM (system-wide, needs admin).
- Every tweak must be fully revertible.

## How I want you to work
- Answer in English, short and concrete. Show the actual values or code
  instead of abstract explanations.
- After each change, explain what changed and why, step by step.
- Let me write small parts myself (like adding new tweaks) so I learn.