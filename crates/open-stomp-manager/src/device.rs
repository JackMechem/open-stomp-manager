//! Background engine: owns the MIDI connection (or a capture replay), performs the sync
//! handshake, sends edits and forwards every message to the UI.
//!
//! The pedal handles one SysEx request at a time, so requests and writes are queued and each
//! waits for its reply or ack (1 s timeout) before the next is sent, like the official app
//! (docs/03). Live CCs/PCs bypass the queue.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, TimeZone};
use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use stomp_proto::capture::{self, Entry};
use stomp_proto::midi::{Msg, req};
use stomp_proto::{Assembler, Origin};

const REPLY_TIMEOUT: Duration = Duration::from_secs(1);
const SCAN_INTERVAL: Duration = Duration::from_millis(1500);
const CLIENT_NAME: &str = "Open Stomp Manager";

/// Port names the official app matches (`.*JME.*`, `.*SORAN AMPEY.*`), plus the product name.
pub fn is_stomp_port(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    n.contains("JME") || n.contains("SORAN") || n.contains("STOMPBOX")
}

pub enum Command {
    Connect(String),
    Disconnect,
    Resync,
    /// Whether the app may send anything (sync reads and edits). Off = pure listening.
    SetSync(bool),
    /// Send a live message (CC/PC) right away.
    Send(Vec<u8>),
    /// Queue messages to send in order; SysEx writes wait for the pedal's ack.
    Write(Vec<Vec<u8>>),
    Replay { path: PathBuf, speed: f32 },
    StopReplay,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Disconnected,
    Connected { port: String },
    Replaying { file: String, progress: f32 },
}

pub enum Update {
    Ports(Vec<String>),
    Status(Status),
    Sync { done: usize, total: usize },
    Message { at: DateTime<Local>, origin: Origin, bytes: Vec<u8> },
    Mark { at: DateTime<Local>, text: String },
    Info { at: DateTime<Local>, text: String, error: bool },
    /// A new session starts (connect or replay): the UI should reset its device model.
    Reset,
}

enum Internal {
    Cmd(Command),
    Input(Vec<u8>),
}

pub struct Handle {
    tx: Sender<Internal>,
    pub rx: Receiver<Update>,
}

impl Handle {
    pub fn send(&self, c: Command) {
        let _ = self.tx.send(Internal::Cmd(c));
    }
}

pub fn spawn(ctx: egui::Context) -> Handle {
    let (tx, rx_internal) = mpsc::channel();
    let (tx_ui, rx) = mpsc::channel();
    let engine_tx = tx.clone();
    thread::Builder::new()
        .name("stomp-engine".into())
        .spawn(move || Engine::new(ctx, tx_ui, engine_tx).run(rx_internal))
        .expect("spawn engine thread");
    Handle { tx, rx }
}

struct Request {
    bytes: Vec<u8>,
    /// `None`: nothing comes back (CC/PC), done once sent.
    expect: Option<Expect>,
    /// Part of the startup sync (counts towards its progress).
    sync: bool,
}

#[derive(Clone, Copy)]
enum Expect {
    /// Short frame reply of this kind with a payload.
    Short(u8),
    /// NEP reply (`dir` 2) to this cmd.
    Reply(u8),
    /// NEP write ack (`dir` 3) for this cmd.
    Ack(u8),
}

impl Expect {
    fn matches(self, msg: &Msg) -> bool {
        match (self, msg) {
            (Expect::Short(k), Msg::Short { kind, payload }) => *kind == k && !payload.is_empty(),
            (Expect::Reply(c), Msg::Nep { cmd, dir, .. }) => *cmd == c && *dir == 2,
            (Expect::Ack(c), Msg::Nep { cmd, dir, .. }) => *cmd == c && *dir == 3,
            _ => false,
        }
    }

    fn of(bytes: &[u8]) -> Option<Expect> {
        Some(match Msg::parse(bytes) {
            Msg::Short { kind: 0x00, .. } => Expect::Short(0x10),
            Msg::Short { kind, .. } => Expect::Short(kind),
            Msg::Nep { cmd, dir: 1, .. } => Expect::Ack(cmd),
            Msg::Nep { cmd, .. } => Expect::Reply(cmd),
            _ => return None,
        })
    }
}

struct Connection {
    port: String,
    _input: MidiInputConnection<()>,
    output: Option<MidiOutputConnection>,
}

struct Replay {
    stop: Arc<AtomicBool>,
}

