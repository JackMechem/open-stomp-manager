> Raw notebook written live during capture session `logs/01-main-session.log`. Kept for provenance. **The docs in
> `docs/` supersede it.** Known corrections: SysEx preset index == Program Change number == app P-number − 1 (not
> "PC − 1"); the module head bit 6 means OFF (fixed below); see `docs/` for the final layouts.

# Live capture findings (pedal-side, session 01-main-session.log)

## Standard MIDI from device (ch 1)
- Program Change `C0 nn` = preset change; nn = preset number as shown in app (P85 -> 0x55). Followed by CC dump of active preset.
- `CC 0x3B` = page shown on pedal: 00 = OD/FX, 01 = home, 06 = Gate
- Module head CC: bit6 (0x40) = BYPASSED/OFF (0 = on), bits0-5 = model index (0-based; app list is 1-based). Confirmed: amp off->0x40, on->0x00; app shows OD/FX "OFF" + grey block when CC00=0x48
- Tuner: CC 0x45 note-detected (1/0), CC 0x46 note pitch class (C=0), CC 0x48 cents (0x32 = in tune?)

## Gate
- CC 0x06 head (bit6 = off), CC 0x08 threshold, CC 0x09 decay

## OD/FX (head CC 0x00, params CC 0x0C, 0x0D, 0x0E, 0x0F in app display order)
| wire | app | name | P1 (0C) | P2 (0D) | P3 (0E) | P4 (0F) |
|---|---|---|---|---|---|---|
| 0 | 01 | Red Comp | OUTPUT | SENSITIVITY | | |
| 1 | 02 | RS Comp | SUSTAIN | LEVEL | | |
| 2 | 03 | Yellow Dist | OUTPUT | Distortion | | |
| 3 | 04 | Boost RC | GAIN | VOLUME | BASS | TREBLE |
| 4 | 05 | Boost AC | GAIN | VOLUME | BASS | TREBLE |
| 5 | 06 | DS1 | GAIN | TONE | VOLUME | |
| 6 | 07 | TS9 | DRIVE | TONE | LEVEL | |
| 7 | 08 | BD2 | LEVEL | TONE | GAIN | |
| 8 | 09 | Klone | VOLUME | DRIVE | TONE | |
| 9 | 10 | Ratt2 | DIST | FILTER | VOLUME | |
| 10 | 11 | Red Drive | DRIVE | TONE | LEVEL | |
| 11 | 12 | Cruncher | VOLUME | TONE | GAIN | |
| 12 | 13 | Muff | VOLUME | TONE | SUSTAIN | |
| 13 | 14 | Boost | GAIN BOOST (switch 0/1) | VOLUME | | |
| 14 | 15 | Face Fuzz | GAIN | TONE | OUTPUT | |
| 15 | 16 | ST Drive | VOLUME | GAIN | FILTER | |
Values = 0..100 as displayed in the app.
NOTE: static effect_db_readable.txt param lists are offset by one model vs reality; this table is ground truth.
NOTE: model names for wire 0-4 & 12-15 come from app list order (01..16) — names 01-05 from the earlier screenshot.

## AMP (head CC 0x01, params CC 0x12..0x17)
All stock amps share the same 6 knobs: 0x12 GAIN, 0x13 BASS, 0x14 MIDDLE, 0x15 TREBLE, 0x16 PRESENCE, 0x17 MASTER (0..100).
Verified on models 0-7; app list order matches static DB:
0 Black Twin, 1 Tweed 5F1, 2 Bassguy, 3 Prince Rev, 4 Tweed 5F8, 5 Deluxe Rev, 6 Bdotone CL, 7 Voxy 30HW,
8 Mar J800, 9 Mar J45, 10 Mar Plexi100, 11 Jazz CL, 12 Mes Rector, 13 Mes MKIV Lead, 14 Mes MKV Red,
15 Mes LStar CL, 16 Cornfed M50, 17 ODS-100, 18 Spawn Qrod, 19 Soloist 100, 20 Fryman HB, 21 Fryman DS,
22 PVH 61, 23 Eddie 51, 24 2Rock J35, 25 Ampug SV Bass, 26 Ampug Port Bass, 27 FR Bass, 28 Mes 400 Bass,
29 Agula DP751, 30..39 = USER AMP 01..10 (NAM slots; same 6 knobs CC 0x12-0x17). Verified full sweep 0..0x27.

