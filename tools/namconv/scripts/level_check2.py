import sys, time
import numpy as np
sys.path.insert(0, sys.argv[1])
from audio_io import SR, find, playrec
from open_stomp import Device, DeviceState, Module

mtrack, pedal = find("M-Track"), find("STOMPBOX")

def tone(db, f=110.0, secs=1.5):
    t = np.arange(int(secs * SR)) / SR
    x = (10 ** (db / 20) * np.sin(2 * np.pi * f * t)).astype(np.float32)
    x[:480] *= np.linspace(0, 1, 480); x[-480:] *= np.linspace(1, 0, 480)
    return x

def measure(rec, f=110.0):
    x = rec[int(0.8 * SR):int(1.6 * SR), 0].astype(np.float64)
    X = np.abs(np.fft.rfft(x * np.hanning(len(x)))); fr = np.fft.rfftfreq(len(x), 1 / SR)
    h = [X[np.argmin(abs(fr - f * k))] for k in (1, 2, 3)]
    return 20*np.log10(np.sqrt(np.mean(x**2))+1e-12), 20*np.log10(np.abs(x).max()+1e-12), 20*np.log10(h[1]/h[0]+1e-12), 20*np.log10(h[2]/h[0]+1e-12)

st = DeviceState()
with Device() as dev:
    tt = time.time()
    while time.time() - tt < 20:
        u = dev.recv(timeout=0.3)
        if u: st.feed(u)
        if st.live_known and all(p is not None for p in st.presets): break
    saved = {m: st.live.module(m) for m in Module.EFFECTS}
    print("blocks on now:", [m.label for m, s in saved.items() if s.on], "| volume", st.live.volume)
    try:
        for m, s in saved.items():
            dev.set_head(m, s.model, False)
        time.sleep(0.4)
        for db in (-30, -18, -12):
            r = measure(playrec(tone(db), mtrack, pedal))
            print(f"  ALL OFF  send {db:4d} dBFS -> rms {r[0]:6.1f} peak {r[1]:6.1f} dBFS | H2 {r[2]:6.1f} H3 {r[3]:6.1f} dB")
        amp = saved[Module.AMP]
        dev.set_head(Module.AMP, amp.model, True); time.sleep(0.4)
        for db in (-30, -18, -12):
            r = measure(playrec(tone(db), mtrack, pedal))
            print(f"  AMP only send {db:4d} dBFS -> rms {r[0]:6.1f} peak {r[1]:6.1f} dBFS | H2 {r[2]:6.1f} H3 {r[3]:6.1f} dB")
    finally:
        for m, s in saved.items():
            dev.set_head(m, s.model, s.on)
        time.sleep(0.3)
        print("blocks restored")
