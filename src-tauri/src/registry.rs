use std::collections::HashMap;
use std::fmt;

// A registry value can have different types. We support the two most common ones.
// `untagged` = in JSON a string becomes Sz and a number becomes Dword,
// so backups.json stays readable: "400" or 0, not {"Sz":"400"}.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum RegValue {
    Sz(String), // REG_SZ: text
    Dword(u32), // REG_DWORD: 32-bit number
}

// How a value is printed in the terminal
impl fmt::Display for RegValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RegValue::Sz(s) => write!(f, "\"{}\" (REG_SZ)", s),
            RegValue::Dword(n) => write!(f, "{} (REG_DWORD)", n),
        }
    }
}

// What every registry (fake or real) must be able to do.
// All paths are relative to HKEY_CURRENT_USER.
// read: Ok(None) = the value doesn't exist, Err = something went wrong (e.g. unsupported type).
pub trait Registry: Send {
    fn read(&self, key: &str, name: &str) -> Result<Option<RegValue>, String>;
    fn write(&mut self, key: &str, name: &str, value: &RegValue) -> Result<(), String>;
    fn delete(&mut self, key: &str, name: &str) -> Result<(), String>;
}

// The fake one: just a list in memory. Used on macOS (on Windows only by tests).
#[cfg_attr(windows, allow(dead_code))]
pub struct FakeRegistry {
    data: HashMap<String, RegValue>,
}

#[cfg_attr(windows, allow(dead_code))]
impl FakeRegistry {
    pub fn new() -> Self {
        use RegValue::{Dword, Sz};
        let mut data = HashMap::new();
        // Start with the Windows default values.
        // Values that don't exist on a fresh Windows install are left out on purpose
        // (e.g. AppCaptureEnabled), so reverting has to delete them again.
        let defaults = [
            (r"Control Panel\Mouse\MouseSpeed", Sz("1".into())),
            (r"Control Panel\Mouse\MouseThreshold1", Sz("6".into())),
            (r"Control Panel\Mouse\MouseThreshold2", Sz("10".into())),
            (r"Control Panel\Accessibility\StickyKeys\Flags", Sz("510".into())),
            (r"Control Panel\Accessibility\Keyboard Response\Flags", Sz("126".into())),
            (r"Control Panel\Accessibility\ToggleKeys\Flags", Sz("62".into())),
            (r"Control Panel\Keyboard\KeyboardDelay", Sz("1".into())),
            (r"Control Panel\Desktop\MenuShowDelay", Sz("400".into())),
            (r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced\HideFileExt", Dword(1)),
        ];
        for (path, value) in defaults {
            data.insert(path.to_string(), value);
        }
        Self { data }
    }

    fn path(key: &str, name: &str) -> String {
        format!("{}\\{}", key, name)
    }
}

impl Registry for FakeRegistry {
    fn read(&self, key: &str, name: &str) -> Result<Option<RegValue>, String> {
        Ok(self.data.get(&Self::path(key, name)).cloned())
    }

    fn write(&mut self, key: &str, name: &str, value: &RegValue) -> Result<(), String> {
        println!("[fake registry] {}\\{} = {}", key, name, value);
        self.data.insert(Self::path(key, name), value.clone());
        Ok(())
    }

    fn delete(&mut self, key: &str, name: &str) -> Result<(), String> {
        println!("[fake registry] delete {}\\{}", key, name);
        self.data.remove(&Self::path(key, name));
        Ok(())
    }
}
