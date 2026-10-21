//! Data records carried in SysEx (docs/04).

use crate::catalog::Module;
use crate::pack::unpack;

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
    }
}
