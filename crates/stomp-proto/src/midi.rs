//! MIDI message parsing, SysEx framing and stream reassembly (docs/02, docs/03).

pub const MFR: [u8; 3] = [0x00, 0x21, 0x7A];
pub const UID: u8 = 0x70;

/// Pedal reports on channel 1, the host commands on channel 2.
pub const PEDAL_CH: u8 = 0;
pub const HOST_CH: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cmd {
    IrSlot = 0x09,
    Preset = 0x0B,
    EditBuffer = 0x0C,
    IrData = 0x10,
    PresetName = 0x12,
    System = 0x14,
    CurState = 0x15,
    Loop = 0x18,
    Drum = 0x19,
    UserAmp = 0x58,
    Tuner = 0x6F,
    Notify = 0x7E,
}

impl Cmd {
    pub fn from_u8(b: u8) -> Option<Cmd> {
        use Cmd::*;
        Some(match b {
            0x09 => IrSlot,
            0x0B => Preset,
            0x0C => EditBuffer,
            0x10 => IrData,
            0x12 => PresetName,
            0x14 => System,
            0x15 => CurState,
            0x18 => Loop,
            0x19 => Drum,
            0x58 => UserAmp,
            0x6F => Tuner,
            0x7E => Notify,
            _ => return None,
        })
    }

    pub fn label(b: u8) -> &'static str {
        match b {
            0x09 => "user IR slot",
            0x0B => "preset",
            0x0C => "edit buffer",
            0x0D => "module link",
            0x10 => "IR data",
            0x12 => "preset name",
            0x14 => "system settings",
            0x15 => "current state",
            0x16 => "delete IR",
            0x18 => "looper",
            0x19 => "drum",
            0x1B => "bluetooth",
            0x58 => "user amp (NAM)",
            0x6F => "tuner",
            0x7E => "notify",
            _ => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    Read = 0,
    Write = 1,
    Reply = 2,
    Ack = 3,
}

impl Dir {
    pub fn label(b: u8) -> &'static str {
        match b {
            0 => "read",
            1 => "write",
            2 => "reply",
            3 => "ack",
            _ => "?",
        }
    }
}

/// Build a NEP frame `F0 00 21 7A 70 <cmd> <dir> <payload> F7`.
pub fn frame(cmd: Cmd, dir: Dir, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(payload.len() + 8);
    v.push(0xF0);
    v.extend_from_slice(&MFR);
    v.push(UID);
    v.push(cmd as u8);
    v.push(dir as u8);
    debug_assert!(payload.iter().all(|b| *b < 0x80));
    v.extend_from_slice(payload);
    v.push(0xF7);
    v
}

/// Short identification request (`0x00` version, `0x60` product name).
pub fn short_request(kind: u8) -> Vec<u8> {
    vec![0xF0, MFR[0], MFR[1], MFR[2], kind, 0xF7]
}

/// Read requests used by the official app's handshake.
pub mod req {
    use super::*;
    use crate::pack::pack;

    pub fn version() -> Vec<u8> {
        short_request(0x00)
    }
    pub fn product() -> Vec<u8> {
        short_request(0x60)
    }
    pub fn settings() -> Vec<u8> {
        frame(Cmd::System, Dir::Read, &[])
    }
    pub fn cur_state() -> Vec<u8> {
        frame(Cmd::CurState, Dir::Read, &[])
    }
    pub fn drum() -> Vec<u8> {
        frame(Cmd::Drum, Dir::Read, &[])
    }
    pub fn looper() -> Vec<u8> {
        frame(Cmd::Loop, Dir::Read, &[])
    }
    pub fn tuner() -> Vec<u8> {
        frame(Cmd::Tuner, Dir::Read, &[])
    }
    pub fn edit_buffer() -> Vec<u8> {
        frame(Cmd::EditBuffer, Dir::Read, &[])
    }
    pub fn ir_slot(slot: u8) -> Vec<u8> {
        frame(Cmd::IrSlot, Dir::Read, &[slot])
    }
    pub fn preset(index: u8) -> Vec<u8> {
        let mut p = vec![0u8; 14];
        p[0] = index & 0x7F;
        frame(Cmd::Preset, Dir::Read, &p)
    }
    pub fn user_amp(slot: u8) -> Vec<u8> {
        let mut hdr = [0u8; 12];
        hdr[1] = slot;
        hdr[2] = 4; // sub-command: read slot info
        frame(Cmd::UserAmp, Dir::Read, &pack(&hdr))
    }
}

