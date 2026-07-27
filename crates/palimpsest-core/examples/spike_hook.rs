//! Phase 0 — Spike A: prove a `WH_KEYBOARD_LL` hook installs, survives, and
//! forwards keystrokes off the OS thread without perceptible lag.
//!
//! Run: `cargo run -p palimpsest-core --example spike_hook`
//! Type anywhere on the desktop; press ESC to quit.

use std::thread;

use crossbeam_channel::bounded;
use palimpsest_core::{hook, RawKey};

const VK_ESCAPE: u32 = 0x1B;

fn main() {
    println!("Spike A: low-level keyboard hook active.");
    println!("Type anywhere — keystrokes are logged here. Press ESC to quit.\n");

    let (tx, rx) = bounded::<RawKey>(1024);

    // The hook owns a dedicated thread with its own message pump.
    thread::spawn(move || {
        if let Err(e) = hook::run(tx) {
            eprintln!("hook error: {e}");
        }
    });

    // `hook::run` installs the hook but leaves it disarmed — the callback drops
    // every event until something arms it (in the app that is `Runtime::start`).
    // Without this the spike installs a working hook and then prints nothing at
    // all, which reads as "the hook is broken" when it is only switched off.
    hook::set_armed(true);

    // Consume on the main thread. In the real app this is the engine thread.
    for key in rx.iter() {
        if !key.up {
            println!(
                "vk=0x{:02X}  scan=0x{:02X}  injected={}  ours={}",
                key.vk, key.scan, key.injected, key.ours
            );
        }
        if key.vk == VK_ESCAPE && !key.up {
            println!("\nESC pressed — exiting.");
            break;
        }
    }
}
