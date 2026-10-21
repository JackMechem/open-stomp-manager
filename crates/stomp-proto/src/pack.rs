//! 8-in-7 packing: 8-bit structs carried as a little-endian 7-bit bitstream (docs/03).

/// 8-bit struct bytes -> 7-bit MIDI bytes (LSB first).
pub fn pack(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 8 / 7 + 1);
    let (mut acc, mut nbits) = (0u32, 0u32);
    for &b in data {
        acc |= (b as u32) << nbits;
        nbits += 8;
        while nbits >= 7 {
            out.push((acc & 0x7F) as u8);
            acc >>= 7;
            nbits -= 7;
        }
    }
    if nbits > 0 {
        out.push((acc & 0x7F) as u8);
    }
    out
}

/// 7-bit MIDI bytes -> 8-bit struct bytes. A trailing partial byte is dropped.
pub fn unpack(midi: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(midi.len() * 7 / 8);
    let (mut acc, mut nbits) = (0u32, 0u32);
    for &b in midi {
        acc |= ((b & 0x7F) as u32) << nbits;
        nbits += 7;
        if nbits >= 8 {
            out.push((acc & 0xFF) as u8);
            acc >>= 8;
            nbits -= 8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let data: Vec<u8> = (0..110u32).map(|i| (i * 37 + 11) as u8).collect();
        let packed = pack(&data);
        assert_eq!(packed.len(), 126);
        assert!(packed.iter().all(|b| *b < 0x80));
        assert_eq!(unpack(&packed), data);
    }

    #[test]
    fn nam_slot_header() {
        // docs/03: slot 1, sub-command 4 packs to MIDI `00 02 10`
        assert_eq!(&pack(&[0, 1, 4])[..3], &[0x00, 0x02, 0x10]);
    }
}
