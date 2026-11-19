//! Preset files (`.ospreset`): one preset, or a whole-pedal backup (every preset, the user AMP / IR
//! slots and the system settings), to restore on this pedal or another one.
//!
//! An export reads each user slot's model data back from the pedal (the 2048 converted floats; the
//! `.nam` or WAV never reaches it), so it restores the slot exactly. The app also keeps the source
//! file of every model it imports ([`Library`]) and adds it, so a model can be converted again
//! later. On import, a model the pedal already holds (same name) is used where it is; the others go
//! into slots the user picks, and the presets are rewritten to point at wherever their models
//! ended up.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use open_stomp::records::{PRESET_COUNT, PRESET_LEN, SETTINGS_LEN};
use open_stomp::upload::{USER_MODEL_BASE, USER_SLOTS};
use open_stomp::{DeviceState, Module, Preset, Settings};
use serde::{Deserialize, Serialize};

use crate::import::{self, Kind};

pub const FORMAT: &str = "open-stomp-presets";
pub const VERSION: u32 = 1;

/// A preset file.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PresetFile {
    pub format: String,
    pub version: u32,
    /// `Open Stomp Manager 0.9.1`.
    pub created_by: String,
    /// RFC 3339 local time.
    pub created: String,
    #[serde(default)]
    pub device: FileDevice,
    pub presets: Vec<FilePreset>,
    /// The user slots the presets use (every slot in use, for a backup).
    #[serde(default)]
    pub models: Vec<SlotModel>,
    /// The 17-byte system settings record, hex (backups only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileDevice {
    pub product: Option<String>,
    pub firmware: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FilePreset {
    /// 0-based slot it came from (P01 = 0).
    pub index: u8,
    pub name: String,
    /// The pedal's 110-byte record, hex. Authoritative; `name` is for people reading the file.
    pub record: String,
}

/// What one user AMP or IR slot holds.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SlotModel {
    pub kind: Kind,
    /// 0–9 (USER AMP 01 = 0).
    pub slot: u8,
    /// The slot's name on the pedal.
    pub name: String,
    /// The file it was converted from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    /// What was uploaded: restores the slot exactly, without converting again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub converted: Option<ConvertedData>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub file_name: String,
    /// The file's bytes, base64.
    pub data: String,
    /// The conversion that made `converted`, or to use when there is none.
    pub optimised: bool,
    /// NAM only.
    pub normalise: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConvertedData {
    /// The 2048 floats as little-endian bytes, base64.
    pub data: String,
    pub factor: f32,
}

impl ConvertedData {
    pub fn new(floats: &[f32], factor: f32) -> Self {
        let bytes: Vec<u8> = floats.iter().flat_map(|f| f.to_le_bytes()).collect();
        ConvertedData { data: B64.encode(bytes), factor }
    }

    pub fn floats(&self) -> Result<Vec<f32>, String> {
        let bytes = B64.decode(&self.data).map_err(|e| format!("bad model data: {e}"))?;
        Ok(bytes.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect())
    }
}

impl SlotModel {
    /// The model can be put on a pedal (the file has more than its name).
    pub fn has_data(&self) -> bool {
        self.source.is_some() || self.converted.is_some()
    }
}

/// A name as the pedal stores it: ASCII (others become `?`), at most 31 characters.
pub fn stored_name(name: &str) -> String {
    let n: String = name.chars().take(31).map(|c| if (' '..='~').contains(&c) { c } else { '?' }).collect();
    n.trim().to_owned()
}

/// A slot named `pedal_name` holds the model named `name`.
pub fn same_name(name: &str, pedal_name: Option<&str>) -> bool {
    import::slot_in_use(pedal_name).is_some_and(|p| stored_name(p) == stored_name(name))
}

fn slot_names(state: &DeviceState, kind: Kind) -> &[Option<String>; 10] {
    match kind {
        Kind::Amp => &state.user_amps,
        Kind::Ir => &state.user_irs,
    }
}

