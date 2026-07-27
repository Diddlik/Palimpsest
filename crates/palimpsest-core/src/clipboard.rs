//! Minimal Win32 clipboard text get/set/clear for the selection-conversion hotkey.
//!
//! The "convert selection" flow can't read another application's selection
//! directly, so it copies the selection to the clipboard (`Ctrl+C`) and reads it
//! back here. To stay polite it snapshots the user's clipboard first and restores
//! it afterwards, and it watches [`sequence`] to tell "nothing was selected" from
//! a real copy without having to disturb the clipboard at all.
//!
//! Everything is `CF_UNICODETEXT`-only — Palimpsest deals exclusively in text.
//!
//! **Be sparing with `OpenClipboard`.** It is a single system-wide lock, and the
//! app we just sent `Ctrl+C` needs it to publish the copy. Every attempt we make
//! while waiting is an attempt that can lock it out, which showed up as a storm
//! of `ERROR_ACCESS_DENIED` and a visibly frozen Notepad. Poll [`sequence`] —
//! which takes no lock — and open only once it has moved.

use std::sync::OnceLock;
use std::time::Duration;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard,
    SetClipboardData,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DispatchMessageW, GetMessageW, TranslateMessage, HWND_MESSAGE, MSG,
    WINDOW_EX_STYLE, WINDOW_STYLE,
};

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

/// Handle of the message-only window that owns the clipboard while we hold it,
/// stored as a raw `isize` so it can live in a `static`. Created once, lazily.
static OWNER: OnceLock<isize> = OnceLock::new();

/// A window handle to pass to `OpenClipboard`.
///
/// This must not be `NULL`. `EmptyClipboard` documents that opening with a NULL
/// handle sets the clipboard owner to NULL, "which causes `SetClipboardData` to
/// fail" — so the restore half of a selection conversion silently never happened
/// and the user's clipboard was left empty by every press of the hotkey.
///
/// A message-only window (`HWND_MESSAGE` parent) is the cheapest legitimate
/// owner: invisible, never enumerated, and needing no class of our own, since the
/// built-in `STATIC` class is always registered.
///
/// **It lives on its own thread with a real message pump, and must keep doing
/// so.** Owning the clipboard is not passive: when any other process calls
/// `EmptyClipboard` — which every `Ctrl+C` does — Windows `SendMessage`s
/// `WM_DESTROYCLIPBOARD` to the owner, and a `SendMessage` to a thread that never
/// pumps blocks the *sender*. Parking this window on the engine thread therefore
/// froze whichever app the user was copying in, mid-copy, while it held the
/// clipboard lock — which then denied us the lock in turn. Do not move it to a
/// thread that does not pump.
///
/// Returns `NULL` if creation fails, which degrades to the old behaviour: reads
/// still work, writes still fail.
fn owner_window() -> HWND {
    let raw = *OWNER.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("palimpsest-clipboard".into())
            .spawn(move || {
                let hwnd = unsafe {
                    CreateWindowExW(
                        WINDOW_EX_STYLE(0),
                        w!("STATIC"),
                        PCWSTR::null(),
                        WINDOW_STYLE(0),
                        0,
                        0,
                        0,
                        0,
                        HWND_MESSAGE,
                        None,
                        None,
                        None,
                    )
                };
                let hwnd = hwnd.map(|h| h.0 as isize).unwrap_or(0);
                let _ = tx.send(hwnd);
                if hwnd == 0 {
                    return;
                }
                // Pump for the life of the process. The window must answer
                // clipboard messages promptly or it hangs whoever sent them; the
                // work itself is `DefWindowProc`, so this thread is idle.
                let mut msg = MSG::default();
                unsafe {
                    while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            });
        match spawned {
            Ok(_) => rx.recv().unwrap_or(0),
            Err(_) => 0,
        }
    });
    HWND(raw as _)
}

/// The system clipboard's change counter. Bumped by every successful
/// `SetClipboardData`/`EmptyClipboard` anywhere on the desktop.
///
/// This takes **no lock**, which is the whole point: it lets the selection flow
/// wait for a copy to land without competing for `OpenClipboard` against the very
/// app that is trying to publish it.
pub fn sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

/// Open the clipboard, retrying while another process holds it. Runs on the
/// engine thread only — never the hook thread, which must never sleep.
unsafe fn open() -> windows::core::Result<()> {
    let owner = owner_window();
    let mut last = windows::core::Error::from_win32();
    for attempt in 0..OPEN_ATTEMPTS {
        match OpenClipboard(owner) {
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

// There is deliberately no `clear()`. Emptying the clipboard before the copy was
// how the flow used to tell "nothing was selected" from stale contents, but it
// made Palimpsest the clipboard owner on every press — including presses that
// then bailed out — and destroyed the user's clipboard whenever the restore
// failed. [`sequence`] answers the same question without touching anything, so
// the read path is now entirely non-destructive: only the restore writes.
