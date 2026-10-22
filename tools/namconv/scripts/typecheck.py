from open_stomp import Device, DeviceState, Module, Preset
from open_stomp.midi.host import cc
st = DeviceState()
p: Preset = st.live
p.set_param(Module.AMP, 0, 100)
b: bytes = cc(0x12, 64)
with Device(sync=False) as d:
    u = d.recv(0.1)
    if u is not None and u.kind == "message" and u.bytes is not None and u.origin is not None:
        print(st.apply(u.bytes, u.origin).subject)
