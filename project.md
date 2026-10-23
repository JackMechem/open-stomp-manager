# project.md: Open Stomp Manager (desktop editor + reverse-engineering workbench)

## Goal

A native editor (Rust, egui) for **Linux, macOS and Windows** for the SORAN Dual Stomp Lite with the features of the
official *Stomp Manager*: live editing, preset management, settings, looper/drum/tuner, and NAM/IR import.

All protocol work (encode/decode, state, connection engine, uploads, NAM/IR conversion) and the protocol docs live
in the [`open-stomp`](https://github.com/JackMechem/open-stomp-lib) library (`~/Projects/open-stomp/open-stomp-lib`),
which this app depends on by path. The protocol cheat sheet, how it was recovered, and the protocol roadmap are in
**that repo's project.md**. This file covers the app and the reverse-engineering workbench kept here.

## What lives here

* `crates/open-stomp-manager/`: the egui app.
* The reverse-engineering workbench that produced the library's docs
  ([07](https://github.com/JackMechem/open-stomp-lib/blob/main/docs/07-capture-methodology.md),
  [08](https://github.com/JackMechem/open-stomp-lib/blob/main/docs/08-static-analysis.md)):
  `tools/sniffer` (CoreMIDI interposer), `tools/decode` (decoders, OCR), `tools/namconv`, `analysis/`
  (static-analysis artefacts), `captures/` (raw logs, `LAB_NOTES.md`).

## Roadmap

- [x] GUI live monitor: auto-connect, read-only sync, active preset, chain, params, tuner/looper/drum, coalescing
      activity log, capture replay
- [x] Cross-platform (Linux/macOS/Windows): CI on all three, tag-triggered release packages, icons, versioning
      (single source: workspace `Cargo.toml`, see CHANGELOG.md)
- [x] GUI editor: send CC/PC edits, preset save/rename, settings page (save sequence verified byte-for-byte
      against the capture; live CCs verified on hardware; preset/settings *writes* not yet run on hardware)
- [x] Import NAM model (0.6.0): background conversion with progress/cancel, optimised or official algorithm
- [x] Import IR: optimised (default) or official conversion, upload to user IR slots
- [ ] Run preset/settings writes on hardware from the app
- [ ] Release the IR import (currently Unreleased in CHANGELOG.md)
- [ ] Captures for the library's open questions (`0x4A` loop mode, tuner control, preset unknown bytes, edit-buffer
      write; library docs/09)

## Environment notes

* The re-signed instrumented copy of the official app lives in `work/` (git-ignored). Rebuild with
  `tools/sniffer/build.sh`.
* Pedal firmware during captures: V1.1.1. Stomp Manager: 1.0.2+1.
* `STOMP_DEV=import` / `STOMP_DEV=import-ir` open the NAM / IR import dialogs at startup (screenshots).