struct Engine {
    ctx: egui::Context,
    ui: Sender<Update>,
    self_tx: Sender<Internal>,
    scanner: Option<MidiInput>,
    ports: Vec<String>,
    last_scan: Option<Instant>,
    conn: Option<Connection>,
    replay: Option<Replay>,
    auto_connect: bool,
    sync: bool,
    asm: Assembler,
    queue: VecDeque<Request>,
    in_flight: Option<(Request, Instant)>,
    sync_total: usize,
    sync_done: usize,
}

impl Engine {
    fn new(ctx: egui::Context, ui: Sender<Update>, self_tx: Sender<Internal>) -> Self {
        let scanner = MidiInput::new(&format!("{CLIENT_NAME} scan")).ok();
        Engine {
            ctx,
            ui,
            self_tx,
            scanner,
            ports: vec![],
            last_scan: None,
            conn: None,
            replay: None,
            auto_connect: true,
            sync: true,
            asm: Assembler::default(),
            queue: VecDeque::new(),
            in_flight: None,
            sync_total: 0,
            sync_done: 0,
        }
    }

    fn emit(&self, u: Update) {
        let _ = self.ui.send(u);
        self.ctx.request_repaint();
    }

    fn info(&self, text: impl Into<String>, error: bool) {
        self.emit(Update::Info { at: Local::now(), text: text.into(), error });
    }