## CAB (head CC 0x02, page 0x3B=02, params CC 0x1A..0x1E)
| CC | param | range |
|---|---|---|
| 0x1A | MIC | 0..7 = E906, M160, MD421, R121, SM57, SM58, SM7B, U47 |
| 0x1B | POSITION | 0..2 = EDGE, MIDDLE, CENTER |
| 0x1C | LOW CUT (Hz) | raw 0..100; raw 0 = 20 Hz (curve TBD) |
| 0x1D | HIGH CUT (Hz) | raw 0..100; raw 100 = 20000 Hz (curve TBD) |
| 0x1E | VOLUME | 0..100 |
Same controls for all cabs. Models (app list order = static DB order):
0 Twin 212, 1 Tweed 110, 2 Bassguy 410, 3 Prince 112, 4 Tweed 210, 5 Deluxe 112, 6 Bdotone 212, 7 Voxy 212,
8 Mar G12M 412, 9 Mar Silver 212, 10 Mar V30 412, 11 Jazz 212, 12 Mes OS 412, 13 Mes Trad 412, 14 Mes V30 212,
15 Mes EV 112, 16 Cornfed 212, 17 ODS 212, 18 Spawn 412, 19 SLO 412, 20 Fryman 412, 21 Fryman 212, 22 PVH 412,
23 Eddie 412, 24 2Rock 212, 25 Ampug 810, 26 Ampug 410, 27 FR 210, 28 Mes 410, 29 Agula 810, 30..39 = user IR slots (app 31-40, "--" when empty). User slots hide MIC/POSITION but keep LOW CUT=0x1C, HIGH CUT=0x1D, VOLUME=0x1E (verified).

## EQ (head CC 0x07, page 0x3B=07, params CC 0x20..0x2B; raw 50 = centre / 0 dB presumably)
- model 0 = 6-band:  0x20 100Hz, 0x21 220Hz, 0x22 500Hz, 0x23 1.2kHz, 0x24 2.5kHz, 0x25 6.4kHz
- model 1 = 10-band: 0x20 GAIN, 0x21 31Hz, 0x22 62.5Hz, 0x23 125Hz, 0x24 250Hz, 0x25 500Hz, 0x26 1kHz,
                     0x27 2kHz, 0x28 4kHz, 0x29 8kHz, 0x2A 16kHz, 0x2B VOLUME
- dB scaling TBD (check slider dB display at raw 0 / 100)

