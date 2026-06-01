//! Phase 4 — Context spike: prove corrections stay silent where they should.
//!
//! Runs the armed runtime for 60s at DEBUG level, so suppressed corrections log
//! a reason. Type the wrong-layout word `ghbdtn<space>` in different places:
//!
//!   - Notepad            → corrects to `привет ` (INFO: "correcting to ...").
//!   - Windows Terminal    → suppressed (DEBUG: "...suppressed by context"),
//!     because `WindowsTerminal.exe` is on the default ignore-list.
//!   - VS Code (`Code.exe`) → suppressed for the same reason.
//!   - A native password box (e.g. the Run dialog won't show one; try an old
//!     Win32 login prompt) → suppressed ("password field").
//!
//! Run: `cargo run -p palimpsest-core --example spike_context`

use std::thread;
use std::time::Duration;

use palimpsest_core::{Config, Runtime};

fn main() {
    tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).init();

    println!("Phase 4 context spike — armed for 60s, DEBUG log.");
    println!("Type 'ghbdtn'+SPACE in different apps:");
    println!("  Notepad           -> corrects");
    println!("  Windows Terminal  -> suppressed (ignore-list)");
    println!("  VS Code           -> suppressed (ignore-list)");
    println!("  native password   -> suppressed (password field)\n");

    let _runtime = Runtime::start(Config::default());
    thread::sleep(Duration::from_secs(60));

    println!("\nDone — shutting down.");
}
