mod backups;
mod registry;
mod stats;
#[cfg(windows)]
mod windows_registry;

use backups::Backups;
use registry::{RegValue, Registry};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

// The value a tweak writes. Same as RegValue, but with &'static str instead of String,
// because a `const` can't create a String (that needs memory allocation at runtime).
enum Value {
    Sz(&'static str),
    Dword(u32),
}
use Value::{Dword, Sz};

impl Value {
    fn to_reg(&self) -> RegValue {
        match self {
            Sz(s) => RegValue::Sz(s.to_string()),
            Dword(n) => RegValue::Dword(*n),
        }
    }
}

// One registry value that a tweak changes
struct Setting {
    name: &'static str,
    on: Value,              // what the tweak writes
    default: Option<Value>, // the Windows default. None = doesn't exist on a fresh install
}

// One tweak = a registry key + the values it sets
struct Tweak {
    id: &'static str,
    page: &'static str, // which sidebar page shows it: "tweaks" | "input" | "debloat"
    name: &'static str,
    description: &'static str,
    key: &'static str,
    values: &'static [Setting],
}

const TWEAKS: &[Tweak] = &[
    Tweak {
        id: "mouse_accel_off",
        page: "input",
        name: "Disable Mouse Acceleration",
        description: "Linear 1:1 cursor movement (turns off \"Enhance pointer precision\")",
        key: r"Control Panel\Mouse",
        values: &[
            Setting { name: "MouseSpeed", on: Sz("0"), default: Some(Sz("1")) },
            Setting { name: "MouseThreshold1", on: Sz("0"), default: Some(Sz("6")) },
            Setting { name: "MouseThreshold2", on: Sz("0"), default: Some(Sz("10")) },
        ],
    },
    // The 3 accessibility "Flags" values are bit fields.
    // Bit 0x4 = "the keyboard shortcut is active". Each tweak clears only that bit.
    Tweak {
        id: "sticky_keys_shortcut_off",
        page: "input",
        name: "Disable Sticky Keys Shortcut",
        description: "Pressing Shift 5 times no longer opens the Sticky Keys popup",
        key: r"Control Panel\Accessibility\StickyKeys",
        // 510 = 0x1FE -> 506 = 0x1FA
        values: &[Setting { name: "Flags", on: Sz("506"), default: Some(Sz("510")) }],
    },
    Tweak {
        id: "filter_keys_shortcut_off",
        page: "input",
        name: "Disable Filter Keys Shortcut",
        description: "Holding right Shift for 8 seconds no longer turns on Filter Keys",
        key: r"Control Panel\Accessibility\Keyboard Response",
        // 126 = 0x7E -> 122 = 0x7A
        values: &[Setting { name: "Flags", on: Sz("122"), default: Some(Sz("126")) }],
    },
    Tweak {
        id: "toggle_keys_shortcut_off",
        page: "input",
        name: "Disable Toggle Keys Shortcut",
        description: "Holding Num Lock for 5 seconds no longer turns on Toggle Keys",
        key: r"Control Panel\Accessibility\ToggleKeys",
        // 62 = 0x3E -> 58 = 0x3A
        values: &[Setting { name: "Flags", on: Sz("58"), default: Some(Sz("62")) }],
    },
    Tweak {
        id: "keyboard_delay_min",
        page: "input",
        name: "Shortest Key Repeat Delay",
        description: "A held key starts repeating after ~250 ms instead of ~500 ms",
        key: r"Control Panel\Keyboard",
        // scale 0-3 (0 = ~250 ms, 3 = ~1 s)
        values: &[Setting { name: "KeyboardDelay", on: Sz("0"), default: Some(Sz("1")) }],
    },
    Tweak {
        id: "menu_delay_off",
        page: "tweaks",
        name: "Instant Menus",
        description: "Submenus open immediately instead of after a 400 ms hover delay",
        key: r"Control Panel\Desktop",
        // milliseconds
        values: &[Setting { name: "MenuShowDelay", on: Sz("0"), default: Some(Sz("400")) }],
    },
    Tweak {
        id: "show_file_extensions",
        page: "tweaks",
        name: "Show File Extensions",
        description: "File Explorer shows .exe, .txt, etc., so files like photo.jpg.exe are easy to spot",
        key: r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        // 1 = hide, 0 = show
        values: &[Setting { name: "HideFileExt", on: Dword(0), default: Some(Dword(1)) }],
    },
    Tweak {
        id: "game_dvr_off",
        page: "debloat",
        name: "Disable Game DVR Capture",
        description: "Turns off Xbox Game Bar game capture (recording clips and screenshots)",
        key: r"Software\Microsoft\Windows\CurrentVersion\GameDVR",
        // doesn't exist until changed in Settings; missing = capture allowed
        values: &[Setting { name: "AppCaptureEnabled", on: Dword(0), default: None }],
    },
    Tweak {
        id: "bing_search_off",
        page: "debloat",
        name: "Disable Web Results in Start Search",
        description: "Start menu search shows only local results, no Bing web results",
        key: r"Software\Microsoft\Windows\CurrentVersion\Search",
        // missing = web results on
        values: &[Setting { name: "BingSearchEnabled", on: Dword(0), default: None }],
    },
];

// Everything the app remembers while running
struct AppState {
    registry: Box<dyn Registry>,
    backups: Backups,
    backup_path: PathBuf, // where backups.json lives
}

fn find_tweak(id: &str) -> Result<&'static Tweak, String> {
    TWEAKS
        .iter()
        .find(|t| t.id == id)
        .ok_or(format!("Unknown tweak: {}", id))
}

