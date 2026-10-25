//! The hub: a thread that owns the pedal engine and the `DeviceState`, and keeps the frontend in
//! step with them.
//!
//! The engine wakes the hub whenever it has updates. The hub applies them, rebuilds the view
//! sections ([`crate::view`]) and emits the ones that changed as one `state` event, at most once
//! per frame, so a knob sweep doesn't flood the webview. Commands from the frontend arrive on the
//! same channel as the wake-ups, so they run in order with the updates.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Local};
use open_stomp::device::{Config, Device, Status, Update};
use open_stomp::midi::host;
use open_stomp::{Category, DeviceState, Module, Origin};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tauri::{AppHandle, Emitter};

use crate::import::{self, Job, Kind};
use crate::log::{FilterArgs, Log, RowKind};
use crate::view::{self, ImportView, Inputs, Outcome};

/// After a preset change from the pedal, its messages are held until it has been quiet this long,
/// then applied together, so turning the preset knob several clicks jumps instead of stepping.
const SETTLE: Duration = Duration::from_millis(150);
/// ...but never held longer than this.
const MAX_HOLD: Duration = Duration::from_secs(5);
/// Shortest gap between two `state` events (one frame at 60 Hz).
const FRAME: Duration = Duration::from_millis(16);

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
    SetLogFilter { filter: FilterArgs },
    ClearLog,
    Connect { port: String },
    Disconnect,
    Resync,
    SetListenOnly { on: bool },
    /// Replay a capture log; without a path, the default one (`replayPath` in `conn`).
    Replay { path: Option<String>, speed: Option<f32> },
    StopReplay,
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
        thread::Builder::new()
            .name("open-stomp-hub".into())
            .spawn(move || worker.run(rx))
            .expect("spawn hub thread");
        Hub { tx, latest, log }
    }

    pub fn command(&self, c: Cmd) {
        let _ = self.tx.send(Msg::Cmd(c));
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
                Ok(Msg::Wake) => self.dirty |= self.job.is_some(),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            self.poll();
            self.release_held();
            self.finish_import();
            if self.dirty && self.last_emit.elapsed() >= FRAME {
                self.emit();
            }
        }
    }

    /// When the loop must wake up by itself: to release held messages or send a pending event.
    fn next_deadline(&self) -> Option<Instant> {
        let release = self.held_since.map(|since| (self.pedal_msg_at + SETTLE).min(since + MAX_HOLD));
        let emit = self.dirty.then(|| self.last_emit + FRAME);
        release.into_iter().chain(emit).min()
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
                if let Err(e) = sent {
                    self.note(RowKind::Error, format!("Rename failed: {e}"));
                }
            }
            Cmd::StartImport { request } => {
                if self.job.is_none() {
                    self.start_import(request, false);
                }
            }
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
            // edits while disconnected or listen-only: the UI disables them, so just drop them
            Cmd::SetVolume { .. }
            | Cmd::LooperButton { .. }
            | Cmd::SetLooper { .. }
            | Cmd::SetDrum { .. }
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
    }
}
