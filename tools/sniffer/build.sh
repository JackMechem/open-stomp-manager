#!/bin/zsh
# Builds the sniffer dylib + midimon, and prepares a re-signed copy of Stomp Manager
# that accepts DYLD_INSERT_LIBRARIES (the original has hardened runtime enabled).
set -e
HERE=${0:A:h}
ROOT=${HERE:h:h}
APP="$ROOT/work/StompManager-sniff.app"

clang -arch arm64 -arch x86_64 -dynamiclib -fblocks -O2 -Wall \
  -framework CoreMIDI -framework CoreFoundation \
  -o "$HERE/midisniff.dylib" "$HERE/midisniff.c"
clang -arch arm64 -fblocks -O2 -Wall -framework CoreMIDI -framework CoreFoundation \
  -o "$HERE/midimon" "$HERE/midimon.c"

if [[ ! -d "$APP" ]]; then
  ditto "/Applications/Stomp Manager.app" "$APP"
fi
# Ad-hoc re-sign without hardened runtime so dyld honours DYLD_INSERT_LIBRARIES.
codesign --force --deep --sign - "$APP"
codesign --force --sign - "$HERE/midisniff.dylib"
echo "built OK"
