//! Open Stomp Manager: an open editor and live monitor for the SORAN Dual Stomp.
//!
//! Rust runs the pedal engine ([`hub`]) and hands the frontend (`app/src`, React) the pedal state
//! as JSON sections ([`view`]); the frontend sends edits back through the `command` command.
//!
//! Usage: `open-stomp-manager [--replay <capture.log>] [--speed <x>] [--version]`

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod hub;
mod import;
mod log;
mod logbook;
mod view;

use std::path::PathBuf;

use hub::{Cmd, Hub, Snapshot};
use tauri::{Manager, State};

/// The release version (workspace `Cargo.toml`, the single source of truth).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Short git commit of the build, empty when built outside a git checkout.
pub const GIT_HASH: &str = env!("STOMP_GIT_HASH");
/// Version of the open-stomp library the app was built against (from `Cargo.lock`).
pub const LIB_VERSION: &str = env!("STOMP_LIB_VERSION");

/// `0.9.0 (1a2b3c4d)`, or just the version without git info.
pub fn version_long() -> String {
    if GIT_HASH.is_empty() { VERSION.to_owned() } else { format!("{VERSION} ({GIT_HASH})") }
}

/// Release builds on Windows use the GUI subsystem (no console window), so terminal output from
/// `--help` / `--version` would vanish. Attach to the console of the shell that launched us.
#[cfg(windows)]
fn attach_console() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    // SAFETY: no preconditions; fails harmlessly when there is no parent console.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

/// Everything the frontend needs to start: the latest value of every section.
#[tauri::command]
fn snapshot(hub: State<'_, Hub>) -> Snapshot {
    hub.snapshot()
}

#[tauri::command]
fn command(hub: State<'_, Hub>, cmd: Cmd) {
    hub.command(cmd);
}

/// Rows `start..start + count` of the filtered activity log.
#[tauri::command]
fn log_rows(hub: State<'_, Hub>, start: usize, count: usize) -> Vec<log::RowView> {
    hub.log().rows(start, count)
}

/// The filtered activity log as text.
#[tauri::command]
fn log_export(hub: State<'_, Hub>) -> String {
    hub.log().export()
}

fn main() {
    #[cfg(windows)]
    if std::env::args().len() > 1 {
        attach_console();
    }
    let mut opts = hub::Options { replay: None, speed: 4.0 };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--replay" => opts.replay = args.next().map(PathBuf::from),
            "--speed" => opts.speed = args.next().and_then(|s| s.parse().ok()).unwrap_or(opts.speed),
            "-h" | "--help" => {
                println!("Open Stomp Manager {} · open-stomp {LIB_VERSION}", version_long());
                println!("usage: open-stomp-manager [--replay <capture.log>] [--speed <x>] [--version]");
                return;
            }
            "-V" | "--version" => {
                println!("open-stomp-manager {}", version_long());
                println!("open-stomp {LIB_VERSION}");
                return;
            }
            other => eprintln!("ignoring unknown argument {other:?}"),
        }
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            // `STOMP_DEV=devtools` opens the web inspector (debug builds; or right-click → Inspect)
            #[cfg(debug_assertions)]
            if std::env::var("STOMP_DEV").is_ok_and(|f| f.split(',').any(|f| f.trim() == "devtools"))
                && let Some(w) = app.get_webview_window("main")
            {
                w.open_devtools();
            }
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_title(&format!("Open Stomp Manager {VERSION}"));
            }
            let hub = Hub::spawn(app.handle().clone(), opts);
            app.manage(hub);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![snapshot, command, log_rows, log_export])
        .run(tauri::generate_context!())
        .expect("run the Tauri app");
}
