#!/usr/bin/env python3
"""Render the app icon (an amber rounded square with two rings, the two footswitches) to the files
tauri.conf.json bundles: icon.png (1024 px, the window icon), 32x32.png, 128x128.png and
256x256.png (Linux), icon.ico (Windows, 16-256 px) and icon.icns (macOS, 16-1024 px).

Pure standard library: python3 app/src-tauri/icons/make_icons.py
"""
import math
import os
import struct
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
AMBER = (0xFF, 0xAE, 0x42)
DARK = (0x0D, 0x0F, 0x13)


def render(n: int, ss: int = 4) -> bytes:
    """RGBA pixels, `ss`×`ss` supersampled. Geometry is defined on a 128-unit square."""
    k = n / 128.0
    r, ring_r, ring_half, off = 28 * k, 22 * k, 6 * k, 20 * k
    s = float(n)
    out = bytearray(n * n * 4)
    for y in range(n):
        for x in range(n):
            acc = [0.0, 0.0, 0.0, 0.0]
            for sy in range(ss):
                for sx in range(ss):
                    fx, fy = x + (sx + 0.5) / ss, y + (sy + 0.5) / ss
                    cx, cy = min(max(fx, r), s - r), min(max(fy, r), s - r)
                    if math.hypot(fx - cx, fy - cy) > r:
                        continue
                    ring = any(
                        abs(math.hypot(fx - ox, fy - s / 2) - ring_r) < ring_half
                        for ox in (s / 2 - off, s / 2 + off)
                    )
                    c = DARK if ring else AMBER
                    acc[0] += c[0]
                    acc[1] += c[1]
                    acc[2] += c[2]
                    acc[3] += 255
            a = acc[3]
            i = (y * n + x) * 4
            if a:
                out[i : i + 4] = bytes([round(acc[0] * 255 / a), round(acc[1] * 255 / a), round(acc[2] * 255 / a), round(a / ss / ss)])
    return bytes(out)


def png(n: int, rgba: bytes) -> bytes:
    def chunk(t, b):
        return struct.pack(">I", len(b)) + t + b + struct.pack(">I", zlib.crc32(t + b) & 0xFFFFFFFF)

    raw = b"".join(b"\0" + rgba[y * n * 4 : (y + 1) * n * 4] for y in range(n))
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", n, n, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def main():
    sizes = [16, 32, 48, 64, 128, 256, 512, 1024]
    pngs = {n: png(n, render(n, 4 if n <= 256 else 2)) for n in sizes}

    for name, n in [("icon.png", 1024), ("32x32.png", 32), ("128x128.png", 128), ("256x256.png", 256)]:
        with open(os.path.join(HERE, name), "wb") as f:
            f.write(pngs[n])

    # ICO with PNG-compressed entries (Vista+)
    ico_sizes = [16, 32, 48, 64, 128, 256]
    header = struct.pack("<HHH", 0, 1, len(ico_sizes))
    offset = 6 + 16 * len(ico_sizes)
    entries, data = b"", b""
    for n in ico_sizes:
        b = pngs[n]
        entries += struct.pack("<BBBBHHII", n % 256, n % 256, 0, 0, 1, 32, len(b), offset + len(data))
        data += b
    with open(os.path.join(HERE, "icon.ico"), "wb") as f:
        f.write(header + entries + data)

    # ICNS with PNG entries
    types = {16: b"icp4", 32: b"icp5", 64: b"icp6", 128: b"ic07", 256: b"ic08", 512: b"ic09", 1024: b"ic10"}
    body = b"".join(t + struct.pack(">I", len(pngs[n]) + 8) + pngs[n] for n, t in types.items())
    with open(os.path.join(HERE, "icon.icns"), "wb") as f:
        f.write(b"icns" + struct.pack(">I", len(body) + 8) + body)


if __name__ == "__main__":
    main()
