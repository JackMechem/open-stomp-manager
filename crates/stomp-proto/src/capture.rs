//! Reader for the capture logs in `captures/logs/` (docs/07):
//! `<unix ms> <TX|RX> <len> [<endpoint>] <hex…>` and `# MARK <ms> <text>`.

use crate::midi::Assembler;
use crate::state::Origin;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Mark { ms: u64, text: String },
    Message { ms: u64, origin: Origin, bytes: Vec<u8> },
}

/// Parse a whole capture log into complete messages, reassembling split SysEx.
pub fn parse(text: &str) -> Vec<Entry> {
    let (mut tx, mut rx) = (Assembler::default(), Assembler::default());
    let mut out = vec![];
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# MARK ") {
            let (ms, text) = rest.split_once(' ').unwrap_or((rest, ""));
            out.push(Entry::Mark { ms: ms.parse().unwrap_or(0), text: text.to_owned() });
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(3, ' ');
        let (Some(ms), Some(dir), Some(rest)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let (origin, asm) = match dir {
            "TX" => (Origin::Host, &mut tx),
            "RX" | "MON" => (Origin::Pedal, &mut rx),
            _ => continue,
        };
        let Some(hex) = rest.split_once(']').map(|(_, h)| h) else { continue };
        let data: Vec<u8> = hex.split_whitespace().filter_map(|h| u8::from_str_radix(h, 16).ok()).collect();
        let ms = ms.parse().unwrap_or(0);
        for bytes in asm.push(&data) {
            out.push(Entry::Message { ms, origin, bytes });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Module;
    use crate::state::{Category, DeviceState};

    fn main_session() -> Vec<Entry> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../captures/logs/01-main-session.log");
        parse(&std::fs::read_to_string(path).expect("capture log"))
    }

    #[test]
    fn replay_main_session() {
        let mut st = DeviceState::default();
        let mut unknown = 0;
        let mut handshake_done = false;
        for e in main_session() {
            let Entry::Message { origin, bytes, .. } = e else { continue };
            let ev = st.apply(&bytes, origin);
            if ev.category == Category::Unknown {
                unknown += 1;
            }
            if !handshake_done && st.presets.iter().all(|p| p.is_some()) {
                handshake_done = true;
                assert_eq!(st.firmware.as_deref(), Some("V1.1.1"));
                assert_eq!(st.model_id.as_deref(), Some("JME-01"));
                assert_eq!(st.product.as_deref(), Some("JME STOMPBOX"));
                for (i, p) in st.presets.iter().enumerate() {
                    let p = p.as_ref().unwrap();
                    assert_eq!(p.index as usize, i);
                    assert!(!p.name.is_empty(), "preset {i} has a name");
                    assert!(p.name.chars().all(|c| c.is_ascii_graphic() || c == ' '), "{:?}", p.name);
                    assert!(p.modules[Module::Amp.index()].model < 40);
                }
                // the edit buffer after the handshake is P01
                assert!(st.live_known);
            }
        }
        assert!(handshake_done);
        assert!(unknown < 10, "{unknown} unknown messages");
        let s = st.settings.expect("settings");
        assert!(s.input_trim <= 20);
    }

    #[test]
    fn preset_switch_applies_cached_preset() {
        let mut st = DeviceState::default();
        for e in main_session() {
            if let Entry::Message { origin, bytes, .. } = e {
                st.apply(&bytes, origin);
                if bytes[0] & 0xF0 == 0xC0 && st.presets.iter().all(|p| p.is_some()) {
                    let i = bytes[1];
                    assert_eq!(st.current, Some(i));
                    assert_eq!(st.live.name, st.presets[i as usize].as_ref().unwrap().name);
                }
            }
        }
    }
}
