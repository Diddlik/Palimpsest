//! Minimal Win32 clipboard text get/set/clear for the selection-conversion hotkey.
//!
//! The "convert selection" flow can't read another application's selection
//! directly, so it copies the selection to the clipboard (`Ctrl+C`) and reads it
//! back here. To stay polite it snapshots the user's clipboard first and restores
//! it afterwards, and it empties the clipboard before the copy so it can tell
//! "nothing was selected" (clipboard still empty) from a real selection.
//!
//! Everything is `CF_UNICODETEXT`-only — Palimpsest deals exclusively in text.

use std::time::Duration;

use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};
use windows::Win32::System::Ole::CF_UNICODETEXT;

/// `CF_UNICODETEXT` as the plain `u32` the clipboard get/set calls expect.
const CF_TEXT: u32 = CF_UNICODETEXT.0 as u32;

/// How many times to retry `OpenClipboard` before giving up, and how long to
/// wait between attempts.
///
/// The Windows clipboard is a single process-wide lock: whoever opens it holds
/// it until they close it, and every other `OpenClipboard` fails outright with
/// `ERROR_ACCESS_DENIED`. Clipboard managers, browsers, Office and Teams all
/// open it briefly and routinely, so a *single* attempt fails often enough to
/// look random to the user. Retrying is the documented way to use this API.
const OPEN_ATTEMPTS: u32 = 10;
/// Gap between `OpenClipboard` attempts — 10 × 10 ms covers the brief holds
/// other apps take, while staying well inside the selection-copy budget.
const OPEN_RETRY: Duration = Duration::from_millis(10);

/// Open the clipboard, retrying while another process holds it. Runs on the
/// engine thread only — never the hook thread, which must never sleep.
unsafe fn open() -> windows::core::Result<()> {
    let mut last = windows::core::Error::from_win32();
    for attempt in 0..OPEN_ATTEMPTS {
        match OpenClipboard(HWND::default()) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = e;
                if attempt + 1 < OPEN_ATTEMPTS {
                    std::thread::sleep(OPEN_RETRY);
                }
            }
        }
    }
    Err(last)
}

/// Read the clipboard's Unicode text, or `None` when it holds no text (or could
/// not be opened even after retrying).
pub fn get_text() -> Option<String> {
    unsafe {
        open().ok()?;
        let text = read_unicode_text();
        let _ = CloseClipboard();
        text
    }
}

/// Read `CF_UNICODETEXT` from the already-open clipboard. Caller owns open/close.
unsafe fn read_unicode_text() -> Option<String> {
    let handle = GetClipboardData(CF_TEXT).ok()?;
    let hglobal = HGLOBAL(handle.0);
    let ptr = GlobalLock(hglobal) as *const u16;
    if ptr.is_null() {
        return None;
    }
    // `GlobalSize` is in bytes; the buffer is NUL-terminated UTF-16.
    let units = GlobalSize(hglobal) / 2;
    let mut len = 0;
    while len < units && *ptr.add(len) != 0 {
        len += 1;
    }
    let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
    let _ = GlobalUnlock(hglobal);
    Some(text)
}

/// Replace the clipboard's contents with `text` as `CF_UNICODETEXT`. Used to
/// restore the user's clipboard after a selection conversion.
pub fn set_text(text: &str) -> windows::core::Result<()> {
    let mut utf16: Vec<u16> = text.encode_utf16().collect();
    utf16.push(0); // NUL terminator the clipboard requires
    unsafe {
        open()?;
        let result = write_unicode_text(&utf16);
        let _ = CloseClipboard();
        result
    }
}

/// Write a NUL-terminated UTF-16 buffer as `CF_UNICODETEXT` into the already-open
/// clipboard. On success the allocated block is owned by the system.
unsafe fn write_unicode_text(utf16: &[u16]) -> windows::core::Result<()> {
    EmptyClipboard()?;
    let hglobal = GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of_val(utf16))?;
    let dst = GlobalLock(hglobal) as *mut u16;
    if dst.is_null() {
        return Err(windows::core::Error::from_win32());
    }
    std::ptr::copy_nonoverlapping(utf16.as_ptr(), dst, utf16.len());
    let _ = GlobalUnlock(hglobal);
    // Hands ownership of `hglobal` to the system; we must not free it now.
    SetClipboardData(CF_TEXT, HANDLE(hglobal.0))?;
    Ok(())
}

/// Empty the clipboard. Called before the copy so an empty read afterwards means
/// "nothing was selected".
pub fn clear() -> windows::core::Result<()> {
    unsafe {
        open()?;
        let result = EmptyClipboard();
        let _ = CloseClipboard();
        result
    }
}
