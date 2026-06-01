//! Phase 2 — Detection spike: prove the heuristics fire on real typing.
//!
//! Runs the live runtime at INFO level so only *decisions* print, not every
//! keystroke. Type a word in the wrong layout followed by a boundary (space)
//! and watch for a `correction:` line naming the text it would retype. Nothing
//! is injected — Phase 3 turns these decisions into actual corrections.
//!
//! Run: `cargo run -p palimpsest-core --example spike_detect`
//! With an EN layout active, type:  ghbdtn<space>   → should detect "привет"
//!                                  ghbdtnm<space>  → "привед"? (try a few)
//! A correctly typed English word (e.g. "hello ") should produce no line.

use std::thread;
use std::time::Duration;

use palimpsest_core::{Config, Runtime};

fn main() {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();

    println!("Phase 2 detection spike — armed for 30s. INFO log only.");
    println!("With EN active, type e.g.  ghbdtn<space>  and watch for a 'correction:' line.");
    println!("Correctly typed words should stay silent. (No text is injected yet.)\n");

    let _runtime = Runtime::start(Config::default());
    thread::sleep(Duration::from_secs(30));

    println!("\nDone — shutting down.");
}
