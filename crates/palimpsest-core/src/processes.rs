//! Enumerate running processes — the source list for the ignore-list picker in
//! the settings UI, so users can choose an app to silence instead of typing its
//! executable name by hand.

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::null_mut;
use std::slice;

use serde::Serialize;
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

use crate::context::image_path_for_pid;

#[derive(Serialize)]
pub struct RunningProcess {
    pub exe: String,
    pub display_name: Option<String>,
}

/// Distinct running executables, sorted case-insensitively, with optional names
/// from their Windows version resources.
pub fn running() -> Vec<RunningProcess> {
    let mut names = BTreeMap::new();
    let mut path = vec![0u16; 32_768];
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
                let exe = String::from_utf16_lossy(&entry.szExeFile[..end]);
                if !exe.is_empty() {
                    let process = names.entry(exe.to_ascii_lowercase()).or_insert(RunningProcess {
                        exe,
                        display_name: None,
                    });
                    if process.display_name.is_none() {
                        if let Some(len) = image_path_for_pid(entry.th32ProcessID, &mut path) {
                            path[len] = 0;
                            process.display_name = display_name(PCWSTR(path.as_ptr()));
                        }
                    }
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    names.into_values().collect()
}

unsafe fn display_name(path: PCWSTR) -> Option<String> {
    let size = GetFileVersionInfoSizeW(path, None);
    if size == 0 {
        return None;
    }
    let mut data = vec![0u32; (size as usize).div_ceil(size_of::<u32>())];
    let block = data.as_mut_ptr().cast::<c_void>();
    GetFileVersionInfoW(path, 0, size, block).ok()?;

    let mut value = null_mut();
    let mut len = 0;
    if !VerQueryValueW(block, w!("\\VarFileInfo\\Translation"), &mut value, &mut len).as_bool()
        || value.is_null() || len < 4
    {
        return None;
    }
    // Win32 returns translation lengths in bytes, and string lengths in UTF-16 units.
    let translations = slice::from_raw_parts(value.cast::<u16>(), len as usize / 2);
    for field in ["FileDescription", "ProductName"] {
        for pair in translations.chunks_exact(2) {
            let key = HSTRING::from(format!("\\StringFileInfo\\{:04x}{:04x}\\{field}", pair[0], pair[1]));
            if VerQueryValueW(block, &key, &mut value, &mut len).as_bool()
                && !value.is_null() && len != 0
            {
                let name = String::from_utf16_lossy(slice::from_raw_parts(value.cast::<u16>(), len as usize));
                let name = name.trim_end_matches('\0').trim();
                if !name.is_empty() {
                    return Some(name.to_owned());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_display_name_from_windows_executable() {
        let root = std::env::var_os("SystemRoot").expect("Windows system directory");
        let exe = std::path::Path::new(&root).join("System32").join("cmd.exe");
        let path = HSTRING::from(exe.as_os_str());
        assert!(unsafe { display_name(PCWSTR(path.as_ptr())) }.is_some());
    }
}
