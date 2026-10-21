//! Device state tracker: applies MIDI traffic (either direction) to a model of the pedal and
//! describes each message in plain English.

use crate::catalog::{Module, NOTES, drum_pattern_name};
use crate::midi::{Cmd, Dir, HOST_CH, Msg, hex};
use crate::records::{PRESET_COUNT, Preset, Settings, parse_ir_slot, parse_user_amp, u14};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Origin {
    /// Pedal → host.
    Pedal,
    /// Host → pedal (this app, or the official app in a replayed capture).
    Host,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Preset,
    Effect(Module),
    Volume,
    Navigation,
    Looper,
    Drum,
    Tuner,
    Settings,
    Device,
    /// Request/reply plumbing (handshake reads, acks, upload chunks).
    Protocol,
    Unknown,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::Preset => "PRESET",
            Category::Effect(m) => m.name(),
            Category::Volume => "VOLUME",
            Category::Navigation => "PAGE",
            Category::Looper => "LOOPER",
            Category::Drum => "DRUM",
            Category::Tuner => "TUNER",
            Category::Settings => "SETTINGS",
            Category::Device => "DEVICE",
            Category::Protocol => "SYSEX",
            Category::Unknown => "???",
        }
    }
}

/// A human-readable description of one MIDI message.
#[derive(Clone, Debug)]
pub struct Event {
    pub origin: Origin,
    pub category: Category,
    /// What changed, e.g. `"Mar J800 · GAIN"`.
    pub subject: String,
    /// The new value, e.g. `"64"`.
    pub value: String,
    /// Consecutive events with the same key may be merged into one log row (knob sweeps).
    pub coalesce: Option<u64>,
    /// The message didn't change the tracked state (e.g. the CC burst after a preset change
    /// repeating values already known from the preset record, or values for unused param slots).
    pub redundant: bool,
    pub raw: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct Looper {
    /// 0 empty, 1 recording, 2 playing, 3 overdubbing, 4 stopped.
    pub state: u8,
    /// 0x1 = overdub layer on top (undo available), 0x2 = base layer only.
    pub layers: u8,
    pub rec_level: Option<u8>,
    pub play_level: Option<u8>,
    pub sync: Option<bool>,
    /// 0 NR, 1 AR.
    pub mode: Option<u8>,
}

impl Looper {
    pub fn set(&mut self, v: u8) {
        self.state = v & 0x0F;
        self.layers = v >> 4;
    }

    pub fn state_name(&self) -> &'static str {
        looper_state_name(self.state)
    }
}

pub fn looper_state_name(s: u8) -> &'static str {
    match s {
        0 => "Empty",
        1 => "Recording",
        2 => "Playing",
        3 => "Overdubbing",
        4 => "Stopped",
        _ => "Unknown",
    }
}

#[derive(Clone, Debug, Default)]
pub struct Drum {
    pub playing: bool,
    pub pattern: Option<u8>,
    pub level: Option<u8>,
    pub bpm: Option<u16>,
}

#[derive(Clone, Debug, Default)]
pub struct Tuner {
    pub active: bool,
    pub detected: bool,
    pub note: Option<u8>,
    /// Raw, 50 = in tune.
    pub cents: Option<u8>,
}

impl Tuner {
    pub fn cents_offset(&self) -> Option<i16> {
        self.cents.map(|c| c as i16 - 50)
    }
}

#[derive(Clone, Debug)]
pub struct DeviceState {
    pub firmware: Option<String>,
    pub model_id: Option<String>,
    pub product: Option<String>,
    pub presets: Vec<Option<Preset>>,
    /// Active preset index (0-based).
    pub current: Option<u8>,
    /// The live edit buffer: stored preset + every change since.
    pub live: Preset,
    pub live_known: bool,
    pub page: Option<Module>,
    pub looper: Looper,
    pub drum: Drum,
    pub tuner: Tuner,
    pub settings: Option<Settings>,
    pub user_amps: [Option<String>; 10],
    pub user_irs: [Option<String>; 10],
}

