from pathlib import Path
import struct
from open_stomp import capture, Msg
from open_stomp.pack import unpack
ents = capture.parse(open(Path(__file__).resolve().parents[3] / "captures/logs/01-main-session.log").read())
seen=set()
for i,e in enumerate(ents):
    if e.kind!="message": continue
    m=Msg.parse(e.bytes)
    if m.kind!="nep" or m.cmd!=0x58: continue
    u=unpack(m.payload)
    if len(u)<140: continue
    rec=u[12:140]
    key=(m.dir,u[1],rec)
    if key in seen: continue
    seen.add(key)
    who="TX" if str(e.origin).endswith("HOST") else "RX"
    print(f"--- entry {i} {who} dir={m.dir} slot={u[1]} sub={u[2]}")
    for o in range(0,128,16):
        b=rec[o:o+16]
        fl=" ".join(f"{x:10.4g}" for x in struct.unpack("<4f",b))
        print(f"  {o:3d}: {b.hex(' ')}  |{''.join(chr(c) if 32<=c<127 else '.' for c in b)}|  {fl}")
