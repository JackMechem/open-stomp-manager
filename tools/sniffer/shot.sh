#!/bin/zsh
# Screenshot the Stomp Manager window; filename = unix ms (matches capture log timestamps) + label.
ROOT=${0:A:h:h:h}
mkdir -p "$ROOT/captures/screens"
WID=$(swift -e 'import CoreGraphics
let l = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String:Any]]
for w in l where (w["kCGWindowOwnerName"] as? String ?? "") == "Stomp Manager" && (w["kCGWindowLayer"] as? Int ?? 1) == 0 { print(w["kCGWindowNumber"]!); break }' 2>/dev/null)
[[ -z "$WID" ]] && { echo "window not found"; exit 1; }
TS=$(python3 -c 'import time;print(int(time.time()*1000))')
OUT="$ROOT/captures/screens/${TS}_${1:-shot}.png"
screencapture -x -o -l "$WID" /tmp/_sm_shot.png && sips -Z 1200 /tmp/_sm_shot.png --out "$OUT" >/dev/null && rm /tmp/_sm_shot.png
echo "$OUT"
