//! Stomp Manager RE: an open editor for the SORAN Dual Stomp.
//!
//! A live monitor and editor: it shows and edits the active preset, signal chain, looper, drum
//! and system settings, saves presets, and logs everything that happens on the pedal.
//!
//! Usage: `stomp-manager [--replay <capture.log>] [--speed <x>] [--version]`

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod device;
mod logbook;
mod theme;
mod widgets;

use std::path::PathBuf;

/// The release version (workspace `Cargo.toml`, the single source of truth).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Short git commit of the build, empty when built outside a git checkout.
pub const GIT_HASH: &str = env!("STOMP_GIT_HASH");

/// `0.2.0 (1a2b3c4d)`, or just the version without git info.
pub fn version_long() -> String {
    if GIT_HASH.is_empty() { VERSION.to_owned() } else { format!("{VERSION} ({GIT_HASH})") }
}

fn icon() -> egui::IconData {
    // amber rounded square with two rings (the two footswitches)
    let n = 128u32;
    let mut rgba = vec![0u8; (n * n * 4) as usize];
    let s = n as f32;
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let r = 28.0;
            let (cx, cy) = (fx.clamp(r, s - r), fy.clamp(r, s - r));
            let inside = (fx - cx).hypot(fy - cy) <= r;
            let ring = |ox: f32| ((fx - ox).hypot(fy - s / 2.0) - 22.0).abs() < 6.0;
            let px = &mut rgba[((y * n + x) * 4) as usize..][..4];
            if inside {
                px.copy_from_slice(if ring(s / 2.0 - 20.0) || ring(s / 2.0 + 20.0) {
                    &[0x0D, 0x0F, 0x13, 0xFF]
                } else {
                    &[0xFF, 0xAE, 0x42, 0xFF]
                });
            }
        }
    }
    egui::IconData { rgba, width: n, height: n }
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

fn main() -> eframe::Result {
    #[cfg(windows)]
    if std::env::args().len() > 1 {
        attach_console();
    }
    let mut opts = app::Options { replay: None, speed: 4.0 };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--replay" => opts.replay = args.next().map(PathBuf::from),
            "--speed" => opts.speed = args.next().and_then(|s| s.parse().ok()).unwrap_or(opts.speed),
            "-h" | "--help" => {
                println!("Dual Stomp Manager {}", version_long());
                println!("usage: stomp-manager [--replay <capture.log>] [--speed <x>] [--version]");
                return Ok(());
            }
            "-V" | "--version" => {
                println!("stomp-manager {}", version_long());
                return Ok(());
            }
            other => eprintln!("ignoring unknown argument {other:?}"),
        }
    }

    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("Dual Stomp Manager {VERSION}"))
            .with_app_id("stomp-manager")
            .with_inner_size([1560.0, 960.0])
            .with_min_inner_size([1100.0, 680.0])
            .with_icon(icon()),
        ..Default::default()
    };
    eframe::run_native("Dual Stomp Manager", native, Box::new(|cc| Ok(Box::new(app::StompApp::new(cc, opts)))))
}
