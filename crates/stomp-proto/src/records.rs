//! Data records carried in SysEx (docs/04).

use crate::catalog::Module;
use crate::pack::{pack, unpack};

pub const PRESET_LEN: usize = 110;
pub const PRESET_COUNT: usize = 99;

/// One effect module's state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModuleState {
    pub on: bool,
    pub model: u8,
    pub params: [u8; 12],
}

impl ModuleState {
    pub fn set_head(&mut self, head: u8) {
        self.on = head & 0x40 == 0;
        self.model = head & 0x3F;
    }

    pub fn head(&self) -> u8 {
        (if self.on { 0 } else { 0x40 }) | (self.model & 0x3F)
    }
}

/// A preset (SysEx `0x0B` / `0x0C`), kept with its raw record so unknown bytes survive a round trip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    pub index: u8,
    /// Indexed by `Module as usize` for the eight effect modules.
    pub modules: [ModuleState; 8],
    pub volume: u8,
    /// Chain order of the seven non-gate modules (module indices).
    pub chain: [u8; 7],
    pub name: String,
    pub raw: [u8; PRESET_LEN],
}

/// (count offset, first param offset) per effect module, indexed by module.
const PARAM_LAYOUT: [(usize, usize); 8] = [
    (25, 26), // OD/FX
    (32, 33), // AMP
    (41, 42), // CAB
    (61, 62), // MOD
    (66, 67), // DELAY
    (73, 74), // REVERB
    (20, 21), // GATE
    (48, 49), // EQ
];

impl Default for Preset {
    fn default() -> Self {
        Preset {
            index: 0,
            modules: Default::default(),
            volume: 80,
            chain: DEFAULT_CHAIN,
            name: String::new(),
            raw: [0; PRESET_LEN],
        }
    }
}

pub const DEFAULT_CHAIN: [u8; 7] = [0, 1, 2, 7, 3, 4, 5];

impl Preset {
    /// From the packed SysEx body (after `<dir>`).
    pub fn from_packed(midi: &[u8]) -> Option<Preset> {
        Preset::from_raw(&unpack(midi))
    }

    pub fn from_raw(b: &[u8]) -> Option<Preset> {
        let raw: [u8; PRESET_LEN] = b.get(..PRESET_LEN)?.try_into().ok()?;
        let mut p = Preset { index: raw[0], raw, ..Default::default() };
        for m in Module::EFFECTS {
            let i = m.index();
            let st = &mut p.modules[i];
            st.set_head(raw[12 + i]);
            let (_, first) = PARAM_LAYOUT[i];
            let n = m.param_ccs().map(|x| x.1 as usize).unwrap_or(0);
            st.params[..n].copy_from_slice(&raw[first..first + n]);
        }
        p.volume = raw[78];
        p.chain.copy_from_slice(&raw[83..90]);
        if p.chain.iter().any(|c| *c > 7) {
            p.chain = DEFAULT_CHAIN;
        }
        p.name = ascii(&raw[94..110]);
        Some(p)
    }

    /// The 110-byte record: the original raw bytes with every decoded field written back.
    /// Param-count bytes are only recomputed for modules whose model changed.
    pub fn to_raw(&self) -> [u8; PRESET_LEN] {
        let mut raw = self.raw;
        raw[0] = self.index;
        for m in Module::EFFECTS {
            let i = m.index();
            let st = &self.modules[i];
            if raw[12 + i] & 0x3F != st.model {
                raw[PARAM_LAYOUT[i].0] = param_count(m, st.model);
            }
            raw[12 + i] = st.head();
            let (_, first) = PARAM_LAYOUT[i];
            let n = m.param_ccs().map(|x| x.1 as usize).unwrap_or(0);
            raw[first..first + n].copy_from_slice(&st.params[..n]);
        }
        raw[78] = self.volume;
        raw[83..90].copy_from_slice(&self.chain);
        raw[94..110].copy_from_slice(&name_bytes(&self.name));
        raw
    }

    /// Packed SysEx body for a `0x0B` write.
    pub fn to_packed(&self) -> Vec<u8> {
        pack(&self.to_raw())
    }

    pub fn module(&self, m: Module) -> Option<&ModuleState> {
        self.modules.get(m.index())
    }

    pub fn module_mut(&mut self, m: Module) -> Option<&mut ModuleState> {
        self.modules.get_mut(m.index())
    }

    /// Whether the decoded sound (modules, volume, chain) is the same, ignoring unused params.
    pub fn same_sound(&self, other: &Preset) -> bool {
        self.volume == other.volume
            && self.chain == other.chain
            && Module::EFFECTS.into_iter().all(|m| {
                let (a, b) = (&self.modules[m.index()], &other.modules[m.index()]);
                a.on == b.on
                    && a.model == b.model
                    && m.params(a.model)
                        .iter()
                        .all(|p| a.params[p.slot as usize] == b.params[p.slot as usize])
            })
    }
}

pub const SETTINGS_LEN: usize = 17;

/// Value of a module's param-count byte for a model (the number of params the model uses).
/// CAB always carries all five: user IRs only hide MIC/POSITION in the UI.
pub fn param_count(m: Module, model: u8) -> u8 {
    match m {
        Module::Cab => 5,
        _ => m.params(model).len() as u8,
    }
}

