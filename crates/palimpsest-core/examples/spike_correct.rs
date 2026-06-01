//! Phase 3 — Correction spike: the full Palimpsest effect, live.
//!
//! Runs the armed runtime for 60s. Unlike `spike_detect`, this one *acts*: it
//! backspaces the wrong-layout word, retypes the corrected text as Unicode, and
//! switches the OS layout so continued typing is correct.
//!
//! Run: `cargo run -p palimpsest-core --example spike_correct`
//! Then click into **Notepad** (not this terminal) and, with EN active:
//!   - Auto:   type  `ghbdtn`  then SPACE   → becomes `привет ` and you're in RU.
//!   - Manual: type  `ghbdtn`  (no space) then press **Pause/Break** → `привет`.
//!
//! Correctly typed words are left untouched. The exit criterion for Phase 3 is
//! exactly the auto case: `ghbdtn ` → `привет `, layout left in Russian.

use std::thread;
use std::time::Duration;

use palimpsest_core::{Config, Runtime};

fn main() {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();

    println!("Phase 3 correction spike — armed for 60s.");
    println!("Click into Notepad, then with EN active:");
    println!("  auto:   type 'ghbdtn' + SPACE         -> 'привет ' (layout switches to RU)");
    println!("  manual: type 'ghbdtn' then Pause/Break -> 'привет'");
    println!("Real words are left alone. Watch this log for 'correcting to ...'.\n");

    let _runtime = Runtime::start(Config::default());
    thread::sleep(Duration::from_secs(60));

    println!("\nDone — shutting down.");
}
