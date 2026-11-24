//! The hub: a thread that owns the pedal engine and the `DeviceState`, and keeps the frontend in
//! step with them.
//!
//! The engine wakes the hub whenever it has updates. The hub applies them, rebuilds the view
//! sections ([`crate::view`]) and emits the ones that changed as one `state` event, at most once
//! per frame, so a knob sweep doesn't flood the webview. Commands from the frontend arrive on the
//! same channel as the wake-ups, so they run in order with the updates.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Local};
use open_stomp::device::{Config, Device, Status, Update};
use open_stomp::midi::host;
use open_stomp::midi::req;
use open_stomp::upload::{self, Chunk, Reassembler, UserAmpInfo, amp_sub};
use open_stomp::{Category, DeviceState, Module, Msg as Midi, Origin};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tauri::{AppHandle, Emitter};

use crate::backup::{self, ConvertedData, Library, SlotModel};
use crate::import::{self, Job, Kind};
use crate::log::{FilterArgs, Log, RowKind};
use crate::update::{self, UpdateView};
use crate::view::{self, ExportView, ImportView, Inputs, Outcome, RestoreJobView, RestoreView};

/// After a preset change from the pedal, its messages are held until it has been quiet this long,
/// then applied together, so turning the preset knob several clicks jumps instead of stepping.
const SETTLE: Duration = Duration::from_millis(150);
/// ...but never held longer than this.
const MAX_HOLD: Duration = Duration::from_secs(5);
/// Shortest gap between two `state` events (one frame at 60 Hz).
const FRAME: Duration = Duration::from_millis(16);
/// A restore that has heard no ack for this long is taken as finished (some writes went unanswered).
const ACK_IDLE: Duration = Duration::from_secs(8);
/// SysEx commands whose write acks a restore counts: user AMP, IR data, IR name, preset name,
/// preset, settings.
const RESTORE_ACKS: [u8; 6] = [0x58, 0x10, 0x09, 0x12, 0x0B, 0x14];
/// An export reading slots back gives up when the pedal has been silent this long.
const READ_IDLE: Duration = Duration::from_secs(5);

/// Something the frontend asks for (the `command` Tauri command). Module numbers are
/// `Module as u8` (`OD/FX` 0, `AMP` 1, … `DRUM` 9).
#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Cmd {
    SelectPreset { index: u8 },
    SetHead { module: u8, model: u8, on: bool },
    SetParam { module: u8, slot: u8, value: u8 },
    SetVolume { value: u8 },
    ShowPage { module: u8 },
    /// Store the live sound in `slot` under `name`, then select it.
    SavePreset { slot: u8, name: String },
    LooperButton { button: LoopButton },
    SetLooper { rec_level: Option<u8>, play_level: Option<u8>, sync: Option<bool>, ar_mode: Option<bool> },
    SetDrum { playing: Option<bool>, pattern: Option<u8>, level: Option<u8> },
    SetTuner { on: bool },
    /// Change some system settings; the rest stay as read from the pedal.
    SetSettings {
        input_trim: Option<u8>,
        eq_bass: Option<u8>,
        eq_mid: Option<u8>,
        eq_treble: Option<u8>,
        wireless: Option<bool>,
        loopback: Option<bool>,
    },
    RenameSlot { kind: Kind, slot: u8, name: String },
    StartImport { request: import::Request },
    CancelImport,
    ClearImportOutcome,
    /// Put the presets and models of a preset file on the pedal (see [`backup`]).
    StartRestore { request: backup::RestoreRequest },
    CancelRestore,
    ClearRestoreOutcome,
    SetLogFilter { filter: FilterArgs },
    ClearLog,
    Connect { port: String },
    Disconnect,
    Resync,
    SetListenOnly { on: bool },
    /// Replay a capture log; without a path, the default one (`replayPath` in `conn`).
    Replay { path: Option<String>, speed: Option<f32> },
    StopReplay,
    CheckUpdates,
    /// Open the release page the `update` section links to.
    OpenRelease,
    DismissUpdate,
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum LoopButton {
    RecPlayDub,
    Stop,
    UndoRedo,
    Clear,
}

/// The latest value of every section, with the number of the event that last changed it.
#[derive(Serialize, Clone, Default)]
pub struct Snapshot {
    seq: u64,
    sections: Map<String, Value>,
}

/// One `state` event: the sections that changed, plus what just happened (for highlights).
#[derive(Serialize, Clone)]
struct Patch {
    seq: u64,
    sections: Map<String, Value>,
    activity: Vec<Activity>,
    /// Messages received from / sent to the pedal since the last event (the In / Out lights).
    rx: u32,
    tx: u32,
}

/// Something the user should see light up.
#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
enum Activity {
    Module { module: u8, slot: Option<u8>, host: bool },
    Preset { host: bool },
}

enum Msg {
    Wake,
    Cmd(Cmd),
    Call(Call, Sender<Result<Value, String>>),
    Update(update::Event),
}

/// Something the frontend asks for and waits on (the preset-file Tauri commands).
pub enum Call {
    /// Read a preset file and say what importing it would do ([`backup::Plan`]).
    Plan { path: String },
    /// Save preset `preset`, or everything (`None`), to `path`, reading its models from the pedal.
    Export { preset: Option<u8>, path: String },
}

