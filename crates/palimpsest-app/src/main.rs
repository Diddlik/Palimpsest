//! Palimpsest desktop entry point.
//!
//! Tauri owns the main thread (it must, on Windows, to drive the tray and
//! window messages). From `setup()` we load the persisted config, start the
//! core [`Runtime`] (hook + engine threads), forward engine events to the
//! settings window, and build the tray. The window is hidden by default and
//! only hides — never exits — on close, so Palimpsest lives in the tray.

#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod commands;
mod logging;
mod state;
mod tray;

use palimpsest_core::Runtime;
use tauri::{Emitter, Manager};

use state::AppState;

fn main() {
    // Held for the whole process so buffered log lines flush on exit.
    let _log_guard = logging::init();

    tauri::Builder::default()
        // Single-instance must be registered first so a second launch is
        // intercepted before any other plugin or the hook is set up again.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            tray::show_settings(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::set_enabled,
            commands::undo,
            commands::get_autostart,
            commands::set_autostart,
            commands::open_logs,
            commands::list_processes,
            commands::list_layouts,
            commands::set_hotkey,
            commands::clear_hotkey,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let mut config = state::load_config(&handle);

            // Reconcile saved/default layout pairs with the layouts actually
            // installed, so the UI never shows "(not installed)" dummy pairs;
            // persist the result so it sticks across sessions. Skipped if the
            // installed list can't be read, to avoid wiping real pairs.
            let installed = palimpsest_core::layouts::installed();
            if !installed.is_empty()
                && palimpsest_core::layouts::sanitize_pairs(&mut config, &installed)
            {
                state::save_config(&handle, &config);
            }

            let enabled = config.enabled;
            let runtime = Runtime::start(config);

            // Drain engine events on a background thread and relay them to the
            // settings window. The loop ends when the runtime (and its sender)
            // is dropped at shutdown.
            let events = runtime.events();
            let event_handle = handle.clone();
            std::thread::spawn(move || {
                while let Ok(event) = events.recv() {
                    let _ = event_handle.emit("engine-event", event);
                }
            });

            let toggle_item = tray::build(&handle, enabled)?;
            app.manage(AppState { runtime, toggle_item });
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the settings window hides it instead of quitting; the tray
            // keeps Palimpsest running.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "settings" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to build Palimpsest")
        .run(|_app, event| {
            // A window-driven exit (no code) is suppressed so the app survives in
            // the tray; an explicit Quit (`app.exit(0)`) carries a code and is
            // allowed through.
            if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