/// A preset name as the 16-byte zero-padded ASCII field (non-ASCII becomes `?`).
pub fn name_bytes(name: &str) -> [u8; 16] {
    let mut b = [0u8; 16];
    for (d, c) in b.iter_mut().zip(name.chars()) {
        *d = if (' '..='~').contains(&c) { c as u8 } else { b'?' };
    }
    b
}

/// Global system settings (SysEx `0x14`), 17 bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// 0–20 → −10…+10 dB.
    pub input_trim: u8,
    pub eq_bass: u8,
    pub eq_mid: u8,
    pub eq_treble: u8,
    pub wireless: bool,
    pub loopback: bool,
    pub raw: Vec<u8>,
}

impl Settings {
    pub fn from_packed(midi: &[u8]) -> Option<Settings> {
        let raw = unpack(midi);
        if raw.len() < 11 {
            return None;
        }
        Some(Settings {
            input_trim: raw[1],
            eq_bass: raw[4],
            eq_mid: raw[5],
            eq_treble: raw[6],
            wireless: raw[9] != 0,
            loopback: raw[10] != 0,
            raw,
        })
    }

    /// The record with the decoded fields written back over the original bytes.
    pub fn to_raw(&self) -> Vec<u8> {
        let mut raw = self.raw.clone();
        if raw.len() < SETTINGS_LEN {
            raw.resize(SETTINGS_LEN, 0);
        }
        raw[1] = self.input_trim.min(20);
        raw[4] = self.eq_bass.min(100);
        raw[5] = self.eq_mid.min(100);
        raw[6] = self.eq_treble.min(100);
        raw[9] = self.wireless as u8;
        raw[10] = self.loopback as u8;
        raw
    }

    pub fn to_packed(&self) -> Vec<u8> {
        pack(&self.to_raw())
    }

    pub fn input_trim_db(&self) -> i16 {
        self.input_trim as i16 - 10
    }
}

/// User IR slot reply payload (`<slot> <packed: 4 B field, name>`).
pub fn parse_ir_slot(payload: &[u8]) -> Option<(u8, String)> {
    let (&slot, rest) = payload.split_first()?;
    let raw = unpack(rest);
    Some((slot, ascii(raw.get(4..)?)))
}

/// User AMP (NAM) slot info reply payload (whole body packed, name at 12).
pub fn parse_user_amp(payload: &[u8]) -> Option<(u8, String)> {
    let raw = unpack(payload);
    if raw.get(2) != Some(&4) {
        return None;
    }
    Some((raw[1], ascii(raw.get(12..44.min(raw.len()))?)))
}

/// Plain 7-bit 14-bit value, high 7 bits first.
pub fn u14(hi: u8, lo: u8) -> u16 {
    ((hi as u16 & 0x7F) << 7) | (lo as u16 & 0x7F)
}

/// Zero-terminated, zero-padded ASCII.
pub fn ascii(b: &[u8]) -> String {
    b.iter()
        .take_while(|c| **c != 0)
        .map(|&c| if (0x20..0x7F).contains(&c) { c as char } else { '?' })
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::pack;

    #[test]
    fn preset_decode() {
        let mut raw = [0u8; PRESET_LEN];
        raw[0] = 4;
        raw[13] = 0x48; // AMP off, model 8
        raw[33] = 64; // AMP gain
        raw[78] = 80;
        raw[83..90].copy_from_slice(&DEFAULT_CHAIN);
        raw[94..99].copy_from_slice(b"Hello");
        let p = Preset::from_packed(&pack(&raw)).unwrap();
        assert_eq!(p.index, 4);
        assert!(!p.modules[1].on);
        assert_eq!(p.modules[1].model, 8);
        assert_eq!(p.modules[1].params[0], 64);
        assert_eq!(p.name, "Hello");
    }

    #[test]
    fn settings_factory() {
        let raw = [0x00, 0x0A, 0x1E, 0x0A, 0x32, 0x32, 0x32, 0x32, 0x32, 1, 1, 0x3C, 0xB4, 0, 0, 0, 0];
        let s = Settings::from_packed(&pack(&raw)).unwrap();
        assert_eq!(s.input_trim_db(), 0);
        assert_eq!(s.eq_mid, 50);
        assert!(s.wireless && s.loopback);
        assert_eq!(s.to_raw(), raw);
        let mut s2 = s.clone();
        s2.input_trim = 13;
        s2.loopback = false;
        let r = Settings::from_packed(&s2.to_packed()).unwrap();
        assert_eq!((r.input_trim, r.loopback, r.raw[12]), (13, false, 0xB4));
    }

    #[test]
    fn preset_encode_patches_fields() {
        let mut raw = [0u8; PRESET_LEN];
        raw[8] = 0x62;
        raw[12] = 0x05; // OD/FX DS1 (3 params)
        raw[25] = 3;
        raw[83..90].copy_from_slice(&DEFAULT_CHAIN);
        let mut p = Preset::from_raw(&raw).unwrap();
        assert_eq!(p.to_raw(), raw);
        p.index = 9;
        p.modules[0].model = 3; // Boost RC (4 params)
        p.modules[0].on = false;
        p.modules[0].params[3] = 77;
        p.name = "Lead ünicode name too long".into();
        let r = p.to_raw();
        assert_eq!((r[0], r[8], r[12], r[25], r[29]), (9, 0x62, 0x43, 4, 77));
        assert_eq!(&r[94..110], b"Lead ?nicode nam");
        assert_eq!(Preset::from_packed(&p.to_packed()).unwrap().to_raw(), r);
    }
}
