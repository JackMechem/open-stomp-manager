#!/usr/bin/env python3
"""Analyze midisniff capture logs.

usage: analyze.py <log> [--raw] [--uniq] [--gap MS]

- Reassembles SysEx (F0..F7) that CoreMIDI may split across packets.
- Prints a timeline with marks, grouping bursts separated by > --gap ms.
- --uniq: summarise distinct message "shapes" (first N header bytes) with counts.
"""
import sys, collections, argparse

ap = argparse.ArgumentParser()
ap.add_argument("log")
ap.add_argument("--raw", action="store_true", help="print every packet, no reassembly")
ap.add_argument("--uniq", action="store_true")
ap.add_argument("--hdr", type=int, default=7, help="header bytes used as key for --uniq")
ap.add_argument("--gap", type=int, default=400)
ap.add_argument("--max", type=int, default=64, help="max bytes to print per message")
a = ap.parse_args()

events = []  # (ts, dir, bytes) or (ts, 'MARK', text)
partial = {}
for line in open(a.log, errors="replace"):
    line = line.rstrip("\n")
    if line.startswith("# MARK"):
        _, _, ts, *txt = line.split(" ")
        events.append((int(ts), "MARK", " ".join(txt)))
        continue
    if not line or line.startswith("#"):
        continue
    parts = line.split(" ")
    ts, d = int(parts[0]), parts[1]
    if d == "INFO":
        continue
    rb = line.index("]")
    data = bytes(int(x, 16) for x in line[rb + 1:].split())
    if a.raw:
        events.append((ts, d, data))
        continue
    buf = partial.get(d, b"") + data
    # split into complete messages
    while buf:
        if buf[0] == 0xF0:
            end = buf.find(b"\xF7")
            if end < 0:
                break
            events.append((ts, d, buf[: end + 1]))
            buf = buf[end + 1:]
        else:
            # channel/realtime message: cut at next status byte or F0
            i = 1
            while i < len(buf) and buf[i] < 0x80:
                i += 1
            events.append((ts, d, buf[:i]))
            buf = buf[i:]
    partial[d] = buf


def hx(b):
    s = " ".join(f"{x:02X}" for x in b[: a.max])
    return s + (f" ...(+{len(b) - a.max})" if len(b) > a.max else "")


if a.uniq:
    c = collections.Counter()
    ex = {}
    lens = collections.defaultdict(set)
    for ts, d, b in events:
        if d == "MARK":
            continue
        k = (d, b[: a.hdr])
        c[k] += 1
        ex.setdefault(k, b)
        lens[k].add(len(b))
    for (d, h), n in sorted(c.items(), key=lambda kv: (kv[0][1], kv[0][0])):
        L = sorted(lens[(d, h)])
        print(f"{d:5} x{n:<5} len={L[0]}..{L[-1]:<5} {hx(h)}   e.g. {hx(ex[(d, h)])}")
    sys.exit()

t0 = events[0][0] if events else 0
last = None
for ts, d, b in events:
    if last is not None and ts - last > a.gap:
        print("-" * 40)
    last = ts
    if d == "MARK":
        print(f"{(ts - t0) / 1000:9.3f}  ===== {b} =====")
    else:
        print(f"{(ts - t0) / 1000:9.3f}  {d:5} {len(b):5}  {hx(b)}")
