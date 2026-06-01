//! Phase 0 — Spike B: prove the no-feedback-loop mechanism.
//!
//! We inject Unicode text and confirm the hook sees those events flagged as
//! *ours* (via the `dwExtraInfo` signature) — so the engine can ignore its own
//! output and never re-correct it. This is the single most important guard in
//! the whole tool: without it, retyped text would be re-analysed forever.
//!
//! Run: `cargo run -p palimpsest-core --example spike_inject`
//! Don't type during the run, so the only events are the injected ones.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::bounded;
use palimpsest_core::{hook, inject, RawKey};

fn main() {
    println!("Spike B: inject Unicode, confirm the hook tags it as ours (ignored).");
    println!("Please don't type while this runs (~1s).\n");

    let (tx, rx) = bounded::<RawKey>(1024);
    let ours = Arc::new(AtomicUsize::new(0));
    let real = Arc::new(AtomicUsize::new(0));

    // Consumer: classify each key-down as ours (injected by us) or real.
    {
        let (ours, real) = (ours.clone(), real.clone());
        thread::spawn(move || {
            for key in rx.iter() {
                if key.up {
                    continue;
                }
                if key.ours {
                    ours.fetch_add(1, Ordering::Relaxed);
                    println!("  [OURS  — ignored ]  vk=0x{:02X} scan=0x{:02X}", key.vk, key.scan);
                } else {
                    real.fetch_add(1, Ordering::Relaxed);
                    println!(
                        "  [REAL  — observed]  vk=0x{:02X} scan=0x{:02X} injected={}",
                        key.vk, key.scan, key.injected
                    );
                }
            }
        });
    }

    // Hook on its own thread.
    let hook_tx = tx.clone();
    thread::spawn(move || {
        let _ = hook::run(hook_tx);
    });

    // Let the hook install before injecting.
    thread::sleep(Duration::from_millis(500));

    let sample = "привет";
    println!("Injecting {sample:?} ...");
    if let Err(e) = inject::type_unicode(sample) {
        eprintln!("inject error: {e}");
    }

    thread::sleep(Duration::from_millis(500));

    let (o, r) = (ours.load(Ordering::Relaxed), real.load(Ordering::Relaxed));
    let expected = sample.chars().count();
    println!("\nResult: {o} injected key-downs tagged OURS (ignored), {r} real key-downs observed.");
    if o >= expected && r == 0 {
        println!("Spike B PASS — injected input is recognised as ours; no feedback loop.");
    } else {
        println!("Spike B INCONCLUSIVE — review output (did you type, or lose focus?).");
    }
}
