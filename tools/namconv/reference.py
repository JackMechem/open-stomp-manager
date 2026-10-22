#!/usr/bin/env python3
"""Reference implementation of Stomp Manager's NAM -> device conversion (imagelib NamFileThread::run),
from the disassembly, checked stage by stage against a harness dump.

usage: reference.py <dump dir>     (dump made with: harness <model.nam> <dir> --log-io; needs numpy)

The model itself is not run here: its input/output for each call is read from model_io.bin. Constants
and the order of operations follow the binary; see docs/05 in open-stomp-lib for the description.
"""
import sys
from pathlib import Path

import numpy as np

N = 8192                      # FFT size (48 kHz -> 5.859375 Hz per bin)
HALF = N // 2
F32 = np.float32

# DC blocker applied to every model output (global state, never reset between calls)
B = (F32(0.9953827), F32(-1.990765), F32(0.9953827))
A = (F32(-1.990744), F32(0.9907867))


class DcBlocker:
    def __init__(self):
        self.x1 = self.x2 = self.y1 = self.y2 = F32(0)

    def __call__(self, x):
        out = np.empty_like(x)
        b0, b1, b2 = B
        a1, a2 = A
        x1, x2, y1, y2 = self.x1, self.x2, self.y1, self.y2
        for n, x0 in enumerate(x):
            y0 = F32(F32(F32(x1 * b1) + F32(x0 * b0)) + F32(x2 * b2)) - F32(F32(y2 * a2) + F32(y1 * a1))
            out[n] = y0
            x2, x1, y2, y1 = x1, x0, y1, y0
        self.x1, self.x2, self.y1, self.y2 = x1, x2, y1, y2
        return out


def spectrum(x, magnitude):
    """getSpectrum: 8192-point FFT (no window, no scaling), bins 0..4095, power or magnitude."""
    p = np.abs(np.fft.fft(x.astype(np.float64), N)[:HALF]) ** 2
    return np.sqrt(p) if magnitude else p


def smooth(h):
    """1/24-octave power smoothing with log-frequency triangular weights (bins 2..4095)."""
    r = 1.0145453                # 2^(1/48)
    out = np.empty(HALF)
    p = h.astype(np.float64) ** 2
    for k in range(2, HALF):
        lo = max(int(np.floor(np.float32(k) / np.float32(r))), 2)   # fcvtms
        hi = max(int(np.ceil(np.float32(k) * np.float32(r))), 2)     # fcvtps
        hi = max(min(hi, HALF - 1), lo)
        m = max(k, lo)
        lm, llo, lhi = np.log10(m), np.log10(lo - 1), np.log10(hi + 1)
        j = np.arange(lo, hi + 1)
        lj = np.log10(j)
        w = np.where(j > m, (lhi - lj) / (lhi - lm) if lhi != lm else 1.0, (lj - llo) / (lm - llo))
        out[k] = (w * p[lo:hi + 1]).sum() / w.sum()
    out[0] = out[1] = out[2]
    return np.sqrt(out)


def interp_linear(values, bins, below, size=HALF):
    out = np.empty(size)
    out[:bins[0]] = below
    for i in range(len(bins) - 1):
        k = np.arange(bins[i], bins[i + 1])
        t = (k - bins[i]) / ((k - bins[i]) + (bins[i + 1] - k))
        out[k] = t * values[i + 1] + (1 - t) * values[i]
    out[bins[-1]:] = values[-1]
    return out


def interp_log(values, bins):
    out = np.empty(HALF)
    out[:bins[0]] = values[0]
    lb = np.log10(bins.astype(np.float64))
    for i in range(len(bins) - 1):
        k = np.arange(bins[i], bins[i + 1])
        lk = np.log10(k.astype(np.float64))
        t = (lk - lb[i]) / ((lk - lb[i]) + (lb[i + 1] - lk))
        out[k] = t * values[i + 1] + (1 - t) * values[i]
    out[bins[-1]:] = values[-1]
    return out


def min_phase(db):
    """dB curve over bins 0..4095 -> 8192-tap minimum-phase impulse response (Magnitude2MinPhase)."""
    m = 10 ** (db / 20)
    full = np.concatenate([m, m[::-1]])                 # the app mirrors m[8191-k] = m[k]
    log_m = np.log(np.where(full == 0, np.exp(-30.0), full))
    X = np.fft.fft(log_m)
    h = np.zeros(N); h[0] = h[HALF] = 1; h[1:HALF] = 2
    phase = -np.imag(np.fft.ifft(X * h))                # -Hilbert(log|m|)
    return np.real(np.fft.ifft(full * np.exp(1j * phase)))


def convert(calls, bins, bins3):
    """calls: [(input, output)] for the BLA run then each tone. Returns every stage."""
    dc = DcBlocker()
    bla_x, bla_y = calls[0]
    y = dc(bla_y)
    H = spectrum(y, True) / spectrum(bla_x, True)
    H_db = 20 * np.log10(H)
    Hs = smooth(H)

    lv = np.empty(len(bins)); lv3 = np.empty(len(bins3))
    for i, (_, out) in enumerate(calls[1:]):
        P = spectrum(dc(out)[2048:], False)
        lv[i] = np.sqrt(P[bins[i]]) * (np.pi / 4) / 4096
        if i < len(bins3):
            lv3[i] = np.sqrt(P[bins3[i]]) * (3 * np.pi / 4) / 4096

    F = interp_linear(lv, bins, lv[0])
    T = interp_linear(lv3, bins3, 0.0)
    M = np.maximum(F, T)
    g1_db = 20 * np.log10(Hs[bins] / M[bins])

    G1_db = interp_log(g1_db, bins)
    G2_db = H_db - G1_db
    G1, G2 = min_phase(G1_db), min_phase(G2_db)
    return dict(levels=lv, levels3=lv3, g1GainsDb=g1_db, G1IR=G1, G2IR=G2,
                g1=G1[:1024][::-1], g2=G2[:1024][::-1])


def read_calls(path):
    b = path.read_bytes()
    i, calls = 0, []
    while i < len(b):
        n = int(np.frombuffer(b[i:i + 4], "<i4")[0]); i += 4
        x = np.frombuffer(b[i:i + 4 * n], "<f4"); i += 4 * n
        y = np.frombuffer(b[i:i + 4 * n], "<f4"); i += 4 * n
        calls.append((x, y))
    return calls


def main():
    d = Path(sys.argv[1])
    ld = lambda n, t="<f4": np.fromfile(d / f"{n}.bin", t)  # noqa: E731
    bins = ld("ToneTest_FftBins", "<i4"); bins3 = ld("ToneTest_FftBinsThird", "<i4")
    r = convert(read_calls(d / "model_io.bin"), bins, bins3)
    ref = dict(levels=ld("ToneTestOutputLevels"), levels3=ld("ToneTestOutputLevelsThird"),
               g1GainsDb=ld("g1GainsDb")[:len(bins)], G1IR=ld("G1IR"), G2IR=ld("G2IR"),
               g1=ld("cb_g1"), g2=ld("cb_g2"))
    for k, v in r.items():
        e = np.abs(v - ref[k]).max()
        print(f"{k:10} max abs err {e:10.3g}   (peak {np.abs(ref[k]).max():.4g}, rel {e / np.abs(ref[k]).max():.2g})")


if __name__ == "__main__":
    main()
