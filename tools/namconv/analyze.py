#!/usr/bin/env python3
"""Check a namconv harness dump: the result against a captured upload, and how the buffers relate.

usage: analyze.py <dump dir> [captured device_floats.bin]      (needs numpy)
"""
import sys
from pathlib import Path

import numpy as np

SR, NFFT = 48000, 8192


def main() -> None:
    d = Path(sys.argv[1])
    ld = lambda n, t="<f4": np.fromfile(d / f"{n}.bin", t)  # noqa: E731
    g1, g2, factor = ld("cb_g1"), ld("cb_g2"), ld("cb_factor")[0]
    G1, G2 = ld("G1IR"), ld("G2IR")
    bins, bins3 = ld("ToneTest_FftBins", "<i4"), ld("ToneTest_FftBinsThird", "<i4")
    lv, lv3, gdb = ld("ToneTestOutputLevels"), ld("ToneTestOutputLevelsThird"), ld("g1GainsDb")

    print(f"factor = {factor}")
    if len(sys.argv) > 2:
        up = np.fromfile(sys.argv[2], "<f4")
        print("g1 ‖ g2 == captured upload:", np.concatenate([g1, g2]).tobytes() == up.tobytes())
    print("g1 == reverse(G1IR[:1024]):", np.array_equal(g1, G1[:1024][::-1]))
    print("g2 == reverse(G2IR[:1024]):", np.array_equal(g2, G2[:1024][::-1]))
    print(f"tone bins: {len(bins)}, {bins[0]}..{bins[-1]} = {bins[0] * SR / NFFT:.1f}..{bins[-1] * SR / NFFT:.0f} Hz")
    print("third-harmonic bins == 3 × fundamental bins:", np.array_equal(bins3, 3 * bins[: len(bins3)]))
    H1 = 20 * np.log10(np.abs(np.fft.rfft(G1, NFFT)) + 1e-30)
    H2 = 20 * np.log10(np.abs(np.fft.rfft(G2, NFFT)) + 1e-30)
    n = np.count_nonzero(gdb)
    diff = H1[bins[:n]] - gdb[:n]
    print(f"|G1IR| (dB) at tone bins − g1GainsDb: mean {diff.mean():+.3f}, std {diff.std():.3f}")
    diff2 = H2[bins] - 20 * np.log10(lv + 1e-30)
    print(f"|G2IR| (dB) at tone bins − 20·log10(levels): mean {diff2.mean():+.3f}, std {diff2.std():.3f}")
    print("levels (dB), first 8:", np.round(20 * np.log10(lv[:8]), 2))
    print("3rd-harmonic levels (dB), first 8:", np.round(20 * np.log10(lv3[:8] + 1e-30), 2))
    print("g1GainsDb, first 8:", np.round(gdb[:8], 2))


if __name__ == "__main__":
    main()