/// Handle kept in Tauri's state.
pub struct Hub {
    tx: Sender<Msg>,
    latest: Arc<Mutex<Snapshot>>,
    log: Arc<Mutex<Log>>,
}

pub struct Options {
    pub replay: Option<PathBuf>,
    pub speed: f32,
    /// Where the app keeps its copy of each user slot's model; `None` keeps them in memory only.
    pub library: Option<PathBuf>,
}

fn local(at: SystemTime) -> DateTime<Local> {
    at.into()
}

impl Hub {
    pub fn spawn(app: AppHandle, opts: Options) -> Hub {
        let (tx, rx) = mpsc::channel();
        let wake = tx.clone();
        // starting on a replay must not grab the pedal first
        let config = Config {
            client_name: "Open Stomp Manager".into(),
            auto_connect: opts.replay.is_none(),
            ..Config::default()
        };
        let dev = Device::spawn_with_waker(config, move || {
            let _ = wake.send(Msg::Wake);
        });
        let replay_path = opts.replay.clone().or_else(default_replay).map(|p| p.display().to_string());
        if let Some(path) = opts.replay {
            dev.replay(path, opts.speed);
        }
        // `STOMP_DEV=log,import` opens those at startup (see docs/building.md)
        let dev_flags = std::env::var("STOMP_DEV")
            .unwrap_or_default()
            .split(',')
            .map(|f| f.trim().to_owned())
            .filter(|f| !f.is_empty())
            .collect();
        let latest = Arc::new(Mutex::new(Snapshot::default()));
        let log = Arc::new(Mutex::new(Log::default()));
        let mut worker = Worker {
            dev,
            app,
            tx: tx.clone(),
            latest: latest.clone(),
            log: log.clone(),
            state: DeviceState::default(),
            status: Status::Disconnected,
            ports: vec![],
            sync: (0, 0),
            listen_only: false,
            followed: Module::Amp,
            held: vec![],
            held_since: None,
            pedal_msg_at: Instant::now(),
            replay_path,
            replay_speed: opts.speed,
            dev_flags,
            job: None,
            outcome: None,
            library: Library::open(opts.library),
            restore: None,
            restore_outcome: None,
            export: None,
            update: UpdateView::Idle,
            pending_update: None,
            activity: vec![],
            received: 0,
            sent: 0,
            dirty: true,
            last_emit: Instant::now() - FRAME,
        };
        // `STOMP_DEBUG_IMPORT=<file>` converts a file at startup without uploading it
        if let Ok(path) = std::env::var("STOMP_DEBUG_IMPORT") {
            let kind = if path.to_ascii_lowercase().ends_with(".nam") { Kind::Amp } else { Kind::Ir };
            worker.start_import(import::Request { kind, slot: 9, path, optimised: true, normalise: true }, true);
        }
        // debug builds only check when asked (and never install)
        if !cfg!(debug_assertions) {
            worker.check_updates(false);
        }
        thread::Builder::new()
            .name("open-stomp-hub".into())
            .spawn(move || worker.run(rx))
            .expect("spawn hub thread");
        Hub { tx, latest, log }
    }

    pub fn command(&self, c: Cmd) {
        let _ = self.tx.send(Msg::Cmd(c));
    }

    /// Run `c` on the hub and wait for its answer.
    pub fn call(&self, c: Call) -> Result<Value, String> {
        let (tx, rx) = mpsc::channel();
        self.tx.send(Msg::Call(c, tx)).map_err(|_| "the app is shutting down".to_owned())?;
        // an export reads models from the pedal: about 6 s a slot over Bluetooth
        rx.recv_timeout(Duration::from_secs(600)).map_err(|_| "no answer from the pedal engine".to_owned())?
    }

    pub fn snapshot(&self) -> Snapshot {
        self.latest.lock().expect("hub snapshot lock").clone()
    }

    pub fn log(&self) -> std::sync::MutexGuard<'_, Log> {
        self.log.lock().expect("log lock")
    }
}

/// The capture shipped in this repo, for trying the app without a pedal.
fn default_replay() -> Option<PathBuf> {
    let here = PathBuf::from("captures/logs/01-main-session.log");
    let repo = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../captures/logs/01-main-session.log"));
    [here, repo].into_iter().find(|p| p.exists()).and_then(|p| p.canonicalize().ok())
}

struct Worker {
    dev: Device,
    app: AppHandle,
    /// For the import thread's wake-ups.
    tx: Sender<Msg>,
    latest: Arc<Mutex<Snapshot>>,
    log: Arc<Mutex<Log>>,
    state: DeviceState,
    status: Status,
    ports: Vec<String>,
    sync: (usize, usize),
    listen_only: bool,
    followed: Module,
    held: Vec<(SystemTime, Origin, Vec<u8>)>,
    held_since: Option<Instant>,
    pedal_msg_at: Instant,
    replay_path: Option<String>,
    replay_speed: f32,
    dev_flags: Vec<String>,
    job: Option<Job>,
    outcome: Option<Outcome>,
    /// The app's copy of each user slot's model.
    library: Library,
    restore: Option<Restore>,
    restore_outcome: Option<Outcome>,
    /// An export waiting for its models to be read from the pedal.
    export: Option<Export>,
    update: UpdateView,
    /// Downloaded, waiting for the import / restore / export in progress to finish.
    pending_update: Option<(Box<tauri_plugin_updater::Update>, Vec<u8>)>,
    activity: Vec<Activity>,
    /// Messages from / to the pedal since the last event.
    received: u32,
    sent: u32,
    /// Something changed since the last `state` event.
    dirty: bool,
    last_emit: Instant,
}

