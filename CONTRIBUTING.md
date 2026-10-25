# Contributing to Open Stomp Manager

Thanks for your interest in Open Stomp Manager! You don't need to know Rust to help: bug reports, UI feedback,
testing on your OS or pedal, and docs fixes are all valuable.

## Ways to help

- **Report a bug or suggest a feature.**
  [Open an issue](https://github.com/JackMechem/open-stomp-manager/issues/new/choose).
- **Test on your setup.** Your OS, desktop environment, or a pedal other than the Dual Stomp Lite.
- **Improve the docs**, especially the [user guide](docs/user-guide.md).
- **Write code.** See [where help is wanted](#where-help-is-wanted) below.

For anything bigger than a small fix, please open an issue first so we can agree on the approach before you spend
time on it.

## Which repository?

| Repository | What goes there |
|---|---|
| **open-stomp-manager** (this one) | The app's UI, packaging, and the capture and reverse-engineering tools |
| [open-stomp-lib](https://github.com/JackMechem/open-stomp-lib) | The protocol, device behaviour, NAM/IR conversion, protocol docs |

If a feature needs the pedal to do something new (a message, a record field, a command), add it to the library
first, then use it here. The library has its own [contributing guide](https://github.com/JackMechem/open-stomp-lib/blob/main/CONTRIBUTING.md).

## Setting up

You need [Rust](https://rustup.rs) 1.95 or newer. On Linux, also install the ALSA and D-Bus headers:

```sh
sudo apt install libasound2-dev libdbus-1-dev pkg-config     # Debian, Ubuntu
sudo dnf install alsa-lib-devel dbus-devel pkgconf           # Fedora
```

The app uses the library from a sibling checkout, so clone both side by side:

```sh
git clone https://github.com/JackMechem/open-stomp-lib
git clone https://github.com/JackMechem/open-stomp-manager
cd open-stomp-manager
cargo run -p open-stomp-manager
```

Library changes in `../open-stomp-lib` are picked up by the next build. With Nix, `nix develop` gives you a shell
with everything.

### Project layout

```
crates/open-stomp-manager/  the app: main.rs (startup), app.rs (panels), slot_import.rs (NAM/IR import),
                            logbook.rs (activity log), widgets.rs, theme.rs
docs/                       user guide, building, architecture
packaging/                  icons, Linux .desktop file, macOS bundle script, release helpers
tools/dev/                  live-reload and screenshot scripts for UI work
tools/, captures/, analysis/  reverse-engineering tools and data (see below)
```

### Working without a pedal

Replay a recorded session to develop the UI without a pedal:

```sh
cargo run -p open-stomp-manager -- --replay captures/logs/01-main-session.log --speed 8
tools/dev/watch.sh      # rebuild and relaunch on every save, replaying a capture (needs Nix)
```

In debug builds, **F12** opens egui's style editor and inspector. [docs/building.md](docs/building.md#live-preview-styling)
has more development shortcuts.

### Working with a pedal

Live edits only change the pedal's current sound, so they are safe to experiment with. Saving presets and changing
settings overwrite stored data: back up your presets first. **Listen only** in the menu stops the app from
sending anything.

## Checks

CI runs these on Linux, macOS and Windows. Run them before opening a pull request:

```sh
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Code guidelines

- **Match the surrounding code.** The codebase isn't formatted with `rustfmt`, so please don't reformat code you
  aren't otherwise changing. Lines go up to about 120 columns.
- **Read state, send through the engine.** Panels only read `DeviceState`; user actions go through the `Device`
  helpers so listen-only mode and the activity log keep working. [docs/architecture.md](docs/architecture.md)
  explains why.
- **Reuse the widgets and theme.** `widgets.rs` and `theme.rs` keep new UI consistent with the rest.
- **Include a screenshot** in pull requests that change the UI.

## Pull requests

1. Fork the repository and create a branch from `main`.
2. Make your change, and update the [user guide](docs/user-guide.md) if users would see it.
3. Add a line to the `[Unreleased]` section of [CHANGELOG.md](CHANGELOG.md) if users would notice the change.
4. Run the [checks](#checks).
5. Open a pull request that says what changed and why. Link the issue if there is one, and say which OS you tested
   on and whether you used a real pedal.

If your change needs an unreleased library change, open the library pull request first and link it.

## Where help is wanted

- **Preset backup and restore** to a file.
- **Preset and settings writes on hardware.** They send exactly what the official app sends, but haven't been run
  on a pedal from the app yet. Reports either way help.
- **Linux packaging** beyond the tarball and Nix: Flatpak, AppImage, distribution packages.
- **New captures** for the library's [open questions](https://github.com/JackMechem/open-stomp-lib/blob/main/docs/09-open-questions.md)
  (see below).

## Reverse-engineering tools

This repository also holds the tools that recovered the protocol. You only need them to investigate something the
library doesn't know yet.

| Path | What it is |
|---|---|
| `tools/sniffer/` | macOS CoreMIDI interposer that records the official app's MIDI traffic |
| `tools/decode/` | Python decoders for capture logs, and screenshot OCR |
| `tools/namconv/` | Harness and scripts used to work out the official NAM conversion |
| `captures/logs/` | Recorded sessions (also the library's test vectors) |
| `captures/LAB_NOTES.md` | The notebook from the original capture session |
| `analysis/` | Static-analysis dumps of the vendor app, and the scripts that made them |

How to make new captures is described in the library's
[capture methodology](https://github.com/JackMechem/open-stomp-lib/blob/main/docs/07-capture-methodology.md).
Never commit vendor binaries, firmware, or third-party NAM/IR files; `.gitignore` already excludes the usual
locations.

## Releases

Releases are made by the maintainer; [docs/building.md](docs/building.md#releasing) describes the steps.

## License

By contributing, you agree that your contributions are dual-licensed under [MIT](LICENSE-MIT) and
[Apache-2.0](LICENSE-APACHE), like the rest of the project.
