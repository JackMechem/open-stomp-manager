import numpy as np
d="analysis/extracted/namconv/FNDR/"
lt=np.fromfile(d+"levelsTesterItem.bin","<f4"); primes=np.fromfile(d+"oddPrimeNumbers.bin","<u4")

class BsdRand:                      # macOS libc rand(): Park-Miller "minimal standard", Schrage
    def __init__(self, seed): self.x = seed
    def __call__(self):
        x = self.x or 123459876
        hi, lo = divmod(x, 127773)
        x = 16807 * lo - 2836 * hi
        if x < 0: x += 0x7fffffff
        self.x = x
        return x

def levels_tester(n_primes=0x76a):
    rnd = BsdRand(0x21)
    out = np.zeros(8192, np.float32)
    n = np.arange(8192, dtype=np.float64)
    for p in primes[:n_primes]:
        r = rnd()
        f = np.float32(np.float32(p) * np.float32(5.859375))
        if not (40.0 <= f < 24000.0):
            continue
        ph = float(np.float32(np.float32(np.float32(r) / np.float32(2.1474836e9)) * np.float32(6.2831855)))
        w = float(f) / 48000.0 * 6.283185307179586
        amp = float(np.float32(np.sqrt(np.float32(1.0) / f)))
        out = (out.astype(np.float64) + np.sin(ph + n * w) * amp).astype(np.float32)
    return out / np.abs(out).max()

ours = levels_tester()
print("max |ours - levelsTesterItem| =", np.abs(ours - lt).max(), " bit-identical:", np.array_equal(ours, lt))
