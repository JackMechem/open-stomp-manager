#!/usr/bin/env bash
# Build the namconv harness (macOS, needs the Xcode command-line tools and Stomp Manager installed).
set -euo pipefail
cd "$(dirname "$0")"
clang -O1 -g -Wall -o harness harness.c \
    -framework CoreFoundation \
    -Wl,-rpath,"/Applications/Stomp Manager.app/Contents/Frameworks"
echo "built tools/namconv/harness"
