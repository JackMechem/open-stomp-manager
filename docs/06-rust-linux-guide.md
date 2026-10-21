# 06 — Building the Linux app in Rust

A practical plan for a Rust editor that does what Stomp Manager does (minus NAM/IR conversion, for now).

## 0. Sanity-check the device on Linux first (no code)

```sh
lsusb | grep 3962                       # 3962:2000 SORAN JME STOMPBOX
amidi -l                                # e.g.  IO  hw:2,0,0  JME STOMPBOX MIDI 1
# version request -> expect F0 00 21 7A 10 56 31 2E 31 2E 31 ... (ascii "V1.1.1")
amidi -p hw:2,0,0 -S 'F0 00 21 7A 00 F7' -d -t 1
# watch what the pedal reports while you turn knobs
amidi -p hw:2,0,0 -d
# switch to preset P05 (index 4) — channel 2 program change
amidi -p hw:2,0,0 -S 'C1 04'
# turn the amp off / on   (head CC 0x01, bit6 = off; keep the current model in the low bits!)
amidi -p hw:2,0,0 -S 'B1 01 40'   ;   amidi -p hw:2,0,0 -S 'B1 01 00'
```

If those work, everything in this repo applies unchanged on Linux.

## 1. Crates

| Need | Crate | Notes |
|---|---|---|
| MIDI I/O | [`midir`](https://crates.io/crates/midir) | ALSA backend on Linux; delivers SysEx whole (enable `ignore(Ignore::None)`) |
| (alt) raw ALSA | [`alsa`](https://crates.io/crates/alsa) | rawmidi if you want full control over SysEx buffering |
| Errors | `thiserror` / `anyhow` | |
| Async / channels | `crossbeam-channel` or `tokio` | midir's input callback runs on its own thread |
| GUI | `egui`/`eframe` (fastest to build), `iced`, or `slint` | |
| Serialization (preset export) | `serde`, `serde_json` | |

Find the port by name: match `"JME"` (the official app uses the regexes `.*JME.*` and `.*SORAN AMPEY.*`).

## 2. Suggested crate layout

```
stomp/
├── stomp-proto/        # no I/O: pure encode/decode, fully unit-testable
│   ├── cc.rs           # CC numbers, module table, model/param catalogue (docs/02)
│   ├── sysex.rs        # frame build/parse, Cmd/Dir enums, notifications (docs/03)
│   ├── pack.rs         # 8-in-7 bitstream pack/unpack
│   ├── preset.rs       # Preset struct <-> 110-byte record (docs/04)
│   ├── settings.rs     # SystemSettings <-> 17-byte record
│   └── catalog.rs      # model names / param names / enums
├── stomp-device/       # midir connection, request/response engine, state sync
└── stomp-gui/          # egui front end
```

Keep `stomp-proto` free of I/O so the captured logs in `captures/logs/` can serve as test vectors.

## 3. Core protocol code

### 7-bit packing

```rust
/// 8-bit struct bytes -> 7-bit MIDI bytes (little-endian bitstream, LSB first).
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
    if nbits > 0 { out.push((acc & 0x7F) as u8); }
    out
}

/// 7-bit MIDI bytes -> 8-bit struct bytes. Trailing partial byte is dropped.
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
```

Unit test: unpacking the captured 126-byte preset bodies must give 110 bytes with the name at 94..110, and
`pack(unpack(x)) == x` (modulo the final partial byte).

### Frames

```rust
pub const MFR: [u8; 3] = [0x00, 0x21, 0x7A];
pub const UID: u8 = 0x70;

#[repr(u8)] #[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir { Read = 0, Write = 1, Reply = 2, Ack = 3 }

#[repr(u8)] #[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    IrSlot = 0x09, Preset = 0x0B, EditBuffer = 0x0C, IrData = 0x10, PresetName = 0x12,
    System = 0x14, CurState = 0x15, Loop = 0x18, Drum = 0x19, UserAmp = 0x58, Tuner = 0x6F,
    Notify = 0x7E,
}

pub fn frame(cmd: Cmd, dir: Dir, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(payload.len() + 8);
    v.push(0xF0); v.extend_from_slice(&MFR); v.push(UID);
    v.push(cmd as u8); v.push(dir as u8);
    debug_assert!(payload.iter().all(|b| *b < 0x80));
    v.extend_from_slice(payload);
    v.push(0xF7);
    v
}

pub enum Incoming<'a> {
    Nep { cmd: u8, dir: u8, payload: &'a [u8] },
    Notify { what: u8, payload: &'a [u8] },          // F0 00 21 7A 70 7E 02 <what> 02 <payload> F7
    Short { kind: u8, payload: &'a [u8] },           // F0 00 21 7A <kind> ... F7 (0x10 version, 0x60 name)
    Cc { ch: u8, cc: u8, val: u8 },
    Pc { ch: u8, prog: u8 },
}
```

### Live control (no SysEx needed)

```rust
fn cc(cc: u8, v: u8) -> [u8; 3] { [0xB1, cc, v] }            // channel 2
fn program(index: u8) -> [u8; 2] { [0xC1, index] }           // P01 == 0
fn head(model: u8, on: bool) -> u8 { (if on { 0 } else { 0x40 }) | (model & 0x3F) }

// e.g. select Klone (model 8) on OD/FX and switch it on, set DRIVE (2nd param) to 66
out.send(&cc(0x00, head(8, true)))?;
out.send(&cc(0x0D, 66))?;
```

### Request/response engine

The pedal processes one NEP request at a time; the official app serialises with a mutex and waits for the reply.

```rust
// pseudo-code
fn request(&self, cmd: Cmd, dir: Dir, payload: &[u8]) -> Result<Vec<u8>> {
    let _guard = self.lock.lock();
    self.out.send(&frame(cmd, dir, payload))?;
    let want = if dir == Dir::Read { 2 } else { 3 };
    // wait on the channel fed by the midir callback for (cmd, want), timeout 1s,
    // while routing CC/PC/notifications to the state updater in the meantime
    self.rx.recv_matching(|m| m.cmd == cmd as u8 && m.dir == want, Duration::from_secs(1))
}
```

### Connect sequence

Mirror the official app ([03](03-sysex-protocol.md#startup-handshake-what-the-official-app-does-on-connect-320-ms-total)):
version → 0x14 → 0x15 → 0x19 → 0x09 ×10 → product name → 0x58 ×10 → 0x0B ×99 → 0x0C → 0x18.
Then keep state in sync from:

* ch-1 CCs/PCs from the pedal (apply to the edit buffer / current preset),
* `7E` notifications (re-read the indicated object if the payload isn't inline),
* your own writes (the pedal does not echo them).

## 4. Data model

```rust
pub struct Module { pub on: bool, pub model: u8, pub params: [u8; 12] /* only first N used */ }

pub struct Preset {
    pub index: u8,
    pub gate: Module, pub odfx: Module, pub amp: Module, pub cab: Module,
    pub eq: Module, pub modu: Module, pub delay: Module, pub reverb: Module,
    pub volume: u8,
    pub chain: [u8; 7],
    pub name: String,          // <= 16 ASCII
    raw: [u8; 110],            // keep the original; patch known fields; write back
}
```

**Always round-trip through `raw`.** Several bytes are still unknown, so decode into `raw`, change only the fields you
understand, and write `raw` back. The official app does the same for system settings.

Writing a preset to slot N: build the 110-byte record with `raw[0] = N`, `pack()` it, and send `70 0B 01 <packed>`.
To rename, also send `70 12 01 <slot> <16 ASCII bytes>` (see [03](03-sysex-protocol.md#rename--save-as-app-save-dialog-changing-name-and-number)).

## 5. Testing without the pedal

* `tools/decode/analyze.py` and `tools/decode/live.py` show the expected traffic for every UI action.
* Write `#[test]`s that parse `captures/logs/01-main-session.log` (one line per packet,
  `<ms> <TX|RX> <len> [<port>] <hex…>`), reassemble SysEx, and check your decoder against it:
  all 99 presets decode with sane names, the settings decode to `trim=10, bass/mid/treble=50`, etc.
* A virtual ALSA port (`snd-virmidi`) plus a tiny fake-pedal that answers from the captures lets you develop the GUI
  offline.

## 6. Roadmap suggestion

1. CLI (`stompctl`): `info`, `presets list`, `preset get N --json`, `preset set N file.json`, `cc`, `monitor`
   (port of `live.py`).
2. Live editor GUI: preset list, chain view, module/model/param editing via CCs, tuner view, looper/drum panels.
3. Settings page (0x14 read-modify-write).
4. Backup/restore all 99 presets to JSON (the format is yours to define).
5. IR upload (after finishing the transform research in [05](05-uploads-nam-ir.md)).
6. NAM conversion (hardest: needs the `imagelib` algorithm).