impl Default for DeviceState {
    fn default() -> Self {
        DeviceState {
            firmware: None,
            model_id: None,
            product: None,
            presets: vec![None; PRESET_COUNT],
            current: None,
            live: Preset::default(),
            live_known: false,
            page: None,
            looper: Looper::default(),
            drum: Drum::default(),
            tuner: Tuner::default(),
            settings: None,
            user_amps: Default::default(),
            user_irs: Default::default(),
        }
    }
}

fn key(parts: &[u64]) -> Option<u64> {
    Some(parts.iter().fold(0xcbf29ce484222325u64, |h, p| (h ^ p).wrapping_mul(0x100000001b3)))
}

pub fn preset_label(i: u8) -> String {
    format!("P{:02}", i as u16 + 1)
}

impl DeviceState {
    pub fn stored(&self, i: u8) -> Option<&Preset> {
        self.presets.get(i as usize)?.as_ref()
    }

    pub fn preset_name(&self, i: u8) -> Option<&str> {
        self.stored(i).map(|p| p.name.as_str())
    }

    /// The live edit buffer differs from the stored preset.
    pub fn modified(&self) -> bool {
        match (self.current, self.live_known) {
            (Some(i), true) => self.stored(i).is_some_and(|p| !p.same_sound(&self.live)),
            _ => false,
        }
    }

    /// Model name for display, using user slot names where known.
    pub fn model_name(&self, m: Module, model: u8) -> String {
        let user = match m {
            Module::Amp if model >= 30 => self.user_amps.get(model as usize - 30),
            Module::Cab if model >= 30 => self.user_irs.get(model as usize - 30),
            _ => None,
        };
        match user.and_then(|n| n.as_deref()) {
            Some(n) if !n.is_empty() && !n.starts_with("--") && n != m.model_name(model) => {
                format!("{} · {n}", m.model_name(model))
            }
            _ => m.model_name(model),
        }
    }

    /// Apply one complete MIDI message and describe it.
    pub fn apply(&mut self, bytes: &[u8], origin: Origin) -> Event {
        let mut ev = Event {
            origin,
            category: Category::Unknown,
            subject: String::new(),
            value: String::new(),
            coalesce: None,
            redundant: false,
            raw: bytes.to_vec(),
        };
        match Msg::parse(bytes) {
            Msg::Cc { ch, cc, val } => self.apply_cc(&mut ev, ch, cc, val),
            Msg::Pc { prog, .. } => {
                self.select_preset(prog);
                ev.category = Category::Preset;
                ev.subject = "Preset selected".into();
                ev.value = match self.preset_name(prog) {
                    Some(n) => format!("{} · {n}", preset_label(prog)),
                    None => preset_label(prog),
                };
            }
            Msg::Short { kind, payload } => self.apply_short(&mut ev, kind, &payload),
            Msg::Notify { what, payload } => self.apply_notify(&mut ev, what, &payload),
            Msg::Nep { cmd, dir, payload } => self.apply_nep(&mut ev, cmd, dir, &payload),
            Msg::Other(b) => {
                ev.subject = "Unrecognised message".into();
                ev.value = hex(&b);
            }
        }
        ev
    }

    pub fn select_preset(&mut self, prog: u8) {
        self.current = Some(prog);
        if let Some(p) = self.stored(prog) {
            self.live = p.clone();
            self.live_known = true;
        }
    }

