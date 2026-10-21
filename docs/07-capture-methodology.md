# 07 — Capture methodology and tools

## Why not a USB bus sniffer?

On Apple Silicon macOS, bus-level USB capture (Wireshark `XHCx` interfaces) is not available without disabling SIP.
Every byte the editor exchanges with the pedal goes through **CoreMIDI**, so we hook CoreMIDI *inside the app*
instead. That gives exact payloads in both directions, with port names and millisecond timestamps.

On Linux the same thing is trivial: `amidi -d`, `aseqdump`, or `usbmon` + Wireshark.

## The macOS sniffer (`tools/sniffer/`)

| File | What it does |
|---|---|
| `midisniff.c` | A dylib that interposes (`__DATA,__interpose`) `MIDISend`, `MIDISendSysex`, `MIDISendEventList`, `MIDIReceived`, `MIDIInputPortCreate*`, `MIDIDestinationCreateWithBlock` and `MIDIPortConnectSource`, and logs every packet to `$SNIFF_LOG` |
| `midimon.c` | Standalone CoreMIDI tool: `list`, `listen [filter] [log]`, `send <filter> <hex…>` (sends raw bytes and prints replies for 1.5 s) |
| `build.sh` | Builds both, copies `/Applications/Stomp Manager.app` to `work/StompManager-sniff.app` and **ad-hoc re-signs it without hardened runtime**, so dyld honours `DYLD_INSERT_LIBRARIES` |
| `run.sh [name]` | Launches the instrumented copy, logging to `captures/logs/<name>.log`, and records the path in `captures/CURRENT` |
| `mark.sh "text"` | Appends `# MARK <ms> text` to the current log, for labelling what you're about to do |
| `shot.sh [label]` | Screenshots the Stomp Manager window (finds it via CGWindowList) to `captures/screens/<ms>_<label>.png` |
| `autoshot.py [cc…]` | Tails the log and screenshots the app ~0.6 s after a model/page CC arrives (used to pair model numbers with names) |

The original app in `/Applications` is never modified. Quit it before launching the instrumented copy, since two
editors talking to the pedal at once muddles the capture.

### Log format

```
# midisniff attached pid=6340 at 1792560244265 -> …/01-main-session.log
1792560250347 TX 6 [JME STOMPBOX] F0 00 21 7A 00 F7
1792560250349 RX 46 [JME STOMPBOX] F0 00 21 7A 10 56 31 2E …
# MARK 1792560304615 DEVICE: switching presets on the device
```

`<unix ms> <DIR> <len> [<endpoint>] <hex>`. DIR: `TX` app→pedal, `RX` pedal→app, `VSRC`/`VDST` virtual endpoints,
`INFO` port events. CoreMIDI may split SysEx across packets; the decoders reassemble.

## Decoders (`tools/decode/`)

| File | Use |
|---|---|
| `live.py` | `python3 tools/decode/live.py [--all] [log]` decodes a log live (or whole) into English: `PEDAL->APP AMP OFF model 29 = Agula DP751`, `APP->PEDAL [ch2] LOOPER button: UNDO/REDO`, `SYSEX NOTIFY drum/tempo: BPM = 94`… Unknown messages print as `??` |
| `analyze.py` | `python3 tools/decode/analyze.py <log> [--uniq] [--raw] [--gap ms]` gives a timeline with marks and burst separation, or a summary of distinct message shapes |
| `unpack.py` | 8-in-7 bitstream helpers (`unpack_bytes`, `bits_of`, `read`) |
| `preset_layout.py` | Re-derives the preset field offsets by correlating SysEx preset reads with CC dumps |
| `nam_extract.py` | Extracts the float32 payload of a NAM (`0x58`) or IR (`0x10`, `data_sub=0`) upload |
| `ocr.swift` | Vision-framework OCR of screenshots (build: `swiftc -O ocr.swift -o ocr`). Prints text with positions; used to read model and parameter names off the app |

## How the main session was run

`captures/logs/01-main-session.log` (about 2 h) was recorded with the pedal on firmware V1.1.1. Every action was
announced and marked first, and the app was screenshotted. Order:

1. App connect / handshake.
2. **Pedal side** (`DEVICE:` marks): preset switching, tuner, gate, OD/FX (all 16 models, each knob), AMP (all 40
   models), CAB (models, mic/position, user IR slots), EQ (6 and 10-band), MOD (13), DELAY (7), REVERB (6), looper
   (record/overdub/undo/redo/clear, 4 options), drum (on/off, tap tempo, level, BPM, every style, two styles' full
   pattern lists), global EQ on pedal (no traffic), save/rename on pedal, VOL.
3. **App side** (`APP:` marks): preset select, gate edits, amp model changes, looper buttons (twice), rename +
   save-as, settings (global EQ, input trim, wireless, loopback), NAM import ×3 (one delete + re-import), IR import.

`captures/LAB_NOTES.md` is the raw running notebook from that session. The docs supersede it where they differ.

## Making new captures

```sh
cd tools/sniffer && ./build.sh          # once
./run.sh 02-my-test                      # launches instrumented app
./mark.sh "APP: doing X"                 # before each action (second terminal)
python3 ../decode/live.py                # live decode (third terminal)
```

To probe the pedal directly without the app: `./midimon send STOMPBOX F0 00 21 7A 70 14 00 F7`.

**Do not run firmware updates while sniffing.** They don't use MIDI anyway.
