#!/usr/bin/env python3
"""Helpers for the pedal's packed SysEx payloads: 7 bits per byte, LSB-first little-endian bitstream."""
def bits_of(data: bytes) -> str:
    return "".join(format(x & 0x7F, "07b")[::-1] for x in data)

def read(bits: str, off: int, n: int) -> int:
    return int(bits[off:off + n][::-1], 2)

def unpack_bytes(data: bytes) -> bytes:
    """Repack the 7-bit stream into plain 8-bit bytes (LSB-first)."""
    b = bits_of(data)
    return bytes(read(b, i, 8) for i in range(0, len(b) - 7, 8))
