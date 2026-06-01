//! Low-level keyboard hook (`WH_KEYBOARD_LL`).
//!
//! The hook callback runs inside a hard OS latency budget
//! (`LowLevelHooksTimeout`, ~300 ms). To stay well under it the callback does
//! the absolute minimum: check an atomic "armed" flag, read the event, build a
//! [`RawKey`], `try_send` it over a channel, and immediately call
//! `CallNextHookEx`. No allocation, no locks, no blocking. If the receiver is
//! briefly behind we drop the event rather than stall the OS input pipeline.
//!
//! `WH_KEYBOARD_LL` delivers callbacks via the installing thread's message
//! queue, so the thread that calls [`run`] must pump messages — which [`run`]
//! does, blocking until `WM_QUIT`. [`request_stop`] posts that `WM_QUIT` to the
//! hook thread for a clean uninstall.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::OnceLock;

use crossbeam_channel::Sender;
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG,
    WH_KEYBOARD_LL, WM_KEYUP, WM_QUIT, WM_SYSKEYUP,
};

use crate::{RawKey, PALIMPSEST_SIGNATURE};

/// Sender shared with the `extern "system"` callback, which cannot capture an
/// environment. Set once by [`run`] before the hook is installed.
static SENDER: OnceLock<Sender<RawKey>> = OnceLock::new();

/// Global on/off. The callback forwards events only while this is `true`, so a
/// disarmed hook is effectively a no-op pass-through with no channel traffic.
/// Starts disarmed; the runtime arms it once the engine is consuming.
static ARMED: AtomicBool = AtomicBool::new(false);

/// Thread id of the live hook pump, captured by [`run`]. Used by
/// [`request_stop`] to post `WM_QUIT`. `0` means no hook is running.
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);

/// Arm or disarm event forwarding. Lock-free; safe to call from any thread.
pub fn set_armed(armed: bool) {
    ARMED.store(armed, Ordering::Relaxed);
}

/// Whether the hook is currently forwarding events.
pub fn is_armed() -> bool {
    ARMED.load(Ordering::Relaxed)
}

/// The hook procedure invoked by Windows for every keystroke system-wide.
///
/// # Safety
/// Called by the OS with a valid `KBDLLHOOKSTRUCT` pointer in `lparam` when
/// `code == HC_ACTION`. We dereference it only in that case.
unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && ARMED.load(Ordering::Relaxed) {
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let key = RawKey {
            vk: kb.vkCode,
            scan: kb.scanCode,
            injected: (kb.flags.0 & LLKHF_INJECTED.0) != 0,
            ours: kb.dwExtraInfo == PALIMPSEST_SIGNATURE,
            up: matches!(wparam.0 as u32, WM_KEYUP | WM_SYSKEYUP),
            time: kb.time,
        };
        if let Some(sender) = SENDER.get() {
            // Non-blocking on purpose: never make the OS wait on us.
            let _ = sender.try_send(key);
        }
    }
    // First arg is ignored by modern Windows; pass a default handle.
    CallNextHookEx(None, code, wparam, lparam)
}

/// Install the low-level keyboard hook and pump messages until `WM_QUIT`.
///
/// Blocks the calling thread, so run it on a dedicated thread. Every observed
/// keystroke is forwarded to `sender` *while armed* (see [`set_armed`]).
/// Returns when the message loop ends — e.g. after [`request_stop`] — or on a
/// Win32 error. The hook is uninstalled before returning.
pub fn run(sender: Sender<RawKey>) -> windows::core::Result<()> {
    // Ignore the error if a sender was already set: the first one wins. The
    // hook is a process-global resource, so a single global sender is correct.
    let _ = SENDER.set(sender);

    unsafe {
        HOOK_THREAD_ID.store(GetCurrentThreadId(), Ordering::SeqCst);

        let hmod = GetModuleHandleW(None)?;
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), HINSTANCE(hmod.0), 0)?;

        let mut msg = MSG::default();
        // GetMessageW returns >0 normally, 0 for WM_QUIT, -1 on error.
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        UnhookWindowsHookEx(hook)?;
        HOOK_THREAD_ID.store(0, Ordering::SeqCst);
    }
    Ok(())
}

/// Ask the hook thread to stop by posting `WM_QUIT` to it, breaking the message
/// loop in [`run`] so the hook uninstalls cleanly. No-op if no hook is running.
pub fn request_stop() {
    let tid = HOOK_THREAD_ID.load(Ordering::SeqCst);
    if tid != 0 {
        unsafe {
            // Failure only means the thread already exited — nothing to do.
            let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}
