//! Active-window context — the false-positive killer.
//!
//! Before correcting, the engine asks two questions about the foreground window:
//! *which process owns it* (to honour the ignore-list) and *is the focused
//! control a password field* (never touch those). Both are resolved here.
//!
//! Process lookup is cached by **PID**: `GetWindowThreadProcessId` is cheap and
//! gives us the PID on every call, so we only pay for `OpenProcess` +
//! `QueryFullProcessImageNameW` the first time we see a process. (Caching by PID
//! rather than HWND means one entry per app instead of one per window, and
//! sidesteps HWND reuse.)
//!
//! Password detection is **best-effort and Win32-only**: it inspects the focused
//! control's class and `ES_PASSWORD` style. That covers native edit controls;
//! browser/Electron password fields are opaque (the UI honestly can't promise
//! coverage there — lean on the ignore-list and instant undo for those).

use std::collections::HashMap;
use std::mem::size_of;

use windows::core::PWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowLongPtrW,
    GetWindowThreadProcessId, GUITHREADINFO, GWL_STYLE,
};

/// `ES_PASSWORD` edit-control style (winuser.h). Not re-exported by the bindings
/// in a convenient form, so we name it directly.
const ES_PASSWORD: u32 = 0x0020;

/// Resolves and caches the foreground window's owning process.
#[derive(Default)]
pub struct Context {
    /// PID → lowercased executable file name (no path).
    cache: HashMap<u32, String>,
}

impl Context {
    pub fn new() -> Self {
        Self::default()
    }

    /// Executable name (lowercased, no path) of the process that owns the
    /// foreground window, e.g. `"notepad.exe"`. `None` if there's no foreground
    /// window or the process can't be queried.
    pub fn foreground_exe(&mut self) -> Option<String> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return None;
            }
            if let Some(exe) = self.cache.get(&pid) {
                return Some(exe.clone());
            }
            let exe = exe_for_pid(pid)?;
            self.cache.insert(pid, exe.clone());
            Some(exe)
        }
    }
}

/// Query the full image path of `pid` and reduce it to a lowercased file name.
unsafe fn exe_for_pid(pid: u32) -> Option<String> {
    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let mut buf = [0u16; 260];
    let mut len = buf.len() as u32;
    let result =
        QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
    let _ = CloseHandle(handle);
    result.ok()?;

    let path = String::from_utf16_lossy(&buf[..len as usize]);
    let file = path.rsplit(['\\', '/']).next().unwrap_or(&path);
    Some(file.to_ascii_lowercase())
}

/// Best-effort: is the control with keyboard focus a native password edit box?
///
/// Returns `false` when it can't tell (no focus, non-Win32 control, etc.) — a
/// false negative is recoverable (the user can undo), whereas refusing to ever
/// correct would defeat the tool.
pub fn focused_is_password() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }
        let tid = GetWindowThreadProcessId(hwnd, None);
        if tid == 0 {
            return false;
        }
        let mut info = GUITHREADINFO {
            cbSize: size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if GetGUIThreadInfo(tid, &mut info).is_err() || info.hwndFocus.0.is_null() {
            return false;
        }
        let focus = info.hwndFocus;

        let mut class = [0u16; 32];
        let n = GetClassNameW(focus, &mut class);
        let class = String::from_utf16_lossy(&class[..n as usize]);
        if !class.eq_ignore_ascii_case("edit") {
            return false;
        }
        let style = GetWindowLongPtrW(focus, GWL_STYLE) as u32;
        style & ES_PASSWORD != 0
    }
}