    fn run(mut self, rx: Receiver<Internal>) {
        if self.scanner.is_none() {
            self.info("MIDI is unavailable on this system (could not create a MIDI client)", true);
        }
        loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Internal::Cmd(c)) => self.command(c),
                Ok(Internal::Input(data)) => self.input(&data),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            self.pump();
            #[cfg(target_os = "macos")]
            run_loop_tick();
            if self.last_scan.is_none_or(|t| t.elapsed() > SCAN_INTERVAL) {
                self.scan();
            }
        }
    }

    fn command(&mut self, c: Command) {
        match c {
            Command::Connect(port) => {
                self.stop_replay();
                self.auto_connect = true;
                self.connect(&port);
            }
            Command::Disconnect => {
                self.auto_connect = false;
                self.disconnect("Disconnected");
            }
            Command::Resync => {
                if self.conn.is_some() {
                    self.emit(Update::Reset);
                    self.start_sync();
                }
            }
            Command::SetSync(on) => {
                self.sync = on;
                if !on {
                    self.queue.clear();
                    self.in_flight = None;
                    self.emit(Update::Sync { done: 0, total: 0 });
                }
            }
            Command::Send(bytes) => {
                if self.writable() {
                    self.transmit(&bytes);
                }
            }
            Command::Write(msgs) => {
                if self.writable() {
                    for bytes in msgs {
                        self.enqueue_write(bytes);
                    }
                }
            }
            Command::Replay { path, speed } => {
                self.auto_connect = false;
                self.disconnect("Disconnected for replay");
                self.start_replay(path, speed);
            }
            Command::StopReplay => {
                self.stop_replay();
                self.emit(Update::Status(Status::Disconnected));
            }
        }
    }

    fn scan(&mut self) {
        self.last_scan = Some(Instant::now());
        let Some(scanner) = &self.scanner else { return };
        let ports: Vec<String> = scanner.ports().iter().filter_map(|p| scanner.port_name(p).ok()).collect();
        if ports != self.ports {
            self.ports = ports.clone();
            self.emit(Update::Ports(ports.clone()));
        }
        if let Some(c) = &self.conn
            && !ports.contains(&c.port)
        {
            self.disconnect("Pedal unplugged");
        }
        if self.conn.is_none()
            && self.replay.is_none()
            && self.auto_connect
            && let Some(p) = ports.iter().find(|p| is_stomp_port(p))
        {
            self.connect(&p.clone());
        }
    }

    fn connect(&mut self, port_name: &str) {
        self.disconnect_quiet();
        let result = (|| -> Result<Connection, String> {
            let mut input = MidiInput::new(CLIENT_NAME).map_err(|e| e.to_string())?;
            input.ignore(Ignore::None);
            let port = input
                .ports()
                .into_iter()
                .find(|p| input.port_name(p).as_deref() == Ok(port_name))
                .ok_or_else(|| format!("port “{port_name}” not found"))?;
            let tx = self.self_tx.clone();
            let ctx = self.ctx.clone();
            let input = input
                .connect(
                    &port,
                    "stomp-in",
                    move |_ts, data, _| {
                        let _ = tx.send(Internal::Input(data.to_vec()));
                        ctx.request_repaint();
                    },
                    (),
                )
                .map_err(|e| e.to_string())?;
            let output = open_output(port_name);
            Ok(Connection { port: port_name.to_owned(), _input: input, output })
        })();
        match result {
            Ok(c) => {
                let has_out = c.output.is_some();
                let port = c.port.clone();
                self.conn = Some(c);
                self.emit(Update::Reset);
                self.emit(Update::Status(Status::Connected { port: port.clone() }));
                self.info(format!("Connected to {port}"), false);
                if !has_out {
                    self.info("No MIDI output to the pedal: listening only", true);
                }
                self.start_sync();
            }
            Err(e) => self.info(format!("Could not connect: {e}"), true),
        }
    }

    fn disconnect_quiet(&mut self) {
        self.conn = None;
        self.queue.clear();
        self.in_flight = None;
        self.asm = Assembler::default();
    }

    fn disconnect(&mut self, why: &str) {
        if self.conn.is_some() {
            self.disconnect_quiet();
            self.emit(Update::Status(Status::Disconnected));
            self.emit(Update::Sync { done: 0, total: 0 });
            self.info(why, false);
        }
    }

    /// The official app's startup reads (docs/03), all read-only. Presets come before the edit
    /// buffer so the active preset can be identified by matching the edit buffer against them.
    fn start_sync(&mut self) {
        self.queue.clear();
        self.in_flight = None;
        if !self.sync || self.conn.as_ref().is_none_or(|c| c.output.is_none()) {
            return;
        }
        let mut reqs = vec![req::version(), req::product(), req::settings(), req::cur_state(), req::drum()];
        reqs.extend((0..10).map(req::ir_slot));
        reqs.extend((0..10).map(req::user_amp));
        reqs.extend((0..99).map(req::preset));
        reqs.extend([req::edit_buffer(), req::looper(), req::tuner()]);
        self.sync_total = reqs.len();
        self.sync_done = 0;
        for r in reqs {
            self.enqueue(r);
        }
        self.emit(Update::Sync { done: 0, total: self.sync_total });
    }

    fn enqueue(&mut self, bytes: Vec<u8>) {
        if self.sync && !self.queue.iter().any(|r| r.bytes == bytes) {
            let expect = Expect::of(&bytes);
            self.queue.push_back(Request { bytes, expect, sync: self.sync_total > 0 });
        }
    }

    /// Edits are allowed when connected with an output and not in listen-only mode.
    fn writable(&self) -> bool {
        let ok = self.sync && self.conn.as_ref().is_some_and(|c| c.output.is_some());
        if !ok {
            self.info("Not sent: the app is in listen-only mode or has no MIDI output to the pedal", true);
        }
        ok
    }

    fn enqueue_write(&mut self, bytes: Vec<u8>) {
        let expect = Expect::of(&bytes);
        // settings are rewritten whole on every change: only the latest queued one matters
        if let Some(Expect::Ack(0x14)) = expect
            && let Some(r) = self.queue.iter_mut().find(|r| matches!(r.expect, Some(Expect::Ack(0x14))))
        {
            r.bytes = bytes;
            return;
        }
        self.queue.push_back(Request { bytes, expect, sync: false });
    }

    /// Send one message now and show it in the log. The pedal never echoes host messages.
    fn transmit(&mut self, bytes: &[u8]) -> bool {
        let Some(out) = self.conn.as_mut().and_then(|c| c.output.as_mut()) else { return false };
        if let Err(e) = out.send(bytes) {
            self.info(format!("Send failed: {e}"), true);
            return false;
        }
        self.emit(Update::Message { at: Local::now(), origin: Origin::Host, bytes: bytes.to_vec() });
        true
    }

    /// Send the next queued request when nothing is in flight; expire timed-out requests.
    fn pump(&mut self) {
        if let Some((r, t)) = &self.in_flight
            && t.elapsed() > REPLY_TIMEOUT
        {
            let what = stomp_proto::midi::hex(&r.bytes[..r.bytes.len().min(8)]);
            let sync = r.sync;
            self.in_flight = None;
            self.info(format!("No reply from pedal to {what} …"), true);
            if sync {
                self.progress();
            }
        }
        while self.in_flight.is_none() {
            let Some(r) = self.queue.pop_front() else { return };
            if self.conn.as_ref().is_none_or(|c| c.output.is_none()) {
                self.queue.clear();
                return;
            }
            if !self.transmit(&r.bytes) {
                return;
            }
            if r.expect.is_some() {
                self.in_flight = Some((r, Instant::now()));
            }
        }
    }

    fn progress(&mut self) {
        if self.sync_total > 0 {
            self.sync_done += 1;
            if self.sync_done >= self.sync_total {
                self.sync_total = 0;
                self.sync_done = 0;
            }
            self.emit(Update::Sync { done: self.sync_done, total: self.sync_total });
        }
    }

    fn input(&mut self, data: &[u8]) {
        if self.conn.is_none() {
            return;
        }
        let at = Local::now();
        for bytes in self.asm.push(data) {
            let msg = Msg::parse(&bytes);
            if let Some((r, _)) = &self.in_flight
                && r.expect.is_some_and(|e| e.matches(&msg))
            {
                let sync = r.sync;
                self.in_flight = None;
                if sync {
                    self.progress();
                }
            }
            self.react(&msg);
            self.emit(Update::Message { at, origin: Origin::Pedal, bytes });
        }
    }

    /// Follow-up reads after pedal-side changes that carry no data themselves.
    fn react(&mut self, msg: &Msg) {
        match msg {
            Msg::Notify { what: 0x0B, payload } => {
                let i = payload.first().copied().unwrap_or(0);
                self.enqueue(req::preset(i));
                self.enqueue(req::edit_buffer());
            }
            Msg::Notify { what: 0x6F, .. } => self.enqueue(req::tuner()),
            _ => {}
        }
    }

    fn start_replay(&mut self, path: PathBuf, speed: f32) {
        self.stop_replay();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                self.info(format!("Could not read {}: {e}", path.display()), true);
                return;
            }
        };
        let entries = capture::parse(&text);
        let file = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let stop = Arc::new(AtomicBool::new(false));
        self.replay = Some(Replay { stop: stop.clone() });
        self.emit(Update::Reset);
        self.emit(Update::Status(Status::Replaying { file: file.clone(), progress: 0.0 }));
        self.info(format!("Replaying {file} ({} messages) at {speed}×", entries.len()), false);
        let (ui, ctx) = (self.ui.clone(), self.ctx.clone());
        thread::spawn(move || replay(entries, file, speed, stop, ui, ctx));
    }

    fn stop_replay(&mut self) {
        if let Some(r) = self.replay.take() {
            r.stop.store(true, Ordering::Relaxed);
        }
    }
}

