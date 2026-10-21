# 03 — SysEx protocol

SysEx is used for everything that isn't a single live parameter: identification, reading/writing whole presets,
names, system settings, looper/drum/tuner state, and uploads (NAM/IR). Live parameter changes use CCs
([02](02-midi-cc-reference.md)).

Confidence: **confirmed** = seen on the wire and consistent; *inferred* = consistent with the captures and the static
analysis but not independently proven.

## Manufacturer ID and frame types

All vendor SysEx uses the 3-byte manufacturer ID **`00 21 7A`**. Two frame shapes exist:

### A. Short identification frames (no command byte)

| Request | Reply | Meaning |
|---|---|---|
| `F0 00 21 7A 00 F7` | `F0 00 21 7A 10 <32 B ascii version> <ascii model> 00 00 F7` | Version: `"V1.1.1"` (zero-padded to 32 bytes) then `"JME-01"` |
| `F0 00 21 7A 60 F7` | `F0 00 21 7A 60 <24 B ascii product name> 0A 00 00 00 00 00 00 00 F7` | Product: `"JME STOMPBOX"`, then `0x0A` (10, meaning unknown; maybe the number of user slots) |

The pedal also answers the *universal* identity request `F0 7E 7F 06 01 F7` with `F0 00 21 7A 10 F7`, which is not a
standard identity reply. Two unsolicited `F0 00 21 7A 10 F7` messages also appeared mid-session; meaning unknown.

### B. NEP command frames (everything else)

```
F0  00 21 7A  70  <cmd>  <dir>  <payload ...>  F7
    \_mfr__/  uid  msgId  direction
```

* `uid` is always `0x70`.
* `cmd` (msgId) selects the data object (table below).
* `dir`:

| dir | Meaning | Sent by |
|---|---|---|
| `00` | read request | host |
| `01` | write | host |
| `02` | read reply (data) | pedal |
| `03` | write acknowledge | pedal |
| `7E` | used as `cmd` = **notification** (see below) | pedal |

* **No checksum** observed.
* The official app is strictly **request → response**. It waits for the `02`/`03` before sending the next frame. Copy
  that behaviour; a 1 s timeout per request is sensible (the static analysis shows a 1000 ms timeout and a mutex in
  the sender).
