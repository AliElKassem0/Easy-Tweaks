mod backups;
mod live;
mod registry;
mod stats;
#[cfg(windows)]
mod windows_registry;

use backups::Backups;
use live::Live;
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
    page: &'static str,    // which sidebar page shows it: "input" | "visual" | "windows" | "debloat"
    section: &'static str, // heading it's grouped under on that page
    name: &'static str,
    description: &'static str,
    key: &'static str,
    values: &'static [Setting],
    live: Option<Live>, // Some = can take effect right away (see live.rs), None = after sign-out
}

const TWEAKS: &[Tweak] = &[
    // ───────────── Input ─────────────
    Tweak {
        id: "mouse_accel_off",
        page: "input",
        section: "Mouse",
        name: "Disable Mouse Acceleration",
        description: "Linear 1:1 cursor movement (turns off \"Enhance pointer precision\")",
        key: r"Control Panel\Mouse",
        values: &[
            Setting { name: "MouseSpeed", on: Sz("0"), default: Some(Sz("1")) },
            Setting { name: "MouseThreshold1", on: Sz("0"), default: Some(Sz("6")) },
            Setting { name: "MouseThreshold2", on: Sz("0"), default: Some(Sz("10")) },
        ],
        live: Some(Live::Mouse),
    },
    Tweak {
        id: "mouse_hover_fast",
        page: "input",
        section: "Mouse",
        name: "Faster Hover Detection",
        description: "Apps that react to the cursor resting on something do so after 100 ms instead of 400 ms",
        key: r"Control Panel\Mouse",
        values: &[Setting { name: "MouseHoverTime", on: Sz("100"), default: Some(Sz("400")) }], // milliseconds
        live: Some(Live::HoverTime),
    },
    // The 3 accessibility "Flags" values are bit fields.
    // Bit 0x4 = "the keyboard shortcut is active". Each tweak clears only that bit.
    Tweak {
        id: "sticky_keys_shortcut_off",
        page: "input",
        section: "Keyboard",
        name: "Disable Sticky Keys Shortcut",
        description: "Pressing Shift 5 times no longer opens the Sticky Keys popup",
        key: r"Control Panel\Accessibility\StickyKeys",
        // 510 = 0x1FE -> 506 = 0x1FA
        values: &[Setting { name: "Flags", on: Sz("506"), default: Some(Sz("510")) }],
        live: Some(Live::StickyKeys),
    },
    Tweak {
        id: "filter_keys_shortcut_off",
        page: "input",
        section: "Keyboard",
        name: "Disable Filter Keys Shortcut",
        description: "Holding right Shift for 8 seconds no longer turns on Filter Keys",
        key: r"Control Panel\Accessibility\Keyboard Response",
        // 126 = 0x7E -> 122 = 0x7A
        values: &[Setting { name: "Flags", on: Sz("122"), default: Some(Sz("126")) }],
        live: Some(Live::FilterKeys),
    },
    Tweak {
        id: "toggle_keys_shortcut_off",
        page: "input",
        section: "Keyboard",
        name: "Disable Toggle Keys Shortcut",
        description: "Holding Num Lock for 5 seconds no longer turns on Toggle Keys",
        key: r"Control Panel\Accessibility\ToggleKeys",
        // 62 = 0x3E -> 58 = 0x3A
        values: &[Setting { name: "Flags", on: Sz("58"), default: Some(Sz("62")) }],
        live: Some(Live::ToggleKeys),
    },
    Tweak {
        id: "narrator_hotkey_off",
        page: "input",
        section: "Keyboard",
        name: "Disable Narrator Shortcut",
        description: "Win + Ctrl + Enter no longer starts Narrator by accident",
        key: r"Software\Microsoft\Narrator\NoRoam",
        values: &[Setting { name: "WinEnterLaunchEnabled", on: Dword(0), default: None }],
        live: None,
    },
    Tweak {
        id: "keyboard_delay_min",
        page: "input",
        section: "Keyboard",
        name: "Shortest Key Repeat Delay",
        description: "A held key starts repeating after ~250 ms instead of ~500 ms",
        key: r"Control Panel\Keyboard",
        // scale 0-3 (0 = ~250 ms, 3 = ~1 s)
        values: &[Setting { name: "KeyboardDelay", on: Sz("0"), default: Some(Sz("1")) }],
        live: Some(Live::KeyboardDelay),
    },
    // ───────────── Visual ─────────────
    Tweak {
        id: "window_animations_off",
        page: "visual",
        section: "Effects",
        name: "Disable Window Animations",
        description: "Windows minimize and maximize instantly, without the zoom animation",
        key: r"Control Panel\Desktop\WindowMetrics",
        values: &[Setting { name: "MinAnimate", on: Sz("0"), default: Some(Sz("1")) }],
        live: Some(Live::MinAnimate),
    },
    Tweak {
        id: "transparency_off",
        page: "visual",
        section: "Effects",
        name: "Disable Transparency",
        description: "Taskbar, Start and Settings use solid colors instead of see-through blur",
        key: r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        values: &[Setting { name: "EnableTransparency", on: Dword(0), default: Some(Dword(1)) }],
        live: None,
    },
    Tweak {
        id: "aero_peek_off",
        page: "visual",
        section: "Effects",
        name: "Disable Peek",
        description: "Hovering the far corner of the taskbar no longer makes all windows see-through",
        key: r"Software\Microsoft\Windows\DWM",
        values: &[Setting { name: "EnableAeroPeek", on: Dword(0), default: Some(Dword(1)) }],
        live: None,
    },
    Tweak {
        id: "menu_delay_off",
        page: "visual",
        section: "Speed",
        name: "Instant Menus",
        description: "Submenus open immediately instead of after a 400 ms hover delay",
        key: r"Control Panel\Desktop",
        // milliseconds
        values: &[Setting { name: "MenuShowDelay", on: Sz("0"), default: Some(Sz("400")) }],
        live: Some(Live::MenuDelay),
    },
    Tweak {
        id: "startup_delay_off",
        page: "visual",
        section: "Speed",
        name: "Remove Startup App Delay",
        description: "Startup apps launch right after sign-in instead of after Windows' built-in delay",
        key: r"Software\Microsoft\Windows\CurrentVersion\Explorer\Serialize",
        values: &[Setting { name: "StartupDelayInMSec", on: Dword(0), default: None }],
        live: None,
    },
    // ───────────── Windows ─────────────
    // Most Explorer settings live in the same key
    Tweak {
        id: "taskbar_search_off",
        page: "windows",
        section: "Taskbar",
        name: "Hide Taskbar Search",
        description: "Removes the search box from the taskbar. Press Win and type to search, as before",
        key: r"Software\Microsoft\Windows\CurrentVersion\Search",
        values: &[Setting { name: "SearchboxTaskbarMode", on: Dword(0), default: None }],
        live: None,
    },
    Tweak {
        id: "task_view_button_off",
        page: "windows",
        section: "Taskbar",
        name: "Hide Task View Button",
        description: "Removes the Task View button from the taskbar. Win + Tab still works",
        key: r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        values: &[Setting { name: "ShowTaskViewButton", on: Dword(0), default: None }],
        live: None,
    },
    Tweak {
        id: "show_file_extensions",
        page: "windows",
        section: "File Explorer",
        name: "Show File Extensions",
        description: "File Explorer shows .exe, .txt, etc., so files like photo.jpg.exe are easy to spot",
        key: r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        // 1 = hide, 0 = show
        values: &[Setting { name: "HideFileExt", on: Dword(0), default: Some(Dword(1)) }],
        live: None,
    },
    Tweak {
        id: "show_hidden_files",
        page: "windows",
        section: "File Explorer",
        name: "Show Hidden Files",
        description: "File Explorer shows hidden files and folders, like AppData",
        key: r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        // 1 = show, 2 = hide
        values: &[Setting { name: "Hidden", on: Dword(1), default: Some(Dword(2)) }],
        live: None,
    },
    Tweak {
        id: "snap_assist_off",
        page: "windows",
        section: "Windows",
        name: "Disable Snap Assist",
        description: "Snapping a window to a side no longer suggests windows to fill the other side",
        key: r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        values: &[Setting { name: "SnapAssist", on: Dword(0), default: None }],
        live: None,
    },
    Tweak {
        id: "aero_shake_off",
        page: "windows",
        section: "Windows",
        name: "Disable Shake to Minimize",
        description: "Shaking a window by its title bar no longer minimizes all others (already off by default on Windows 11)",
        key: r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        values: &[Setting { name: "DisallowShaking", on: Dword(1), default: None }],
        live: None,
    },
    Tweak {
        id: "recent_files_off",
        page: "windows",
        section: "Activity history",
        name: "Don't Track Recent Files",
        description: "Start, Jump Lists and File Explorer stop listing recently opened files",
        key: r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        values: &[Setting { name: "Start_TrackDocs", on: Dword(0), default: None }],
        live: None,
    },
    Tweak {
        id: "app_tracking_off",
        page: "windows",
        section: "Activity history",
        name: "Don't Track App Launches",
        description: "Windows stops counting which apps you open, so Start shows no \"Most used\" list",
        key: r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        values: &[Setting { name: "Start_TrackProgs", on: Dword(0), default: None }],
        live: None,
    },
    // ───────────── Debloat ─────────────
    // "SubscribedContent-<number>Enabled" are the switches behind
    // Settings > Privacy > General and Settings > Notifications > Additional settings
    Tweak {
        id: "start_suggestions_off",
        page: "debloat",
        section: "Suggestions & ads",
        name: "Disable App Suggestions in Start",
        description: "Start no longer shows promoted apps from the Microsoft Store (Windows 10)",
        key: r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager",
        values: &[
            Setting { name: "SubscribedContent-338388Enabled", on: Dword(0), default: Some(Dword(1)) },
            Setting { name: "SystemPaneSuggestionsEnabled", on: Dword(0), default: Some(Dword(1)) },
        ],
        live: None,
    },
    Tweak {
        id: "start_recommendations_off",
        page: "debloat",
        section: "Suggestions & ads",
        name: "Disable Start Recommendations",
        description: "Start no longer recommends tips, shortcuts and new apps (Windows 11)",
        key: r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
        values: &[Setting { name: "Start_IrisRecommendations", on: Dword(0), default: None }],
        live: None,
    },
    Tweak {
        id: "settings_suggestions_off",
        page: "debloat",
        section: "Suggestions & ads",
        name: "Disable Suggestions in Settings",
        description: "The Settings app no longer shows suggested content",
        key: r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager",
        values: &[
            Setting { name: "SubscribedContent-338393Enabled", on: Dword(0), default: Some(Dword(1)) },
            Setting { name: "SubscribedContent-353694Enabled", on: Dword(0), default: Some(Dword(1)) },
            Setting { name: "SubscribedContent-353696Enabled", on: Dword(0), default: Some(Dword(1)) },
        ],
        live: None,
    },
    Tweak {
        id: "windows_tips_off",
        page: "debloat",
        section: "Suggestions & ads",
        name: "Disable Tips Notifications",
        description: "No more \"tips and suggestions\" notifications while using Windows",
        key: r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager",
        values: &[Setting { name: "SubscribedContent-338389Enabled", on: Dword(0), default: Some(Dword(1)) }],
        live: None,
    },
    Tweak {
        id: "lock_screen_tips_off",
        page: "debloat",
        section: "Suggestions & ads",
        name: "Disable Lock Screen Tips",
        description: "The lock screen no longer shows fun facts, tips and promotions",
        key: r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager",
        values: &[
            Setting { name: "RotatingLockScreenOverlayEnabled", on: Dword(0), default: Some(Dword(1)) },
            Setting { name: "SubscribedContent-338387Enabled", on: Dword(0), default: Some(Dword(1)) },
        ],
        live: None,
    },
    Tweak {
        id: "silent_installs_off",
        page: "debloat",
        section: "Suggestions & ads",
        name: "Stop Silent App Installs",
        description: "Windows stops installing suggested apps in the background (doesn't remove ones already installed)",
        key: r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager",
        values: &[Setting { name: "SilentInstalledAppsEnabled", on: Dword(0), default: Some(Dword(1)) }],
        live: None,
    },
    Tweak {
        id: "bing_search_off",
        page: "debloat",
        section: "Suggestions & ads",
        name: "Disable Web Results in Start Search",
        description: "Start menu search shows only local results, no Bing web results (mainly Windows 10; newer Windows 11 builds may ignore it)",
        key: r"Software\Microsoft\Windows\CurrentVersion\Search",
        // missing = web results on
        values: &[Setting { name: "BingSearchEnabled", on: Dword(0), default: None }],
        live: None,
    },
    Tweak {
        id: "advertising_id_off",
        page: "debloat",
        section: "Privacy",
        name: "Disable Advertising ID",
        description: "Apps can no longer use your advertising ID to show personalized ads",
        key: r"Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo",
        values: &[Setting { name: "Enabled", on: Dword(0), default: Some(Dword(1)) }],
        live: None,
    },
    Tweak {
        id: "tailored_experiences_off",
        page: "debloat",
        section: "Privacy",
        name: "Disable Tailored Experiences",
        description: "Microsoft stops using your diagnostic data for personalized tips and ads",
        key: r"Software\Microsoft\Windows\CurrentVersion\Privacy",
        values: &[Setting { name: "TailoredExperiencesWithDiagnosticDataEnabled", on: Dword(0), default: Some(Dword(1)) }],
        live: None,
    },
    Tweak {
        id: "feedback_prompts_off",
        page: "debloat",
        section: "Privacy",
        name: "Disable Feedback Prompts",
        description: "Windows stops asking you for feedback (Feedback frequency: Never)",
        key: r"Software\Microsoft\Siuf\Rules",
        values: &[Setting { name: "NumberOfSIUFInPeriod", on: Dword(0), default: None }],
        live: None,
    },
    Tweak {
        id: "game_dvr_master_off",
        page: "debloat",
        section: "Xbox Game Bar",
        name: "Disable Game DVR",
        description: "Turns off Game DVR, Windows' built-in game recording",
        key: r"System\GameConfigStore",
        values: &[Setting { name: "GameDVR_Enabled", on: Dword(0), default: Some(Dword(1)) }],
        live: None,
    },
    Tweak {
        id: "game_dvr_off", // older ID kept on purpose: backups.json is keyed by ID
        page: "debloat",
        section: "Xbox Game Bar",
        name: "Disable Game Bar Capture",
        description: "Turns off recording clips and screenshots with Xbox Game Bar",
        key: r"Software\Microsoft\Windows\CurrentVersion\GameDVR",
        // doesn't exist until changed in Settings; missing = capture allowed
        values: &[Setting { name: "AppCaptureEnabled", on: Dword(0), default: None }],
        live: None,
    },
    Tweak {
        id: "game_bar_tips_off",
        page: "debloat",
        section: "Xbox Game Bar",
        name: "Disable Game Bar Tips",
        description: "Game Bar no longer shows its tips panel when it opens",
        key: r"Software\Microsoft\GameBar",
        values: &[Setting { name: "ShowStartupPanel", on: Dword(0), default: None }],
        live: None,
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

// Make the change take effect right away, using the values now in the registry
// (so it works the same after apply and revert). Best effort: the registry is already
// correct, so if this fails the change still takes effect after sign-out.
fn push_live(tweak: &Tweak, registry: &dyn Registry) {
    let Some(live) = tweak.live else { return };
    let numbers = tweak
        .values
        .iter()
        .map(|v| match registry.read(tweak.key, v.name)? {
            Some(RegValue::Dword(n)) => Ok(n),
            Some(RegValue::Sz(s)) => s.parse().map_err(|_| format!("{} is not a number: {:?}", v.name, s)),
            None => Err(format!("{} is missing", v.name)),
        })
        .collect::<Result<Vec<u32>, String>>();
    if let Err(e) = numbers.and_then(|n| live::push(live, &n)) {
        println!("[live] {} takes effect after sign-out: {}", tweak.id, e);
    }
}

// What the frontend gets for each row (serde turns it into JSON)
#[derive(serde::Serialize)]
struct TweakInfo {
    id: &'static str,
    section: &'static str,
    name: &'static str,
    description: &'static str,
    applied: bool,     // Easy Tweaks turned it on (has a backup) -> toggle on
    already_set: bool, // Windows already has these values, but not from us -> toggle off + note
    live: bool,        // takes effect right away; false = after sign-out
}

fn info(s: &AppState, tweak: &'static Tweak) -> TweakInfo {
    let set = values_set(tweak, s.registry.as_ref());
    let ours = s.backups.contains_key(tweak.id);
    TweakInfo {
        id: tweak.id,
        section: tweak.section,
        name: tweak.name,
        description: tweak.description,
        applied: set && ours,
        already_set: set && !ours,
        live: tweak.live.is_some(),
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

    // 3. Make it take effect now
    push_live(tweak, s.registry.as_ref());
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
    push_live(tweak, s.registry.as_ref());

    // 2. Only now forget the backup, so it's never lost before the restore is done
    if s.backups.remove(tweak.id).is_some() {
        backups::save(&s.backup_path, &s.backups)?;
    }
    Ok(())
}

// For the Dashboard: how many tweaks Easy Tweaks has turned on
#[derive(serde::Serialize)]
struct TweakSummary {
    applied: usize,
    total: usize,
}

#[tauri::command]
fn tweak_summary(state: tauri::State<Mutex<AppState>>) -> TweakSummary {
    let s = state.lock().unwrap();
    TweakSummary {
        applied: TWEAKS.iter().filter(|t| info(&s, t).applied).count(),
        total: TWEAKS.len(),
    }
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
            tweak_summary,
            stats::live_stats,
            stats::system_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}