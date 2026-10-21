#!/usr/bin/env python3
"""Tail the current capture log; when a module-model CC (or page CC 0x3B / program change) arrives,
screenshot the Stomp Manager window after a short delay. Filenames: <ms>_<tag>.png"""
import os, subprocess, sys, time
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
log = open(os.path.join(ROOT, "captures", "CURRENT")).read().strip()
SHOT = os.path.join(ROOT, "tools", "sniffer", "shot.sh")
TRIG = {int(x, 16) for x in (sys.argv[1:] or ["00", "06", "3B"])}
f = open(log); f.seek(0, 2)
pending, last = None, 0
while True:
    line = f.readline()
    if not line:
        if pending and time.time() - last > 0.6:
            subprocess.run([SHOT, pending], capture_output=True); print("shot", pending, flush=True); pending = None
        time.sleep(0.1); continue
    p = line.split()
    if len(p) < 6 or p[1] != "RX": continue
    b = [int(x, 16) for x in line[line.index("]") + 1:].split()]
    tag = None
    if b[0] == 0xB0 and b[1] in TRIG: tag = f"cc{b[1]:02X}-{b[2]:02X}"
    elif b[0] == 0xC0: tag = f"pc{b[1]:02X}"
    if tag:
        pending, last = tag, time.time()