/// CoreMIDI updates a client's view of the devices (hotplug) through notifications delivered on
/// the run loop of the thread that created the client. The engine thread has no run loop of its
/// own, so give it one turn per tick without blocking.
#[cfg(target_os = "macos")]
fn run_loop_tick() {
    use core_foundation_sys::runloop::{CFRunLoopRunInMode, kCFRunLoopDefaultMode};
    // SAFETY: plain CoreFoundation call on the current thread; returns immediately (0 s timeout).
    unsafe {
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.0, 1);
    }
}

fn open_output(port_name: &str) -> Option<MidiOutputConnection> {
    let out = MidiOutput::new(CLIENT_NAME).ok()?;
    let ports = out.ports();
    let names: Vec<String> = ports.iter().map(|p| out.port_name(p).unwrap_or_default()).collect();
    // exact name first (ALSA, CoreMIDI), else any pedal-looking port (Windows names differ)
    let i = names.iter().position(|n| n == port_name).or_else(|| names.iter().position(|n| is_stomp_port(n)))?;
    out.connect(&ports[i], "stomp-out").ok()
}

/// Play a capture log back with its original timing (gaps capped at 1.5 s), scaled by `speed`.
fn replay(
    entries: Vec<Entry>,
    file: String,
    speed: f32,
    stop: Arc<AtomicBool>,
    ui: Sender<Update>,
    ctx: egui::Context,
) {
    let total = entries.len().max(1);
    let mut prev_ms: Option<u64> = None;
    for (i, e) in entries.into_iter().enumerate() {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let ms = match &e {
            Entry::Mark { ms, .. } | Entry::Message { ms, .. } => *ms,
        };
        if let Some(p) = prev_ms {
            let gap = ms.saturating_sub(p).min(1500) as f32 / speed.max(0.01);
            if gap >= 1.0 {
                thread::sleep(Duration::from_millis(gap as u64));
            }
        }
        prev_ms = Some(ms);
        let at = Local.timestamp_millis_opt(ms as i64).single().unwrap_or_else(Local::now);
        let u = match e {
            Entry::Mark { text, .. } => Update::Mark { at, text },
            Entry::Message { origin, bytes, .. } => Update::Message { at, origin, bytes },
        };
        if ui.send(u).is_err() {
            return;
        }
        if i % 25 == 0 {
            let progress = i as f32 / total as f32;
            let _ = ui.send(Update::Status(Status::Replaying { file: file.clone(), progress }));
        }
        ctx.request_repaint();
    }
    let _ = ui.send(Update::Status(Status::Replaying { file, progress: 1.0 }));
    let _ = ui.send(Update::Info { at: Local::now(), text: "Replay finished".into(), error: false });
    ctx.request_repaint();
}
