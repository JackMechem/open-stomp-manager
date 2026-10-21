# 02 — MIDI CC / Program Change reference

Everything on this page is **confirmed from USB captures** (Dual Stomp Lite, firmware V1.1.1, Stomp Manager 1.0.2)
unless marked *(unverified)*.

## Channels — who talks on which

| Direction | MIDI channel | Status bytes |
|---|---|---|
| Pedal → host (reports) | **1** | `B0` (CC), `C0` (PC) |
| Host → pedal (commands) | **2** | `B1` (CC), `C1` (PC) |

The pedal reports every change made on the hardware as a CC/PC on channel 1. The official app *commands* the pedal on
channel 2 using **the same CC numbers**. The pedal does **not** echo host commands back, but it does report the
consequences (e.g. after a model change it sends the new model's knob values).

For a few controls (`0x3B`, `0x3F`, `0x41`, `0x49`) the official app mirrors the pedal's ch-1 report back on ch 2.
Doing that is optional, as far as we can tell.

## Value conventions

* Knob/slider values: **0–100**, the exact number shown in the app (not 0–127).
* Enumerations/dropdowns: 0-based index (order = order shown in the app).
* **Module head byte** (first CC of each module) = `(bypassed ? 0x40 : 0x00) | model_index`
  * bit 6 set = module **OFF** (this is confirmed: turning the amp off sends `0x40`, on sends `0x00`).
  * bits 0–5 = model index, **0-based** (the app's list is 1-based: app "09. Klone" = model 8).

## Program change

| Message | Meaning |
|---|---|
| `C0 nn` (pedal→host) | pedal switched to preset index `nn` |
| `C1 nn` (host→pedal) | select preset index `nn` |

`nn` is **0-based**; the app labels presets `P01…P99`, so `P(nn+1)`. Example: app "P85 DB751 Bass" = index 84 = `0x54`.
The same 0-based index is used in SysEx preset reads/writes.

After a preset change the pedal sends a burst of CCs with the new preset's values. The burst appears to contain
**only values that differ from the previous preset**, so keep full state from a SysEx preset read and apply the CCs on
top.

## Module map

`CC 0x3B` (page) value == module index. "Head" = model + on/off CC.

| Module index (CC 0x3B) | Module | Head CC | Param CCs | Models |
|---|---|---|---|---|
| 0 | OD/FX | `0x00` | `0x0C`–`0x0F` | 16 |
| 1 | AMP | `0x01` | `0x12`–`0x17` | 30 stock + 10 user (NAM) |
| 2 | CAB | `0x02` | `0x1A`–`0x1E` | 30 stock + 10 user (IR) |
| 3 | MOD | `0x03` | `0x2C`–`0x2F` | 13 |
| 4 | DELAY | `0x04` | `0x30`–`0x33` | 7 |
| 5 | REVERB | `0x05` | `0x36`–`0x39` | 6 |
| 6 | GATE | `0x06` | `0x08`–`0x09` | 1 |
| 7 | EQ | `0x07` | `0x20`–`0x2B` | 2 |
| 8 | LOOP (page only) | — | see Looper | |
| 9 | DRUM (page only) | — | see Drum | |

Params are in **app display order**: first slider = first CC of the range, and so on.

---

## OD/FX — head `0x00`, params `0x0C 0x0D 0x0E 0x0F`

| # | App | Model | `0x0C` | `0x0D` | `0x0E` | `0x0F` |
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

## AMP — head `0x01`, params `0x12`–`0x17`

All amps (stock and user) have the same six knobs:

| CC | `0x12` | `0x13` | `0x14` | `0x15` | `0x16` | `0x17` |
|---|---|---|---|---|---|---|
| Knob | GAIN | BASS | MIDDLE | TREBLE | PRESENCE | MASTER |

| # | Model | # | Model | # | Model |
|---|---|---|---|---|---|
| 0 | Black Twin | 10 | Mar Plexi100 | 20 | Fryman HB |
| 1 | Tweed 5F1 | 11 | Jazz CL | 21 | Fryman DS |
| 2 | Bassguy | 12 | Mes Rector | 22 | PVH 61 |
| 3 | Prince Rev | 13 | Mes MKIV Lead | 23 | Eddie 51 |
| 4 | Tweed 5F8 | 14 | Mes MKV Red | 24 | 2Rock J35 |
| 5 | Deluxe Rev | 15 | Mes LStar CL | 25 | Ampug SV Bass |
| 6 | Bdotone CL | 16 | Cornfed M50 | 26 | Ampug Port Bass |
| 7 | Voxy 30HW | 17 | ODS-100 | 27 | FR Bass |
| 8 | Mar J800 | 18 | Spawn Qrod | 28 | Mes 400 Bass |
| 9 | Mar J45 | 19 | Soloist 100 | 29 | Agula DP751 |
| 30–39 | **USER AMP 01–10** (NAM slots, see [05](05-uploads-nam-ir.md)) | | | | |

## CAB — head `0x02`, params `0x1A`–`0x1E`

| CC | Control | Values |
|---|---|---|
| `0x1A` | MIC | 0 E906, 1 M160, 2 MD421, 3 R121, 4 SM57, 5 SM58, 6 SM7B, 7 U47 |
| `0x1B` | POSITION | 0 EDGE, 1 MIDDLE, 2 CENTER |
| `0x1C` | LOW CUT (Hz) | raw 0–100, raw 0 = 20 Hz (Hz curve *unverified*, probably a log table 20…1000 Hz) |
| `0x1D` | HIGH CUT (Hz) | raw 0–100, raw 100 = 20000 Hz (*unverified* curve, probably 5000…20000 Hz) |
| `0x1E` | VOLUME | 0–100 |

User IR slots (models 30–39) hide MIC/POSITION but use the **same** CCs for LOW CUT/HIGH CUT/VOLUME.

| # | Model | # | Model | # | Model |
|---|---|---|---|---|---|
| 0 | Twin 212 | 10 | Mar V30 412 | 20 | Fryman 412 |
| 1 | Tweed 110 | 11 | Jazz 212 | 21 | Fryman 212 |
| 2 | Bassguy 410 | 12 | Mes OS 412 | 22 | PVH 412 |
| 3 | Prince 112 | 13 | Mes Trad 412 | 23 | Eddie 412 |
| 4 | Tweed 210 | 14 | Mes V30 212 | 24 | 2Rock 212 |
| 5 | Deluxe 112 | 15 | Mes EV 112 | 25 | Ampug 810 |
| 6 | Bdotone 212 | 16 | Cornfed 212 | 26 | Ampug 410 |
| 7 | Voxy 212 | 17 | ODS 212 | 27 | FR 210 |
| 8 | Mar G12M 412 | 18 | Spawn 412 | 28 | Mes 410 |
| 9 | Mar Silver 212 | 19 | SLO 412 | 29 | Agula 810 |
| 30–39 | **USER IR 01–10** (shown as `--` when empty) | | | | |

## MOD — head `0x03`, params `0x2C 0x2D 0x2E 0x2F`

| # | Model | `0x2C` | `0x2D` | `0x2E` | `0x2F` |
|---|---|---|---|---|---|
| 0 | Chorus-1 | INTENSITY | DEPTH | RATE | |
| 1 | Chorus-2 | RATE | DEPTH | | |
| 2 | Stereo Chorus | INTENSITY | WIDTH | RATE | |
| 3 | Vibrato | RATE | DEPTH | | |
| 4 | Detune | L | MIX | R | |
| 5 | Flanger | LEVEL | RATE | WIDTH | FEEDBACK |
| 6 | Phaser 90 | SPEED | | | |
| 7 | Phaser 100 | INTENSITY | SPEED | | |
| 8 | Chorus+ | SPEED | WIDTH | MODE (0 CHORUS, 1 FLANGER) | INTENSITY |
| 9 | Uni Vibe | SPEED | VOLUME | INTENSITY | MODE (0 CHORUS, 1 VIBE) |
| 10 | Tremolo | RATE | DEPTH | | |
| 11 | Chorus-3 | RATE | DEPTH | TONE | |
| 12 | Harmony | KEY (0–11: C C# D Eb E F F# G Ab A Bb B) | MODE (0 MINOR, 1 MAJOR) | HARM (0–9: -8TH -6TH -5TH -4TH -3TH 3TH 4TH 5TH 6TH 8TH) | MIX |

## DELAY — head `0x04`, params `0x30 0x31 0x32 0x33`

| # | Model | `0x30` | `0x31` | `0x32` | `0x33` |
|---|---|---|---|---|---|
| 0 | Analog Delay | TIME | FEEDBACK | LEVEL | |
| 1 | Digital Delay | TIME | FEEDBACK | LEVEL | |
| 2 | Mod Delay | TIME | FEEDBACK | MOD | LEVEL |
| 3 | Tape Delay | TIME | FEEDBACK | LEVEL | |
| 4 | Reverse Delay | TIME | FEEDBACK | LEVEL | |
| 5 | Pan Delay | TIME | FEEDBACK | LEVEL | |
| 6 | Physical Delay | TIME | FEEDBACK | LEVEL | |

TIME is shown as 0–100 in the app (not ms).

## REVERB — head `0x05`, params `0x36 0x37 0x38 0x39`

| # | Model | `0x36` | `0x37` | `0x38` | `0x39` |
|---|---|---|---|---|---|
| 0 | Room | DECAY | TONE | LEVEL | |
| 1 | Hall | DECAY | TONE | PRE DELAY (ms) | LEVEL |
| 2 | Plate | DECAY | LEVEL | | |
| 3 | Spring | DECAY | LEVEL | | |
| 4 | Shimmer | LEVEL | DECAY | SHIMMER | |
| 5 | Damp | LEVEL | DEPTH | | |

## GATE — head `0x06`, params `0x08 0x09`

Single model (Noise Gate). `0x08` THRESHOLD, `0x09` DECAY. (`0x07` is the EQ head, not a gate param.)

## EQ — head `0x07`, params `0x20`–`0x2B`

Raw 50 = slider centre (presumably 0 dB; the dB scaling is *unverified*).

| # | Model | `0x20` | `0x21` | `0x22` | `0x23` | `0x24` | `0x25` | `0x26` | `0x27` | `0x28` | `0x29` | `0x2A` | `0x2B` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | 6-band EQ | 100Hz | 220Hz | 500Hz | 1.2kHz | 2.5kHz | 6.4kHz | | | | | | |
| 1 | 10-band EQ | GAIN | 31Hz | 62.5Hz | 125Hz | 250Hz | 500Hz | 1kHz | 2kHz | 4kHz | 8kHz | 16kHz | VOLUME |

---

## Global / transport CCs

| CC | Dir | Meaning | Values |
|---|---|---|---|
| `0x3A` | both | Preset VOL (top slider in app) | 0–100 |
| `0x3B` | both | Page / module shown on the pedal. Host→pedal makes the pedal display jump to that module | module index (table above) |
| `0x3F` | both | Drum on/off | 1 playing, 0 stopped |
| `0x40` | pedal→host | Drum pattern (flat) | `style*10 + pattern`, 0–99 |
| `0x41` | both | Drum level | 0–100 |
| `0x42` | pedal→host | Looper REC level | 0–100 |
| `0x43` | pedal→host | Looper PLAYBACK level | 0–100 |
| `0x44` | pedal→host | Looper **state** | see below |
| `0x44` | host→pedal | Looper **button command** | see below |
| `0x45` | pedal→host | Tuner: note detected | 1 detected, 0 released |
| `0x46` | pedal→host | Tuner: pitch class | 0–11, C=0 |
| `0x48` | pedal→host | Tuner: cents | 0x32 (50) = in tune, lower = flat, higher = sharp (~1 unit/cent, *unverified*) |
| `0x49` | both | Looper LOOP SYNC | 0 Off, 1 On |
| `0x4A` | pedal→host | Looper LOOP MODE | 0 NR, 1 AR |

Host→pedal `0x40` (drum pattern) and `0x42`/`0x43` (loop levels) were confirmed on Linux by sending the ch-2 CC
and reading `0x19`/`0x18` back: the values change, and the pedal sends no ch-1 report or drum notify in reply.
Host→pedal `0x4A` is still unverified (`0x18` has no field for the loop mode).

### Drum patterns (`CC 0x40`)

Styles (`value / 10`): 0 Pop, 1 Rock, 2 Metal, 3 Disco, 4 Funk, 5 Blues, 6 Hip Hop, 7 Jazz, 8 Latin, 9 Metro.
Pattern within style = `value % 10` (app shows `Pop 01`…`Pop 10`). Metro patterns are time signatures:
4/4, 2/4, 3/4, 5/4, 6/8, 7/8, 9/8, 10/8, 11/8, 12/8.

Changing the pattern also makes the pedal send a SysEx drum NOTIFY carrying that pattern's default BPM
(see [03](03-sysex-protocol.md#notifications)). **BPM has no CC**; it only travels in SysEx `0x19`.

### Looper

**State report (pedal → host, `B0 44 vv`)**: `vv = (layer_flags << 4) | state`

| state (low nibble) | meaning |
|---|---|
| 0 | empty / cleared |
| 1 | recording |
| 2 | playing |
| 3 | overdubbing |
| 4 | stopped |

| flags (high nibble) | meaning |
|---|---|
| 0x1_ | an overdub layer is on top (UNDO available) |
| 0x2_ | base layer only (after the first take, or after an undo; REDO toggles back to 0x1_) |

Clean sequence captured: `01` rec → `22` play (base only) → `03` overdub → `12` play (+layer) → undo `22` → redo `12`
→ `04` stop → `00` clear.

**Button commands (host → pedal, `B1 44 vv`)**

| vv | Button |
|---|---|
| `01` | REC / PLAY / DUB (record → play → overdub → play …) |
| `02` | STOP |
| `04` | CLEAR |
| `08` | UNDO / REDO |

Footswitch behaviour on the pedal (for reference): L = rec/play/dub, hold L = undo, R = stop, hold R = clear.