// Does the registry currently hold the tweak's values? (no matter who set them)
fn values_set(tweak: &Tweak, registry: &dyn Registry) -> bool {
    tweak
        .values
        .iter()
        .all(|v| registry.read(tweak.key, v.name) == Ok(Some(v.on.to_reg())))
}

// Put backed-up values back. None = the value didn't exist before -> delete it.
fn restore(registry: &mut dyn Registry, key: &str, old: &[(String, Option<RegValue>)]) -> Result<(), String> {
    for (name, value) in old {
        match value {
            Some(v) => registry.write(key, name, v)?,
            None => registry.delete(key, name)?,
        }
    }
    Ok(())
}

// What the frontend gets for each row (serde turns it into JSON)
#[derive(serde::Serialize)]
struct TweakInfo {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    applied: bool,     // Easy Tweaks turned it on (has a backup) -> toggle on
    already_set: bool, // Windows already has these values, but not from us -> toggle off + note
}

fn info(s: &AppState, tweak: &'static Tweak) -> TweakInfo {
    let set = values_set(tweak, s.registry.as_ref());
    let ours = s.backups.contains_key(tweak.id);
    TweakInfo {
        id: tweak.id,
        name: tweak.name,
        description: tweak.description,
        applied: set && ours,
        already_set: set && !ours,
    }
}

#[tauri::command]
fn list_tweaks(page: &str, state: tauri::State<Mutex<AppState>>) -> Vec<TweakInfo> {
    let s = state.lock().unwrap();
    TWEAKS
        .iter()
        .filter(|t| t.page == page)
        .map(|t| info(&s, t))
        .collect()
}

// Turn a tweak on. Plain function (no Tauri types), so it can be tested on its own.
fn apply(s: &mut AppState, tweak: &Tweak) -> Result<(), String> {
    // 1. Back up the old values (only the first time) and save them to disk.
    //    This happens BEFORE touching the registry: if saving fails, nothing is changed.
    if !s.backups.contains_key(tweak.id) {
        // collect() into a Result stops at the first read error (`?` returns it)
        let old = tweak
            .values
            .iter()
            .map(|v| Ok((v.name.to_string(), s.registry.read(tweak.key, v.name)?)))
            .collect::<Result<Vec<_>, String>>()?;
        s.backups.insert(tweak.id.to_string(), old);
        if let Err(e) = backups::save(&s.backup_path, &s.backups) {
            s.backups.remove(tweak.id); // keep memory in sync with the file
            return Err(e);
        }
    }

    // 2. Write the new values. If one fails halfway (e.g. access denied),
    //    undo the ones already written so the tweak isn't left half-applied.
    for v in tweak.values {
        if let Err(e) = s.registry.write(tweak.key, v.name, &v.on.to_reg()) {
            let _ = restore(s.registry.as_mut(), tweak.key, &s.backups[tweak.id]);
            return Err(e);
        }
    }
    Ok(())
}

// Turn a tweak off
fn revert(s: &mut AppState, tweak: &Tweak) -> Result<(), String> {
    // 1. Restore the registry. If this fails, the backup is kept so you can try again.
    match s.backups.get(tweak.id) {
        // Normal case: put back exactly what was there before the app changed it
        Some(old) => restore(s.registry.as_mut(), tweak.key, old)?,
        // No backup: normally can't happen (the toggle is only on when there is one),
        // but if backups.json got deleted, Windows defaults are the safest fallback.
        None => {
            let defaults: Vec<(String, Option<RegValue>)> = tweak
                .values
                .iter()
                .map(|v| (v.name.to_string(), v.default.as_ref().map(Value::to_reg)))
                .collect();
            restore(s.registry.as_mut(), tweak.key, &defaults)?
        }
    }

    // 2. Only now forget the backup, so it's never lost before the restore is done
    if s.backups.remove(tweak.id).is_some() {
        backups::save(&s.backup_path, &s.backups)?;
    }
    Ok(())
}

// The commands React calls. They lock the state, hand over to apply/revert,
// and return the tweak's new state so the row shows what Rust sees.
#[tauri::command]
fn apply_tweak(id: &str, state: tauri::State<Mutex<AppState>>) -> Result<TweakInfo, String> {
    let tweak = find_tweak(id)?;
    let mut s = state.lock().unwrap();
    apply(&mut s, tweak)?;
    Ok(info(&s, tweak))
}

#[tauri::command]
fn revert_tweak(id: &str, state: tauri::State<Mutex<AppState>>) -> Result<TweakInfo, String> {
    let tweak = find_tweak(id)?;
    let mut s = state.lock().unwrap();
    revert(&mut s, tweak)?;
    Ok(info(&s, tweak))
}

// Pick the registry at compile time: the real one on Windows, the fake one everywhere else
#[cfg(windows)]
fn make_registry() -> Box<dyn Registry> {
    Box::new(windows_registry::WindowsRegistry)
}

// The fake starts with the Windows defaults of every tweak, like a fresh install
#[cfg(not(windows))]
fn make_registry() -> Box<dyn Registry> {
    let defaults = TWEAKS.iter().flat_map(|t| {
        t.values
            .iter()
            .filter_map(move |v| Some((t.key, v.name, v.default.as_ref()?.to_reg())))
    });
    Box::new(registry::FakeRegistry::new(defaults))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(stats::Stats::new())
        // setup runs once at startup. We need the app handle to find the app data folder,
        // so the state is created here instead of before the Builder.
        .setup(|app| {
            let backup_path = app.path().app_data_dir()?.join("backups.json");
            println!("[backups] using {}", backup_path.display());
            let state = AppState {
                registry: make_registry(),
                backups: backups::load(&backup_path),
                backup_path,
            };
            app.manage(Mutex::new(state));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_tweaks,
            apply_tweak,
            revert_tweak,
            stats::live_stats,
            stats::system_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}