/// The user slot a preset uses for `kind`, if any (AMP / CAB models 30–39).
pub fn user_slot(p: &Preset, kind: Kind) -> Option<u8> {
    let m = p.modules[module(kind).index()].model;
    (USER_MODEL_BASE..USER_MODEL_BASE + USER_SLOTS).contains(&m).then(|| m - USER_MODEL_BASE)
}

fn module(kind: Kind) -> Module {
    match kind {
        Kind::Amp => Module::Amp,
        Kind::Ir => Module::Cab,
    }
}

fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    s.len().is_multiple_of(2).then_some(())?;
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

// ───────────────────────────── the app's copy of each slot ─────────────────────────────

/// The models the app imported, one per user slot, in the app's data folder
/// (`user-slots/amp-01.json` …). An entry only counts while the pedal's slot still has its name.
#[derive(Default)]
pub struct Library {
    dir: Option<PathBuf>,
    entries: HashMap<(Kind, u8), SlotModel>,
}

impl Library {
    /// Load the entries in `dir`; `None` keeps them in memory only.
    pub fn open(dir: Option<PathBuf>) -> Library {
        let mut lib = Library { dir, entries: HashMap::new() };
        if let Some(dir) = &lib.dir {
            for kind in [Kind::Amp, Kind::Ir] {
                for slot in 0..USER_SLOTS {
                    let m = fs::read(dir.join(Self::file_name(kind, slot)))
                        .ok()
                        .and_then(|b| serde_json::from_slice::<SlotModel>(&b).ok());
                    if let Some(m) = m.filter(|m| m.kind == kind && m.slot == slot) {
                        lib.entries.insert((kind, slot), m);
                    }
                }
            }
        }
        lib
    }

    fn file_name(kind: Kind, slot: u8) -> String {
        let k = if kind == Kind::Amp { "amp" } else { "ir" };
        format!("{k}-{:02}.json", slot + 1)
    }

    /// The model in `slot`, if the pedal's slot (named `pedal_name`) still holds it.
    pub fn matching(&self, kind: Kind, slot: u8, pedal_name: Option<&str>) -> Option<&SlotModel> {
        self.entries.get(&(kind, slot)).filter(|m| m.has_data() && same_name(&m.name, pedal_name))
    }

    pub fn store(&mut self, m: SlotModel) -> Result<(), String> {
        if let Some(dir) = &self.dir {
            fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let path = dir.join(Self::file_name(m.kind, m.slot));
            let json = serde_json::to_vec(&m).map_err(|e| e.to_string())?;
            fs::write(&path, json).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        self.entries.insert((m.kind, m.slot), m);
        Ok(())
    }

    /// Keep the entry in step with a slot rename.
    pub fn rename(&mut self, kind: Kind, slot: u8, name: &str) -> Result<(), String> {
        match self.entries.get(&(kind, slot)) {
            Some(m) => {
                let m = SlotModel { name: name.to_owned(), ..m.clone() };
                self.store(m)
            }
            None => Ok(()),
        }
    }

}

pub fn read_source(path: &Path, optimised: bool, normalise: bool) -> Result<Source, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(Source { file_name, data: B64.encode(bytes), optimised, normalise })
}

// ───────────────────────────── export ─────────────────────────────

/// Model data read back from the pedal, by slot.
pub type PedalData = HashMap<(Kind, u8), ConvertedData>;

/// The user slots an export of preset `one` (or everything) includes: kind, slot, name.
pub fn export_slots(state: &DeviceState, one: Option<u8>) -> Vec<(Kind, u8, String)> {
    let mut out = vec![];
    for kind in [Kind::Amp, Kind::Ir] {
        let names = slot_names(state, kind);
        for slot in 0..USER_SLOTS {
            let used = match one {
                Some(i) => state.stored(i).and_then(|p| user_slot(p, kind)) == Some(slot),
                None => true,
            };
            if let Some(name) = import::slot_in_use(names[slot as usize].as_deref()).filter(|_| used) {
                out.push((kind, slot, name.to_owned()));
            }
        }
    }
    out
}

