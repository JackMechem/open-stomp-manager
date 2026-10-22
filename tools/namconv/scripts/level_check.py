import sys, time
import numpy as np
sys.path.insert(0, sys.argv[1])
from audio_io import SR, find, playrec
from open_stomp import Device, DeviceState, Module

mtrack, pedal = find("M-Track"), find("STOMPBOX")
t = np.arange(int(2.0 * SR)) / SR
tone = (10 ** (-30 / 20) * np.sin(2 * np.pi * 110 * t)).astype(np.float32)
tone[:480] *= np.linspace(0, 1, 480); tone[-480:] *= np.linspace(1, 0, 480)

def report(label, rec):
    seg = rec[int(0.8 * SR):int(2.0 * SR)]
    for ch in range(rec.shape[1]):
        x = seg[:, ch].astype(np.float64)
        rms = np.sqrt(np.mean(x ** 2)) + 1e-12
        X = np.abs(np.fft.rfft(x * np.hanning(len(x))))
        f = np.fft.rfftfreq(len(x), 1 / SR)
        h = [X[np.argmin(abs(f - 110 * k))] for k in (1, 2, 3)]
        print(f"  {label:10} ch{ch + 1}: rms {20*np.log10(rms):6.1f} dBFS, peak {20*np.log10(np.abs(x).max()+1e-12):6.1f} dBFS, "
              f"2nd/3rd harmonic {20*np.log10(h[1]/h[0]+1e-12):6.1f} / {20*np.log10(h[2]/h[0]+1e-12):6.1f} dB")

st = DeviceState()
with Device() as dev:
    tt = time.time()
    while time.time() - tt < 20:
        u = dev.recv(timeout=0.3)
        if u: st.feed(u)
        if st.live_known and all(p is not None for p in st.presets): break
    amp = st.live.module(Module.AMP)
    print(f"preset {st.current}, AMP model {amp.model} ({st.model_name(Module.AMP, amp.model)}), on={amp.on}")
    report("amp ON", playrec(tone, mtrack, pedal))
    dev.set_head(Module.AMP, amp.model, False); time.sleep(0.3)
    report("amp OFF", playrec(tone, mtrack, pedal))
    dev.set_head(Module.AMP, amp.model, True); time.sleep(0.3)
    print("AMP block restored (on)")