## MOD (head CC 0x03, params CC 0x2C..0x2F in app display order)
| wire | app | name | 0x2C | 0x2D | 0x2E | 0x2F |
|---|---|---|---|---|---|---|
| 0 | 01 | Chorus-1 | INTENSITY | DEPTH | RATE | |
| 1 | 02 | Chorus-2 | RATE | DEPTH | | |
| 2 | 03 | Stereo Chorus | INTENSITY | WIDTH | RATE | |
| 3 | 04 | Vibrato | RATE | DEPTH | | |
| 4 | 05 | Detune | L | MIX | R | |
| 5 | 06 | Flanger | LEVEL | RATE | WIDTH | FEEDBACK |
| 6 | 07 | Phaser 90 | SPEED | | | |
| 7 | 08 | Phaser 100 | INTENSITY | SPEED | | |
| 8 | 09 | Chorus+ | SPEED | WIDTH | MODE (0 CHORUS / 1 FLANGER) | INTENSITY |
| 9 | 10 | Uni Vibe | SPEED | VOLUME | INTENSITY | MODE (0 CHORUS / 1 VIBE) |
| 10 | 11 | Tremolo | RATE | DEPTH | | |
| 11 | 12 | Chorus-3 | RATE | DEPTH | TONE | |
| 12 | 13 | Harmony | KEY (0..11 C C# D Eb E F F# G Ab A Bb B) | MODE (0 MINOR / 1 MAJOR) | HARM (0..9: -8TH -6TH -5TH -4TH -3TH 3TH 4TH 5TH 6TH 8TH) | MIX |
(enum orders from static DB; MODE/enum value order assumed = string order)

## DELAY (head CC 0x04, page 0x3B=04, params CC 0x30..0x33)
0 Analog Delay, 1 Digital Delay, 3 Tape Delay, 4 Reverse Delay, 5 Pan Delay, 6 Physical Delay: 0x30 TIME, 0x31 FEEDBACK, 0x32 LEVEL
2 Mod Delay: 0x30 TIME, 0x31 FEEDBACK, 0x32 MOD, 0x33 LEVEL
TIME is shown in app as 0..100 (not ms). Tap tempo / BPM sync not yet observed.

## REVERB (head CC 0x05, page 0x3B=05, params CC 0x36..0x39)
| wire | name | 0x36 | 0x37 | 0x38 | 0x39 |
|---|---|---|---|---|---|
| 0 | Room | DECAY | TONE | LEVEL | |
| 1 | Hall | DECAY | TONE | PRE DELAY (ms) | LEVEL |
| 2 | Plate | DECAY | LEVEL | | |
| 3 | Spring | DECAY | LEVEL | | |
| 4 | Shimmer | LEVEL | DECAY | SHIMMER | |
| 5 | Damp | LEVEL | DEPTH | | |

## Module index summary (page CC 0x3B value == module index)
0 OD/FX (head 0x00, params 0x0C-0x0F) | 1 AMP (0x01, 0x12-0x17) | 2 CAB (0x02, 0x1A-0x1E) | 3 MOD (0x03, 0x2C-0x2F)
4 DELAY (0x04, 0x30-0x33) | 5 REVERB (0x05, 0x36-0x39) | 6 GATE (0x06, 0x08-0x09) | 7 EQ (0x07, 0x20-0x2B)

## LOOP (page 0x3B = 08; footswitch-driven)
- On entering loop page: pedal sends CC 0x3B=08; APP sends `B1 3B 08` (CC on MIDI CHANNEL 2 — app echoes page back?),
  then app reads SysEx 0x18 (loop): reply payload `01 32 32 <state> 00..`, app writes same 5 bytes back (01), pedal acks (03).
  Guess: byte0 = loop enabled?, byte1/2 = levels (50/50), byte3 = loop state.
- CC 0x44 = looper state (pedal -> app):
  00 empty/cleared, 01 recording, 02 playing (single layer), 03 overdubbing, 04 stopped/paused,
  CC 0x44 value = (flags << 4) | state. state: 0 empty, 1 recording, 2 playing, 3 overdubbing, 4 stopped.
  flags (high nibble): 0x10 = overdub layer present (UNDO available); 0x20 = base layer only (after first take, or after
  undo -> REDO toggles back to 0x10). Retest from app (clean): rec 01 -> finish 0x22 -> overdub 03 -> finish 0x12 ->
  undo 0x22 -> redo 0x12 ... -> stop 04 -> clear 00. (One early pedal-side take reported plain 02.) Not a pause state.
  Hold L = undo: pedal first reports 03 (press registers as overdub), then 0x22 once the hold triggers undo.
  Hold R = clear -> 00.
- Finishing the first recording triggers NOTIFY `70 7E 02 19 ...` (drum/tempo register 0x19 changed — loop length likely sets BPM).
  Payload seen: `00 00 32 00 00 7F` then `00 00 32 00 01 00`.
- Loop options (app panel shows buttons Rec / Stop / Clear / Undo + 4 options):
  CC 0x42 REC level (0..100), CC 0x43 PLAYBACK level (0..100), CC 0x49 LOOP SYNC (0 Off / 1 On), CC 0x4A LOOP MODE (0 NR / 1 AR)
  When pedal sends CC 0x49 the APP echoes it on channel 2 (`B1 49 vv`) — same as `B1 3B 08`. Ch-2 echo seen only for 0x3B and 0x49 so far.
  These likely map onto the SysEx 0x18 loop payload `01 32 32 <state>`: 32/32 = REC/PLAYBACK level 50.

## DRUM (page 0x3B = 09)
- CC 0x3F drum on/off (1/0); app echoes on ch2 (`B1 3F vv`).
- Tap tempo (right switch) -> NOTIFY `70 7E 02 19 02 | 00 00 32 00 HH LL` each tap; BPM = (HH<<7)|LL (MidiUint14, big-endian 7-bit).
  Taps ~0.67 s apart gave 0x58..0x5F = 88..95 BPM. Initial drum read: `70 19 02 | 00 00 32 00 00 78 00 00` -> 120 BPM.
  Loop recording earlier set it to `00 01 00` = 128 BPM / `00 00 7F` = 127 (loop length sets tempo).
- NOTIFY format: `70 7E 02 <reg> 02 <payload...>` — carries data inline (6 bytes for 0x19).
- App drum panel: STYLE dropdown (01.Pop ...), PATTERN dropdown (Pop 01 ...), DRUM SYNC Off/On, DRUM (level) slider, BPM.
- CC 0x41 = DRUM level (0..100), app echoes on ch2.
- BPM knob on pedal -> no CC, only NOTIFY 0x19 with new BPM (same as tap). Payload byte2 (0x32=50) = drum level copy.
- CC 0x40 = drum pattern, flat index = style*10 + pattern (0..99). Styles: 0 Pop, 1 Rock, 2 Metal, 3 Disco, 4 Funk,
  5 Blues, 6 Hip Hop, 7 Jazz, 8 Latin, 9 Metro (Metro patterns: 4/4, 2/4, 3/4, 5/4, 6/8, 7/8, 9/8, 10/8, 11/8, 12/8).
  Each pattern change also sends NOTIFY 0x19 with that pattern's default BPM.
- Drum register 0x19 payload (6 bytes in notify after the leading 02): [0]=? [1]=pattern index [2]=level [3]=? [4..5]=BPM (14-bit)

## Preset save on pedal / preset data format (SysEx 0x0B)
- Saving on pedal -> NOTIFY `70 7E 02 0B 02 54 00..` (preset 0x0B changed, index 0x54).
- App then reads it: TX `70 0B 00 54 00*13`; RX 134 bytes `70 0B 02 <126 bytes>`.
- PAYLOAD PACKING (confirmed): the 126 data bytes are a little-endian bitstream, 7 bits per MIDI byte, LSB-first.
  Re-grouping into 8-bit bytes gives 110 bytes (tools/unpack.py: unpack_bytes()).
  Unpacked[0] = preset index (0x54); userName[16] at unpacked offset 94..109 = "DB751 Bass!" (null padded).
- INDEXING: P85 "DB751 Bass" = SysEx index 0x54 (84) but Program Change 0x55 (85). => SysEx index = PC - 1 (?)
  i.e. app P-number == PC value; SysEx preset index is 0-based. VERIFY with another preset.
- Unpacked P85 (110 B): 54 00 00 00 00 00 00 00 62 00 00 00 4f 27 27 4c 46 45 40 41 02 14 32 00 00 03 21 27 42 00 00 00
  06 32 33 32 32 32 34 00 00 05 00 00 04 5e 32 00 0c 32*12 04 00 00 00 32 03 2f 33 2c 00 00 00 02 24 2a 00 00 50 00 00 00
  06 00 01 02 07 03 04 05 00 00 00 00 "DB751 Bass!" 00*5
  (field map: see analysis/device_struct_schema.txt — map offsets to module fields offline. "00 01 02 07 03 04 05" near
  offset 87 looks like the module chain order / link[] array.)

## Preset volume
- CC 0x3A = VOL (preset master volume, 0..100; app top slider "VOL: 80" = 0x50)

## APP -> PEDAL remote control (confirmed)
- App uses MIDI CHANNEL 2 for everything it sends; pedal reports on channel 1.
- Preset select: `C1 nn` (Program Change ch2). Pedal replies with a CC dump on ch1 that appears to contain only
  params differing from the previous preset (delta), so keep full state from SysEx preset reads.
- Param edit: same CC numbers as pedal-side, on ch2: e.g. gate off/on `B1 06 40` / `B1 06 00`, threshold `B1 08 vv`,
  decay `B1 09 vv`. Sent continuously while dragging (duplicates common). Pedal does NOT echo app edits back.
- The pedal->app CCs on ch1 are mirrored by the app on ch2 for some controls (0x3B page, 0x3F drum on, 0x41, 0x49) = probably
  the app "confirming/syncing" UI state.
- Model change from app: `B1 01 mm` (amp); pedal replies on ch1 with that model's knob values (delta only).
- Opening a module editor in the app sends `B1 3B <module>` -> pedal display jumps to that module page.
- LOOP from app: opening the loop panel -> `B1 3B 08`, then read+write-back of SysEx 0x18 (same as pedal-side).
  Loop transport buttons = CC 0x44 on ch2 (command values, different from the state values the pedal reports):
    `B1 44 01` = REC button (record -> play -> overdub -> play cycle)
    `B1 44 02` = STOP
    `B1 44 04` = CLEAR
    `B1 44 08` = UNDO / REDO toggle
  Pedal answers with the new state on ch1 CC 0x44. NOTE: after the first take from the app the state was 0x22 (not 0x02)
  and undo -> 0x22, redo -> 0x12; so flag meaning (0x10/0x20) still slightly ambiguous: 0x10 = has overdub layer on top,
  0x20 = overdub layer undone/redo-able OR "single base layer". Verify.

## Rename + save-as from app (confirmed sequence)
1. TX `70 12 01 | 44 | 34 4F 6F 7A 21 00*11` = set preset name: 1 byte (0x44=68, source slot index? or flag) + Char[16] RAW ASCII
   ("4Ooz!"), NOT bit-packed. RX `70 12 03 00*8` ack.
2. TX `70 0B 01 | 15 | ...126 bytes total` = write full preset to index 0x15 (P22). Name inside packed body at unpacked off 94.
   RX `70 0B 03 | 15 00.. 44 01 00 00 00` ack (echoes first 14 raw bytes).
3. TX `C1 15` = select the new slot.
PACKING AMBIGUITY: either the whole 126-byte body is one 7-bit LSB-first stream, or raw[0..7] = plain header
(index + 7 bytes) and the packed stream starts at raw[8]. Both put the name byte-aligned; decide via struct schema offline.

## System settings (SysEx 0x14) — app rewrites the WHOLE block on every change
- TX `70 14 01 <20 raw bytes>` -> RX `70 14 03 00*8` ack. Read: `70 14 00` -> `70 14 02 <20 raw>`.
- Payload = 7-bit LSB-first packed; unpacked to 17 bytes (tools/unpack.py). Current values:
  `0 10 30 10 50 50 50 50 50 1 1 60 180 0 0 0 0`
| unpacked byte | meaning | range |
|---|---|---|
| 0 | ? (0) | |
| 1 | INPUT TRIM | 0..20 => -10..+10 dB (10 = 0 dB) — confirmed |
| 2 | ? (30) | |
| 3 | ? (10) | |
| 4 | GLOBAL EQ Bass | 0..100 (50 centre) |
| 5 | GLOBAL EQ Mid | 0..100 |
| 6 | GLOBAL EQ Treble | 0..100 |
| 7, 8 | ? (50, 50) | |
| 9 | WIRELESS | 0 off / 1 on |
| 10 | LOOPBACK | 0 off / 1 on |
| 11 | ? (60) | |
| 12 | ? (180) | e.g. auto-off / timeout? |
| 13..16 | 0 | |
- Settings page also has Language (中文/English) — app-local, no traffic. Shows Version v1.0.2+1, Firmware v1.1.1.
- Global EQ changed on the PEDAL sends nothing over USB (app only learns on re-read).

## NAM import (SysEx 0x58 = USER AMP / NAM slot)  — file: captures/files/FNDR BFSR VB Edge WRM2 CAB FREE.nam
- Startup: app reads 10 slots: TX `70 58 00 | 00 SS 10 00*12` (SS = 0,2,4..? slot*2?) -> RX 168B `70 58 02 | 00 SS 10 ...` (slot info/name).
- Import sequence (12 s, 62 msgs each ack'd):
  1. TX 168B `70 58 01 | 00 00 0C 00 00 00 00 00 00 00 02 00 00 | <packed info>` = write slot info (sub 0x0C) ; RX `70 58 03 ...` ack
  2. 61x TX `70 58 01 | 00 00 04 00 00 LL HH 00 00 00 00 01 00 | <packed data>` = data chunk (sub 0x04), offset (HH<<7|LL)
     counts in 8-byte units: 0,17,34..1020 (17*8 = 136 bytes per chunk). Each ack'd `70 58 03 <same header>` before next.
  3. TX `B1 01 1E` = select amp model 30 (USER AMP 01). Pedal replies knob CCs.
- Total payload = exactly 8192 bytes (after 7-bit unpack of chunk bodies from raw offset 14).
- File is a NAM "SlimmableContainer" with 2 WaveNet submodels (3ch: 1871 weights; 8ch: 12146 weights). 8192 B fits the
  small one, BUT the data is NOT raw float32/float16 of the weights (searched LE/BE, all bit offsets, several unpackings).
  Entropy 7.63 bits/byte. => app transforms the model (imagelib native lib: "g1/g2/factor" outputs) and/or obfuscates it.
  OPEN: reverse imagelib (analysis/extracted/native) to get the conversion. Determinism test: import same file twice.
- DETERMINISTIC: deleting + re-importing the same file produced byte-identical info + 8192-byte data. => captures are
  valid test vectors for reimplementing the conversion.
- Actual order: info (sub 0x0C) first, then 61 data chunks (sub 0x04) in ~100 ms (each ack'd), then `B1 01 1E`.
- DELETE slot: TX `70 58 01 | 00 00 18 00*11` (sub 0x18) -> RX `70 58 03 03` ; then app re-reads slot info (sub 0x10).
- SLOT ADDRESSING: startup reads use raw `00 SS 10` with SS = 0,2,4..0x12 for slots 0..9. That is exactly what you get if
  the WHOLE body (header included) is the 7-bit LSB-first packed stream: unpacked[0]=0, unpacked[1]=slot, then sub-cmd...
  => treat every 0x58 body (and probably 0x0B/0x14 bodies) as one packed stream; header fields are inside it.
- Two unsolicited RX `F0 00 21 7A 10 F7` appeared mid-session (status/NAK? same as reply to universal identity request).
- PAYLOAD DECODED (framing): each data chunk = packed stream: 12-byte header (unpacked: [0]=0,[1]=slot,[2]=sub(1=data,
  3=info, 4=read-info, 6=delete), offset field at bits 40..55 (chunk index?), ...) + float32 LE values from bit 96.
  tools/nam_extract.py dumps them. FNDR file -> 2048 float32 (captures/files/FNDR.device_floats.bin).
  Jcm800 -> captures/files/Jcm800.device.bin (raw unpacked; re-extract floats with nam_extract.py marker "imported Jcm800").
- The floats are NOT the NAM weights (only chance matches). Structure = two 1024-float halves:
  [0..1023]: grows smoothly/exponentially to a huge value at the end (max ~2209) — looks like a lookup/transfer table or a
  time-reversed filter; [1024..2047]: tiny values, energy rising at the end (time-reversed IR?).
  HYPOTHESIS: the app renders the WaveNet into a simplified model the DSP runs (e.g. filter(s) + static nonlinearity,
  imagelib outputs "g1/g2/factor"). The info message (sub 3) likely carries name + the scalar "factor"/gain.
  => To support NAM in Rust you must port imagelib's conversion (static RE of analysis/extracted/native/imagelib),
     validating against these captured outputs.
- USER AMP 03 import: header slot=2, app sent `B1 01 20` (model 32).