    fn apply_cc(&mut self, ev: &mut Event, ch: u8, cc: u8, val: u8) {
        let o = ev.origin as u64;
        match cc {
            0x00..=0x07 => {
                let m = Module::from_index(cc).unwrap();
                let st = &mut self.live.modules[m.index()];
                let (was_on, was_model) = (st.on, st.model);
                ev.redundant = st.head() == val && self.live_known;
                st.set_head(val);
                let (on, model) = (st.on, st.model);
                ev.category = Category::Effect(m);
                let name = self.model_name(m, model);
                if model != was_model {
                    ev.subject = "Model".into();
                    ev.value = format!("#{:02} {name}", model + 1);
                    ev.coalesce = key(&[o, 1, cc as u64]);
                } else {
                    ev.subject = name;
                    ev.value = if on { "ON" } else { "OFF" }.into();
                }
                if on != was_on && model != was_model {
                    ev.value.push_str(if on { " · ON" } else { " · OFF" });
                }
            }
            0x08..=0x39 if Module::from_param_cc(cc).is_some() => {
                let (m, slot) = Module::from_param_cc(cc).unwrap();
                let st = &mut self.live.modules[m.index()];
                let model = st.model;
                ev.redundant = (st.params[slot] == val && self.live_known) || m.param(model, slot).is_none();
                st.params[slot] = val;
                ev.category = Category::Effect(m);
                let (pname, pval) = match m.param(model, slot) {
                    Some(p) => (p.name.to_owned(), p.format(val)),
                    None => (format!("P{}", slot + 1), val.to_string()),
                };
                ev.subject = format!("{} · {pname}", m.model_name(model));
                ev.value = pval;
                ev.coalesce = key(&[o, 2, cc as u64, model as u64]);
            }
            0x3A => {
                ev.redundant = self.live.volume == val && self.live_known;
                self.live.volume = val;
                ev.category = Category::Volume;
                ev.subject = "Preset volume".into();
                ev.value = val.to_string();
                ev.coalesce = key(&[o, 3]);
            }
            0x3B => {
                self.page = Module::from_index(val);
                ev.category = Category::Navigation;
                ev.subject = "Pedal page".into();
                ev.value = self.page.map(|m| m.name().to_owned()).unwrap_or_else(|| val.to_string());
            }
            0x3F => {
                self.drum.playing = val != 0;
                ev.category = Category::Drum;
                ev.subject = "Drum".into();
                ev.value = if val != 0 { "PLAYING" } else { "STOPPED" }.into();
            }
            0x40 => {
                self.drum.pattern = Some(val);
                ev.category = Category::Drum;
                ev.subject = "Pattern".into();
                ev.value = drum_pattern_name(val);
                ev.coalesce = key(&[o, 4]);
            }
            0x41 => {
                self.drum.level = Some(val);
                ev.category = Category::Drum;
                ev.subject = "Drum level".into();
                ev.value = val.to_string();
                ev.coalesce = key(&[o, 5]);
            }
            0x42 | 0x43 => {
                let (slot, name) = if cc == 0x42 {
                    (&mut self.looper.rec_level, "Record level")
                } else {
                    (&mut self.looper.play_level, "Playback level")
                };
                *slot = Some(val);
                ev.category = Category::Looper;
                ev.subject = name.into();
                ev.value = val.to_string();
                ev.coalesce = key(&[o, 6, cc as u64]);
            }
            0x44 if ch == HOST_CH => {
                ev.category = Category::Looper;
                ev.subject = "Looper button".into();
                ev.value = match val {
                    1 => "REC / PLAY / DUB",
                    2 => "STOP",
                    4 => "CLEAR",
                    8 => "UNDO / REDO",
                    _ => "?",
                }
                .into();
            }
            0x44 => {
                self.looper.set(val);
                ev.category = Category::Looper;
                ev.subject = "Looper".into();
                ev.value = self.looper.state_name().to_uppercase();
                match self.looper.layers {
                    1 => ev.value.push_str(" · overdub layer (undo available)"),
                    2 => ev.value.push_str(" · base layer"),
                    _ => {}
                }
            }
            0x45 | 0x46 | 0x48 => {
                match cc {
                    0x45 => self.tuner.detected = val != 0,
                    0x46 => self.tuner.note = Some(val % 12),
                    _ => self.tuner.cents = Some(val),
                }
                ev.category = Category::Tuner;
                ev.subject = "Pitch".into();
                ev.value = match (self.tuner.detected, self.tuner.note, self.tuner.cents_offset()) {
                    (false, _, _) => "—".into(),
                    (true, Some(n), Some(c)) => format!("{} {c:+} ¢", NOTES[n as usize]),
                    (true, Some(n), None) => NOTES[n as usize].into(),
                    _ => "detecting…".into(),
                };
                ev.coalesce = key(&[o, 7]);
            }
            0x49 => {
                self.looper.sync = Some(val != 0);
                ev.category = Category::Looper;
                ev.subject = "Loop sync".into();
                ev.value = if val != 0 { "ON" } else { "OFF" }.into();
            }
            0x4A => {
                self.looper.mode = Some(val);
                ev.category = Category::Looper;
                ev.subject = "Loop mode".into();
                ev.value = if val == 0 { "NR" } else { "AR" }.into();
            }
            _ => {
                ev.subject = format!("CC 0x{cc:02X} (ch {})", ch + 1);
                ev.value = val.to_string();
                ev.coalesce = key(&[o, 8, cc as u64]);
            }
        }
    }

