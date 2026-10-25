//! What the frontend sees: the pedal state cut into sections and turned into JSON-friendly
//! structs. The hub sends a section again only when it changed, so each one is sized around how
//! often it changes (connection, preset list, live sound, model lists).

use open_stomp::catalog::{DRUM_STYLES, NOTES, ParamKind, drum_pattern_name};
use open_stomp::device::Status;
use open_stomp::records::{DEFAULT_CHAIN, PRESET_COUNT};
use open_stomp::state::preset_label;
use open_stomp::{DeviceState, Module, is_ble_port};
use serde::Serialize;

use crate::import::{self, JobView};
use crate::log::Summary;

/// Everything the hub knows that the views are built from.
pub struct Inputs<'a> {
    pub state: &'a DeviceState,
    pub status: &'a Status,
    pub ports: &'a [String],
    pub sync: (usize, usize),
    pub listen_only: bool,
    /// The effect last touched on the pedal or in the app; the detail panel follows it.
    pub followed: Module,
    pub replay_path: Option<&'a str>,
    pub log: Summary,
    pub import: ImportView,
    /// `STOMP_DEV` flags: dialogs and panels to open at startup (for screenshots).
    pub dev_flags: &'a [String],
}

/// The import in progress, and how the last one ended.
#[derive(Serialize, Default)]
pub struct ImportView {
    pub job: Option<JobView>,
    pub outcome: Option<Outcome>,
}

#[derive(Serialize, Clone)]
pub struct Outcome {
    pub text: String,
    pub error: bool,
}

impl Inputs<'_> {
    /// The app may send to the pedal: connected and not in listen-only mode.
    pub fn editable(&self) -> bool {
        matches!(self.status, Status::Connected { .. }) && !self.listen_only
    }

    /// Effect edits also need the live preset, since a head CC carries model and on/off together.
    pub fn can_edit_sound(&self) -> bool {
        self.editable() && self.state.live_known
    }
}

/// Every section by name, ready to compare with what was sent last.
pub fn sections(i: &Inputs) -> Vec<(&'static str, serde_json::Value)> {
    let json = |v: serde_json::Result<serde_json::Value>| v.expect("view structs serialize");
    vec![
        ("conn", json(serde_json::to_value(conn(i)))),
        ("presets", json(serde_json::to_value(presets(i.state)))),
        ("live", json(serde_json::to_value(live(i)))),
        ("models", json(serde_json::to_value(models(i.state)))),
        ("extras", json(serde_json::to_value(extras(i.state)))),
        ("device", json(serde_json::to_value(device(i.state)))),
        ("log", json(serde_json::to_value(&i.log))),
        ("import", json(serde_json::to_value(&i.import))),
        ("catalog", json(serde_json::to_value(catalog(i.dev_flags)))),
    ]
}

// ───────────────────────────── connection ─────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conn<'a> {
    status: ConnStatus,
    ports: &'a [String],
    sync: SyncProgress,
    listen_only: bool,
    editable: bool,
    can_edit_sound: bool,
    firmware: Option<&'a str>,
    product: Option<&'a str>,
    replay_path: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum ConnStatus {
    Disconnected,
    Connected { port: String, output: bool, bluetooth: bool },
    Replaying { file: String, progress: f32 },
}

#[derive(Serialize)]
struct SyncProgress {
    done: usize,
    total: usize,
}

fn conn<'a>(i: &'a Inputs) -> Conn<'a> {
    let status = match i.status {
        Status::Disconnected => ConnStatus::Disconnected,
        Status::Connected { port, output } => {
            ConnStatus::Connected { port: port.clone(), output: *output, bluetooth: is_ble_port(port) }
        }
        Status::Replaying { file, progress } => ConnStatus::Replaying { file: file.clone(), progress: *progress },
    };
    Conn {
        status,
        ports: i.ports,
        sync: SyncProgress { done: i.sync.0, total: i.sync.1 },
        listen_only: i.listen_only,
        editable: i.editable(),
        can_edit_sound: i.can_edit_sound(),
        firmware: i.state.firmware.as_deref(),
        product: i.state.product.as_deref(),
        replay_path: i.replay_path,
    }
}

// ───────────────────────────── presets ─────────────────────────────

#[derive(Serialize)]
pub struct Presets<'a> {
    current: Option<u8>,
    modified: bool,
    items: Vec<PresetItem<'a>>,
}

