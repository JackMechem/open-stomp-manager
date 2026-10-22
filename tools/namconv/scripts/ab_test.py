from pathlib import Path
"""A/B test of NAM import on the pedal: USER AMP 02 = the app's conversion uploaded by us,
USER AMP 03 = our conversion. Then re-read all slots from the pedal to confirm."""
import array, sys, time
from open_stomp import Device, DeviceState, nam

M = str(Path(__file__).resolve().parents[3]) + "/"
official = array.array("f"); official.frombytes(open(M + "captures/files/Jcm800.device_floats.bin", "rb").read())
print("converting Jcm800.nam with our converter …")
ours = nam.convert(M + "captures/files/Jcm800.nam")

st = DeviceState()

def pump(dev, until, timeout):
    t = time.time()
    while time.time() - t < timeout:
        u = dev.recv(timeout=0.2)
        if u is None:
            if until(): return True
            continue
        st.feed(u)
        if u.kind == "info": print("  ", ("ERROR " if u.error else "") + u.text)
    return until()

with Device() as dev:
    synced = lambda: all(p is not None for p in st.presets) and all(a is not None for a in st.user_amps)
    if not pump(dev, synced, 20): sys.exit("pedal did not sync")
    print("synced; slots before:", st.user_amps[:3])

    dev.import_user_amp(1, "Jcm800 app data", list(official))   # USER AMP 02
    dev.import_user_amp(2, "Jcm800 open", ours.data)              # USER AMP 03
    print("uploading 2 × 62 messages …")
    pump(dev, lambda: False, 6)                                   # let the queue drain (acks)

    st = DeviceState(); dev.resync()                              # re-read everything from the pedal
    if not pump(dev, synced, 20): sys.exit("re-read failed")
    print("slots after (read back from the pedal):", st.user_amps[:3])
    ok = st.user_amps[1] == "Jcm800 app data" and st.user_amps[2] == "Jcm800 open"
    print("PASS: both uploads stored" if ok else "FAIL: names not as expected")