    fn apply_short(&mut self, ev: &mut Event, kind: u8, payload: &[u8]) {
        ev.category = Category::Device;
        let strings: Vec<String> = payload
            .split(|b| *b == 0)
            .filter(|s| !s.is_empty() && s.iter().all(|c| (0x20..0x7F).contains(c)))
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect();
        match kind {
            0x00 => {
                ev.category = Category::Protocol;
                ev.subject = "Version request".into();
            }
            0x10 if strings.is_empty() => {
                ev.subject = "Identity ping".into();
            }
            0x10 => {
                self.firmware = strings.first().cloned();
                self.model_id = strings.get(1).cloned();
                ev.subject = "Firmware".into();
                ev.value = strings.join(" · ");
            }
            0x60 if payload.is_empty() => {
                ev.category = Category::Protocol;
                ev.subject = "Product name request".into();
            }
            0x60 => {
                self.product = strings.first().cloned();
                ev.subject = "Product".into();
                ev.value = strings.first().cloned().unwrap_or_default();
            }
            _ => {
                ev.category = Category::Unknown;
                ev.subject = format!("Short SysEx 0x{kind:02X}");
                ev.value = hex(payload);
            }
        }
    }

    fn apply_notify(&mut self, ev: &mut Event, what: u8, payload: &[u8]) {
        match what {
            0x19 => {
                if payload.len() >= 6 {
                    self.drum.pattern = Some(payload[1]);
                    self.drum.bpm = Some(u14(payload[4], payload[5]));
                }
                ev.category = Category::Drum;
                ev.subject = "Tempo".into();
                ev.value = self.drum.bpm.map(|b| format!("{b} BPM")).unwrap_or_default();
                ev.coalesce = key(&[ev.origin as u64, 9]);
            }
            0x0B => {
                let i = payload.first().copied().unwrap_or(0);
                ev.category = Category::Preset;
                ev.subject = "Saved on pedal".into();
                ev.value = preset_label(i);
            }
            0x6F => {
                ev.category = Category::Tuner;
                ev.subject = "Tuner".into();
                ev.value = "toggled on pedal".into();
            }
            _ => {
                ev.category = Category::Unknown;
                ev.subject = format!("Notify 0x{what:02X} ({})", Cmd::label(what));
                ev.value = hex(payload);
            }
        }
    }

