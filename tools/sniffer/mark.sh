#!/bin/zsh
# Append an annotation to the current capture log:  mark.sh "turned gain knob up"
ROOT=${0:A:h:h:h}
LOG=$(cat "$ROOT/captures/CURRENT")
ts=$(python3 -c 'import time;print(int(time.time()*1000))')
echo "# MARK $ts $*" >> "$LOG"
echo "marked: $*"