/// Host → pedal commands (docs/02 live control, docs/03 writes). Live changes are channel-2
/// CCs/PCs; the pedal does not echo them, so apply them to the local model too.
pub mod host {
    use super::*;
    use crate::catalog::Module;
    use crate::records::{Preset, Settings, name_bytes};

    pub fn cc(cc: u8, val: u8) -> Vec<u8> {
        vec![0xB0 | HOST_CH, cc & 0x7F, val & 0x7F]
    }

    /// Select preset `index` (0-based, P01 = 0).
    pub fn program(index: u8) -> Vec<u8> {
        vec![0xC0 | HOST_CH, index & 0x7F]
    }

    /// Model + on/off of an effect module.
    pub fn head(m: Module, model: u8, on: bool) -> Option<Vec<u8>> {
        Some(cc(m.head_cc()?, (if on { 0 } else { 0x40 }) | (model & 0x3F)))
    }

    /// A module parameter by slot (app display order).
    pub fn param(m: Module, slot: usize, val: u8) -> Option<Vec<u8>> {
        let (first, n) = m.param_ccs()?;
        (slot < n as usize).then(|| cc(first + slot as u8, val))
    }

    pub fn volume(v: u8) -> Vec<u8> {
        cc(0x3A, v.min(100))
    }

    /// Make the pedal display jump to a module's page.
    pub fn page(m: Module) -> Vec<u8> {
        cc(0x3B, m as u8)
    }

    pub const LOOP_REC_PLAY_DUB: u8 = 0x01;
    pub const LOOP_STOP: u8 = 0x02;
    pub const LOOP_CLEAR: u8 = 0x04;
    pub const LOOP_UNDO_REDO: u8 = 0x08;

    pub fn looper_button(b: u8) -> Vec<u8> {
        cc(0x44, b)
    }

    /// Set the edit buffer's name. The official app sends the slot being edited first
    /// (the active preset, even when saving to another slot).
    pub fn preset_name(source_slot: u8, name: &str) -> Vec<u8> {
        let mut p = Vec::with_capacity(17);
        p.push(source_slot & 0x7F);
        p.extend(name_bytes(name).iter().map(|b| b & 0x7F));
        frame(Cmd::PresetName, Dir::Write, &p)
    }

    /// Store a whole preset record in slot `preset.index`.
    pub fn preset_write(preset: &Preset) -> Vec<u8> {
        frame(Cmd::Preset, Dir::Write, &preset.to_packed())
    }

    /// The official app's save sequence: name, record, then select the saved slot.
    pub fn save_preset(source_slot: u8, preset: &Preset) -> Vec<Vec<u8>> {
        vec![preset_name(source_slot, &preset.name), preset_write(preset), program(preset.index)]
    }

    pub fn settings_write(s: &Settings) -> Vec<u8> {
        frame(Cmd::System, Dir::Write, &s.to_packed())
    }
}

/// One parsed MIDI message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    Cc { ch: u8, cc: u8, val: u8 },
    Pc { ch: u8, prog: u8 },
    /// `F0 00 21 7A 70 <cmd> <dir> <payload> F7`
    Nep { cmd: u8, dir: u8, payload: Vec<u8> },
    /// `F0 00 21 7A 70 7E 02 <what> 02 <payload> F7`
    Notify { what: u8, payload: Vec<u8> },
    /// `F0 00 21 7A <kind> <payload> F7` (`00`/`10` version, `60` product)
    Short { kind: u8, payload: Vec<u8> },
    /// Anything else (other manufacturers, realtime, note on/off...).
    Other(Vec<u8>),
}

impl Msg {
    pub fn parse(b: &[u8]) -> Msg {
        let Some(&s) = b.first() else {
            return Msg::Other(vec![]);
        };
        match s & 0xF0 {
            0xB0 if b.len() >= 3 => return Msg::Cc { ch: s & 0x0F, cc: b[1], val: b[2] },
            0xC0 if b.len() >= 2 => return Msg::Pc { ch: s & 0x0F, prog: b[1] },
            _ => {}
        }
        if s != 0xF0 || b.len() < 6 || b[1..4] != MFR {
            return Msg::Other(b.to_vec());
        }
        let end = if b.last() == Some(&0xF7) { b.len() - 1 } else { b.len() };
        let body = &b[4..end];
        if body[0] != UID || body.len() < 3 {
            return Msg::Short { kind: body[0], payload: body[1..].to_vec() };
        }
        let (cmd, dir, payload) = (body[1], body[2], &body[3..]);
        if cmd == Cmd::Notify as u8 && payload.len() >= 2 {
            return Msg::Notify { what: payload[0], payload: payload[2..].to_vec() };
        }
        Msg::Nep { cmd, dir, payload: payload.to_vec() }
    }
}

