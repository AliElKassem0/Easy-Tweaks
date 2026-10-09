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
    All current tweaks are HKCU. Each value is a `Setting { name, on, default }`, typed as
    `Sz("0")` (REG_SZ) or `Dword(0)` (REG_DWORD); `default: None` = missing on a fresh install.
  - `apply()` / `revert()` hold the logic (take `&mut AppState`, testable without Tauri);
    the `apply_tweak` / `revert_tweak` commands just lock the state and call them.
  - Toggle is on only when Easy Tweaks applied it (`applied` = backup exists + values match).
    Values already in Windows but not from us: toggle off + "Already set on this PC" note
    (`already_set`). Apply/revert return the new `TweakInfo` so the row shows Rust's state.
  - Revert uses the backup; if there is none (backups.json deleted), the Windows defaults.
  - `FakeRegistry::new(defaults)` is seeded from the `default`s in TWEAKS.
  - `registry.rs` has `enum RegValue { Sz(String), Dword(u32) }` (serde `untagged`, so
    backups.json stores `"400"` / `1` / `null`). The const-friendly `Value` in lib.rs
    converts with `to_reg()`.
  - `AppState { registry: Box<dyn Registry>, backups: HashMap<id, Vec<(name, Option<old value>)>> }`
    in a `Mutex`, managed by Tauri.
  - Commands: `list_tweaks(page)` (returns id, name, description, applied),
    `apply_tweak(id)` (backs up old values only the first time, then writes),
    `revert_tweak(id)` (restores the backup, or deletes the value if it didn't exist before).
- Tested on the Windows PC: works, but that PC already had all tweaks set
  (shows "Already set on this PC"). Tested on a clean laptop: all 33 tweaks
  write their values, and revert restores them. Bing search and Peek are kept even if
  Windows 11 may ignore them (decision: they still help on Windows 10).
- `src-tauri/src/stats.rs`: Dashboard data via `sysinfo` 0.39 (feature `system` only).
  `Stats(Mutex<System>)` is managed by Tauri (CPU usage = diff between refreshes).
  Commands: `live_stats` (cpu %, ram used/total in bytes), `system_info` (CPU name, threads, OS).
  Dashboard polls `live_stats` every 1 s; GB = 1024³ like Task Manager.
- `src/App.tsx`: sidebar (Dashboard, Input, Visual, Windows, Debloat) using `useState`.
  `TweakList` owns the page's tweaks (state lifted up): sections, "X of N active",
  Apply all / Revert all (sequential, per-row errors). `TweakRow` only displays.
  Every toggle and Apply all / Revert all show a `Toast` (bottom-right): spinner + progress
  while Apply all runs, then "<name> applied" / "X tweaks applied" + text based on
  `TweakInfo.effect`: any "signout" -> "Please restart your PC to see the full changes";
  only "explorer" -> a **Restart Explorer** button (`restart_explorer` command); all "now" ->
  "All changes are active now". Warning if some failed. Closes after 5 s (10 s while the
  button waits; CSS countdown, paused on hover/focus/restart). `key={report.run}` = new toast per run.
  Dashboard: CPU, RAM, active tweaks (`tweak_summary`) tiles.
- `src/App.css`: "Crimson" theme using CSS variables in `:root`
  (`--bg`, `--panel`, `--line`, `--hover`, `--accent`, `--text`, `--muted`, `--warn`).
- `src-tauri/src/backups.rs`: backups are saved to `backups.json` in Tauri's app data
  folder (written before the registry is touched, atomic temp-file + rename) and loaded
  at startup in `.setup()`. A corrupt file is moved to `backups.json.broken`.
- The apply → backup → revert cycle works and has been tested in the terminal.
- 33 HKCU tweaks (39 registry values) on 4 pages: Input 7, Visual 5, Windows 8, Debloat 13.
  Each `Tweak` has a `section` (heading on its page). Tweak IDs must never change:
  backups.json is keyed by ID (that's why "Disable Game Bar Capture" has id `game_dvr_off`).
  Full triage of planned/dropped tweaks: ROADMAP.md.
- One `key` per tweak. A tweak that needs values under two keys needs a refactor first
  (not done yet: no tweak needs it). Backups store values by name only, so value names
  must be unique within a tweak, and never add a value to an existing tweak (old backups
  wouldn't have it): make a new tweak with a new ID instead.
- Each `Tweak` has `effect` (when it shows up) and `windows` (which versions):
  - `Now(Live::X)` (8): `src-tauri/src/live.rs` pushes it live via `SystemParametersInfo`
    (`windows-sys` 0.61): mouse accel, hover time, sticky/filter/toggle keys shortcut,
    keyboard delay, window animations, menu delay. `push_live()` in lib.rs runs after apply
    and revert, reads the values back from the registry; best effort (failure only prints).
    Accessibility tweaks only change bit 0x4 of the live Flags.
  - `Explorer` (7): taskbar search, Task View, file extensions, hidden files, Snap Assist,
    Shake, Peek. Best guess, still to confirm on Windows. `src-tauri/src/explorer.rs`:
    `taskkill /f /im explorer.exe`, waits for the taskbar (`Shell_TrayWnd`) to come back,
    starts `explorer.exe` itself if Windows doesn't within 3 s.
  - `SignOut` (18): everything else.
  - `windows: Win10` (start_suggestions_off) / `Win11` (start_recommendations_off) / `Any`.
    `visible()` hides a tweak on the other version, unless it has a backup (so it can
    always be reverted). Build number via `sysinfo::System::kernel_version()`;
    22000+ = Windows 11. On macOS: 22631, or `EASY_TWEAKS_FAKE_BUILD=19045 npm run tauri dev`.
  - On macOS `[fake live] ...` / `[fake explorer] restart` are printed instead.
- Tests in lib.rs (`cargo test`, run on Mac and in CI): tweak table checks, apply/revert
  cycle on FakeRegistry, version filter. Tests only apply non-`Now` tweaks, so CI never
  changes real live settings.

## Next steps
1. Test on Windows (CI build): the 8 `Now` tweaks work without signing out; the 7
   `Explorer` tweaks show up after Restart Explorer (move any that don't to `SignOut`);
   Start Recommendations is hidden on Windows 10 / Start Suggestions on Windows 11.
2. Admin-only tweaks later, with a clear "needs admin" label. Includes the Windows 11
   web-search fix `DisableSearchBoxSuggestions` (HKCU\Software\Policies\... is normally
   read-only without admin, to confirm; so it belongs here).

## Rules for tweaks
- Only include tweaks with real, verifiable effects. No inflated claims like
  "10–25% FPS" or "10–40 ms ping" without benchmarks.
- Label honestly: HKCU (user-only) vs HKLM (system-wide, needs admin).
- Every tweak must be fully revertible.

## How I want you to work
- Answer in English, short and concrete. Show the actual values or code
  instead of abstract explanations.
- After each change, explain what changed and why, step by step.
- Write the code yourself; don't hand me tasks. Still explain what changed so I learn.