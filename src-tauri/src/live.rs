// Makes a change take effect right away instead of after sign-out.
// The registry only holds what Windows loads at sign-in; SystemParametersInfo
// changes the setting in the running session too.
// On macOS this only prints what it would do, like FakeRegistry.

// Which live setting a tweak updates. The numbers passed to `push` are the
// tweak's registry values, in the same order as its `values` list.
#[derive(Clone, Copy, Debug)]
pub enum Live {
    Mouse,         // MouseSpeed, MouseThreshold1, MouseThreshold2
    HoverTime,     // milliseconds
    StickyKeys,    // Flags
    FilterKeys,    // Flags
    ToggleKeys,    // Flags
    KeyboardDelay, // 0-3
    MinAnimate,    // 0 = off, 1 = on
    MenuDelay,     // milliseconds
}

#[cfg(not(windows))]
pub fn push(live: Live, values: &[u32]) -> Result<(), String> {
    println!("[fake live] {:?} = {:?}", live, values);
    Ok(())
}

#[cfg(windows)]
pub use windows::push;

#[cfg(windows)]
mod windows {
    use super::Live;
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr::null_mut;
    use windows_sys::Win32::UI::Accessibility::{FILTERKEYS, STICKYKEYS, TOGGLEKEYS};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    // Bit 0x4 of the 3 accessibility Flags = "the keyboard shortcut is active"
    // (same bit for Sticky, Filter and Toggle Keys)
    const HOTKEY_ACTIVE: u32 = 0x4;

    // One SystemParametersInfo call. SPIF_SENDCHANGE tells open apps that a setting
    // changed. No SPIF_UPDATEINIFILE: we already wrote the registry ourselves.
    fn spi(action: SYSTEM_PARAMETERS_INFO_ACTION, ui_param: u32, pv_param: *mut c_void) -> Result<(), String> {
        // unsafe: Windows writes through pv_param, so it must point to the right struct
        if unsafe { SystemParametersInfoW(action, ui_param, pv_param, SPIF_SENDCHANGE) } == 0 {
            return Err(format!("SystemParametersInfo failed: {}", std::io::Error::last_os_error()));
        }
        Ok(())
    }

    // A pointer to any struct, in the form SystemParametersInfo wants
    fn ptr<T>(x: &mut T) -> *mut c_void {
        (x as *mut T).cast()
    }

    // Keep the current state (e.g. Sticky Keys currently on), copy only the shortcut bit
    fn with_hotkey_bit(current: u32, registry: u32) -> u32 {
        (current & !HOTKEY_ACTIVE) | (registry & HOTKEY_ACTIVE)
    }

    pub fn push(live: Live, values: &[u32]) -> Result<(), String> {
        println!("[live] {:?} = {:?}", live, values);
        match (live, values) {
            (Live::Mouse, &[speed, threshold1, threshold2]) => {
                // SPI_SETMOUSE takes [threshold1, threshold2, speed]
                let mut mouse = [threshold1 as i32, threshold2 as i32, speed as i32];
                spi(SPI_SETMOUSE, 0, mouse.as_mut_ptr().cast())
            }
            // For these, the number goes straight into ui_param
            (Live::HoverTime, &[ms]) => spi(SPI_SETMOUSEHOVERTIME, ms, null_mut()),
            (Live::KeyboardDelay, &[delay]) => spi(SPI_SETKEYBOARDDELAY, delay, null_mut()),
            (Live::MenuDelay, &[ms]) => spi(SPI_SETMENUSHOWDELAY, ms, null_mut()),
            (Live::MinAnimate, &[on]) => {
                let mut a = ANIMATIONINFO { cbSize: size_of::<ANIMATIONINFO>() as u32, iMinAnimate: on as i32 };
                spi(SPI_SETANIMATION, a.cbSize, ptr(&mut a))
            }
            // Accessibility: read the current struct, change one bit, write it back
            (Live::StickyKeys, &[flags]) => {
                let mut k = STICKYKEYS { cbSize: size_of::<STICKYKEYS>() as u32, dwFlags: 0 };
                spi(SPI_GETSTICKYKEYS, k.cbSize, ptr(&mut k))?;
                k.dwFlags = with_hotkey_bit(k.dwFlags, flags);
                spi(SPI_SETSTICKYKEYS, k.cbSize, ptr(&mut k))
            }
            (Live::FilterKeys, &[flags]) => {
                let mut k = FILTERKEYS {
                    cbSize: size_of::<FILTERKEYS>() as u32,
                    dwFlags: 0,
                    iWaitMSec: 0,
                    iDelayMSec: 0,
                    iRepeatMSec: 0,
                    iBounceMSec: 0,
                };
                spi(SPI_GETFILTERKEYS, k.cbSize, ptr(&mut k))?;
                k.dwFlags = with_hotkey_bit(k.dwFlags, flags);
                spi(SPI_SETFILTERKEYS, k.cbSize, ptr(&mut k))
            }
            (Live::ToggleKeys, &[flags]) => {
                let mut k = TOGGLEKEYS { cbSize: size_of::<TOGGLEKEYS>() as u32, dwFlags: 0 };
                spi(SPI_GETTOGGLEKEYS, k.cbSize, ptr(&mut k))?;
                k.dwFlags = with_hotkey_bit(k.dwFlags, flags);
                spi(SPI_SETTOGGLEKEYS, k.cbSize, ptr(&mut k))
            }
            _ => Err(format!("{:?} got {} values", live, values.len())),
        }
    }
}
