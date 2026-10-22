import numpy as np, time
x = np.fromfile("analysis/extracted/namconv/FNDR/BLA_x.bin", "<f4").astype(np.float64)
X = np.fft.rfft(x)
meas = np.angle(X)                     # x = A sin(phi + w n)  ->  angle = phi - pi/2
want = np.mod(meas + np.pi / 2, 2 * np.pi)   # phi per bin

def pm_next(s):                        # vectorised Park-Miller step (macOS rand)
    s = np.where(s == 0, 123459876, s)
    hi, lo = np.divmod(s, 127773)
    t = 16807 * lo - 2836 * hi
    return np.where(t < 0, t + 0x7fffffff, t)

def phase(r):
    return (r.astype(np.float32) / np.float32(2.1474836e9) * np.float32(6.2831855)).astype(np.float64)

K = 12                                  # rand values to generate per seed
t0 = time.time()
best = []
for start in range(0, 1 << 22, 1 << 18):
    seeds = np.arange(start, start + (1 << 18), dtype=np.int64)
    s = seeds.copy()
    rs = []
    for _ in range(K):
        s = pm_next(s); rs.append(phase(s))
    R = np.stack(rs, 1)                 # (seeds, K)
    for first_bin in (0, 1, 7):         # which bin the first rand() belongs to
        tgt = want[first_bin:first_bin + K]
        d = np.abs(np.angle(np.exp(1j * (R - tgt))))
        score = d[:, 1:8].max(1)        # ignore DC
        i = np.argmin(score)
        best.append((score[i], int(seeds[i]), first_bin))
best.sort()
print("best (max phase error, seed, first bin):", best[:5], f"({time.time()-t0:.1f}s)")
