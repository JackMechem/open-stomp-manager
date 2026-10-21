#!/bin/zsh
# Launch the instrumented Stomp Manager. Usage: run.sh [session-name]
# Log goes to captures/logs/<session-name>.log (default: timestamp).
HERE=${0:A:h}
ROOT=${HERE:h:h}
NAME=${1:-session-$(date +%Y%m%d-%H%M%S)}
mkdir -p "$ROOT/captures/logs"
LOG="$ROOT/captures/logs/$NAME.log"
echo "$LOG" > "$ROOT/captures/CURRENT"
echo "logging to $LOG"
DYLD_INSERT_LIBRARIES="$HERE/midisniff.dylib" SNIFF_LOG="$LOG" \
  "$ROOT/work/StompManager-sniff.app/Contents/MacOS/Stomp Manager"
