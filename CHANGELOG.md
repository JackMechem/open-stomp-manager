# Changelog

All notable changes to Open Stomp Manager (named Dual Stomp Manager / `stomp-manager` up
to 0.2.0). The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Wireless: with the pedal's Wireless setting on, the app also finds and connects to it over Bluetooth LE MIDI, with
  no pairing (open-stomp `ble` feature). It is listed in the port menu as "JME STOMPBOX (Bluetooth)" with a Bluetooth
  sign, the header says "· Bluetooth", and everything works as over USB except that the sync takes about 20 s.
  Auto-connect and the port menu prefer USB. NAM/IR imports over Bluetooth are untested.

### Changed
- macOS: the bundle's Info.plist has a Bluetooth usage description (macOS kills an app that uses Bluetooth without one).
- Linux: building needs the D-Bus headers (`libdbus-1-dev`) too.
- Turning the pedal's preset knob several clicks jumps to where it stopped instead of stepping through every preset
  on the way: after a preset change from the pedal, its messages are held until it has been quiet for 0.15 s (at
  most 5 s), then applied together. Over Bluetooth each click's data trickles in for a few tenths of a second.
- The edit-buffer read is padded like the official app's (open-stomp fix), so it can't return a stale record.

## [0.7.1] - 2026-10-22

### Added
- **Rename…** on user AMP and IR slots (right-click a slot under User slots): changes only the name, the sound stays
  (open-stomp-lib 0.5.1's `rename_user_amp` / `rename_user_ir`; the official app can't rename). `STOMP_DEV=rename`
  opens the dialog at startup.

### Changed
- Built against open-stomp-lib 0.5.1 (0.5.0 brought the NAM low-end fix, 0.5.1 rename). NAM imports (optimised) are much closer to the model in the low end: the fit
  now weights every octave equally instead of leaving the bass almost unweighted. Re-amping a bass DI through the
  pedal, the error against the model and IR drops from 5.2 to 3.3 dB at 40–160 Hz and from 3.5 to 2.1 dB overall,
  together with the optimised IR import, whose fit now covers 30 Hz – 16 kHz.

## [0.7.0] - 2026-10-22

### Added
- Built against open-stomp-lib 0.4.0.
- Import NAM model: **Normalise level** (on by default), every model at the same volume like NAM plugins, instead of
  the often quiet level the model was captured at. A pure volume change.
- **Import IR…** (menu, the user slots list, or right-click a user IR slot): pick a WAV impulse response and a slot;
  it is converted with open-stomp's optimised IR conversion (default: fitted to the 384 samples the pedal plays,
  much closer to the real IR on the pedal) or exactly like the official app, and uploaded.
- `STOMP_DEV=import-ir` opens the Import IR dialog at startup.

## [0.6.0] - 2026-10-21

