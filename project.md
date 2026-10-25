# project.md: Open Stomp Manager (desktop editor + reverse-engineering workbench)

## Goal

A desktop editor (Rust + Tauri, React interface) for **Linux, macOS and Windows** for the SORAN Dual Stomp Lite with the features of the
official *Stomp Manager*: live editing, preset management, settings, looper/drum/tuner, and NAM/IR import.

All protocol work (encode/decode, state, connection engine, uploads, NAM/IR conversion) and the protocol docs live
in the [`open-stomp`](https://github.com/JackMechem/open-stomp-lib) library (`~/Projects/open-stomp/open-stomp-lib`),
which this app depends on by path. The protocol cheat sheet, how it was recovered, and the protocol roadmap are in
**that repo's project.md**. This file covers the app and the reverse-engineering workbench kept here.

## What lives here

* `app/`: the app. `app/src-tauri/` is the Rust side (hub thread owning the engine + `DeviceState`, JSON sections in
  `view.rs`, commands in `hub::Cmd`); `app/src/` is the React/TypeScript interface, styled with plain CSS
  (`docs/architecture.md`). Replaced the egui app (last egui release: 0.8.1).
* `tools/dev/webdriver.py`, `shot.py`: drive the real app through WebKitWebDriver (Linux), headless under Xvfb for
  screenshots; debug builds expose the store as `window.__stomp`.
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
- [x] Import IR (0.7.0): optimised (default) or official conversion, upload to user IR slots
- [x] Rename user AMP/IR slots (0.7.1): right-click a slot under User slots; changes only the name
- [x] Bluetooth LE MIDI (0.8.0, lib 0.6.0): auto-connects with no pairing, prefers USB; imports work over BLE too
- [x] Release channels: `stable` / `unstable` branches moved by `channels.yml`; pre-releases are `vX.Y.Z-beta.N`
      (`nix run github:JackMechem/open-stomp-manager/stable`)
- [x] Tauri + React app replaces egui (branch `tauri-ui`): parity with egui, tested on the pedal through WebDriver
      (live edits, NAM/IR import into USER AMP/IR 10, rename); `tauri build` installers in release.yml; Nix package
      via cargo-tauri.hook + fetchPnpmDeps (bump the `pnpmDeps` hash when `app/pnpm-lock.yaml` changes)
- [ ] Release it (0.9.0) once CI is green on GitHub (macOS/Windows bundles have only been built in CI config, not run)
- [ ] Run preset/settings writes on hardware from the app
- [ ] Captures for the library's open questions (`0x4A` loop mode, tuner control, preset unknown bytes, edit-buffer
      write; library docs/09)

## Environment notes

* The re-signed instrumented copy of the official app lives in `work/` (git-ignored). Rebuild with
  `tools/sniffer/build.sh`.
* Pedal firmware during captures: V1.1.1. Stomp Manager: 1.0.2+1.
* `STOMP_DEV=log,import,import-ir,save,rename,devtools` opens those at startup; `STOMP_DEBUG_IMPORT=<file>` converts
  without uploading.
* Run: `cd app && nix develop .. --command sh -c 'pnpm tauri dev'` (add `-- -- --replay <log>` to start on a capture
  without touching the pedal). In this environment `nix develop --command <prog>` mangles args; wrap in `sh -c`.
