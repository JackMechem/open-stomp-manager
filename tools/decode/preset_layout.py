#!/usr/bin/env python3
"""Derive the preset struct layout: correlate unpacked preset dumps (SysEx 0x0B reads at startup) with the
CC values the pedal sends after Program Changes. usage: preset_layout.py <capture.log>"""
import sys, os, collections
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from unpack import unpack_bytes

L = open(sys.argv[1]).read().splitlines()
presets = {}
for l in L:
    if ' RX ' in l and ' 70 0B 02 ' in l:
        b = bytes(int(x, 16) for x in l.split(']')[1].split())
        u = unpack_bytes(b[7:-1])
        presets.setdefault(u[0], u)   # first read only (startup state)
print("presets read:", len(presets), "len", len(next(iter(presets.values()))))

# collect (preset_pc, cc, value) from dumps following a program change (until the next non-CC / gap)
obs = []
cur = None; last_ts = 0
for l in L:
    if l.startswith('#') or ']' not in l: continue
    p = l.split(); ts = int(p[0])
    b = bytes(int(x, 16) for x in l.split(']')[1].split())
    if p[1] in ('RX', 'TX') and b[0] & 0xF0 == 0xC0:
        cur = b[1]; last_ts = ts; continue
    if p[1] == 'RX' and cur is not None and ts - last_ts < 50:
        i = 0
        while i + 2 < len(b) + 0 and b[i] == 0xB0:
            obs.append((cur, b[i + 1], b[i + 2])); i += 3
        last_ts = ts
    elif ts - last_ts >= 50:
        cur = None
print("observations:", len(obs))

for shift in (-1, 0):
    # for each cc, find offsets consistent with all observations (index = pc + shift)
    res = {}
    by_cc = collections.defaultdict(list)
    for pc, cc, v in obs:
        if pc + shift in presets: by_cc[cc].append((presets[pc + shift], v))
    good = 0
    for cc, pairs in sorted(by_cc.items()):
        n = len(pairs[0][0])
        cands = [o for o in range(n) if all(u[o] == v for u, v in pairs)]
        if len(pairs) >= 2 and cands: good += 1
        res[cc] = (len(pairs), cands[:6])
    print(f"\nshift {shift}: ccs with consistent offset: {good}/{len(by_cc)}")
    if good > len(by_cc) * 0.6:
        for cc, (n, c) in res.items(): print(f"  CC 0x{cc:02X} (n={n}): offsets {c}")
