// The real Windows registry, using the `winreg` crate.
// This whole file is only compiled on Windows (see `#[cfg(windows)] mod` in lib.rs).
use crate::registry::{RegValue, Registry};
use std::io::ErrorKind;
use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
use winreg::types::FromRegValue;
use winreg::RegKey;

// No fields: Windows itself holds the data, we just talk to it
pub struct WindowsRegistry;

fn hkcu() -> RegKey {
    RegKey::predef(HKEY_CURRENT_USER)
}

impl Registry for WindowsRegistry {
    fn read(&self, key: &str, name: &str) -> Result<Option<RegValue>, String> {
        let k = match hkcu().open_subkey_with_flags(key, KEY_READ) {
            Ok(k) => k,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None), // key doesn't exist
            Err(e) => return Err(format!("Can't open HKCU\\{}: {}", key, e)),
        };
        let raw = match k.get_raw_value(name) {
            Ok(raw) => raw,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None), // value doesn't exist
            Err(e) => return Err(format!("Can't read HKCU\\{}\\{}: {}", key, name, e)),
        };
        // Convert the raw bytes based on the value's type
        let err = |e: std::io::Error| format!("Can't read HKCU\\{}\\{}: {}", key, name, e);
        match raw.vtype {
            RegType::REG_SZ => Ok(Some(RegValue::Sz(String::from_reg_value(&raw).map_err(err)?))),
            RegType::REG_DWORD => Ok(Some(RegValue::Dword(u32::from_reg_value(&raw).map_err(err)?))),
            // Refuse instead of returning None: None would mean "didn't exist",
            // and reverting would then DELETE a value we just don't understand.
            other => Err(format!("HKCU\\{}\\{} has unsupported type {:?}", key, name, other)),
        }
    }

    fn write(&mut self, key: &str, name: &str, value: &RegValue) -> Result<(), String> {
        println!("[registry] HKCU\\{}\\{} = {}", key, name, value);
        // create_subkey opens the key, or creates it if it's missing
        let (k, _) = hkcu()
            .create_subkey(key)
            .map_err(|e| format!("Can't open HKCU\\{}: {}", key, e))?;
        let result = match value {
            RegValue::Sz(s) => k.set_value(name, s),
            RegValue::Dword(n) => k.set_value(name, n),
        };
        result.map_err(|e| format!("Can't write HKCU\\{}\\{}: {}", key, name, e))
    }

    fn delete(&mut self, key: &str, name: &str) -> Result<(), String> {
        println!("[registry] delete HKCU\\{}\\{}", key, name);
        let result = hkcu()
            .open_subkey_with_flags(key, KEY_SET_VALUE)
            .and_then(|k| k.delete_value(name));
        match result {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()), // already gone = goal reached
            Err(e) => Err(format!("Can't delete HKCU\\{}\\{}: {}", key, name, e)),
        }
    }
}

// Runs on the Windows GitHub Actions machine (`cargo test`), against a throwaway key,
// so the real registry code is tested without touching any real setting.
#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: &str = r"Software\EasyTweaksTest";

    #[test]
    fn read_write_delete() {
        let mut r = WindowsRegistry;
        let _ = hkcu().delete_subkey_all(TEST_KEY); // clean leftovers from a failed run

        // Missing key and missing value both read as None
        assert_eq!(r.read(TEST_KEY, "Text").unwrap(), None);

        // Both types round-trip (write creates the key)
        r.write(TEST_KEY, "Text", &RegValue::Sz("400".into())).unwrap();
        r.write(TEST_KEY, "Number", &RegValue::Dword(1)).unwrap();
        assert_eq!(r.read(TEST_KEY, "Text").unwrap(), Some(RegValue::Sz("400".into())));
        assert_eq!(r.read(TEST_KEY, "Number").unwrap(), Some(RegValue::Dword(1)));
        assert_eq!(r.read(TEST_KEY, "Missing").unwrap(), None);

        // Delete works, and deleting again is not an error
        r.delete(TEST_KEY, "Text").unwrap();
        assert_eq!(r.read(TEST_KEY, "Text").unwrap(), None);
        r.delete(TEST_KEY, "Text").unwrap();
        r.delete(r"Software\EasyTweaksTest\NoSuchKey", "x").unwrap();

        // A type we don't support (REG_QWORD) is an error, not None
        hkcu().open_subkey_with_flags(TEST_KEY, KEY_SET_VALUE).unwrap().set_value("Big", &5u64).unwrap();
        assert!(r.read(TEST_KEY, "Big").is_err());

        hkcu().delete_subkey_all(TEST_KEY).unwrap();
    }
}
