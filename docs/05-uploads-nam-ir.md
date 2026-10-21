# 05 — User AMP (NAM) and User IR uploads

Status: **transport fully decoded; content conversion NOT yet reversed.** You can replay a captured upload, but
generating the payload from a `.nam` / `.wav` file still requires reversing the app's converter (open problem #1).

## Common chunk format

Both uploads send a series of SysEx writes, each acknowledged before the next. The body after `<dir>` is one
**packed** stream ([03](03-sysex-protocol.md#2-packed-8-in-7-bitstream-packed)):

```
unpacked chunk = 12-byte header | float32 little-endian samples …
```

* Data starts at **bit 96** of the packed stream (unpacked byte 12).
* Raw chunk length 178 MIDI bytes → 136 data bytes = **34 float32** per chunk; the last chunk is shorter.
* A chunk offset field sits in the header (MIDI bytes 5–6 hold `0, 0x11, 0x22 …` = offsets 0, 17, 34 … in 8-byte
  units, i.e. 136 bytes per step).
* **Every upload so far is exactly 2048 float32 = 8192 bytes**, in 61 chunks (60 × 34 + 8 floats).
* The real transfer takes only ~100 ms. The long wait in the app is the conversion.

`tools/decode/nam_extract.py` extracts the floats from a capture log:

```
python3 tools/decode/nam_extract.py captures/logs/01-main-session.log "<marker text>" out.bin
```

## User AMP / NAM — SysEx `0x58`

Unpacked header byte 1 = slot (0–9 → USER AMP 01–10, amp models 30–39); byte 2 = sub-command:

| Sub (unpacked) | MIDI byte 2 | Meaning |
|---|---|---|
| 1 | `04` | data chunk |
| 3 | `0C` | write slot info (name etc.) |
| 4 | `10` | read slot info |
| 6 | `18` | delete slot |

Captured import (Fender Super Reverb NAM → USER AMP 01):

```
→ 70 58 01 00 00 0C …  (168 B)   write slot info      ← 70 58 03 00 00 0C … ack
→ 70 58 01 00 00 04 00 00 00 00 … (178 B)  data chunk 0   ← ack (echoes header)
→ … 61 data chunks total, offsets 0,17,34…1020 …
→ B1 01 1E                        select amp model 30 (USER AMP 01)
```

Delete: `→ 70 58 01 00 00 18 00…` `← 70 58 03 03`, then the app re-reads the slot info.

### What the 2048 floats are

The pedal does **not** run NAM natively. The app's native library (`imagelib`, which bundles NeuralAmpModelerCore)
loads the `.nam` and converts it. Findings:

* The test files are NAM 0.7 `SlimmableContainer`s with two WaveNet submodels (3-channel: 1871 weights; 8-channel:
  12146 weights). The uploaded floats are **not** those weights (searched all alignments and float16/32).
* The 2048 floats split into two halves:
  * `[0..1023]` rises smoothly and exponentially to a large value (~2200): looks like a **lookup table / transfer
    curve**, or a time-reversed filter.
  * `[1024..2047]` is tiny, with energy rising toward the end: looks like a **time-reversed impulse response**.
* The static analysis shows `imagelib` callbacks deliver `(float* g1, float* g2, float* factor)`. So the converter
  most likely approximates the network as **filter(s) + static nonlinearity** (a Wiener/Hammerstein-style model):
  g1, g2 = two 1024-float arrays, factor = a scalar that probably sits in the slot-info record (sub 3).
* **Deterministic:** importing the same file twice produced byte-identical info and data, so captures are valid
  test vectors.

Test vectors (not in git; see `.gitignore`. Regenerate from the logs):

| Input file | Log marker | Output |
|---|---|---|
| `FNDR BFSR VB Edge WRM2 CAB FREE.nam` | `NAM import 'FNDR` | `captures/files/FNDR.device_floats.bin` |
| `Jcm800.nam` (from "Marshall Jcm800 + V30 1960.zip") | `deleted USER AMP NAM` (the next import after it) | `captures/files/Jcm800.device_floats.bin` |

To finish this: disassemble `imagelib.framework` (`ImageUtility::loadNamFile`, `namModel`, `namBuffer`), most likely
by probing the model with an impulse/sweep and fitting. Then validate against the vectors above.

## User IR — SysEx `0x10` (data) + `0x09` (name)

```
→ B1 3B 02, B1 02 1E            app selects CAB page + USER IR 01 (model 30)
→ 70 10 01 00 00 00 00 00 LL HH 00 00 00 00 01 00 <data…>   ×61, each ← 70 10 03 <header> ack
→ 70 09 01 <slot> <packed: 01 00 00 00 "Delay And Reverb"…>   ← (CC 0x7E 00) + 70 09 03 00…
```

Data header: the same layout as NAM, with sub-command 0 (MIDI byte 2 = `00`). The slot number is presumably inside the
header (only slot 0 was captured).

### What the 2048 floats are

* The app converts any audio file to a 48 kHz WAV first (JUCE in `nux_plugins`: `*_ir_48.wav`).
* The uploaded 2048 samples (≈ 42.7 ms at 48 kHz) are **not** a straight copy of the start of the WAV. The test file
  was a long (10 s / 20 s) delay/reverb "IR", and correlation with either candidate file was weak (≈ −0.3 at best,
  with lead-in trimmed so the peak lands at sample ~253). The app likely trims, normalises and possibly windows or
  minimum-phases the IR.
* Re-test with a short, plain cab IR (< 2048 samples) to pin the transform down. Probably just
  trim + normalise + truncate.

Vector: `captures/files/ir/IR01.device_floats.bin` (log marker `imported Jcm800`, cmd 0x10, sub 0). Which of the two
WAVs was used was not recorded (open question).