impl Worker {
    fn run(&mut self, rx: Receiver<Msg>) {
        loop {
            let timeout = self.next_deadline().map_or(Duration::from_secs(3600), |d| {
                d.saturating_duration_since(Instant::now())
            });
            match rx.recv_timeout(timeout) {
                Ok(Msg::Cmd(c)) => {
                    self.command(c);
                    self.dirty = true;
                }
                Ok(Msg::Call(c, reply)) => {
                    self.call(c, reply);
                    self.dirty = true;
                }
                Ok(Msg::Wake) => self.dirty |= self.job.is_some() || self.restore.is_some(),
                Ok(Msg::Update(e)) => {
                    match e {
                        update::Event::View(v) => self.update = v,
                        update::Event::Ready(u, bytes) => {
                            self.update = UpdateView::Ready { version: u.version.clone() };
                            self.pending_update = Some((u, bytes));
                        }
                    }
                    self.dirty = true;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            self.poll();
            self.release_held();
            self.finish_import();
            self.step_restore();
            self.step_export();
            self.install_update();
            if self.dirty && self.last_emit.elapsed() >= FRAME {
                self.emit();
            }
        }
    }

    /// When the loop must wake up by itself: to release held messages or send a pending event.
    fn next_deadline(&self) -> Option<Instant> {
        let release = self.held_since.map(|since| (self.pedal_msg_at + SETTLE).min(since + MAX_HOLD));
        let emit = self.dirty.then(|| self.last_emit + FRAME);
        let restore = self.restore.as_ref().and_then(|r| r.writing.as_ref()).map(|w| w.last_ack + ACK_IDLE);
        let export = self.export.as_ref().map(|e| e.last + READ_IDLE);
        release.into_iter().chain(emit).chain(restore).chain(export).min()
    }

    fn inputs(&self) -> Inputs<'_> {
        Inputs {
            state: &self.state,
            status: &self.status,
            ports: &self.ports,
            sync: self.sync,
            listen_only: self.listen_only,
            followed: self.followed,
            replay_path: self.replay_path.as_deref(),
            log: self.log.lock().expect("log lock").summary(),
            import: ImportView { job: self.job.as_ref().map(Job::view), outcome: self.outcome.clone() },
            restore: RestoreView { job: self.restore.as_ref().map(Restore::view), outcome: self.restore_outcome.clone() },
            export: self.export.as_ref().map(|e| ExportView { done: e.done, total: e.total }),
            update: &self.update,
            dev_flags: &self.dev_flags,
        }
    }

    fn editable(&self) -> bool {
        matches!(self.status, Status::Connected { .. }) && !self.listen_only
    }

    fn can_edit_sound(&self) -> bool {
        self.editable() && self.state.live_known
    }

    fn note(&self, kind: RowKind, text: String) {
        self.log.lock().expect("log lock").push_note(Local::now(), kind, text);
    }

    fn command(&mut self, c: Cmd) {
        let module = |i: u8| Module::from_index(i);
        let (editable, sound) = (self.editable(), self.can_edit_sound());
        match c {
            Cmd::SelectPreset { index } => self.dev.select_preset(index),
            Cmd::SetHead { module: m, model, on } => {
                if let (Some(m), true) = (module(m), sound) {
                    self.dev.set_head(m, model, on);
                }
            }
            Cmd::SetParam { module: m, slot, value } => {
                if let (Some(m), true) = (module(m), sound) {
                    self.dev.set_param(m, slot as usize, value);
                }
            }
            Cmd::SetVolume { value } if sound => self.dev.set_volume(value),
            Cmd::ShowPage { module: m } => {
                if let (Some(m), true) = (module(m), editable) {
                    self.followed = m;
                    self.dev.show_page(m);
                }
            }
            Cmd::SavePreset { slot, name } => {
                if let (Some(cur), true) = (self.state.current, sound) {
                    let mut p = self.state.live.clone();
                    p.index = slot;
                    p.name = name.trim().to_owned();
                    self.dev.save_preset(cur, &p);
                }
            }
            Cmd::LooperButton { button } if editable => {
                self.dev.looper_button(match button {
                    LoopButton::RecPlayDub => host::LOOP_REC_PLAY_DUB,
                    LoopButton::Stop => host::LOOP_STOP,
                    LoopButton::UndoRedo => host::LOOP_UNDO_REDO,
                    LoopButton::Clear => host::LOOP_CLEAR,
                });
            }
            Cmd::SetLooper { rec_level, play_level, sync, ar_mode } if editable => {
                let ccs = [
                    rec_level.map(|v| (0x42, v)),
                    play_level.map(|v| (0x43, v)),
                    sync.map(|on| (0x49, on as u8)),
                    ar_mode.map(|ar| (0x4A, ar as u8)),
                ];
                for (cc, v) in ccs.into_iter().flatten() {
                    self.dev.send(host::cc(cc, v));
                }
            }
            Cmd::SetTuner { on } if editable => self.dev.set_tuner(on),
            Cmd::SetDrum { playing, pattern, level } if editable => {
                let ccs = [playing.map(|on| (0x3F, on as u8)), pattern.map(|p| (0x40, p)), level.map(|v| (0x41, v))];
                for (cc, v) in ccs.into_iter().flatten() {
                    self.dev.send(host::cc(cc, v));
                }
            }
            Cmd::SetSettings { input_trim, eq_bass, eq_mid, eq_treble, wireless, loopback } if editable => {
                if let Some(s) = &self.state.settings {
                    let mut s = s.clone();
                    s.input_trim = input_trim.unwrap_or(s.input_trim).min(20);
                    s.eq_bass = eq_bass.unwrap_or(s.eq_bass);
                    s.eq_mid = eq_mid.unwrap_or(s.eq_mid);
                    s.eq_treble = eq_treble.unwrap_or(s.eq_treble);
                    s.wireless = wireless.unwrap_or(s.wireless);
                    s.loopback = loopback.unwrap_or(s.loopback);
                    self.dev.write_settings(&s);
                }
            }
            Cmd::RenameSlot { kind, slot, name } if editable => {
                let name = name.trim();
                let sent = match kind {
                    Kind::Amp => self.dev.rename_user_amp(slot, name),
                    Kind::Ir => self.dev.rename_user_ir(slot, name),
                };
                match sent {
                    Err(e) => self.note(RowKind::Error, format!("Rename failed: {e}")),
                    Ok(()) => {
                        if let Err(e) = self.library.rename(kind, slot, name) {
                            self.note(RowKind::Error, format!("Couldn't update the app's copy of {}: {e}", kind.slot_label(slot)));
                        }
                    }
                }
            }
            Cmd::StartImport { request } => {
                if self.job.is_none() && self.restore.is_none() && self.export.is_none() {
                    self.start_import(request, false);
                }
            }
            Cmd::StartRestore { request } => {
                if self.job.is_none() && self.restore.is_none() && self.export.is_none() {
                    self.start_restore(&request);
                }
            }
            Cmd::CancelRestore => {
                // only while converting: once writing has started, the pedal's queue runs to the end
                if self.restore.as_ref().is_some_and(|r| r.writing.is_none())
                    && let Some(r) = self.restore.take()
                {
                    if let Some(c) = &r.converting {
                        c.job.cancel();
                    }
                    self.restore_outcome = Some(Outcome { text: "Import cancelled; nothing was written to the pedal.".into(), error: true });
                }
            }
            Cmd::ClearRestoreOutcome => self.restore_outcome = None,
            Cmd::CancelImport => {
                if let Some(job) = self.job.take() {
                    job.cancel();
                    self.outcome = Some(Outcome { text: "Import cancelled; the slot was not changed.".into(), error: true });
                }
            }
            Cmd::ClearImportOutcome => self.outcome = None,
            Cmd::SetLogFilter { filter } => self.log.lock().expect("log lock").set_filter(filter),
            Cmd::ClearLog => self.log.lock().expect("log lock").clear(),
            Cmd::Connect { port } => self.dev.connect(port),
            Cmd::Disconnect => self.dev.disconnect(),
            Cmd::Resync => self.dev.resync(),
            Cmd::SetListenOnly { on } => {
                self.listen_only = on;
                self.dev.set_sync(!on);
                if !on {
                    self.dev.resync();
                }
            }
            Cmd::Replay { path, speed } => {
                if let Some(path) = path.or_else(|| self.replay_path.clone()) {
                    self.dev.replay(path, speed.unwrap_or(self.replay_speed));
                }
            }
            Cmd::StopReplay => self.dev.stop_replay(),
            Cmd::CheckUpdates if !self.update.busy() => self.check_updates(true),
            Cmd::OpenRelease => {
                if let Some(url) = self.update.url() {
                    update::open(url);
                }
            }
            Cmd::DismissUpdate if !self.update.busy() => self.update = UpdateView::Idle,
            Cmd::CheckUpdates | Cmd::DismissUpdate => {}
            // edits while disconnected or listen-only: the UI disables them, so just drop them
            Cmd::SetVolume { .. }
            | Cmd::LooperButton { .. }
            | Cmd::SetLooper { .. }
            | Cmd::SetDrum { .. }
            | Cmd::SetTuner { .. }
            | Cmd::SetSettings { .. }
            | Cmd::RenameSlot { .. } => {}
        }
    }

    fn start_import(&mut self, req: import::Request, dry_run: bool) {
        let tx = self.tx.clone();
        self.outcome = None;
        self.job = Some(Job::start(req, dry_run, move || {
            let _ = tx.send(Msg::Wake);
        }));
        self.dirty = true;
    }

    /// Upload a finished conversion, and say how it went.
    fn check_updates(&mut self, manual: bool) {
        let tx = self.tx.clone();
        update::check(self.app.clone(), manual, move |e| {
            let _ = tx.send(Msg::Update(e));
        });
    }

    /// Install a downloaded update once nothing is talking to the pedal for the user.
    fn install_update(&mut self) {
        if self.job.is_some() || self.restore.is_some() || self.export.is_some() {
            return;
        }
        let Some((u, bytes)) = self.pending_update.take() else { return };
        self.update = UpdateView::Installing { version: u.version.clone() };
        self.emit();
        self.update = update::install(&self.app, &u, &bytes);
        self.dirty = true;
    }

    fn finish_import(&mut self) {
        let Some(result) = self.job.as_ref().and_then(Job::finished) else { return };
        let Some(job) = self.job.take() else { return };
        self.dirty = true;
        let what = if job.kind == Kind::Amp { "the model" } else { "the IR" };
        let target = job.kind.slot_label(job.slot);
        let (text, error) = match result {
            Err(e) => (format!("Couldn't convert {what}: {e}"), true),
            Ok(_) if job.dry_run => (format!("Converted “{}” (debug import: nothing was uploaded).", job.name), false),
            Ok(_) if !self.editable() => ("Converted, but the pedal isn't connected: nothing was uploaded.".into(), true),
            Ok((data, factor)) => {
                let sent = match job.kind {
                    Kind::Amp => self.dev.import_user_amp(job.slot, &job.name, &data, factor),
                    Kind::Ir => self.dev.import_user_ir(job.slot, &job.name, &data),
                };
                match sent {
                    Err(e) => (format!("Upload failed: {e}"), true),
                    Ok(()) => {
                        self.remember(SlotModel {
                            kind: job.kind,
                            slot: job.slot,
                            name: job.name.clone(),
                            source: backup::read_source(&job.path, job.optimised, job.normalise).ok(),
                            converted: Some(ConvertedData::new(&data, factor)),
                        });
                        let secs = job.elapsed_secs();
                        let how = match (job.kind, job.optimised) {
                            (Kind::Amp, true) => format!(" (optimised, {}:{:02})", secs / 60, secs % 60),
                            (Kind::Ir, true) => " (optimised)".to_owned(),
                            (_, false) => " (official algorithm)".to_owned(),
                        };
                        (format!("Imported “{}” into {target}{how}.", job.name), false)
                    }
                }
            }
        };
        // also in the log, in case the dialog was closed meanwhile
        self.note(if error { RowKind::Error } else { RowKind::Info }, text.clone());
        self.outcome = Some(Outcome { text, error });
    }

    /// Keep `m` as the app's copy of its slot (for backups); a failure only goes to the log.
    fn remember(&mut self, m: SlotModel) {
        let label = m.kind.slot_label(m.slot);
        if let Err(e) = self.library.store(m) {
            self.note(RowKind::Error, format!("Couldn't keep a copy of {label} for backups: {e}"));
        }
    }

    fn call(&mut self, c: Call, reply: Sender<Result<Value, String>>) {
        match c {
            Call::Plan { path } => {
                let plan = backup::plan(Path::new(&path), &self.state).and_then(|p| serde_json::to_value(p).map_err(|e| e.to_string()));
                let _ = reply.send(plan);
            }
            Call::Export { preset, path } => self.start_export(preset, path, reply),
        }
    }

    /// Read the models an export needs from the pedal, then write the file ([`Worker::step_export`]).
    /// Without a connection, the models come from the app's copy or go by name only.
    fn start_export(&mut self, preset: Option<u8>, path: String, reply: Sender<Result<Value, String>>) {
        if self.export.is_some() || self.restore.is_some() {
            let _ = reply.send(Err("Wait for the current import or export to finish.".into()));
            return;
        }
        let mut e = Export { preset, path, reply, data: backup::PedalData::new(), factors: Default::default(), done: 0, total: 0, last: Instant::now(), amp: Reassembler::default(), ir: Reassembler::default() };
        if self.editable() {
            for (kind, slot, _) in backup::export_slots(&self.state, preset) {
                let sent = match kind {
                    Kind::Amp => {
                        // the slot-info record carries the factor
                        self.dev.write(vec![req::user_amp(slot)]);
                        e.total += 1;
                        self.dev.read_user_amp(slot)
                    }
                    Kind::Ir => self.dev.read_user_ir(slot),
                };
                if sent.is_ok() {
                    e.total += upload::user_amp_read(slot).len();
                }
            }
        }
        self.export = Some(e);
        self.step_export();
    }

    /// A reply while an export is reading slots.
    fn export_reply(&mut self, bytes: &[u8]) {
        let Some(e) = &mut self.export else { return };
        let Midi::Nep { cmd, dir: 2, payload } = Midi::parse(bytes) else { return };
        let Some(c) = Chunk::parse(&payload) else { return };
        let done = match cmd {
            0x58 if c.sub == amp_sub::READ_INFO => {
                if let Some(info) = UserAmpInfo::parse(&payload) {
                    e.factors.insert(info.slot, info.factor);
                }
                None
            }
            0x58 if c.sub == amp_sub::READ_DATA => e.amp.push(&c).map(|(s, d)| (Kind::Amp, s, d)),
            0x10 => e.ir.push(&c).map(|(s, d)| (Kind::Ir, s, d)),
            _ => return,
        };
        e.done += 1;
        e.last = Instant::now();
        if let Some((kind, slot, floats)) = done {
            let factor = if kind == Kind::Amp { e.factors.get(&slot).copied().unwrap_or(1.0) } else { 1.0 };
            e.data.insert((kind, slot), ConvertedData::new(&floats, factor));
        }
    }

    /// Write the export once every reply is in (or the pedal went quiet), and answer the frontend.
    fn step_export(&mut self) {
        let Some(e) = &self.export else { return };
        let quiet = e.last.elapsed() >= READ_IDLE;
        if e.done < e.total && !quiet {
            return;
        }
        let Some(e) = self.export.take() else { return };
        self.dirty = true;
        let result = self.write_export(&e, quiet && e.done < e.total);
        let _ = e.reply.send(result);
    }

    fn write_export(&mut self, e: &Export, stalled: bool) -> Result<Value, String> {
        if stalled {
            return Err(format!("The pedal stopped answering while its models were being read ({} of {} replies). Nothing was saved.", e.done, e.total));
        }
        let by = format!("Open Stomp Manager {}", crate::version_long());
        let file = backup::export(&self.state, e.preset, &self.library, &e.data, &by, &Local::now().to_rfc3339())?;
        backup::write_file(&file, Path::new(&e.path))?;
        let missing: Vec<String> = file.models.iter().filter(|m| !m.has_data()).map(|m| m.name.clone()).collect();
        let models = file.models.len() - missing.len();
        self.note(RowKind::Info, format!("Exported {} preset(s) and {models} model(s) to {}", file.presets.len(), e.path));
        Ok(serde_json::json!({ "presets": file.presets.len(), "models": models, "missing": missing }))
    }

    /// Check a restore and start it: convert the models that need it, then write.
    fn start_restore(&mut self, req: &backup::RestoreRequest) {
        self.restore_outcome = None;
        if !self.editable() {
            self.restore_outcome = Some(Outcome { text: "Connect to the pedal (with Listen only off) to import.".into(), error: true });
            return;
        }
        let work = match backup::work(req, &self.state) {
            Ok(w) => w,
            Err(text) => {
                self.restore_outcome = Some(Outcome { text, error: true });
                return;
            }
        };
        let (ready, pending): (Vec<_>, Vec<_>) = work.uploads.into_iter().partition(|(_, _, m)| m.converted.is_some());
        let started_ms = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        self.restore = Some(Restore {
            ready,
            to_convert: pending.len(),
            pending: pending.into(),
            converting: None,
            present: work.present,
            presets: work.presets,
            settings: work.settings,
            writing: None,
            started_ms,
        });
        self.dirty = true;
    }

    /// Move a restore along: finish a conversion, start the next, or queue the writes; once the
    /// pedal has acked them all, re-read it.
    fn step_restore(&mut self) {
        let Some(r) = &mut self.restore else { return };
        if let Some(w) = &r.writing {
            if w.acked < w.expected && w.last_ack.elapsed() < ACK_IDLE {
                return;
            }
            let Some(r) = self.restore.take() else { return };
            self.finish_restore(r);
            return;
        }
        if let Some(c) = &r.converting {
            let Some(result) = c.job.finished() else { return };
            let Some(Converting { kind, slot, mut model, .. }) = r.converting.take() else { return };
            match result {
                Ok((data, factor)) => {
                    model.converted = Some(ConvertedData::new(&data, factor));
                    r.ready.push((kind, slot, model));
                }
                Err(e) => {
                    self.restore = None;
                    let text = format!("Couldn't convert “{}”: {e}. Nothing was written to the pedal.", model.name);
                    self.restore_outcome = Some(Outcome { text, error: true });
                    return;
                }
            }
            self.dirty = true;
        }
        if let Some((kind, slot, model)) = r.pending.pop_front() {
            let src = model.source.clone().expect("pending models have a source");
            let path = match backup::source_to_temp(&model) {
                Ok(p) => p,
                Err(e) => {
                    self.restore = None;
                    self.restore_outcome = Some(Outcome { text: format!("Couldn't convert “{}”: {e}", model.name), error: true });
                    return;
                }
            };
            let tx = self.tx.clone();
            let request = import::Request { kind, slot, path: path.display().to_string(), optimised: src.optimised, normalise: src.normalise };
            let job = Job::start(request, false, move || {
                let _ = tx.send(Msg::Wake);
            });
            r.converting = Some(Converting { kind, slot, model, job });
            self.dirty = true;
            return;
        }
        self.write_restore();
    }

    /// Queue everything on the pedal, in order: models, presets, settings.
    fn write_restore(&mut self) {
        if !self.editable() {
            self.restore = None;
            let text = "The pedal disconnected before the import could be written; nothing was changed.".into();
            self.restore_outcome = Some(Outcome { text, error: true });
            return;
        }
        let current = self.state.current;
        let Some(r) = &mut self.restore else { return };
        let mut expected = 0;
        let mut failed = vec![];
        for (kind, slot, m) in &r.ready {
            let floats = m.converted.as_ref().map(ConvertedData::floats);
            let Some(Ok(floats)) = floats else {
                failed.push(m.name.clone());
                continue;
            };
            let factor = m.converted.as_ref().map_or(1.0, |c| c.factor);
            let sent = match kind {
                Kind::Amp => self.dev.import_user_amp(*slot, &m.name, &floats, factor),
                Kind::Ir => self.dev.import_user_ir(*slot, &m.name, &floats),
            };
            match sent {
                Ok(()) => {
                    expected += match kind {
                        Kind::Amp => upload::user_amp_upload(&UserAmpInfo::new(*slot, &m.name, factor), &floats).len(),
                        Kind::Ir => upload::user_ir_upload(*slot, &m.name, &floats).len(),
                    }
                }
                Err(_) => failed.push(m.name.clone()),
            }
        }
        r.ready.retain(|(_, _, m)| !failed.contains(&m.name));
        for p in &r.presets {
            // select the slot first, so the name write goes to it, like saving in place
            let mut msgs = vec![host::program(p.index)];
            msgs.extend(host::save_preset(p.index, p));
            self.dev.write(msgs);
            expected += 2;
        }
        if let Some(s) = &r.settings {
            self.dev.write_settings(s);
            expected += 1;
        }
        if let Some(c) = current {
            self.dev.write(vec![host::program(c)]);
        }
        r.writing = Some(Writing { expected, acked: 0, last_ack: Instant::now(), failed });
        self.dirty = true;
    }

    fn finish_restore(&mut self, r: Restore) {
        let w = r.writing.expect("finished restores were written");
        let uploaded = r.ready.len();
        for (_, _, m) in r.ready.into_iter().chain(r.present.into_iter().filter(|(_, _, m)| m.has_data())) {
            self.remember(m);
        }
        let mut text = format!(
            "Imported {} preset{} and {} model{}{}.",
            r.presets.len(),
            if r.presets.len() == 1 { "" } else { "s" },
            uploaded,
            if uploaded == 1 { "" } else { "s" },
            if r.settings.is_some() { ", and the system settings" } else { "" },
        );
        let mut error = false;
        if !w.failed.is_empty() {
            text.push_str(&format!(" Couldn't upload: {}.", w.failed.join(", ")));
            error = true;
        }
        if w.acked < w.expected {
            text.push_str(&format!(" The pedal confirmed only {} of {} writes; check the presets.", w.acked, w.expected));
            error = true;
        }
        self.note(if error { RowKind::Error } else { RowKind::Info }, text.clone());
        self.restore_outcome = Some(Outcome { text, error });
        // read everything back, so the app shows what the pedal really holds now
        self.dev.resync();
        self.dirty = true;
    }

    fn poll(&mut self) {
        while let Ok(u) = self.dev.try_recv() {
            self.dirty = true;
            match u {
                Update::Ports(p) => self.ports = p,
                Update::Status(s) => self.status = s,
                Update::Sync { done, total } => self.sync = (done, total),
                Update::Reset => {
                    self.held.clear();
                    self.held_since = None;
                    self.state = DeviceState::default();
                }
                Update::Mark { at, text } => self.log.lock().expect("log lock").push_note(local(at), RowKind::Mark, text),
                Update::Info { at, text, error } => {
                    let kind = if error { RowKind::Error } else { RowKind::Info };
                    self.log.lock().expect("log lock").push_note(local(at), kind, text);
                }
                Update::Message { at, origin, bytes } => {
                    match origin {
                        Origin::Pedal => self.received += 1,
                        Origin::Host => self.sent += 1,
                    }
                    if origin == Origin::Pedal
                        && let Some(w) = self.restore.as_mut().and_then(|r| r.writing.as_mut())
                        && matches!(Midi::parse(&bytes), Midi::Nep { cmd, dir: 3, .. } if RESTORE_ACKS.contains(&cmd))
                    {
                        w.acked += 1;
                        w.last_ack = Instant::now();
                    }
                    if origin == Origin::Pedal && self.export.is_some() {
                        self.export_reply(&bytes);
                    }
                    if origin == Origin::Pedal {
                        if bytes.first().is_some_and(|b| b & 0xF0 == 0xC0) && self.held_since.is_none() {
                            self.held_since = Some(Instant::now());
                        }
                        self.pedal_msg_at = Instant::now();
                    }
                    // everything waits while holding, so host edits keep their order
                    if self.held_since.is_some() {
                        self.held.push((at, origin, bytes));
                    } else {
                        self.apply(at, origin, &bytes);
                    }
                }
            }
        }
    }

    fn release_held(&mut self) {
        let Some(since) = self.held_since else { return };
        if self.pedal_msg_at.elapsed() >= SETTLE || since.elapsed() >= MAX_HOLD {
            self.held_since = None;
            for (at, origin, bytes) in std::mem::take(&mut self.held) {
                self.apply(at, origin, &bytes);
            }
            self.dirty = true;
        }
    }

    fn apply(&mut self, at: SystemTime, origin: Origin, bytes: &[u8]) {
        let ev = self.state.apply(bytes, origin);
        if !ev.redundant {
            self.note_activity(&ev, bytes);
        }
        self.log.lock().expect("log lock").push_event(local(at), ev);
    }

    fn note_activity(&mut self, ev: &open_stomp::Event, bytes: &[u8]) {
        let host = ev.origin == Origin::Host;
        let module = |m: Module, slot| Activity::Module { module: m as u8, slot, host };
        match ev.category {
            Category::Effect(m) => {
                self.followed = m;
                let slot = (bytes.len() == 3 && bytes[0] & 0xF0 == 0xB0)
                    .then(|| Module::from_param_cc(bytes[1]))
                    .flatten()
                    .map(|(_, slot)| slot as u8);
                self.activity.push(module(m, slot));
            }
            Category::Looper => self.activity.push(module(Module::Loop, None)),
            Category::Drum => self.activity.push(module(Module::Drum, None)),
            Category::Preset | Category::Volume => self.activity.push(Activity::Preset { host }),
            Category::Navigation => {
                if let Some(p) = self.state.page {
                    self.followed = p;
                    self.activity.push(module(p, None));
                }
            }
            _ => {}
        }
    }

    /// Send the sections that changed since the last event, if any (or only activity).
    fn emit(&mut self) {
        self.dirty = false;
        self.last_emit = Instant::now();
        let sections = view::sections(&self.inputs());
        let mut latest = self.latest.lock().expect("hub snapshot lock");
        let mut changed = Map::new();
        for (name, value) in sections {
            if latest.sections.get(name) != Some(&value) {
                latest.sections.insert(name.to_owned(), value.clone());
                changed.insert(name.to_owned(), value);
            }
        }
        if changed.is_empty() && self.activity.is_empty() && self.received == 0 && self.sent == 0 {
            return;
        }
        latest.seq += 1;
        let patch = Patch {
            seq: latest.seq,
            sections: changed,
            activity: std::mem::take(&mut self.activity),
            rx: std::mem::take(&mut self.received),
            tx: std::mem::take(&mut self.sent),
        };
        drop(latest);
        let _ = self.app.emit("state", patch);
    }
}

/// An export reading its models back from the pedal.
struct Export {
    preset: Option<u8>,
    path: String,
    reply: Sender<Result<Value, String>>,
    /// Slots read so far.
    data: backup::PedalData,
    /// AMP slot factors, from the slot-info records.
    factors: std::collections::HashMap<u8, f32>,
    /// Replies received / expected.
    done: usize,
    total: usize,
    last: Instant,
    amp: Reassembler,
    ir: Reassembler,
}

/// A preset file being put on the pedal.
struct Restore {
    /// Models with converted data, ready to upload: (kind, target slot, model).
    ready: Vec<(Kind, u8, SlotModel)>,
    /// Models to convert from their source file first.
    pending: VecDeque<(Kind, u8, SlotModel)>,
    to_convert: usize,
    converting: Option<Converting>,
    /// Models the pedal already holds where they go.
    present: Vec<(Kind, u8, SlotModel)>,
    presets: Vec<open_stomp::Preset>,
    settings: Option<open_stomp::Settings>,
    /// Set once everything is queued on the pedal.
    writing: Option<Writing>,
    started_ms: u64,
}

struct Converting {
    kind: Kind,
    slot: u8,
    model: SlotModel,
    job: Job,
}

struct Writing {
    /// Acks the queued writes will bring.
    expected: usize,
    acked: usize,
    last_ack: Instant,
    /// Models that couldn't be uploaded.
    failed: Vec<String>,
}

impl Restore {
    fn view(&self) -> RestoreJobView {
        let (stage, text, done, total) = match (&self.writing, &self.converting) {
            (Some(w), _) => ("writing", "Writing to the pedal…".to_owned(), w.acked, w.expected),
            (None, Some(c)) => {
                let v = c.job.view();
                let n = self.to_convert - self.pending.len();
                let text = format!("Converting “{}” ({n} of {}) for {}", c.model.name, self.to_convert, c.kind.slot_label(c.slot));
                ("converting", text, v.done(), v.total())
            }
            (None, None) => ("converting", "Preparing…".to_owned(), 0, 1),
        };
        RestoreJobView { stage, text, done, total: total.max(1), started_ms: self.started_ms }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The commands as the frontend sends them (src/types.ts `Cmd`).
    #[test]
    fn commands_parse_from_the_frontend_json() {
        let parse = |v: Value| serde_json::from_value::<Cmd>(v).unwrap();
        assert!(matches!(
            parse(json!({"type": "setParam", "module": 1, "slot": 0, "value": 64})),
            Cmd::SetParam { module: 1, slot: 0, value: 64 }
        ));
        assert!(matches!(
            parse(json!({"type": "setLooper", "recLevel": 80})),
            Cmd::SetLooper { rec_level: Some(80), play_level: None, sync: None, ar_mode: None }
        ));
        assert!(matches!(
            parse(json!({"type": "looperButton", "button": "undoRedo"})),
            Cmd::LooperButton { button: LoopButton::UndoRedo }
        ));
        assert!(matches!(
            parse(json!({"type": "renameSlot", "kind": "ir", "slot": 4, "name": "V30"})),
            Cmd::RenameSlot { kind: Kind::Ir, slot: 4, .. }
        ));
        let start = parse(json!({"type": "startImport", "request":
            {"kind": "amp", "slot": 5, "path": "/m.nam", "optimised": true, "normalise": false}}));
        assert!(matches!(start, Cmd::StartImport { request } if request.slot == 5 && !request.normalise));
        assert!(matches!(parse(json!({"type": "replay"})), Cmd::Replay { path: None, speed: None }));
        let restore = parse(json!({"type": "startRestore", "request":
            {"path": "/b.ospreset", "presets": [{"entry": 0, "to": 9}], "models": [2, null], "settings": false}}));
        assert!(matches!(restore, Cmd::StartRestore { request } if request.presets[0].to == 9 && request.models == [Some(2), None]));
    }
}