/// Build a file of preset `one`, or of everything on the pedal (`None`). Each model's data comes
/// from `pedal` (read back just before), else from the app's copy; its source file from the app's
/// copy. A model with neither goes in by name only.
pub fn export(
    state: &DeviceState,
    one: Option<u8>,
    lib: &Library,
    pedal: &PedalData,
    created_by: &str,
    created: &str,
) -> Result<PresetFile, String> {
    let indices: Vec<u8> = match one {
        Some(i) => vec![i],
        None => (0..PRESET_COUNT as u8).collect(),
    };
    let mut presets = Vec::with_capacity(indices.len());
    for i in indices {
        let p = state.stored(i).ok_or("The pedal hasn't been read yet; wait for “Reading the pedal…” to finish.")?;
        presets.push(FilePreset { index: i, name: p.name.clone(), record: to_hex(&p.raw) });
    }
    let models = export_slots(state, one)
        .into_iter()
        .map(|(kind, slot, name)| {
            let kept = lib.matching(kind, slot, Some(&name));
            let converted = pedal.get(&(kind, slot)).cloned().or_else(|| kept.and_then(|m| m.converted.clone()));
            SlotModel { kind, slot, name, source: kept.and_then(|m| m.source.clone()), converted }
        })
        .collect();
    let settings = state.settings.as_ref().filter(|_| one.is_none()).map(|s| to_hex(&s.to_raw()));
    Ok(PresetFile {
        format: FORMAT.into(),
        version: VERSION,
        created_by: created_by.into(),
        created: created.into(),
        device: FileDevice { product: state.product.clone(), firmware: state.firmware.clone() },
        presets,
        models,
        settings,
    })
}

pub fn write_file(file: &PresetFile, path: &Path) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(file).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn read_file(path: &Path) -> Result<PresetFile, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let f: PresetFile = serde_json::from_slice(&bytes).map_err(|_| "This isn't an Open Stomp preset file.".to_owned())?;
    if f.format != FORMAT {
        return Err("This isn't an Open Stomp preset file.".into());
    }
    if f.version > VERSION {
        return Err("This file was made by a newer Open Stomp Manager; update the app to open it.".into());
    }
    for p in &f.presets {
        preset_of(p)?;
    }
    Ok(f)
}

fn preset_of(p: &FilePreset) -> Result<Preset, String> {
    from_hex(&p.record)
        .filter(|b| b.len() == PRESET_LEN && (p.index as usize) < PRESET_COUNT)
        .and_then(|b| Preset::from_raw(&b))
        .ok_or_else(|| format!("The preset file is damaged (preset “{}”).", p.name))
}

pub fn settings_of(f: &PresetFile) -> Option<Settings> {
    let raw = from_hex(f.settings.as_deref()?).filter(|b| b.len() == SETTINGS_LEN)?;
    Settings::from_packed(&open_stomp::pack::pack(&raw))
}

// ───────────────────────────── import ─────────────────────────────

