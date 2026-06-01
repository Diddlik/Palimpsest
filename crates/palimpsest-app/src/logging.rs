//! Diagnostics logging and crash capture.
//!
//! Logs go to a daily-rotated file under [`log_dir`] (and to stderr in debug
//! builds). The default level is `info`; because Palimpsest only ever logs typed
//! text at `debug`, **nothing you type reaches the log file** unless you
//! explicitly raise the level with `RUST_LOG`. A panic hook records crashes so a
//! soak run or a user bug report has something to point at, and the log folder
//! can be opened straight from the UI ([`reveal`]).

use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::EnvFilter;

/// Directory holding the rotating diagnostics logs.
pub fn log_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("com.palimpsest.desktop").join("logs")
}

/// Initialise tracing: a daily-rotated file plus stderr. Returns a
/// [`WorkerGuard`] that the caller must keep alive for the process lifetime, or
/// buffered log lines are dropped on exit.
pub fn init() -> WorkerGuard {
    let dir = log_dir();
    let _ = std::fs::create_dir_all(&dir);
    let file = tracing_appender::rolling::daily(&dir, "palimpsest.log");
    let (writer, guard) = tracing_appender::non_blocking(file);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer.and(std::io::stderr))
        .with_ansi(false)
        .init();

    install_panic_hook();
    guard
}

/// Log panics (then defer to the previous hook) so crashes leave a trace.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(panic = %info, "panic");
        previous(info);
    }));
}

/// Open the log folder in Explorer (the "export diagnostics" affordance).
pub fn reveal() -> std::io::Result<()> {
    std::process::Command::new("explorer").arg(log_dir()).spawn().map(|_| ())
}
