# 04 — Data structures

Offsets are into the **unpacked** struct (after the 8-in-7 decoding in
[03](03-sysex-protocol.md#2-packed-8-in-7-bitstream-packed)) unless the record is marked *plain*.

## Preset — SysEx `0x0B` (by index) and `0x0C` (edit buffer)

126 packed MIDI bytes → **110 struct bytes**. Layout derived by correlating all 99 presets read at startup with the
CC values the pedal sent after program changes (`tools/decode/preset_layout.py`). Offsets marked ✔ were confirmed by
that correlation; ✓ by constant/structural evidence; ? unknown.

| Offset | Size | Field | Notes |
|---|---|---|---|
| 0 | 1 | preset index | 0-based (P01 = 0). `0x0C` reads return 0 here |
| 1–7 | 7 | 0 | reserved / header |
| 8 | 1 | `0x62` (98) | constant in all presets (number of presets − 1? version?) ? |
| 9–11 | 3 | 0 | |
| 12 | 1 | OD/FX head | = CC `0x00` (bit 6 = off, bits 0–5 = model) ✔ |
| 13 | 1 | AMP head | = CC `0x01` ✔ |
| 14 | 1 | CAB head | = CC `0x02` ✔ |
| 15 | 1 | MOD head | = CC `0x03` ✔ |
| 16 | 1 | DELAY head | = CC `0x04` ✔ |
| 17 | 1 | REVERB head | = CC `0x05` ✔ |
| 18 | 1 | GATE head | = CC `0x06` ✔ |
| 19 | 1 | EQ head | = CC `0x07` ✔ |
| 20 | 1 | GATE param count (2) | ✓ |
| 21–22 | 2 | GATE THRESHOLD, DECAY | = CC `0x08`, `0x09` ✔ |
| 23–24 | 2 | pad | |
| 25 | 1 | OD/FX param count (2–4) | ✓ |
| 26–29 | 4 | OD/FX params 1–4 | = CC `0x0C`–`0x0F` ✔ |
| 30–31 | 2 | pad | |
| 32 | 1 | AMP param count (6) | ✓ |
| 33–38 | 6 | AMP GAIN…MASTER | = CC `0x12`–`0x17` ✔ |
| 39–40 | 2 | pad | |
| 41 | 1 | CAB param count (5) | ✓ |
| 42–46 | 5 | CAB MIC, POSITION, LOW CUT, HIGH CUT, VOLUME | = CC `0x1A`–`0x1E` ✔ |
| 47 | 1 | pad | |
| 48 | 1 | EQ param count (6 or 12) | ✓ |
| 49–60 | 12 | EQ bands | = CC `0x20`–`0x2B` ✔ |
| 61 | 1 | MOD param count (1–4) | ✓ |
| 62–65 | 4 | MOD params | = CC `0x2C`–`0x2F` ✔ |
| 66 | 1 | DELAY param count (3–4) | ✓ |
| 67–70 | 4 | DELAY params | = CC `0x30`–`0x33` ✓ (by pattern) |
| 71–72 | 2 | pad | |
| 73 | 1 | REVERB param count (2–4) | ✓ |
| 74–77 | 4 | REVERB params | = CC `0x36`–`0x39` ✔ |
| 78 | 1 | VOL | = CC `0x3A` (80 in every factory preset) ✓ |
| 79–81 | 3 | 0 | ? (BPM / flags?) |
| 82 | 1 | 6 | constant ? (chain length − 1?) |
| 83–89 | 7 | signal-chain order | module indices, e.g. `00 01 02 07 03 04 05` = OD/FX, AMP, CAB, EQ, MOD, DLY, RVB. Varies per preset ✓ |
| 90–93 | 4 | 0 | ? |
| 94–109 | 16 | name | ASCII, zero padded ✔ |

Notes:

* The param-count bytes equal the selected model's number of parameters; the official app keeps them consistent
  with the model. Unused param bytes keep stale values.
* **Packing:** the whole 126-byte body (from the index byte onwards) is one packed stream.
* The static analysis found a related layout from a sibling product (`NBA150PPresetStruct`: 11 heads, `[model, p…]`
  arrays, `userName[16]`…). The Dual Stomp Lite layout above is similar in spirit but **not** identical; trust the
  table above.

## System settings — SysEx `0x14`

20 packed bytes → 17 struct bytes. Factory values: `00 0A 1E 0A 32 32 32 32 32 01 01 3C B4 00 00 00 00`.

| Offset | Field | Values | Status |
|---|---|---|---|
| 0 | ? | 0 | |
| 1 | **Input Trim** | 0–20 → −10…+10 dB (10 = 0 dB) | confirmed |
| 2 | ? | 30 | |
| 3 | ? | 10 | |
| 4 | **Global EQ Bass** | 0–100 (50 centre) | confirmed |
| 5 | **Global EQ Mid** | 0–100 | confirmed |
| 6 | **Global EQ Treble** | 0–100 | confirmed |
| 7, 8 | ? | 50, 50 | |
| 9 | **Wireless** | 0 off, 1 on | confirmed |
| 10 | **LoopBack** | 0 off, 1 on | confirmed |
| 11 | ? | 60 | (energy-saving timeout? *guess*) |
| 12 | ? | 180 | |
| 13–16 | ? | 0 | |

Always read, modify, write back, like the official app. Global EQ changed **on the pedal** is not reported over USB.

## User IR slot info — SysEx `0x09`

```
read:   70 09 00 <slot>                      (slot 0..9)
reply:  70 09 02 <slot> <packed record>      (43 MIDI bytes total payload)
write:  70 09 01 <slot> <packed record>
```

Unpacked record: `[0..3] = 4 bytes (CRC?)`, `[4..] = name` (ASCII, zero padded; field is probably 32 bytes, per the
static `NBA150PIrNameStruct`). Empty slot: `00 00 00 00 "--------"`. After an IR upload the app wrote
`01 00 00 00 "Delay And Reverb"`, so the 4-byte field was 1 there. Whether it is a real CRC is open.

## User AMP (NAM) slot info — SysEx `0x58`, sub-command "read info"

```
read:   70 58 00 <packed: 00 <slot> 04 00 ...>
reply:  70 58 02 <packed, 160 MIDI bytes>
```

Unpacked reply: `[0]=0, [1]=slot, [2]=4 (sub-cmd), [8]=0x80, [12..]= name "USER AMP 01"…`, zero padded. More fields
follow (a `01` at offset ~45). Full upload protocol: [05](05-uploads-nam-ir.md).

## Current state — SysEx `0x15` (plain)

Reply payload: `50 01 00 00 00 00 00 00`. `0x50` = 80 (likely VOL, *guess*), `01` unknown.

## Looper — SysEx `0x18` (plain)

Payload: `01 32 32 <state> 00 …`. Byte 0 = 1 (?), byte 1 = REC level (50), byte 2 = PLAYBACK level (50),
byte 3 = looper state (same encoding as CC `0x44`). The app reads it when the loop panel opens and writes the first
5 bytes back unchanged.

## Drum — SysEx `0x19` (plain)

Payload (read reply, 8 bytes): `00 00 32 00 00 78 00 00`

| Byte | Field |
|---|---|
| 0 | ? (0) |
| 1 | pattern (flat, `style*10+pattern`, = CC `0x40`) |
| 2 | level (= CC `0x41`) |
| 3 | ? (0) |
| 4–5 | **BPM**, 14-bit: `(b4 << 7) \| b5` (`00 78` = 120) |

The notification form is `70 7E 02 19 02 <same 6 bytes>`.

## Tuner — SysEx `0x6F` (plain)

Payload: `<status> 00 0A 00 00`. status 1 = tuner active, 0 = off. `0x0A` is probably the A4 reference
(maybe 430 + 10 = 440 Hz, *guess*). Static field names: status, mode, pitch, mute. Live pitch data comes as CCs
`0x45`/`0x46`/`0x48`.