* Payload bytes are always ≤ 0x7F (SysEx rule). How 8-bit data is carried depends on the command; see
  [Payload encodings](#payload-encodings).

### Notifications

When state changes on the pedal side for something that has no CC, the pedal sends:

```
F0 00 21 7A 70 7E 02 <cmd> 02 <payload...> F7
```

Seen:

| `<cmd>` | Trigger | Payload | What the app does next |
|---|---|---|---|
| `0x6F` | tuner opened/closed on pedal | `00 00 00 00 00 00` | reads `0x6F`, writes it back unchanged |
| `0x19` | tap tempo, BPM knob, drum pattern change, loop recorded | 6 bytes, same layout as the drum record (BPM at bytes 4–5) | nothing (data is inline) |
| `0x0B` | preset saved on the pedal | `<index> 00 00 00 00 00` | reads preset `<index>` (`70 0B 00 <index> 00*13`) |

## Command table

| cmd | Object | Read (dir 0) | Write (dir 1) | Encoding | Status |
|---|---|---|---|---|---|
| `0x09` | User IR slot info (index, CRC, name) | `70 09 00 <slot>` | `70 09 01 <slot> <packed>` | slot byte + packed | confirmed |
| `0x0B` | Preset (by index) | `70 0B 00 <idx> 00*13` | `70 0B 01 <packed 126 B>` | packed (whole body) | confirmed |
| `0x0C` | Current / edit-buffer preset | `70 0C 00` | ? | packed, same layout as `0x0B` (index field 0) | read confirmed |
| `0x0D` | Module link / chain *(static)* | | | | not seen |
| `0x10` | **User IR upload** (data chunks) | | `70 10 01 <packed chunk>` | packed | confirmed |
| `0x12` | Preset name | | `70 12 01 <slot?> <16 B ASCII>` | **plain** ASCII | confirmed |
| `0x14` | System settings | `70 14 00` | `70 14 01 <packed 20 B>` | packed | confirmed |
| `0x15` | Current state | `70 15 00` | ? | plain | read confirmed |
| `0x16` | Delete IR *(static)* | | | | not seen |
| `0x18` | Looper settings/state | `70 18 00` | `70 18 01 <5 B>` | plain | confirmed |
| `0x19` | Drum | `70 19 00` | ? | plain | read confirmed |
| `0x1B` | Bluetooth params *(static)* | | | | not seen |
| `0x58` | **User AMP (NAM) slot**: info, data, delete | `70 58 00 <packed hdr>` | `70 58 01 <packed>` | packed | confirmed |
| `0x6F` | Tuner | `70 6F 00` | `70 6F 01 <5 B>` | plain | confirmed |

Write acks are short: `70 <cmd> 03` + a few bytes (often zeros, sometimes an echo of the request header).

### Startup handshake (what the official app does on connect, ~320 ms total)

```
F0 00 21 7A 00 F7                 version
70 14 00                          system settings
70 15 00                          current state
70 19 00                          drum
70 09 00 <0..9>       ×10         user IR slots
F0 00 21 7A 60 F7                 product name
70 58 00 <slot 0..9>  ×10         user AMP (NAM) slots
70 0B 00 00 …                     preset 0
70 0C 00                          edit buffer
70 18 00                          looper
70 0B 00 <1..98> …    ×98         presets 1..98   (99 presets total: P01..P99)
```

## Payload encodings

Three encodings are in use. Which one applies depends on the command.

### 1. Plain 7-bit bytes

Each byte is a value 0–127. 14-bit values are two bytes, **high 7 bits first**: `value = (hi << 7) | lo`.
Used by `0x15`, `0x18`, `0x19`, `0x6F`, the `0x12` name and notification payloads.

### 2. Packed 8-in-7 bitstream ("packed")

8-bit data packed into 7-bit MIDI bytes as one **little-endian bitstream**: every MIDI data byte contributes its 7
low bits, LSB first, and those bits are concatenated. Regroup into 8-bit bytes LSB first to get the struct. 126 MIDI
bytes → 110 struct bytes (`126*7/8`).

```python
def unpack(midi: bytes) -> bytes:          # tools/decode/unpack.py
    bits = "".join(format(b & 0x7F, "07b")[::-1] for b in midi)
    return bytes(int(bits[i:i+8][::-1], 2) for i in range(0, len(bits) - 7, 8))

def pack(data: bytes) -> bytes:
    bits = "".join(format(b, "08b")[::-1] for b in data)
    bits += "0" * (-len(bits) % 7)
    return bytes(int(bits[i:i+7][::-1], 2) for i in range(0, len(bits), 7))
```

In Rust, think of it as a bit writer/reader with a 7-bit output width (see [06](06-rust-linux-guide.md)).

For `0x0B`, `0x0C`, `0x14`, `0x58` and `0x10` the **entire body after `<dir>` is one packed stream**, including the
header fields. Evidence: the `0x58` slot reads use MIDI bytes `00 00`, `00 02`, `00 04`… for slots 0, 1, 2, which is
exactly how slot = 0, 1, 2 at unpacked byte 1 comes out after packing.

### 3. Mixed: plain index byte + packed record

`0x09` (IR slot info) = one plain byte (slot index), then a packed stream starting at the next MIDI byte.

### Plain text

`0x12` preset name: plain ASCII, one char per byte, zero padded to 16. (Inside packed records, names are just bytes
of the struct.)

## Per-command payloads

Struct layouts for `0x0B`/`0x0C` (preset), `0x14` (settings), `0x09`, `0x58`, `0x18`, `0x19`, `0x6F` are in
[04-data-structures.md](04-data-structures.md). Upload procedures for `0x58` (NAM) and `0x10` (IR) are in
[05-uploads-nam-ir.md](05-uploads-nam-ir.md).

## Captured write sequences

### Rename + save-as (app "save" dialog, changing name and number)

```
→ 70 12 01  44  34 4F 6F 7A 21 00…00      set name "4Ooz!" (first byte 0x44 = the active preset, P69)
← 70 12 03  00*8
→ 70 0B 01  <126 B packed preset, index 0x15>   write full preset to index 21 (P22)
← 70 0B 03  15 00 00 00 00 00 00 00 00 44 01 00 00 00
→ C1 15                                    select the new slot (ch 2 program change)
```

### Settings change (any field)

The app rewrites the **whole** settings block on every change (dozens of times per second while dragging):

```
→ 70 14 01 <20 B packed>    ← 70 14 03 00*8
```

### Tuner open/close on the pedal

```
← 70 7E 02 6F 02 00…           notify
→ 70 6F 00                     read
← 70 6F 02 01 00 0A 00 00 …    (status=1: tuner on)
→ 70 6F 01 01 00 0A 00 00      write back unchanged
← 70 6F 03 01 00 …             ack
```

### Opening the looper panel

```
→ B1 3B 08                     show LOOP page on pedal
→ 70 18 00   ← 70 18 02 01 32 32 <state> 00 …
→ 70 18 01 01 32 32 <state> 00  ← 70 18 03 01 00 …
```
