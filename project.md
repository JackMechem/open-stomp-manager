# project.md: Stomp Manager reverse engineering

## Goal

A native **Linux** editor (Rust) for the SORAN Dual Stomp Lite with the features of the official *Stomp Manager*:
live editing, preset management, settings, looper/drum/tuner, and eventually NAM/IR import.

## Approach taken

1. **Static analysis** of Stomp Manager 1.0.2 (macOS DMG / installed app): Flutter app, Dart 3.12.2 AOT
   (obfuscated). Object pool dumped with a patched blutter; whole `__text` disassembled and annotated; native libs
   (`nux_plugins` = JUCE audio converter, `imagelib` = NeuralAmpModelerCore) and firmware container inspected.
   → [docs/08](docs/08-static-analysis.md)
2. **Live capture** by hooking CoreMIDI inside an ad-hoc re-signed copy of the app (bus-level USB sniffing isn't
   possible on Apple Silicon without disabling SIP). Every pedal and app action was performed one at a time, marked
   in the log, and the app window screenshotted and OCR'd to map model numbers to names.
   → [docs/07](docs/07-capture-methodology.md)
3. **Decoding**: CC map per module, SysEx framing, the 8-in-7 packing, the preset layout (by correlating 99 preset
   records with CC dumps), settings, the looper state machine, and the upload transport.
   → [docs/02](docs/02-midi-cc-reference.md)–[05](docs/05-uploads-nam-ir.md)

## Key facts (cheat sheet)

* USB `3962:2000`, class-compliant UAC2 + USB-MIDI. No vendor interface.
* Pedal → host: ch 1. Host → pedal: ch 2. Same CC numbers both ways.
* Head CC per module = `(off?0x40:0) | model`: OD/FX `0x00`, AMP `0x01`, CAB `0x02`, MOD `0x03`, DLY `0x04`,
  RVB `0x05`, GATE `0x06`, EQ `0x07`. Params: OD/FX `0x0C–0F`, GATE `0x08–09`, AMP `0x12–17`, CAB `0x1A–1E`,
  EQ `0x20–2B`, MOD `0x2C–2F`, DLY `0x30–33`, RVB `0x36–39`. VOL `0x3A`, page `0x3B`.
* Presets: PC index 0–98 (= P01–P99).
* SysEx: `F0 00 21 7A 70 <cmd> <dir> … F7`, dir 0 read / 1 write / 2 reply / 3 ack; `7E` = notify.
  cmds: 09 IR slot, 0B preset, 0C edit buffer, 10 IR data, 12 name, 14 settings, 15 state, 18 loop, 19 drum,
  58 user amp/NAM, 6F tuner.
* Packed records: 7-bit LSB-first bitstream (126 MIDI bytes ↔ 110 struct bytes for a preset).

## Roadmap

- [x] Static analysis
- [x] Capture + decode all pedal-side controls
- [x] Capture + decode app-side live control, rename/save, settings, uploads (transport)
- [x] Documentation + tooling
- [ ] Fill open questions that matter for v1 ([docs/09](docs/09-open-questions.md) #4–#6): one more short capture
      session, or confirm directly with `amidi` on Linux
- [x] `stomp-proto` crate (pure encode/decode + tests against `captures/logs`)
- [x] GUI live monitor (`stomp-manager`, egui): auto-connect, read-only sync, active preset, chain,
      params, tuner/looper/drum, coalescing activity log, capture replay
- [ ] `stompctl` CLI (info, monitor, preset get/set/backup)
- [ ] GUI editor: send CC/PC edits, preset save/rename, settings page
- [ ] IR upload (needs the transform: capture a short cab IR)
- [ ] NAM upload (needs `imagelib` conversion reversed)

## Environment notes

* The re-signed instrumented copy lives in `work/` (git-ignored). Rebuild with `tools/sniffer/build.sh`.
* Pedal firmware during captures: V1.1.1. Stomp Manager: 1.0.2+1.
