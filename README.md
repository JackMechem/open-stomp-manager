# Open Stomp Manager

**An open-source editor for the SORAN Dual Stomp Lite guitar pedal, for Linux, macOS and Windows.**

[![CI](https://github.com/JackMechem/open-stomp-manager/actions/workflows/ci.yml/badge.svg)](https://github.com/JackMechem/open-stomp-manager/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/JackMechem/open-stomp-manager)](https://github.com/JackMechem/open-stomp-manager/releases)
[![Docs](https://img.shields.io/badge/docs-site-blue)](https://jackmechem.github.io/open-stomp-lib/)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

Version **0.8.1** · [Download](https://github.com/JackMechem/open-stomp-manager/releases) ·
[User guide](docs/user-guide.md) · [Changelog](CHANGELOG.md)

Open Stomp Manager does what the vendor's *Stomp Manager* does, and also runs on Linux. Edit presets, watch the
pedal live, and import NAM amp models and cab IRs with conversions that sound closer to the original than the
official app's.

![Open Stomp Manager](docs/images/main-window.png)

> **Compatibility.** Tested on the Dual Stomp Lite, firmware V1.1.1. The Dual Stomp and Tiny Stomp share the
> firmware family and probably work too. Reports from owners are welcome.

## Features

- **Connects by itself** over USB or Bluetooth, with no driver, and reads all 99 presets, settings and user slots.
- **Live view** of the active preset, signal chain, every block's parameters, tuner, looper and drum machine.
  Changes made on the pedal show up instantly.
- **Editing:** pick models, switch blocks, turn knobs, set preset volume, run the looper and drum machine.
- **Save and rename** presets, and change system settings (input trim, global EQ, wireless, USB loopback).
- **Import NAM models and cab IRs** into the user slots
  ([how the conversion compares](https://jackmechem.github.io/open-stomp-lib/analysis/results.html)).
- **Activity log** of everything happening on the pedal, in plain English, with filters and search.
- **Listen-only mode** that never sends anything, and **capture replay** to try the app without a pedal.

Live editing and imports are verified on hardware. Preset save and settings writes send exactly what the official
app sends, but haven't been run on hardware yet, so back up your presets first.

## Install

Download the package for your platform from [Releases](https://github.com/JackMechem/open-stomp-manager/releases):

| Platform | Package |
|---|---|
| Linux (x86_64) | `open-stomp-manager-…-linux-x86_64.tar.gz` |
| macOS 11+ (Apple Silicon and Intel) | `open-stomp-manager-…-macos-universal.zip` |
| Windows 10+ (x86_64) | `open-stomp-manager-…-windows-x86_64.zip` |

- **macOS:** the app isn't notarized. The first time, right-click it and choose *Open*.
- **Windows:** quit the official Stomp Manager first; only one program can use the pedal's MIDI port at a time.
- **Nix:** `nix run github:JackMechem/open-stomp-manager/stable`. Use `/unstable` for pre-releases.

Then plug in the pedal. The [user guide](docs/user-guide.md) explains everything in the window.

## Building from source

The app depends on the [open-stomp](https://github.com/JackMechem/open-stomp-lib) library through a sibling
checkout, so clone both side by side:

```sh
git clone https://github.com/JackMechem/open-stomp-lib
git clone https://github.com/JackMechem/open-stomp-manager
cd open-stomp-manager
cargo run -p open-stomp-manager
```

You need Rust 1.95 or newer. On Linux, also install the ALSA and D-Bus headers (`libasound2-dev libdbus-1-dev` on
Debian/Ubuntu, `alsa-lib-devel dbus-devel` on Fedora). With Nix, `nix run` or `nix develop` covers everything.

No pedal? Replay a recorded session:

```sh
cargo run -p open-stomp-manager -- --replay captures/logs/01-main-session.log --speed 8
```

See [docs/building.md](docs/building.md) for packaging and platform details.

## Documentation

- [User guide](docs/user-guide.md): using the app, troubleshooting
- [Building](docs/building.md): builds, packaging, CI and releases
- [Architecture](docs/architecture.md): how the app is put together

The pedal's protocol, the library and the NAM/IR analysis are documented in
[open-stomp-lib](https://github.com/JackMechem/open-stomp-lib). Everything is on the
[documentation site](https://jackmechem.github.io/open-stomp-lib/).

## Contributing

Bug reports, feature ideas and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) to get started.
Changes to how the app talks to the pedal usually belong in the
[library](https://github.com/JackMechem/open-stomp-lib) first.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option. Unless you
state otherwise, any contribution you submit is dual-licensed the same way, without additional terms.

The bundled Inter font is licensed under the SIL Open Font License 1.1
([details](crates/open-stomp-manager/assets/fonts/README.md)).

## Disclaimer

Open Stomp Manager is an independent interoperability project. It is not affiliated with or endorsed by SORAN,
NUX/Cherub or JiYou; product names are trademarks of their owners. This repository contains no vendor binaries or
firmware. Use it at your own risk.
