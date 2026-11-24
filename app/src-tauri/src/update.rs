//! Updates: at startup, and when asked, look at the newest release's `latest.json` (published by
//! the release workflow; GitHub's `releases/latest` skips pre-releases).
//!
//! Release builds on macOS and installed Windows copies download the update, install it once no
//! import, restore or export is running (the hub decides when) and restart. Everywhere else (Linux,
//! the portable Windows zip, Nix and debug builds) the app only says a newer version is out and
//! links to its release page.

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::{Update, UpdaterBuilder, UpdaterExt};

pub const RELEASES: &str = "https://github.com/JackMechem/open-stomp-manager/releases";

/// The `update` section.
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum UpdateView {
    #[default]
    Idle,
    /// `manual`: the user asked, so the answer is shown even when there's nothing new.
    Checking { manual: bool },
    UpToDate { manual: bool },
    /// A newer release this copy can't install by itself.
    Available { version: String, url: String },
    Downloading { version: String, percent: Option<u8> },
    /// Downloaded; it installs once the pedal work in progress is done.
    Ready { version: String },
    Installing { version: String },
    Failed { message: String, url: String },
}

impl UpdateView {
    /// A check or an install is under way.
    pub fn busy(&self) -> bool {
        matches!(
            self,
            UpdateView::Checking { .. }
                | UpdateView::Downloading { .. }
                | UpdateView::Ready { .. }
                | UpdateView::Installing { .. }
        )
    }

    /// The release page to open, when there is one to show.
    pub fn url(&self) -> Option<&str> {
        match self {
            UpdateView::Available { url, .. } | UpdateView::Failed { url, .. } => Some(url),
            _ => None,
        }
    }
}

/// What a check reports back to the hub.
pub enum Event {
    View(UpdateView),
    /// Downloaded and verified, waiting to be installed.
    Ready(Box<Update>, Vec<u8>),
}

pub fn release_url(version: &str) -> String {
    format!("{RELEASES}/tag/v{version}")
}

/// Whether this copy can replace itself.
pub fn can_install() -> bool {
    if cfg!(debug_assertions) || cfg!(target_os = "linux") {
        return false;
    }
    let Ok(exe) = std::env::current_exe() else { return false };
    if exe.starts_with("/nix/store") {
        return false;
    }
    // the installer puts an uninstaller next to the app; the portable zip has none
    !cfg!(windows) || exe.with_file_name("uninstall.exe").exists()
}

/// Check in the background. A failed check at startup (offline, say) says nothing.
pub fn check(app: AppHandle, manual: bool, send: impl Fn(Event) + Send + Sync + 'static) {
    send(Event::View(UpdateView::Checking { manual }));
    tauri::async_runtime::spawn(async move {
        match run(&app, manual, &send).await {
            Ok(Some(view)) => send(Event::View(view)),
            Ok(None) => {}
            Err(e) if manual => send(Event::View(UpdateView::Failed {
                message: format!("Couldn't check for updates: {e}"),
                url: RELEASES.into(),
            })),
            Err(_) => send(Event::View(UpdateView::Idle)),
        }
    });
}

async fn run(
    app: &AppHandle,
    manual: bool,
    send: &(impl Fn(Event) + Send + Sync),
) -> tauri_plugin_updater::Result<Option<UpdateView>> {
    if !can_install() {
        // `latest.json` has no bundle for these copies, so note the newer version and decline it:
        // the check then ends without looking for one
        let newer = Arc::new(Mutex::new(None));
        let seen = newer.clone();
        builder(app)?
            .version_comparator(move |current, release| {
                if release.version > current {
                    *seen.lock().expect("update lock") = Some(release.version.to_string());
                }
                false
            })
            .build()?
            .check()
            .await?;
        let newer = newer.lock().expect("update lock").take();
        return Ok(Some(match newer {
            Some(version) => UpdateView::Available { url: release_url(&version), version },
            None => UpdateView::UpToDate { manual },
        }));
    }
    let Some(update) = builder(app)?.build()?.check().await? else {
        return Ok(Some(UpdateView::UpToDate { manual }));
    };
    let version = update.version.clone();
    send(Event::View(UpdateView::Downloading { version: version.clone(), percent: None }));
    let (mut done, mut shown) = (0u64, None);
    // shown even after a startup check: this one would fail again next time
    let bytes = match update
        .download(
            |chunk, total| {
                done += chunk as u64;
                let percent = total.filter(|&t| t > 0).map(|t| (done * 100 / t).min(100) as u8);
                if percent != shown {
                    shown = percent;
                    send(Event::View(UpdateView::Downloading { version: version.clone(), percent }));
                }
            },
            || {},
        )
        .await
    {
        Ok(bytes) => bytes,
        Err(e) => {
            return Ok(Some(UpdateView::Failed {
                message: format!("Couldn't download {version}: {e}"),
                url: release_url(&version),
            }));
        }
    };
    send(Event::Ready(Box::new(update), bytes));
    Ok(None)
}

/// `STOMP_UPDATE_URL=<latest.json URL>` (debug builds, http allowed) checks another manifest.
fn builder(app: &AppHandle) -> tauri_plugin_updater::Result<UpdaterBuilder> {
    let builder = app.updater_builder();
    match std::env::var("STOMP_UPDATE_URL") {
        Ok(url) if cfg!(debug_assertions) => builder.endpoints(vec![url.parse()?]),
        _ => Ok(builder),
    }
}

/// Install a downloaded update and restart into it. On Windows the installer closes the app and
/// starts the new one itself. Returns only when the install failed.
pub fn install(app: &AppHandle, update: &Update, bytes: &[u8]) -> UpdateView {
    match update.install(bytes) {
        Ok(()) => app.restart(),
        Err(e) => UpdateView::Failed {
            message: format!("Couldn't install {}: {e}", update.version),
            url: release_url(&update.version),
        },
    }
}

/// Open a release page in the browser.
pub fn open(url: &str) {
    let _ = tauri_plugin_opener::open_url(url, None::<&str>);
}
