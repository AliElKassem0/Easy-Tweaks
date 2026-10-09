use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::registry::RegValue;

// tweak id -> list of (value name, old value). None = the value didn't exist before.
pub type Backups = HashMap<String, Vec<(String, Option<RegValue>)>>;

// Read backups.json. A missing file just means nothing was backed up yet.
pub fn load(path: &Path) -> Backups {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => return Backups::new(),
    };
    match serde_json::from_str(&text) {
        Ok(backups) => backups,
        Err(e) => {
            // Don't overwrite a broken file on the next save: move it aside so it can be inspected
            let broken = path.with_extension("json.broken");
            eprintln!("[backups] {} is invalid ({}), moved to {}", path.display(), e, broken.display());
            let _ = fs::rename(path, broken);
            Backups::new()
        }
    }
}

// Write to a temp file first, then rename it over the real one.
// A rename is all-or-nothing, so a crash mid-write can't leave a half-written backups.json.
pub fn save(path: &Path, backups: &Backups) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Can't create {}: {}", dir.display(), e))?;
    }
    let text = serde_json::to_string_pretty(backups).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(|e| format!("Can't write {}: {}", tmp.display(), e))?;
    fs::rename(&tmp, path).map_err(|e| format!("Can't save {}: {}", path.display(), e))?;
    println!("[backups] saved {} backup(s) to {}", backups.len(), path.display());
    Ok(())
}
