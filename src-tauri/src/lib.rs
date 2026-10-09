mod backups;
mod registry;
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

// One tweak = a registry key + the values it sets
struct Tweak {
    id: &'static str,
    page: &'static str, // which sidebar page shows it: "tweaks" | "input" | "debloat"
    name: &'static str,
    description: &'static str,
    key: &'static str,
    values: &'static [(&'static str, Value)],
}

const TWEAKS: &[Tweak] = &[
    Tweak {
        id: "mouse_accel_off",
        page: "input",
        name: "Disable Mouse Acceleration",
        description: "Linear 1:1 cursor movement (turns off \"Enhance pointer precision\")",
        key: r"Control Panel\Mouse",
        values: &[
            ("MouseSpeed", Sz("0")),
            ("MouseThreshold1", Sz("0")),
            ("MouseThreshold2", Sz("0")),
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
        values: &[("Flags", Sz("506"))], // default 510 (0x1FE) -> 506 (0x1FA)
    },
    Tweak {
        id: "filter_keys_shortcut_off",
        page: "input",
        name: "Disable Filter Keys Shortcut",
        description: "Holding right Shift for 8 seconds no longer turns on Filter Keys",
        key: r"Control Panel\Accessibility\Keyboard Response",
        values: &[("Flags", Sz("122"))], // default 126 (0x7E) -> 122 (0x7A)
    },
    Tweak {
        id: "toggle_keys_shortcut_off",
        page: "input",
        name: "Disable Toggle Keys Shortcut",
        description: "Holding Num Lock for 5 seconds no longer turns on Toggle Keys",
        key: r"Control Panel\Accessibility\ToggleKeys",
        values: &[("Flags", Sz("58"))], // default 62 (0x3E) -> 58 (0x3A)
    },
    Tweak {
        id: "keyboard_delay_min",
        page: "input",
        name: "Shortest Key Repeat Delay",
        description: "A held key starts repeating after ~250 ms instead of ~500 ms",
        key: r"Control Panel\Keyboard",
        values: &[("KeyboardDelay", Sz("0"))], // scale 0-3, default 1
    },
    Tweak {
        id: "menu_delay_off",
        page: "tweaks",
        name: "Instant Menus",
        description: "Submenus open immediately instead of after a 400 ms hover delay",
        key: r"Control Panel\Desktop",
        values: &[("MenuShowDelay", Sz("0"))], // milliseconds, default 400
    },
    Tweak {
        id: "show_file_extensions",
        page: "tweaks",
        name: "Show File Extensions",
        description: "File Explorer shows .exe, .txt, etc., so files like photo.jpg.exe are easy to spot",
        key: r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        values: &[("HideFileExt", Dword(0))], // default 1 = hide
    },
    Tweak {
        id: "game_dvr_off",
        page: "debloat",
        name: "Disable Game DVR Capture",
        description: "Turns off Xbox Game Bar game capture (recording clips and screenshots)",
        key: r"Software\Microsoft\Windows\CurrentVersion\GameDVR",
        values: &[("AppCaptureEnabled", Dword(0))], // usually doesn't exist until changed in Settings
    },
    Tweak {
        id: "bing_search_off",
        page: "debloat",
        name: "Disable Web Results in Start Search",
        description: "Start menu search shows only local results, no Bing web results",
        key: r"Software\Microsoft\Windows\CurrentVersion\Search",
        values: &[("BingSearchEnabled", Dword(0))],
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

// Is the tweak currently on? (all values match)
fn is_applied(tweak: &Tweak, registry: &dyn Registry) -> bool {
    tweak
        .values
        .iter()
        .all(|(name, value)| registry.read(tweak.key, name) == Ok(Some(value.to_reg())))
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
    applied: bool,
}

#[tauri::command]
fn list_tweaks(page: &str, state: tauri::State<Mutex<AppState>>) -> Vec<TweakInfo> {
    let s = state.lock().unwrap();
    TWEAKS
        .iter()
        .filter(|t| t.page == page)
        .map(|t| TweakInfo {
            id: t.id,
            name: t.name,
            description: t.description,
            applied: is_applied(t, s.registry.as_ref()),
        })
        .collect()
}

#[tauri::command]
fn apply_tweak(id: &str, state: tauri::State<Mutex<AppState>>) -> Result<(), String> {
    let tweak = find_tweak(id)?;
    let mut s = state.lock().unwrap();

    // 1. Back up the old values (only the first time) and save them to disk.
    //    This happens BEFORE touching the registry: if saving fails, nothing is changed.
    if !s.backups.contains_key(id) {
        // collect() into a Result stops at the first read error (`?` returns it)
        let old = tweak
            .values
            .iter()
            .map(|(name, _)| Ok((name.to_string(), s.registry.read(tweak.key, name)?)))
            .collect::<Result<Vec<_>, String>>()?;
        s.backups.insert(id.to_string(), old);
        if let Err(e) = backups::save(&s.backup_path, &s.backups) {
            s.backups.remove(id); // keep memory in sync with the file
            return Err(e);
        }
    }

    // 2. Write the new values. If one fails halfway (e.g. access denied),
    //    undo the ones already written so the tweak isn't left half-applied.
    for (name, value) in tweak.values {
        if let Err(e) = s.registry.write(tweak.key, name, &value.to_reg()) {
            let old = s.backups[id].clone();
            let _ = restore(s.registry.as_mut(), tweak.key, &old);
            return Err(e);
        }
    }
    Ok(())
}

#[tauri::command]
fn revert_tweak(id: &str, state: tauri::State<Mutex<AppState>>) -> Result<(), String> {
    let tweak = find_tweak(id)?;
    let mut s = state.lock().unwrap();

    // 1. Restore the registry from the backup
    //    If this fails, the backup is kept so you can try again.
    let old = s.backups.get(id).cloned().ok_or("No backup found for this tweak")?;
    restore(s.registry.as_mut(), tweak.key, &old)?;

    // 2. Only now forget the backup, so it's never lost before the restore is done
    s.backups.remove(id);
    backups::save(&s.backup_path, &s.backups)
}

// Pick the registry at compile time: the real one on Windows, the fake one everywhere else
#[cfg(windows)]
fn make_registry() -> Box<dyn Registry> {
    Box::new(windows_registry::WindowsRegistry)
}

#[cfg(not(windows))]
fn make_registry() -> Box<dyn Registry> {
    Box::new(registry::FakeRegistry::new())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
        .invoke_handler(tauri::generate_handler![list_tweaks, apply_tweak, revert_tweak])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}