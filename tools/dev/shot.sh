#!/usr/bin/env bash
# Render the app to a PNG: build, replay a capture, screenshot after a few seconds, quit.
#
#   tools/dev/shot.sh                      # -> target/shots/main.png
#   tools/dev/shot.sh menu menu            # -> target/shots/menu.png with the menu open
#   tools/dev/shot.sh log log 6            # activity log open, screenshot after 6 s
#
# Arguments: [name] [STOMP_DEV flags: menu,log] [seconds]. The window size is whatever the window
# manager gives it.
set -euo pipefail
cd "$(dirname "$0")/../.."
name=${1:-main} flags=${2:-} secs=${3:-4}
replay=${STOMP_REPLAY:-../open-stomp-lib/captures/logs/01-main-session.log}
mkdir -p target/shots
out=$PWD/target/shots/$name
nix develop --command bash -c "cargo build -q && STOMP_DEV='$flags' STOMP_SCREENSHOT='$out.ppm:$secs' \
    ./target/debug/open-stomp-manager --replay '$replay' --speed '${STOMP_SPEED:-20}' >/dev/null"
nix shell nixpkgs#imagemagick --command magick "$out.ppm" "$out.png"
rm -f "$out.ppm"
echo "$out.png"
