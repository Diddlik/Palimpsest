//! Enumerate running processes — the source list for the ignore-list picker in
//! the settings UI, so users can choose an app to silence instead of typing its
//! executable name by hand.

use std::collections::BTreeSet;
use std::mem::size_of;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

/// Distinct executable names of currently running processes (e.g. `notepad.exe`),
/// sorted case-insensitively. Best-effort: returns an empty list if the snapshot
/// can't be taken.
pub fn running() -> Vec<String> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return Vec::new();
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let end = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
                if !name.is_empty() {
                    names.insert(name);
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    let mut out: Vec<String> = names.into_iter().collect();
    out.sort_by_key(|s| s.to_lowercase());
    out
}