#[derive(Serialize)]
struct PresetItem<'a> {
    index: u8,
    label: String,
    /// `None` until the startup sync has read the preset.
    name: Option<&'a str>,
}

fn presets(s: &DeviceState) -> Presets<'_> {
    let items = (0..PRESET_COUNT as u8)
        .map(|i| PresetItem { index: i, label: preset_label(i), name: s.preset_name(i) })
        .collect();
    Presets { current: s.current, modified: s.modified(), items }
}

// ───────────────────────────── live sound ─────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Live<'a> {
    /// The edit buffer has been read, so blocks and params are real values.
    known: bool,
    name: &'a str,
    volume: u8,
    /// Module indices in signal order: gate, the seven-block chain, then looper and drum.
    chain: Vec<u8>,
    /// Indexed by module index.
    blocks: Vec<Block>,
    followed: u8,
    /// The module page the pedal's display shows, once known.
    page: Option<u8>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Block {
    module: u8,
    /// CSS-friendly name (`amp`, `odfx`, …), for colours.
    key: &'static str,
    title: &'static str,
    effect: bool,
    on: bool,
    /// Model number (0-based) for effects.
    model: u8,
    /// Model name, or the looper/drum state.
    summary: String,
    params: Vec<Param>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Param {
    slot: u8,
    name: &'static str,
    kind: &'static str,
    options: &'static [&'static str],
    value: u8,
    display: String,
}

pub fn key(m: Module) -> &'static str {
    match m {
        Module::OdFx => "odfx",
        Module::Amp => "amp",
        Module::Cab => "cab",
        Module::Mod => "mod",
        Module::Delay => "delay",
        Module::Reverb => "reverb",
        Module::Gate => "gate",
        Module::Eq => "eq",
        Module::Loop => "loop",
        Module::Drum => "drum",
    }
}

pub fn title(m: Module) -> &'static str {
    match m {
        Module::Gate => "Gate",
        Module::OdFx => "OD/FX",
        Module::Amp => "Amp",
        Module::Cab => "Cab",
        Module::Eq => "EQ",
        Module::Mod => "Mod",
        Module::Delay => "Delay",
        Module::Reverb => "Reverb",
        Module::Loop => "Looper",
        Module::Drum => "Drum",
    }
}

fn live<'a>(i: &'a Inputs) -> Live<'a> {
    let s = i.state;
    let chain_src = if s.live_known { s.live.chain } else { DEFAULT_CHAIN };
    let mut chain = vec![Module::Gate as u8];
    chain.extend(chain_src.iter().filter(|m| Module::from_index(**m).is_some()));
    chain.extend([Module::Loop as u8, Module::Drum as u8]);
    let blocks = Module::ALL.iter().map(|m| block(s, *m)).collect();
    Live {
        known: s.live_known,
        name: &s.live.name,
        volume: s.live.volume,
        chain,
        blocks,
        followed: i.followed as u8,
        page: s.page.map(|m| m as u8),
    }
}

fn block(s: &DeviceState, m: Module) -> Block {
    let base = |on, model, summary, params| Block {
        module: m as u8,
        key: key(m),
        title: title(m),
        effect: m.is_effect(),
        on,
        model,
        summary,
        params,
    };
    match m {
        Module::Loop => base(s.looper.state != 0, 0, s.looper.state_name().to_owned(), vec![]),
        Module::Drum => {
            let summary = s.drum.pattern.map(drum_pattern_name).unwrap_or_else(|| "Drums".into());
            base(s.drum.playing, 0, summary, vec![])
        }
        _ => {
            let st = &s.live.modules[m.index()];
            let summary =
                if !s.live_known && st.model == 0 && !st.on { "—".into() } else { s.model_name(m, st.model) };
            let params = m
                .params(st.model)
                .into_iter()
                .map(|pd| {
                    let value = st.params[pd.slot as usize];
                    let (kind, options): (_, &'static [&'static str]) = match pd.kind {
                        ParamKind::Knob => ("knob", &[]),
                        ParamKind::Bipolar => ("bipolar", &[]),
                        ParamKind::Enum(opts) => ("enum", opts),
                    };
                    let display = if s.live_known || value != 0 { pd.format(value) } else { "—".into() };
                    Param { slot: pd.slot, name: pd.name, kind, options, value, display }
                })
                .collect();
            base(st.on, st.model, summary, params)
        }
    }
}

// ───────────────────────────── model lists ─────────────────────────────