    fn apply_nep(&mut self, ev: &mut Event, cmd: u8, dir: u8, payload: &[u8]) {
        ev.category = Category::Protocol;
        ev.subject = format!("{} {}", Cmd::label(cmd), Dir::label(dir));
        let data = dir == Dir::Reply as u8 || dir == Dir::Write as u8;
        let host_write = dir == Dir::Write as u8;
        match Cmd::from_u8(cmd) {
            Some(Cmd::Preset) if data => {
                if let Some(p) = Preset::from_packed(payload) {
                    let i = p.index;
                    ev.value = format!("{} · {}", preset_label(i), p.name);
                    if host_write {
                        ev.category = Category::Preset;
                        ev.subject = "Preset written".into();
                    }
                    let is_current = self.current == Some(i);
                    if let Some(slot) = self.presets.get_mut(i as usize) {
                        *slot = Some(p.clone());
                    }
                    if is_current && (!self.live_known || host_write) {
                        self.live = p;
                        self.live_known = true;
                    }
                }
            }
            Some(Cmd::EditBuffer) if data => {
                if let Some(mut p) = Preset::from_packed(payload) {
                    // The record's index field is always 0 here, and no read is known to return the
                    // active slot, so identify it by matching against the stored presets.
                    if self.current.is_none() {
                        self.current = self
                            .presets
                            .iter()
                            .position(|s| s.as_ref().is_some_and(|s| s.name == p.name && s.same_sound(&p)))
                            .map(|i| i as u8);
                    }
                    p.index = self.current.unwrap_or(0);
                    ev.value = p.name.clone();
                    self.live = p;
                    self.live_known = true;
                }
            }
            Some(Cmd::PresetName) if host_write => {
                let name = crate::records::ascii(payload.get(1..).unwrap_or_default());
                ev.category = Category::Preset;
                ev.subject = "Rename".into();
                ev.value = format!("“{name}”");
                self.live.name = name;
            }
            Some(Cmd::System) if data => {
                if let Some(s) = Settings::from_packed(payload) {
                    ev.value = format!(
                        "trim {:+} dB · EQ {}/{}/{} · wireless {} · loopback {}",
                        s.input_trim_db(),
                        s.eq_bass,
                        s.eq_mid,
                        s.eq_treble,
                        if s.wireless { "on" } else { "off" },
                        if s.loopback { "on" } else { "off" },
                    );
                    if host_write {
                        ev.category = Category::Settings;
                        ev.subject = "System settings".into();
                        ev.coalesce = key(&[ev.origin as u64, 10]);
                    }
                    self.settings = Some(s);
                }
            }
            Some(Cmd::IrSlot) if data => {
                if let Some((slot, name)) = parse_ir_slot(payload) {
                    ev.value = format!("USER IR {:02} · {name}", slot + 1);
                    if host_write {
                        ev.category = Category::Effect(Module::Cab);
                        ev.subject = "User IR slot".into();
                    }
                    if let Some(s) = self.user_irs.get_mut(slot as usize) {
                        *s = Some(name);
                    }
                }
            }
            Some(Cmd::UserAmp) if dir == Dir::Reply as u8 => {
                if let Some((slot, name)) = parse_user_amp(payload) {
                    ev.value = format!("USER AMP {:02} · {name}", slot + 1);
                    if let Some(s) = self.user_amps.get_mut(slot as usize) {
                        *s = Some(name);
                    }
                }
            }
            Some(Cmd::UserAmp) | Some(Cmd::IrData) if host_write => {
                ev.subject = format!("{} upload", Cmd::label(cmd));
                ev.value = format!("{} bytes", payload.len());
                ev.coalesce = key(&[ev.origin as u64, 11, cmd as u64]);
            }
            Some(Cmd::Loop) if data => {
                if payload.len() >= 4 {
                    self.looper.rec_level = Some(payload[1]);
                    self.looper.play_level = Some(payload[2]);
                    self.looper.set(payload[3]);
                    ev.value = format!(
                        "{} · rec {} · play {}",
                        self.looper.state_name(),
                        payload[1],
                        payload[2]
                    );
                }
            }
            Some(Cmd::Drum) if data => {
                if payload.len() >= 6 {
                    self.drum.pattern = Some(payload[1]);
                    self.drum.level = Some(payload[2]);
                    self.drum.bpm = Some(u14(payload[4], payload[5]));
                    ev.value = format!(
                        "{} · level {} · {} BPM",
                        drum_pattern_name(payload[1]),
                        payload[2],
                        u14(payload[4], payload[5])
                    );
                }
            }
            Some(Cmd::Tuner) if data => {
                if let Some(&s) = payload.first() {
                    self.tuner.active = s != 0;
                    ev.value = if s != 0 { "active" } else { "off" }.into();
                }
            }
            _ => {
                if !payload.is_empty() {
                    let shown = &payload[..payload.len().min(16)];
                    ev.value = hex(shown) + if payload.len() > 16 { " …" } else { "" };
                }
            }
        }
    }
}
