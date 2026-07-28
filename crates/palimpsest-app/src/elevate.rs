//! "Run elevated at logon" — a Windows scheduled task that starts Palimpsest with
//! administrator rights when the user signs in.
//!
//! Why this exists: the keyboard hook runs at the process's integrity level, and
//! Windows UIPI stops a normal (medium) process from seeing keystrokes that go to
//! an *elevated* window. So when the user runs, say, an editor "as administrator"
//! and presses a Palimpsest hotkey there, a normal Palimpsest never sees the keys
//! and silently does nothing — the exact "the shortcut didn't work" report that
//! led here.
//!
//! A `requireAdministrator` manifest would elevate too, but it prompts UAC on
//! every manual launch and can't be auto-started at logon. A scheduled task with
//! the `HighestAvailable` run level starts Palimpsest elevated at logon with no
//! recurring prompt — the single UAC is when the task is created here. Creating
//! and deleting the task need elevation, so those go through an elevated
//! `schtasks`; querying it does not.

use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

use palimpsest_core::UiLang;
use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Scheduled-task name. Bare (no folder) so `schtasks /TN Palimpsest` addresses it.
const TASK_NAME: &str = "Palimpsest";
/// `CREATE_NO_WINDOW` — keep the brief `schtasks`/`powershell` consoles hidden.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Windows `ERROR_CANCELLED` — the user dismissed the UAC prompt.
const ERROR_CANCELLED: i32 = 1223;

/// Why enabling or disabling the elevated-logon task failed.
pub enum Error {
    /// The user dismissed the UAC prompt — a choice, not a fault.
    Cancelled,
    /// Anything else (a `schtasks` failure, an I/O error), with a message.
    Failed(String),
}

/// Whether *this* process is currently running elevated (high integrity). Used to
/// mark the window title, so the user can tell an elevated instance — the one that
/// works in administrator windows — from a normal one at a glance.
pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut ret_len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

/// Set the settings window title: `Palimpsest (Administrator)` when elevated, plain
/// `Palimpsest` otherwise. Called at startup and whenever the language changes, so
/// the marker follows the chosen language. Best-effort — a title is cosmetic.
pub fn apply_window_title(app: &AppHandle, lang: UiLang) {
    let title = if is_elevated() {
        format!("Palimpsest ({})", crate::i18n::strings(lang).admin)
    } else {
        "Palimpsest".to_string()
    };
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.set_title(&title);
    }
}

/// Whether the elevated-logon task is currently registered. Reading a task needs
/// no elevation, so this is a plain query.
pub fn is_enabled() -> bool {
    Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Register the task so `exe` runs elevated at the next logon. Prompts UAC once.
pub fn enable(exe: &Path) -> Result<(), Error> {
    let path = std::env::temp_dir().join("palimpsest-task.xml");
    // Task Scheduler reads the XML as UTF-16; write it LE with a BOM.
    let mut bytes = vec![0xFF, 0xFE];
    for u in task_xml(exe).encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    std::fs::write(&path, bytes).map_err(|e| Error::Failed(e.to_string()))?;
    let r = run_elevated(&["/Create", "/TN", TASK_NAME, "/XML", &path.to_string_lossy(), "/F"]);
    let _ = std::fs::remove_file(&path);
    r
}

/// Remove the task. Prompts UAC once.
pub fn disable() -> Result<(), Error> {
    run_elevated(&["/Delete", "/TN", TASK_NAME, "/F"])
}

/// Run `schtasks <args>` elevated via a hidden PowerShell `Start-Process -Verb
/// RunAs`, waiting for it and mapping its exit code. Going through PowerShell
/// keeps the elevation prompt without pulling `ShellExecuteEx` — and a new
/// `windows` crate feature — into this otherwise-thin crate.
fn run_elevated(args: &[&str]) -> Result<(), Error> {
    let list = args.iter().map(|a| ps_single_quote(a)).collect::<Vec<_>>().join(",");
    // A cancelled UAC makes Start-Process throw; map that to ERROR_CANCELLED so
    // the caller can tell "the user said no" from "it broke".
    let script = format!(
        "try {{ $p = Start-Process -FilePath 'schtasks.exe' -Verb RunAs -PassThru -Wait -WindowStyle Hidden -ArgumentList @({list}); exit $p.ExitCode }} catch {{ exit {ERROR_CANCELLED} }}"
    );
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| Error::Failed(e.to_string()))?;
    match status.code() {
        Some(0) => Ok(()),
        Some(ERROR_CANCELLED) => Err(Error::Cancelled),
        Some(c) => Err(Error::Failed(format!("schtasks exited with code {c}"))),
        None => Err(Error::Failed("schtasks was terminated".into())),
    }
}

/// PowerShell single-quoted literal: wrap in `'…'`, doubling any single quote.
fn ps_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Minimal XML-escaping for the two values interpolated into the task.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The task definition: run `exe` at this user's logon, with highest privileges.
fn task_xml(exe: &Path) -> String {
    let user = xml_escape(&format!(
        "{}\\{}",
        std::env::var("USERDOMAIN").unwrap_or_default(),
        std::env::var("USERNAME").unwrap_or_default()
    ));
    let cmd = xml_escape(&exe.to_string_lossy());
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Starts Palimpsest with administrator rights at logon so its hotkeys work in elevated windows.</Description></RegistrationInfo>
  <Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{user}</UserId></LogonTrigger></Triggers>
  <Principals><Principal id="Author"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>HighestAvailable</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <IdleSettings><StopOnIdleEnd>false</StopOnIdleEnd><RestartOnIdle>false</RestartOnIdle></IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
  </Settings>
  <Actions Context="Author"><Exec><Command>{cmd}</Command></Exec></Actions>
</Task>"#
    )
}