/// Model names per module index (user slots carry their names); empty for looper and drum.
fn models(s: &DeviceState) -> Vec<Vec<String>> {
    Module::ALL
        .iter()
        .map(|m| (0..m.models().len() as u8).map(|i| s.model_name(*m, i)).collect())
        .collect()
}

// ───────────────────────────── tuner, looper, drum ─────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Extras {
    tuner: TunerView,
    looper: LooperView,
    drum: DrumView,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TunerView {
    active: bool,
    /// A note is detected right now.
    detected: bool,
    note: Option<&'static str>,
    /// −50…+50.
    cents: Option<i16>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LooperView {
    /// 0 empty, 1 recording, 2 playing, 3 overdubbing, 4 stopped.
    state: u8,
    state_name: &'static str,
    /// 0 none, 1 an overdub (undo available), 2 the base layer only.
    layers: u8,
    rec_level: Option<u8>,
    play_level: Option<u8>,
    sync: Option<bool>,
    /// 0 NR, else AR.
    mode: Option<u8>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DrumView {
    playing: bool,
    pattern: Option<u8>,
    pattern_name: Option<String>,
    level: Option<u8>,
    bpm: Option<u16>,
}

fn extras(s: &DeviceState) -> Extras {
    let t = &s.tuner;
    let l = &s.looper;
    let d = &s.drum;
    Extras {
        tuner: TunerView {
            active: t.active,
            detected: t.detected,
            note: t.note.and_then(|n| NOTES.get(n as usize).copied()),
            cents: t.cents_offset(),
        },
        looper: LooperView {
            state: l.state,
            state_name: l.state_name(),
            layers: l.layers,
            rec_level: l.rec_level,
            play_level: l.play_level,
            sync: l.sync,
            mode: l.mode,
        },
        drum: DrumView {
            playing: d.playing,
            pattern: d.pattern,
            pattern_name: d.pattern.map(drum_pattern_name),
            level: d.level,
            bpm: d.bpm,
        },
    }
}

// ───────────────────────────── device, settings, user slots ─────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device<'a> {
    product: Option<&'a str>,
    firmware: Option<&'a str>,
    model_id: Option<&'a str>,
    settings: Option<SettingsView>,
    user_amps: Vec<SlotView<'a>>,
    user_irs: Vec<SlotView<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsView {
    /// 0–20 = −10…+10 dB.
    input_trim: u8,
    eq_bass: u8,
    eq_mid: u8,
    eq_treble: u8,
    wireless: bool,
    loopback: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SlotView<'a> {
    /// `None` until the sync has read the slot.
    name: Option<&'a str>,
    /// The name when the slot holds an import, `None` when it's empty.
    in_use: Option<&'a str>,
}

fn slots(names: &[Option<String>; 10]) -> Vec<SlotView<'_>> {
    names.iter().map(|n| SlotView { name: n.as_deref(), in_use: import::slot_in_use(n.as_deref()) }).collect()
}

fn device(s: &DeviceState) -> Device<'_> {
    Device {
        product: s.product.as_deref(),
        firmware: s.firmware.as_deref(),
        model_id: s.model_id.as_deref(),
        settings: s.settings.as_ref().map(|x| SettingsView {
            input_trim: x.input_trim,
            eq_bass: x.eq_bass,
            eq_mid: x.eq_mid,
            eq_treble: x.eq_treble,
            wireless: x.wireless,
            loopback: x.loopback,
        }),
        user_amps: slots(&s.user_amps),
        user_irs: slots(&s.user_irs),
    }
}

// ───────────────────────────── fixed lists ─────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog<'a> {
    /// Drum patterns by style: value (the pattern CC) and name.
    drum_styles: Vec<(&'static str, Vec<(u8, String)>)>,
    dev_flags: &'a [String],
    /// `0.9.0 (1a2b3c4d)`.
    app_version: String,
    lib_version: &'static str,
}

fn catalog(dev_flags: &[String]) -> Catalog<'_> {
    let drum_styles = DRUM_STYLES
        .iter()
        .enumerate()
        .map(|(si, style)| {
            let patterns = (0..10u8).map(|k| si as u8 * 10 + k).map(|v| (v, drum_pattern_name(v))).collect();
            (*style, patterns)
        })
        .collect();
    Catalog { drum_styles, dev_flags, app_version: crate::version_long(), lib_version: crate::LIB_VERSION }
}
