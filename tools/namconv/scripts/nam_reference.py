"""Fit the rig's calibration (input gain into the model, output gain) jointly against both pedal
recordings, then score each conversion against the real NAM model rendered in software."""
import json, sys, time
from pathlib import Path
import numpy as np
from open_stomp import nam

D = Path(sys.argv[1]); MODEL = sys.argv[2]
m = json.load(open(D / "map.json")); SR = m["sr"]
played = np.load(D / "played.npy").astype(np.float64)
runs = {n: np.load(D / f"{n}.npy").astype(np.float64) for n in ("A1", "B1")}
segs = m["segments"]

def xcorr_lag(a, b, maxlag):
    n = 1 << int(np.ceil(np.log2(len(a) + len(b))))
    c = np.fft.irfft(np.fft.rfft(b, n) * np.conj(np.fft.rfft(a, n)), n)
    c = np.concatenate([c[-maxlag:], c[:maxlag + 1]])
    i = int(np.argmax(c)); y0, y1, y2 = c[max(i-1, 0)], c[i], c[min(i+1, len(c)-1)]
    den = y0 - 2*y1 + y2
    return i - maxlag + (0.5*(y0 - y2)/den if den else 0.0)

def shift(x, d):
    X = np.fft.rfft(x); f = np.fft.rfftfreq(len(x))
    return np.fft.irfft(X*np.exp(-2j*np.pi*f*d), len(x))

def dc_block_fast(x):
    # 2nd-order IIR via direct form, vectorised in chunks with scipy-free recursion
    b = np.array([0.9953827, -1.990765, 0.9953827]); a = np.array([1.0, -1.990744, 0.9907867])
    y = np.empty_like(x); x1 = x2 = y1 = y2 = 0.0
    xs = x.tolist(); out = [0.0]*len(xs)
    for i, v in enumerate(xs):
        y0 = b[0]*v + b[1]*x1 + b[2]*x2 - a[1]*y1 - a[2]*y2
        out[i] = y0; x2, x1, y2, y1 = x1, v, y1, y0
    return np.array(out)

first = segs[0]
offsets = {n: xcorr_lag(played[first["start"]:first["start"] + SR], r[:first["start"] + 3*SR], 3*SR - 1) for n, r in runs.items()}
PAD = 2000
def rec_seg(n, s):
    o = int(round(offsets[n])) + s["start"] - PAD
    return runs[n][o:o + s["len"] + 2*PAD]
core = lambda s: slice(PAD + 2400, PAD + s["len"] - 2400)

model = nam.Model.load(MODEL)
B = (np.array([0.9953827, -1.990765, 0.9953827]), np.array([1.0, -1.990744, 0.9907867]))

def render(gin_db):
    model.reset()
    y = np.array(model.process((played * 10**(gin_db/20)).astype(np.float32).tolist()), dtype=np.float64)
    return dc_block_fast(y)

def score(y, detail=False):
    """Joint output gain over both recordings; per-segment residual (dB) for each."""
    pairs = []
    yp = np.concatenate([np.zeros(PAD), y, np.zeros(2*PAD)])
    for s in segs:
        ys = yp[s["start"]: s["start"] + s["len"] + 2*PAD]
        for n in runs:
            r = rec_seg(n, s)
            lag = xcorr_lag(ys, r, 2000)
            r_al = shift(r, -lag)[core(s)]
            pairs.append((s["name"], n, ys[core(s)], r_al))
    # weighted LS for one shared gain: minimise sum ||g*y - r||^2 / ||r||^2
    num = sum(np.dot(a, b) / np.dot(b, b) for _, _, a, b in pairs)
    den = sum(np.dot(a, a) / np.dot(b, b) for _, _, a, b in pairs)
    g = num / den
    res = {(s, n): 10*np.log10(np.sum((g*a - b)**2) / np.sum(b**2)) for s, n, a, b in pairs}
    total = 10*np.log10(np.mean([10**(v/10) for v in res.values()]))
    return (total, g, res) if detail else total

t0 = time.time()
grid = {}
for gin in range(-36, 25, 4):
    grid[gin] = score(render(gin)); print(f"  input gain {gin:+4d} dB -> mean residual {grid[gin]:6.2f} dB", flush=True)
best = min(grid, key=grid.get)
for gin in np.arange(best - 3, best + 3.01, 0.5):
    grid[float(gin)] = score(render(gin))
best = min(grid, key=grid.get)
total, g, res = score(render(best), detail=True)
print(f"\nbest input gain {best:+.1f} dB, output gain {20*np.log10(g):+.1f} dB, joint mean residual {total:.2f} dB ({time.time()-t0:.0f}s)")
print(f"\n{'segment':16} {'official vs NAM':>16} {'ours vs NAM':>12}")
wins = 0
for s in segs:
    a, b = res[(s['name'], 'A1')], res[(s['name'], 'B1')]
    wins += b < a
    print(f"{s['name']:16} {a:16.1f} {b:12.1f}  {'ours closer' if b < a else ''}")
mean = lambda n: 10*np.log10(np.mean([10**(res[(s['name'], n)]/10) for s in segs]))
print(f"\nmean residual vs NAM: official {mean('A1'):.2f} dB, ours {mean('B1'):.2f} dB; ours closer in {wins}/{len(segs)} segments")
np.save(D / "nam_render.npy", render(best) * g)
json.dump({"gin_db": best, "gout": g}, open(D / "nam_calibration.json", "w"))
