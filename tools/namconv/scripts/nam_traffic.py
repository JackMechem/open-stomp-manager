from pathlib import Path
import sys
from open_stomp import capture, Msg
from open_stomp.pack import unpack
from open_stomp.midi import hex
log = str(Path(__file__).resolve().parents[3] / "captures/logs/01-main-session.log")
ents = capture.parse(open(log).read())
lo, hi = int(sys.argv[1]), int(sys.argv[2])
marks = [i for i,e in enumerate(ents) if e.kind=="mark"]
rows=[]
for i,e in enumerate(ents):
    if e.kind=="mark":
        rows.append((i, "MARK "+e.text)); continue
    m = Msg.parse(e.bytes)
    who = "TX" if str(e.origin).endswith("HOST") else "RX"
    if m.kind=="nep" and m.cmd in (0x58,0x10,0x09):
        u = unpack(m.payload)
        sub = u[2] if len(u)>2 else None
        rows.append((i, f"{who} {e.ms} cmd={m.cmd:02X} dir={m.dir} midi={len(m.payload)} raw={len(u)} hdr={hex(u[:12])} sub={sub}"))
    elif m.kind in ("cc","pc","notify"):
        rows.append((i, f"{who} {e.ms} {m!r} {hex(e.bytes)}"))
for i,r in rows:
    if lo<=i<=hi: print(i, r)
