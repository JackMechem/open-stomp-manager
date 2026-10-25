# Building, packaging and releasing

The app is a [Tauri](https://tauri.app) program: Rust runs the pedal engine (the
[open-stomp](https://github.com/JackMechem/open-stomp-lib) library) and the interface is React and TypeScript, styled
with plain CSS, shown in the system's web view. [architecture.md](architecture.md) describes how the two halves
talk.

## Getting the source

The library is a path dependency on a **sibling checkout** (`app/src-tauri/Cargo.toml`). Clone both side by side:

```sh
git clone https://github.com/JackMechem/open-stomp-lib
git clone https://github.com/JackMechem/open-stomp-manager
cd open-stomp-manager
```

```
Projects/
├── open-stomp-lib/        the library: protocol, state, connection engine, protocol docs
└── open-stomp-manager/    this repo
    └── app/               the app: package.json, src/ (React), src-tauri/ (Rust)
```

Changes to the library are picked up by the next build; there is nothing to publish in between.

## Building

### With Nix

```sh
nix run                       # build and run
nix build                     # result/bin/open-stomp-manager
nix develop                   # dev shell: Rust, Node, pnpm, WebKitGTK, ALSA, amidi
```

The flake fetches the library as the `open-stomp-lib` input (`github:JackMechem/open-stomp-lib/stable`, the
library's newest release, pinned in `flake.lock`; `nix flake update open-stomp-lib` moves it). To build against your
local checkout instead:

```sh
nix run --override-input open-stomp-lib path:../open-stomp-lib
```

The frontend's npm packages are pinned by `app/pnpm-lock.yaml` and fetched with a fixed hash (`pnpmDeps` in
`flake.nix`). **After changing the lock file**, set that `hash` to `""`, run `nix build`, and paste the hash Nix
prints. CI's `nix build` job catches a stale hash.

### Without Nix

You need Rust ≥ 1.95, Node ≥ 22 and pnpm (the version in `app/package.json`'s `packageManager`; `corepack enable`
picks it up). On Linux, also the WebKitGTK, ALSA and D-Bus headers:

```sh
sudo apt install libwebkit2gtk-4.1-dev librsvg2-dev libasound2-dev libdbus-1-dev pkg-config   # Debian, Ubuntu
sudo dnf install webkit2gtk4.1-devel librsvg2-devel alsa-lib-devel dbus-devel pkgconf         # Fedora
```

macOS and Windows need nothing extra (Windows 10 and 11 have WebView2).

```sh
cd app
pnpm install
pnpm tauri dev                                          # run, reloading as you edit
pnpm tauri dev -- -- --replay ../captures/logs/01-main-session.log --speed 8   # no pedal needed
pnpm tauri build                                        # release build and installers
```

### Checks

```sh
(cd app && pnpm typecheck)                 # TypeScript
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                     # the hub's commands, the activity log, imports
(cd ../open-stomp-lib && cargo test)       # the protocol and engine tests, driven by capture logs
packaging/check-version.sh                 # version consistency
```

## Developing

`pnpm tauri dev` (in `app/`) starts Vite and the app. Saving a `.tsx` or `.css` file updates the window at once and
keeps its state; saving a Rust file rebuilds and restarts the app. Right-click → **Inspect** opens the web inspector
(debug builds), which is the quickest way to try out CSS.

Starting on a replay (`-- -- --replay <log>`) doesn't connect to the pedal, so it's safe while another editor is
using it.

| Variable | Effect |
|---|---|
| `STOMP_DEV=log,import,import-ir,save,rename` | Open the activity log or that dialog at startup |
| `STOMP_DEV=devtools` | Open the web inspector at startup (debug builds) |
| `STOMP_DEBUG_IMPORT=<file.nam or .wav>` | Convert the file at startup into USER AMP/IR 10, **without uploading it** |

### Driving the app from scripts (Linux)

`tools/dev/webdriver.py` controls the real app through WebKitGTK's `WebKitWebDriver`: click, drag, type, read the
state the frontend has (`window.__stomp` in debug builds) and take screenshots. `tools/dev/shot.py` uses it to
screenshot the app on a replayed capture, for the docs:

```sh
nix develop --command tools/dev/shot.py                    # target/shots/main.png
nix develop --command tools/dev/shot.py log --dev log      # with the activity log open
```

## Platforms

| | MIDI | Web view | Release downloads |
|---|---|---|---|
| Linux (x86_64) | ALSA | WebKitGTK 4.1 | `.deb`, `.rpm`, `.AppImage` |
| macOS 11+ (Apple Silicon + Intel) | CoreMIDI | WKWebView | `.dmg`, and the `.app` as a `.zip` |
| Windows 10+ (x86_64) | WinMM | WebView2 | `-setup.exe` installer, portable `.zip` |

`pnpm tauri build` makes the installers for the platform it runs on, in `target/release/bundle/`. Their settings
(name, identifier, icons, Linux package dependencies, minimum macOS version) are in `app/src-tauri/tauri.conf.json`.

* **Linux:** the `.deb` and `.rpm` depend on WebKitGTK, GTK, ALSA and D-Bus (D-Bus is for Bluetooth, through BlueZ).
  The AppImage bundles WebKitGTK, so it is much larger. The Nix package is built from the `.deb`'s contents and
  wrapped so GTK finds its schemas and modules. On NVIDIA with Wayland, a blank window usually needs
  `WEBKIT_DISABLE_DMABUF_RENDERER=1`.
* **macOS:** a universal app (`--target universal-apple-darwin`, needs `rustup target add aarch64-apple-darwin
  x86_64-apple-darwin`), ad-hoc signed (`signingIdentity: "-"`), not notarized. `app/src-tauri/Info.plist` adds the
  Bluetooth usage description; macOS kills an app that uses Bluetooth without one.
* **Windows:** release builds use the GUI subsystem (no console window); `--help`/`--version` attach to the
  launching console. The installer downloads WebView2 if it's missing (Windows 10 before 21H2); the portable `.zip`
  needs it present.

Icons are generated by `app/src-tauri/icons/make_icons.py`.

## Version

The version lives in one place, `version` under `[workspace.package]` in `Cargo.toml`, and follows
[SemVer](https://semver.org) (0.x: anything may change). Tauri reads it from there for the installers. `build.rs`
adds the short git commit (`STOMP_GIT_HASH`, empty outside a git checkout) and the library version from
`Cargo.lock`; `--version`, the window title and the Device card show them.

## Continuous integration

`.github/workflows/ci.yml` runs on every push and pull request, on Linux, macOS and Windows: the frontend type-check
and build, clippy with warnings as errors, tests, and a release build without installers. Other jobs check the
minimum Rust version, `nix build` (which also catches a stale npm hash in `flake.nix`), and
`packaging/check-version.sh`. Each job first clones `open-stomp-lib` next to the checkout, at the ref in
`OPEN_STOMP_LIB_REF` (top of the workflow).

The documentation site is built in open-stomp-lib from both repositories' docs. When `docs/`, `CHANGELOG.md` or
`Cargo.toml` change on `main`, `.github/workflows/docs.yml` starts that build. It needs a `DOCS_SITE_TOKEN` secret:
a fine-grained token with **Actions: Read and write** on open-stomp-lib only. Without it the job skips with a
warning, and the site catches up at its daily rebuild.

## Releasing

1. If the app needs library changes, release the library first (tag it there) and set `OPEN_STOMP_LIB_REF` in both
   workflows to that tag, so the release builds against a fixed library version. Once the library's `stable` branch
   has moved, run `nix flake update open-stomp-lib` so the Nix build uses the same version (the release checks it).
2. Bump `Cargo.toml`, add a `## [x.y.z]` section to [CHANGELOG.md](../CHANGELOG.md), update the version line at the
   top of the README, and run `cargo build` so `Cargo.lock` follows. `packaging/check-version.sh` checks all of this.
3. Commit, then `git tag vx.y.z && git push --tags`. `.github/workflows/release.yml` runs `tauri build` on Linux,
   macOS and Windows and publishes a GitHub release with the changelog section as its notes. The downloads are named
   `open-stomp-manager-vx.y.z-<platform>.<ext>`.

A pre-release is a tag with a suffix, `vx.y.z-beta.n` (with that version in `Cargo.toml`). It needs no README or
CHANGELOG changes (its notes are the `[Unreleased]` section) and is published as a GitHub pre-release. If it needs a
library pre-release, point the flake input at `github:JackMechem/open-stomp-lib/unstable` for it.

**Channels.** After each successful release, `.github/workflows/channels.yml` moves two branches
(`packaging/update-channels.sh`): `stable` to the newest release and `unstable` to the newest release or
pre-release. Ticking or clearing a release's pre-release box, or deleting it, moves them again; Actions → Channels →
Run workflow re-syncs them by hand. A Nix config that follows `github:JackMechem/open-stomp-manager/stable` gets
each release with `nix flake update`; the library comes from this flake's `flake.lock`.
