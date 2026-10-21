#!/usr/bin/env bash
# Live preview while styling: rebuild and relaunch the app on every source change (this crate or the
# sibling open-stomp-lib), replaying a capture so the UI has something to show without a pedal.
#
#   tools/dev/watch.sh                     # replay 01-main-session.log at 20x
#   STOMP_DEV=log tools/dev/watch.sh       # ... with the activity log open
#   STOMP_SPEED=4 STOMP_REPLAY=path.log tools/dev/watch.sh
#
# Press F12 in the window (debug builds) for egui's style editor and inspector.
set -euo pipefail
cd "$(dirname "$0")/../.."
replay=${STOMP_REPLAY:-../open-stomp-lib/captures/logs/01-main-session.log}
exec nix develop --command nix shell nixpkgs#watchexec --command \
    watchexec --restart --watch crates --watch ../open-stomp-lib/src --exts rs,toml -- \
    cargo run -q -- --replay "$replay" --speed "${STOMP_SPEED:-20}"
