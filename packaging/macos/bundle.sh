#!/usr/bin/env bash
# Build "Open Stomp Manager.app" in target/macos/.
#
#   packaging/macos/bundle.sh             universal binary (needs both Rust targets:
#                                         rustup target add aarch64-apple-darwin x86_64-apple-darwin)
#   packaging/macos/bundle.sh --native    this Mac's architecture only
#
# The app is ad-hoc signed, not notarized: on first launch of a downloaded copy, right-click → Open
# (or run `xattr -dr com.apple.quarantine "Open Stomp Manager.app"`).
set -euo pipefail
cd "$(dirname "$0")/../.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
APP="target/macos/Open Stomp Manager.app"

if [[ "${1:-}" == "--native" ]]; then
    cargo build --release -p open-stomp-manager
    bins=(target/release/open-stomp-manager)
else
    bins=()
    for t in aarch64-apple-darwin x86_64-apple-darwin; do
        cargo build --release -p open-stomp-manager --target "$t"
        bins+=("target/$t/release/open-stomp-manager")
    done
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/open-stomp-manager" "${bins[@]}"
cp packaging/icons/icon.icns "$APP/Contents/Resources/open-stomp-manager.icns"
sed "s/@VERSION@/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
codesign --force --deep --sign - "$APP"
echo "built $APP ($VERSION)"