/// Reassembles a raw byte stream (possibly split SysEx) into whole messages.
#[derive(Default)]
pub struct Assembler {
    buf: Vec<u8>,
}

impl Assembler {
    pub fn push(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        self.buf.extend_from_slice(data);
        let mut out = vec![];
        loop {
            // drop stray data bytes in front of a status byte
            let start = self.buf.iter().position(|b| *b >= 0x80).unwrap_or(self.buf.len());
            self.buf.drain(..start);
            let Some(&s) = self.buf.first() else { break };
            let len = if s == 0xF0 {
                match self.buf.iter().position(|b| *b == 0xF7) {
                    Some(e) => e + 1,
                    None => break,
                }
            } else if s >= 0xF8 {
                1
            } else {
                let need = match s & 0xF0 {
                    0xC0 | 0xD0 => 2,
                    0xF0 => 1,
                    _ => 3,
                };
                if self.buf.len() < need {
                    break;
                }
                // a status byte inside the data means a truncated message
                match self.buf[1..need].iter().position(|b| *b >= 0x80) {
                    Some(p) => p + 1,
                    None => need,
                }
            };
            out.push(self.buf.drain(..len).collect());
        }
        out
    }
}

pub fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 3);
    for (i, x) in b.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{x:02X}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic() {
        assert_eq!(Msg::parse(&[0xB0, 0x12, 0x40]), Msg::Cc { ch: 0, cc: 0x12, val: 0x40 });
        assert_eq!(Msg::parse(&[0xC1, 4]), Msg::Pc { ch: 1, prog: 4 });
        assert_eq!(
            Msg::parse(&[0xF0, 0, 0x21, 0x7A, 0x70, 0x7E, 0x02, 0x19, 0x02, 1, 2, 3, 0, 0, 0x78, 0xF7]),
            Msg::Notify { what: 0x19, payload: vec![1, 2, 3, 0, 0, 0x78] }
        );
        assert_eq!(
            Msg::parse(&req::settings()),
            Msg::Nep { cmd: 0x14, dir: 0, payload: vec![] }
        );
    }

    #[test]
    fn assembler_splits_and_joins() {
        let mut a = Assembler::default();
        assert!(a.push(&[0xF0, 0x00, 0x21]).is_empty());
        let m = a.push(&[0x7A, 0x00, 0xF7, 0xB0, 0x01, 0x08, 0xC0]);
        assert_eq!(m, vec![vec![0xF0, 0, 0x21, 0x7A, 0, 0xF7], vec![0xB0, 1, 8]]);
        assert_eq!(a.push(&[0x05]), vec![vec![0xC0, 5]]);
    }

    #[test]
    fn host_commands() {
        use crate::catalog::Module;
        assert_eq!(host::head(Module::Amp, 8, false), Some(vec![0xB1, 0x01, 0x48]));
        assert_eq!(host::param(Module::OdFx, 1, 66), Some(vec![0xB1, 0x0D, 66]));
        assert_eq!(host::param(Module::Gate, 2, 1), None);
        assert_eq!(host::head(Module::Loop, 0, true), None);
        assert_eq!(host::program(84), vec![0xC1, 0x54]);
        assert_eq!(host::page(Module::Gate), vec![0xB1, 0x3B, 0x06]);
        // captured rename (source slot P69 = 0x44, docs/03)
        assert_eq!(
            hex(&host::preset_name(0x44, "4Ooz!")),
            "F0 00 21 7A 70 12 01 44 34 4F 6F 7A 21 00 00 00 00 00 00 00 00 00 00 00 F7"
        );
    }

    #[test]
    fn user_amp_request_matches_capture() {
        assert_eq!(
            hex(&req::user_amp(1)),
            "F0 00 21 7A 70 58 00 00 02 10 00 00 00 00 00 00 00 00 00 00 00 F7"
        );
    }
}
