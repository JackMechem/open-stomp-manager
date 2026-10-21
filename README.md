# Stomp Manager RE: SORAN Dual Stomp Lite protocol

Reverse-engineered USB-MIDI protocol for the **SORAN Dual Stomp Lite** guitar multi-effects pedal (USB `3962:2000`,
"JME STOMPBOX"). The vendor's editor, *Stomp Manager*, only runs on macOS/Windows. This repo documents everything
needed to write an open editor for **Linux**; the reference implementation will be in Rust.

> Probably also applies to the Dual Stomp and Tiny Stomp (same firmware family, same app). Only the Dual Stomp Lite on
> firmware **V1.1.1** with Stomp Manager **1.0.2** has been tested.

## TL;DR

* The pedal is a **class-compliant USB Audio 2.0 + USB-MIDI** device. On Linux it just works with ALSA: no driver, no
  libusb.
* **Live control is plain MIDI.** The pedal reports on **channel 1** and you command it on **channel 2** with the
  **same CC numbers**. Values are the 0–100 numbers the app shows. Each module has a head CC
  `(off ? 0x40 : 0) | model`.
  ```
  C1 04        -> select preset P05
  B1 01 40     -> amp OFF (model 0)        B1 01 08 -> amp ON, model 8 (Mar J800)
  B1 12 64     -> amp GAIN = 100
  B1 44 01     -> looper REC/PLAY/DUB
  ```
* **Bulk data** (presets, names, settings, looper/drum/tuner records, NAM/IR uploads) uses vendor SysEx
  `F0 00 21 7A 70 <cmd> <dir> … F7` (read/write/reply/ack), strictly request → response. Big records are 8-bit
  structs packed into a 7-bit little-endian bitstream.

## Status

| Area | Status |
|---|---|
| Device identification, handshake | ✅ decoded |
| Every effect module / model / parameter ↔ CC (both directions) | ✅ decoded, all 8 modules |
| Preset select, delta CC dumps | ✅ |
| Looper (state + buttons + options), drum (on/off, level, pattern, BPM), tuner (live pitch) | ✅ |
| Preset record (110 B): heads, all params, volume, chain order, name | ✅ layout derived from 99 presets (some bytes unknown) |
| Preset rename / save-as | ✅ sequence captured |
| System settings (input trim, global EQ, wireless, loopback) | ✅ (some bytes unknown) |
| User AMP (NAM) / user IR upload transport | ✅ chunk protocol decoded |
| NAM → device-format conversion, IR pre-processing | ❌ open (app-side DSP, see [docs/05](docs/05-uploads-nam-ir.md)) |
| Rust app (`crates/`) | 🚧 live monitor works: active preset, chain, looper/drum/tuner, activity log. Editing next |

## Documentation

| | |
|---|---|
| [01 Overview](docs/01-overview.md) | Hardware, USB interfaces, protocol summary, app architecture |
| [02 MIDI CC reference](docs/02-midi-cc-reference.md) | **Every CC/PC**: all models and params, looper, drum, tuner |
| [03 SysEx protocol](docs/03-sysex-protocol.md) | Framing, command table, handshake, encodings, captured write sequences |
| [04 Data structures](docs/04-data-structures.md) | Preset, settings, IR/NAM slot, drum/loop/tuner records |
| [05 NAM & IR uploads](docs/05-uploads-nam-ir.md) | Upload transport and what's known about the conversion |
| [06 Rust/Linux guide](docs/06-rust-linux-guide.md) | Crates, architecture, code for packing/framing, roadmap |
| [07 Capture methodology](docs/07-capture-methodology.md) | How it was sniffed on macOS and how to make new captures |
| [08 Static analysis](docs/08-static-analysis.md) | App binary (Flutter/Dart AOT), native libs, firmware notes |
| [09 Open questions](docs/09-open-questions.md) | What's left and how to find out |

## Try it on Linux in 30 seconds

```sh
amidi -l                                            # find "JME STOMPBOX", e.g. hw:2,0,0
amidi -p hw:2,0,0 -S 'F0 00 21 7A 00 F7' -d -t 1    # -> F0 00 21 7A 10 56 31 2E 31 2E 31 ... ("V1.1.1")
amidi -p hw:2,0,0 -d                                # turn knobs on the pedal and watch the CCs
```

## The app (Rust, cross-platform)

`crates/stomp-proto` is the pure protocol library (no I/O, tested against `captures/logs/`).
`crates/stomp-manager` is the egui desktop app. For now it is a **live monitor**: it connects
automatically when the pedal is plugged in, reads the presets and state (the same read-only
SysEx requests the official app sends on connect; tick *Listen only* to send nothing at all) and
shows the active preset, signal chain, module parameters, tuner, looper and drum machine, with a
filterable activity log of everything you do on the pedal.

```sh
nix run                       # build and run
nix develop                   # dev shell: cargo, rust-analyzer, ALSA, amidi
cargo run -p stomp-manager    # inside the dev shell
cargo run -p stomp-manager -- --replay captures/logs/01-main-session.log --speed 8   # no pedal needed
cargo test                    # protocol tests against the captures
cargo run -p stomp-proto --example decode -- captures/logs/01-main-session.log       # text decoder
```

Without Nix you need Rust ≥ 1.88 and, on Linux, the ALSA headers (`libasound2-dev` /
`alsa-lib-devel`). macOS and Windows need nothing extra.

## Repository layout

```
docs/                 the documentation above
crates/stomp-proto/   protocol library: packing, SysEx framing, catalogue, records, state tracker
crates/stomp-manager/ egui desktop app (monitor now, editor later)
flake.nix             Nix package + dev shell
tools/sniffer/        macOS CoreMIDI interposer + helpers used to capture the official app
tools/decode/         Python decoders: live.py (human-readable live decode), analyze.py, unpack.py,
                      preset_layout.py, nam_extract.py; ocr.swift (screenshot OCR)
captures/logs/        raw capture logs (01-main-session.log = 2 h annotated session)
captures/LAB_NOTES.md running notebook from the capture session (superseded by docs/)
analysis/             static-analysis dumps and scripts (large/proprietary artefacts are git-ignored)
project.md            project notes and roadmap
```

Decode the main capture into readable form:

```sh
python3 tools/decode/live.py --all captures/logs/01-main-session.log | less -R
```

## Legal

This is an independent interoperability project. It is not affiliated with or endorsed by SORAN, NUX/Cherub or
JiYou. Product names are trademarks of their owners. The repo contains **no vendor binaries or firmware**: only
documentation, our own tools, and captures of traffic between the author's own pedal and app. Third-party NAM/IR files
used in tests are not included. Use at your own risk: writing bad data to a pedal could corrupt presets. Back them
up first.

## License

TBD by the repository owner. MIT/Apache-2.0 is suggested for the tools and CC-BY-4.0 for the docs.
