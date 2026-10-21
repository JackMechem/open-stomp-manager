# 01 — Overview: hardware, transport, software

## The device

| | |
|---|---|
| Product | SORAN Dual Stomp Lite (same firmware family as Dual Stomp and Tiny Stomp) |
| USB IDs | VID **0x3962**, PID **0x2000** |
| USB strings | Manufacturer "SORAN", Product "**JME STOMPBOX**", serial = a date-like string |
| USB speed | High-speed (480 Mb/s) |
| Firmware tested | **V1.1.1** (pedal reports `V1.1.1` / `JME-01`) |
| MCU | NXP i.MX RT10xx (Cortex-M7), per firmware strings |
| Vendor | The app and firmware are by **NUX** (Cherub). Bundle id `com.jiyou.stompManager`, NUX build paths, NUX DFU container |

### USB interfaces (all class compliant)

| If | Class | Purpose | Linux driver |
|---|---|---|---|
| 0 | Audio Control (UAC2, protocol 0x20) | audio interface | `snd-usb-audio` |
| 1, 2 | Audio Streaming (UAC2) | playback / capture | `snd-usb-audio` |
| 3 | Audio Control (MIDI) | | `snd-usb-audio` |
| 4 | **MIDI Streaming** | **all control traffic** | `snd-usb-audio` → ALSA rawmidi/seq |

There is **no vendor-specific interface**. Everything the editor does goes over standard USB-MIDI, so on Linux you
need no kernel driver, no libusb and no udev rule beyond ALSA access. The device will show up in `amidi -l` /
`aconnect -l` as "JME STOMPBOX".

Firmware updates are different: they use a separate NUX "Device Firmware Updater" over USB **HID** in DFU mode. That
is out of scope here.

Bluetooth: the pedal also speaks BLE-MIDI (service `03b80e5a-ede8-4b33-a751-6ce34ec4c700`, characteristic
`7772e5db-3868-4112-a1a9-f2669d106bf3`), presumably with the same messages. Untested.

## Protocol in one paragraph

Live control is plain MIDI: the pedal **reports** on channel 1 and the host **commands** on channel 2, with the
same CC numbers. Every module has a "head" CC = `(off?0x40:0) | model` plus a block of parameter CCs whose values are
the 0–100 numbers the app shows. Presets switch with Program Change. Bulk data (whole presets, names, global settings,
looper/drum/tuner records, NAM/IR uploads) goes over vendor SysEx `F0 00 21 7A 70 <cmd> <dir> … F7` with
read (0) / write (1) / reply (2) / ack (3), strictly request → response. Larger records are 8-bit structs packed into
a 7-bit little-endian bitstream.

## The official app (what we reversed)

| | |
|---|---|
| App | Stomp Manager 1.0.2 (build 1), macOS universal |
| Framework | **Flutter** (Dart 3.12.2 AOT, obfuscated) |
| MIDI | `flutter_midi_command` (CoreMIDI) |
| BLE | `universal_ble` |
| Native libs | `nux_plugins` (JUCE audio-file → 48 kHz WAV for IRs), `imagelib` (NeuralAmpModelerCore: NAM → device format), `nux_helper` (trivial) |
| Linux | The dependency list includes `flutter_midi_command_linux` and `bluez`, so the vendor builds for Linux internally but doesn't ship it |

App UI layout (for reference when building a clone):

* Left: preset list P01–P99 with save / import / export icons.
* Top: VOL slider, connection indicator, tuner, settings (wrench).
* Signal chain: GATE → OD/FX → AMP → CAB → EQ → MOD → DEL → REV → LOOP → DRUM (coloured = on).
* Edit panel: module list → model list → parameter sliders and an ON/OFF toggle.
* Settings: Input Trim, Global EQ (Bass/Mid/Treble), Wireless, LoopBack, Language, version info.

## Document map

| Doc | Contents |
|---|---|
| [02-midi-cc-reference.md](02-midi-cc-reference.md) | Every CC/PC, all modules/models/params, looper/drum/tuner |
| [03-sysex-protocol.md](03-sysex-protocol.md) | Framing, command table, handshake, encodings, captured sequences |
| [04-data-structures.md](04-data-structures.md) | Preset (110 B), settings, IR/NAM slot info, drum/loop/tuner records |
| [05-uploads-nam-ir.md](05-uploads-nam-ir.md) | NAM/IR upload transport and what's known about the conversion |
| [06-rust-linux-guide.md](06-rust-linux-guide.md) | How to build the Linux app in Rust |
| [07-capture-methodology.md](07-capture-methodology.md) | How the captures were made; how to make more |
| [08-static-analysis.md](08-static-analysis.md) | Static analysis of the app binary, firmware and native libs |
| [09-open-questions.md](09-open-questions.md) | What is still unknown and how to find out |
