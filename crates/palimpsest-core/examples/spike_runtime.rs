//! Phase 1 — Runtime spike: prove the full plumbing.
//!
//! Starts the [`Runtime`] (hook thread + engine thread + shared `ArcSwap`
//! config), then walks through arm → disarm → re-arm so you can see the global
//! on/off toggle take effect live, and finally shuts down cleanly (hook
//! uninstalled, both threads joined).
//!
//! Run: `cargo run -p palimpsest-core --example spike_runtime`
//! Type anywhere during each window and watch the log:
//!   - ARMED   → each key-down prints a `key` trace line, with no typing lag.
//!   - DISARMED→ typing produces *nothing* (the hook stops forwarding).
//!   - ARMED   → trace lines resume.

use std::thread;
use std::time::Duration;

use palimpsest_core::{Config, Runtime};

fn phase(label: &str, secs: u64) {
    println!("\n=== {label} — type for {secs}s ===");
    thread::sleep(Duration::from_secs(secs));
}

fn main() {
    // TRACE level so the engine's per-key trace lines are visible here.
    tracing_subscriber::fmt().with_max_level(tracing::Level::TRACE).init();

    println!("Phase 1 runtime spike: hook + engine + ArcSwap<Config>, armed on start.");

    let runtime = Runtime::start(Config::default());

    phase("ARMED", 5);
    assert!(runtime.is_armed());

    runtime.disarm();
    phase("DISARMED (no output expected)", 5);
    assert!(!runtime.is_armed());

    runtime.arm();
    phase("RE-ARMED", 5);

    println!("\nShutting down...");
    drop(runtime); // disarms, stops the hook + engine, joins both threads.
    println!("Stopped cleanly — hook uninstalled, threads joined.");
}
