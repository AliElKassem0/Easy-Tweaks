// Restarts explorer.exe (taskbar, desktop and File Explorer windows), so changes
// that Explorer only reads when it starts show up without a PC restart.
// On macOS this only prints what it would do, like FakeRegistry.

#[cfg(not(windows))]
pub fn restart() -> Result<(), String> {
    println!("[fake explorer] restart");
    Ok(())
}

#[cfg(windows)]
pub use windows::restart;

#[cfg(windows)]
mod windows {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    use std::thread::sleep;
    use std::time::Duration;
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000; // don't flash a console window for taskkill

    pub fn restart() -> Result<(), String> {
        println!("[explorer] restart");
        // 1. Close every explorer.exe. If none is running, taskkill fails, which is fine:
        //    step 3 starts a new one either way.
        Command::new("taskkill")
            .args(["/f", "/im", "explorer.exe"])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("Can't run taskkill: {}", e))?;

        // 2. Wait until the old taskbar is really gone, or step 3 could see it and stop
        wait_for(|| !taskbar_running(), 3);

        // 3. Windows normally restarts Explorer by itself. If it hasn't after 3 s,
        //    start it ourselves (starting it while it already runs would just open a folder window)
        if !wait_for(taskbar_running, 3) {
            Command::new("explorer.exe")
                .spawn()
                .map_err(|e| format!("Can't start explorer.exe: {}", e))?;
        }
        Ok(())
    }

    // Checks `done` every 100 ms, for at most `secs` seconds
    fn wait_for(done: impl Fn() -> bool, secs: u64) -> bool {
        for _ in 0..secs * 10 {
            if done() {
                return true;
            }
            sleep(Duration::from_millis(100));
        }
        done()
    }

    // The taskbar is the window with class name "Shell_TrayWnd", owned by explorer.exe
    fn taskbar_running() -> bool {
        // Windows wants UTF-16 text ending in a 0
        let class: Vec<u16> = "Shell_TrayWnd".encode_utf16().chain([0]).collect();
        let window = unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) };
        !window.is_null()
    }
}