### Added
- **Import NAM model…** (menu, the user slots list, or right-click a user AMP slot): pick a `.nam` file and a slot;
  the model is converted in the background (progress bar, cancel) and uploaded. The default is open-stomp's
  optimised conversion (about a minute, closer to the real model than the official app's on hardware); the official
  algorithm is the fast alternative. Native file dialogs via `rfd` (XDG portal on Linux through a runtime-loaded libdbus, no
  GTK; the Nix package and dev shell provide libdbus).
- `STOMP_DEV=import` opens the import dialog at startup (for screenshots).

### Changed
- Built against open-stomp-lib 0.3.0 (optimised NAM conversion).
- Debug builds compile the open-stomp library and `rustfft` optimised, so the NAM conversion stays usable.

## [0.5.0] - 2026-10-21

### Added
- Preset search above the preset list: filter by name or slot (`lead`, `5`, `P05`); Ctrl+F (⌘F) focuses it, Enter
  selects the first match, Esc clears it.
- The open-stomp library version is shown in the version chip's tooltip, the menu, `--version` and `--help`.

### Changed
- The top bar is down to the app name, the status pill and one menu button. The menu holds the port picker,
  connect/disconnect, re-sync, listen only, replay and the activity log toggle.
- Calmer look: sentence-case headings and labels instead of all caps, and borders only on what should stand out
  (the active preset, the selected block, focused fields). The menu has proper padding.
- No monospace text: everything is set in Inter, with tabular digits built into the bundled font so changing
  numbers stay put. JetBrains Mono is no longer bundled.
- The activity log is hidden by default; open it from the menu or with Ctrl+L (⌘L on macOS), close it from its
  header.
- Builds against open-stomp-lib v0.2.0 (CI, releases and the Nix flake). The flake now locks
  the library input instead of fetching the library's latest `main`.

## [0.4.0] - 2026-10-21

### Changed
- The protocol library and the connection engine moved to a separate repository and crate,
  [open-stomp](https://github.com/JackMechem/open-stomp-lib) (`open_stomp`), which the app now uses.
  It replaces `crates/stomp-proto` and the app's own `device` module, carries the full protocol
  documentation plus a library guide, and the capture-driven tests. Building the app now needs a
  checkout of `open-stomp-lib` next to this repository (CI, the release workflow and the Nix
  flake fetch it; CI and releases pin open-stomp-lib v0.1.0).
- Documentation split: the protocol and reverse-engineering docs (01–09) now live in
  [open-stomp-lib](https://github.com/JackMechem/open-stomp-lib/tree/main/docs) and this
  repository links there. Its own docs are about the app: a [user guide](docs/user-guide.md),
  [building](docs/building.md) and [architecture](docs/architecture.md), and the README is
  rewritten around the app.

## [0.3.0] - 2026-10-21

### Changed
- **Renamed** the project and app to **Open Stomp Manager**, to keep clear of the vendor's
  *Stomp Manager* name. The binary and crate are now `open-stomp-manager` (was `stomp-manager`),
  the macOS app is `Open Stomp Manager.app`, the Linux desktop entry and icon are
  `open-stomp-manager`, and the repository is
  [JackMechem/open-stomp-manager](https://github.com/JackMechem/open-stomp-manager).

### Fixed
- Repository and release links pointed at the wrong GitHub account.
- The declared minimum Rust version is now 1.95 (what egui 0.36 requires; 1.88 never built).

## [0.2.0] - 2026-10-21

### Added
- **Editor**: control the pedal from the app.
  - Select presets; **Save** and **Save as…** (rename and/or store in another slot) using the official
    app's exact message sequence.
  - Pick effect models, switch blocks on/off, turn knobs (drag, scroll wheel, Shift for fine
    steps), set preset volume.
  - Looper transport (rec/play/dub, stop, undo/redo, clear), levels, sync and mode; drum
    play/stop, pattern and level.
  - System settings: input trim, global EQ, wireless, loopback.
- Opening a block in the app shows its page on the pedal, like the official app.
- `stomp-proto`: preset and settings encoders (`Preset::to_raw`, `Settings::to_raw`) and the
  `midi::host` command builders, tested byte-for-byte against the capture.
- **Cross-platform releases**: CI on Linux, macOS and Windows; tag-triggered release builds
  (Linux tarball, universal macOS `.app`, Windows `.exe`); app icons for every platform.
- Version display in the app (and the window title), `--version`, and this changelog.

### Fixed
- macOS: the pedal is detected when plugged in after the app starts (CoreMIDI hotplug
  notifications need a run loop on the MIDI thread).
- Windows: `--help` / `--version` print to the terminal in release builds.

### Protocol findings
- Host → pedal CCs `0x40` (drum pattern) and `0x42`/`0x43` (loop levels) confirmed on hardware.
- The first byte of the `0x12` preset-name write is the active preset.

## [0.1.0] - 2026-10-21

### Added
- `stomp-proto`: protocol library (8-in-7 packing, SysEx framing, catalogue, preset/settings
  records, state tracker), tested against the capture logs.
- `stomp-manager`: live monitor GUI with auto-connect, read-only sync, active preset, signal
  chain, parameters, tuner/looper/drum, coalescing activity log and capture replay.

[Unreleased]: https://github.com/JackMechem/open-stomp-manager/compare/v0.7.1...HEAD
[0.7.1]: https://github.com/JackMechem/open-stomp-manager/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/JackMechem/open-stomp-manager/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/JackMechem/open-stomp-manager/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/JackMechem/open-stomp-manager/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/JackMechem/open-stomp-manager/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/JackMechem/open-stomp-manager/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/JackMechem/open-stomp-manager/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/JackMechem/open-stomp-manager/releases/tag/v0.1.0
