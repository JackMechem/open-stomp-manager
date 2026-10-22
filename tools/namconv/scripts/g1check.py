import numpy as np, sys
N = 8192
d = f"analysis/extracted/namconv/{sys.argv[1]}/"
ld = lambda n, t="<f4": np.fromfile(d + n + ".bin", t)
bins = ld("ToneTest_FftBins", "<i4"); gdb = ld("g1GainsDb")[:94]; G1 = ld("G1IR")

def g1_curve(bins, gdb):
    out = np.empty(4096, np.float32)
    out[:bins[0]] = gdb[0]
    lb = np.log10(bins.astype(np.float32))
    for i in range(len(bins) - 1):
        k = np.arange(bins[i], bins[i + 1])
        lk = np.log10(k.astype(np.float32))
        t = (lk - lb[i]) / ((lk - lb[i]) + (lb[i + 1] - lk))
        out[k] = t * gdb[i + 1] + (1 - t) * gdb[i]
    out[bins[-1]:] = gdb[-1]
    return out

def mirror(m):            # m[8191-k] = m[k]
    full = np.empty(N, np.float64); full[:4096] = m; full[4096:] = m[::-1]
    return full

def hilbert(x):           # analytic signal, standard definition
    X = np.fft.fft(x); h = np.zeros(N); h[0] = h[N // 2] = 1; h[1:N // 2] = 2
    return np.fft.ifft(X * h)

def minphase(m):
    L = np.where(m == 0, -30.0, np.log(np.where(m == 0, 1, m)))
    phi = -np.imag(hilbert(L))
    return np.real(np.fft.ifft(m * np.exp(1j * phi)))

dB = g1_curve(bins, gdb)
m = mirror(10 ** (dB.astype(np.float64) / 20))
ir = minphase(m)
err = np.abs(ir - G1)
print(f"{sys.argv[1]}: max |ours - G1IR| = {err.max():.3g}  (G1IR peak {np.abs(G1).max():.4g}), rel {err.max()/np.abs(G1).max():.3g}")
for alt, f in [("phi=+imag", lambda m: np.real(np.fft.ifft(m*np.exp(1j*np.imag(hilbert(np.log(m))))))),
               ("std mirror m[N-k]", lambda m_: minphase(np.concatenate([[m_[0]], m_[1:4096], [m_[4095]], m_[1:4096][::-1]])))]:
    pass
print("first taps ours:", np.round(ir[:5], 4), " G1IR:", np.round(G1[:5], 4))
