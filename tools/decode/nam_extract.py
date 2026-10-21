#!/usr/bin/env python3
"""Extract the float32 payload the app sends for a NAM import (SysEx 0x58 sub 'data' chunks).
usage: nam_extract.py <capture.log> <marker-substring> [out.bin]
Each chunk body (after `70 58 01`) is a 7-bit LSB-first stream: 12-byte header, then float32 LE data."""
import struct, sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from unpack import bits_of, read

CMD = 0x58

def chunks_after(log, marker, cmd=None, data_sub=1):
    """cmd 0x58 (NAM): data_sub=1; cmd 0x10 (IR upload): data_sub=0"""
    global CMD
    if cmd is not None: CMD = cmd
    L = open(log).read().splitlines()
    i = [k for k, l in enumerate(L) if marker in l][0]
    out, started = [], False
    for l in L[i + 1:]:
        if ' TX ' not in l or f' 70 {CMD:02X} 01 ' not in l: continue
        body = bytes(int(x, 16) for x in l.split(']')[1].split())[7:-1]
        bs = bits_of(body)
        sub = read(bs, 16, 8)
        if sub == data_sub:  # data chunk
            out.append(body); started = True
        elif started:
            break
    return out

def floats(chunks):
    vals = []
    for c in chunks:
        bs = bits_of(c)
        n = (len(bs) - 96) // 32
        vals += [struct.unpack('<f', struct.pack('<I', read(bs, 96 + 32 * k, 32)))[0] for k in range(n)]
    return vals

if __name__ == "__main__":
    ch = chunks_after(sys.argv[1], sys.argv[2])
    v = floats(ch)
    print(len(ch), "chunks,", len(v), "floats")
    if len(sys.argv) > 3:
        open(sys.argv[3], "wb").write(struct.pack(f"<{len(v)}f", *v))