/// What importing a file would do, for the dialog to show and adjust.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub path: String,
    pub created_by: String,
    pub created: String,
    pub presets: Vec<PlanPreset>,
    pub models: Vec<PlanModel>,
    pub has_settings: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlanPreset {
    pub index: u8,
    pub name: String,
    /// The models it uses, as indices into `models`.
    pub models: Vec<usize>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlanModel {
    pub kind: Kind,
    /// The slot it was in when exported.
    pub slot: u8,
    pub name: String,
    pub has_data: bool,
    /// The file has the converted model, so it uploads without converting again.
    pub converted: bool,
    /// The pedal slot that already holds it (same name).
    pub found: Option<u8>,
    /// Where it goes by default: `found`, else its old slot or the first empty one; `None` to
    /// leave it out.
    pub target: Option<u8>,
}

pub fn plan(path: &Path, state: &DeviceState) -> Result<Plan, String> {
    let f = read_file(path)?;
    let models = plan_models(&f.models, state);
    let presets = f
        .presets
        .iter()
        .map(|p| {
            let pr = preset_of(p).ok();
            let uses = |m: &SlotModel| pr.as_ref().is_some_and(|pr| user_slot(pr, m.kind) == Some(m.slot));
            PlanPreset { index: p.index, name: p.name.clone(), models: (0..f.models.len()).filter(|i| uses(&f.models[*i])).collect() }
        })
        .collect();
    Ok(Plan {
        path: path.display().to_string(),
        created_by: f.created_by.clone(),
        created: f.created.clone(),
        presets,
        models,
        has_settings: settings_of(&f).is_some(),
    })
}

fn plan_models(models: &[SlotModel], state: &DeviceState) -> Vec<PlanModel> {
    let mut taken: Vec<(Kind, u8)> = vec![];
    let mut out: Vec<PlanModel> = models
        .iter()
        .map(|m| {
            let names = slot_names(state, m.kind);
            // its own slot first, so two slots with the same name each keep theirs
            let found = std::iter::once(m.slot)
                .chain(0..USER_SLOTS)
                .find(|s| *s < USER_SLOTS && !taken.contains(&(m.kind, *s)) && same_name(&m.name, names[*s as usize].as_deref()));
            if let Some(s) = found {
                taken.push((m.kind, s));
            }
            PlanModel {
                kind: m.kind,
                slot: m.slot,
                name: m.name.clone(),
                has_data: m.has_data(),
                converted: m.converted.is_some(),
                found,
                target: found,
            }
        })
        .collect();
    // the rest: their old slot if it's free, else the first free one
    for m in out.iter_mut().filter(|m| m.found.is_none() && m.has_data) {
        let names = slot_names(state, m.kind);
        let free = |s: u8, taken: &[(Kind, u8)]| import::slot_in_use(names[s as usize].as_deref()).is_none() && !taken.contains(&(m.kind, s));
        m.target = std::iter::once(m.slot).chain(0..USER_SLOTS).find(|s| free(*s, &taken));
        if let Some(s) = m.target {
            taken.push((m.kind, s));
        }
    }
    out
}

/// The import the user confirmed.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RestoreRequest {
    pub path: String,
    pub presets: Vec<PresetTarget>,
    /// Per model in the file: the slot it goes to, or `None` to leave it out.
    pub models: Vec<Option<u8>>,
    pub settings: bool,
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct PresetTarget {
    /// Index into the file's presets.
    pub entry: usize,
    /// Pedal slot to write it to.
    pub to: u8,
}

/// The work a restore does, in order.
#[derive(Debug)]
pub struct Work {
    /// Models to put on the pedal: (kind, slot, the model with its data).
    pub uploads: Vec<(Kind, u8, SlotModel)>,
    /// Models the pedal already holds, to remember as the slot's copy.
    pub present: Vec<(Kind, u8, SlotModel)>,
    pub presets: Vec<Preset>,
    pub settings: Option<Settings>,
}

pub fn work(req: &RestoreRequest, state: &DeviceState) -> Result<Work, String> {
    let f = read_file(Path::new(&req.path))?;
    if req.models.len() != f.models.len() {
        return Err("The file changed since it was opened; open it again.".into());
    }
    let mut uploads = vec![];
    let mut present = vec![];
    let mut moved: HashMap<(Kind, u8), u8> = HashMap::new();
    let mut targets: Vec<(Kind, u8)> = vec![];
    for (m, target) in f.models.iter().zip(&req.models) {
        let Some(t) = *target else { continue };
        if t >= USER_SLOTS || targets.contains(&(m.kind, t)) {
            return Err(format!("Two models can't both go into {}.", m.kind.slot_label(t)));
        }
        targets.push((m.kind, t));
        moved.insert((m.kind, m.slot), t);
        let slot = SlotModel { slot: t, ..m.clone() };
        if same_name(&m.name, slot_names(state, m.kind)[t as usize].as_deref()) {
            present.push((m.kind, t, slot));
        } else if m.has_data() {
            uploads.push((m.kind, t, slot));
        } else {
            return Err(format!("The file has no copy of “{}”, so it can't go into {}.", m.name, m.kind.slot_label(t)));
        }
    }
    let mut presets = vec![];
    let mut written: Vec<u8> = vec![];
    for t in &req.presets {
        let entry = f.presets.get(t.entry).ok_or("The file changed since it was opened; open it again.")?;
        if t.to as usize >= PRESET_COUNT || written.contains(&t.to) {
            return Err(format!("Two presets can't both go into P{:02}.", t.to as u16 + 1));
        }
        written.push(t.to);
        let mut p = preset_of(entry)?;
        p.index = t.to;
        for kind in [Kind::Amp, Kind::Ir] {
            if let Some(&s) = user_slot(&p, kind).and_then(|old| moved.get(&(kind, old))) {
                p.modules[module(kind).index()].model = USER_MODEL_BASE + s;
            }
        }
        presets.push(p);
    }
    let settings = if req.settings { Some(settings_of(&f).ok_or("The file has no system settings.")?) } else { None };
    Ok(Work { uploads, present, presets, settings })
}

/// Write a model's source file where the converter can read it.
pub fn source_to_temp(m: &SlotModel) -> Result<PathBuf, String> {
    let src = m.source.as_ref().ok_or("no source file")?;
    let bytes = B64.decode(&src.data).map_err(|e| format!("bad source data: {e}"))?;
    let dir = std::env::temp_dir().join(format!("open-stomp-restore-{}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // keep the file name: the converter goes by its extension
    let name = Path::new(&src.file_name).file_name().map(|n| n.to_owned()).unwrap_or_else(|| "model".into());
    let path = dir.join(name);
    fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A state with every preset read, user AMP 03 "Jcm800" and user IR 01 "V30", and P05 using both.
    fn pedal() -> DeviceState {
        let mut s = DeviceState::default();
        for i in 0..PRESET_COUNT as u8 {
            let mut p = Preset { index: i, name: format!("Preset {}", i + 1), ..Default::default() };
            p.raw = p.to_raw();
            s.presets[i as usize] = Some(p);
        }
        let mut p = s.presets[4].clone().unwrap();
        p.modules[Module::Amp.index()].model = 32;
        p.modules[Module::Cab.index()].model = 30;
        p.raw = p.to_raw();
        s.presets[4] = Some(p);
        s.user_amps = std::array::from_fn(|i| Some(format!("USER AMP {:02}", i + 1)));
        s.user_irs = std::array::from_fn(|i| Some(format!("USER IR {:02}", i + 1)));
        s.user_amps[2] = Some("Jcm800".into());
        s.user_irs[0] = Some("V30".into());
        s.settings = Settings::from_packed(&open_stomp::pack::pack(&[0, 10, 0, 0, 50, 50, 50, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]));
        s
    }

    fn library() -> Library {
        let mut lib = Library::open(None);
        let data = ConvertedData::new(&[0.5; 2048], 1.0);
        lib.store(SlotModel { kind: Kind::Amp, slot: 2, name: "Jcm800".into(), source: None, converted: Some(data) }).unwrap();
        lib
    }

    #[test]
    fn hex_and_floats_round_trip() {
        assert_eq!(from_hex(&to_hex(&[0, 1, 0xab, 0xff])), Some(vec![0, 1, 0xab, 0xff]));
        assert_eq!(from_hex("abc"), None);
        let c = ConvertedData::new(&[1.0, -2.5, 3.25], 1.0);
        assert_eq!(c.floats().unwrap(), vec![1.0, -2.5, 3.25]);
    }

    #[test]
    fn names_compare_as_the_pedal_stores_them() {
        assert!(same_name("Jcm800 ", Some("Jcm800")));
        assert!(same_name("Ünï", Some("?n?")));
        assert!(!same_name("USER AMP 03", Some("USER AMP 03")), "an empty slot holds nothing");
        assert!(!same_name("Jcm800", None));
    }

    #[test]
    fn export_one_preset_takes_its_models() {
        let s = pedal();
        let f = export(&s, Some(4), &library(), &PedalData::new(), "test", "now").unwrap();
        assert_eq!(f.presets.len(), 1);
        assert!(f.settings.is_none());
        let kinds: Vec<_> = f.models.iter().map(|m| (m.kind, m.slot, m.name.as_str(), m.has_data())).collect();
        // the AMP comes from the library; the IR has no copy, so it goes by name only
        assert_eq!(kinds, vec![(Kind::Amp, 2, "Jcm800", true), (Kind::Ir, 0, "V30", false)]);
        // data read back from the pedal wins over the app's copy, and fills in what it lacks
        let mut read = PedalData::new();
        read.insert((Kind::Amp, 2), ConvertedData::new(&[0.25; 2048], 1.0));
        read.insert((Kind::Ir, 0), ConvertedData::new(&[0.75; 2048], 1.0));
        let f = export(&s, Some(4), &library(), &read, "test", "now").unwrap();
        assert_eq!(f.models[0].converted.as_ref().unwrap().floats().unwrap()[0], 0.25);
        assert_eq!(f.models[1].converted.as_ref().unwrap().floats().unwrap()[0], 0.75);
        assert_eq!(export_slots(&s, Some(4)).len(), 2);
        // a preset with no user models takes none
        assert!(export(&s, Some(0), &library(), &PedalData::new(), "test", "now").unwrap().models.is_empty());
    }

    #[test]
    fn backup_takes_everything_and_reads_back() {
        let s = pedal();
        let f = export(&s, None, &library(), &PedalData::new(), "test", "now").unwrap();
        assert_eq!((f.presets.len(), f.models.len()), (99, 2));
        let dir = std::env::temp_dir().join(format!("open-stomp-backup-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("all.ospreset");
        write_file(&f, &path).unwrap();
        let back = read_file(&path).unwrap();
        assert_eq!(back.presets[4].record, f.presets[4].record);
        assert_eq!(settings_of(&back).unwrap().to_raw(), s.settings.as_ref().unwrap().to_raw());
        fs::remove_dir_all(&dir).unwrap();
        // not read yet
        assert!(export(&DeviceState::default(), None, &library(), &PedalData::new(), "t", "n").is_err());
    }

    #[test]
    fn plan_finds_models_by_name_and_places_the_rest() {
        let f = export(&pedal(), None, &library(), &PedalData::new(), "t", "n").unwrap();
        // a pedal where Jcm800 sits in AMP 07, and AMP 03 holds something else
        let mut other = DeviceState {
            user_amps: std::array::from_fn(|i| Some(format!("USER AMP {:02}", i + 1))),
            user_irs: std::array::from_fn(|i| Some(format!("USER IR {:02}", i + 1))),
            ..Default::default()
        };
        other.user_amps[6] = Some("Jcm800".into());
        let models = plan_models(&f.models, &other);
        assert_eq!((models[0].found, models[0].target), (Some(6), Some(6)));
        // V30 has no data and isn't on the pedal: left out
        assert_eq!((models[1].found, models[1].target), (None, None));

        // a freshly flashed pedal: everything goes back to its old slot
        let mut models = f.models.clone();
        models[1].converted = Some(ConvertedData::new(&[0.1; 2048], 1.0));
        let mut wiped = other.clone();
        wiped.user_amps[6] = Some("USER AMP 07".into());
        let plan = plan_models(&models, &wiped);
        assert_eq!(plan.iter().map(|m| m.target).collect::<Vec<_>>(), vec![Some(2), Some(0)]);

        // two slots with the same name each find their own
        let mut twins = other.clone();
        twins.user_amps[1] = Some("Jcm800".into());
        twins.user_amps[2] = Some("Jcm800".into());
        let mut models = f.models.clone();
        models.insert(0, SlotModel { slot: 1, ..models[0].clone() });
        let found: Vec<_> = plan_models(&models, &twins).iter().map(|m| m.found).collect();
        assert_eq!(found, vec![Some(1), Some(2), None]);
        let mut models = f.models.clone();
        models[1].converted = Some(ConvertedData::new(&[0.1; 2048], 1.0));

        // the old slot is taken by something else: the first free one
        let mut busy = wiped.clone();
        busy.user_amps[2] = Some("Twin".into());
        busy.user_amps[0] = Some("Plexi".into());
        assert_eq!(plan_models(&models, &busy)[0].target, Some(1));
    }

    #[test]
    fn restore_moves_presets_to_where_their_models_went() {
        let s = pedal();
        let mut f = export(&s, Some(4), &library(), &PedalData::new(), "t", "n").unwrap();
        f.models[1].converted = Some(ConvertedData::new(&[0.1; 2048], 1.0));
        let dir = std::env::temp_dir().join(format!("open-stomp-restore-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p05.ospreset");
        write_file(&f, &path).unwrap();

        // Jcm800 is already in AMP 03 here; V30 goes into IR 05; the preset goes to P10
        let req = RestoreRequest {
            path: path.display().to_string(),
            presets: vec![PresetTarget { entry: 0, to: 9 }],
            models: vec![Some(2), Some(4)],
            settings: false,
        };
        let w = work(&req, &s).unwrap();
        assert_eq!(w.uploads.iter().map(|u| (u.0, u.1)).collect::<Vec<_>>(), vec![(Kind::Ir, 4)]);
        assert_eq!(w.present.iter().map(|u| (u.0, u.1)).collect::<Vec<_>>(), vec![(Kind::Amp, 2)]);
        let p = &w.presets[0];
        assert_eq!((p.index, p.raw[0]), (9, 5 - 1), "raw keeps the old record until written");
        assert_eq!(p.to_raw()[0], 9);
        assert_eq!(p.modules[Module::Amp.index()].model, 32);
        assert_eq!(p.modules[Module::Cab.index()].model, 34);
        assert_eq!(p.name, "Preset 5");

        // an AMP and an IR may share a slot number; two presets may not share a slot
        assert!(work(&RestoreRequest { models: vec![Some(2), Some(2)], ..req.clone() }, &s).is_ok());
        let twice = RestoreRequest { presets: vec![PresetTarget { entry: 0, to: 3 }, PresetTarget { entry: 0, to: 3 }], ..req.clone() };
        assert!(work(&twice, &s).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn library_persists_and_follows_renames() {
        let dir = std::env::temp_dir().join(format!("open-stomp-library-test-{}", std::process::id()));
        let mut lib = Library::open(Some(dir.clone()));
        let m = SlotModel { kind: Kind::Ir, slot: 9, name: "V30".into(), source: None, converted: Some(ConvertedData::new(&[1.0; 4], 1.0)) };
        lib.store(m).unwrap();
        lib.rename(Kind::Ir, 9, "V30 edge").unwrap();
        let lib = Library::open(Some(dir.clone()));
        assert!(lib.matching(Kind::Ir, 9, Some("V30 edge")).is_some());
        assert!(lib.matching(Kind::Ir, 9, Some("V30")).is_none(), "the pedal holds something else now");
        fs::remove_dir_all(&dir).unwrap();
    }
}
