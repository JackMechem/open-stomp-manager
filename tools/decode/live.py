#!/usr/bin/env python3
"""Live decoder: tails the current capture log and prints human-readable labels.

usage: python3 tools/decode/live.py [logfile]      (default: the log named in captures/CURRENT)
       python3 tools/decode/live.py --all [logfile] (decode the whole file from the start)
Unknown CCs/SysEx are printed raw with '??' so we can spot what still needs mapping.
"""
import os, sys, time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
args = [a for a in sys.argv[1:] if not a.startswith("--")]
FROM_START = "--all" in sys.argv
LOG = args[0] if args else open(os.path.join(ROOT, "captures", "CURRENT")).read().strip()

# ---- knowledge so far (keep in sync with captures/findings_live.md) ----
OD = ["Red Comp", "RS Comp", "Yellow Dist", "Boost RC", "Boost AC", "DS1", "TS9", "BD2", "Klone", "Ratt2",
      "Red Drive", "Cruncher", "Muff", "Boost", "Face Fuzz", "ST Drive"]
OD_P = [["OUTPUT", "SENSITIVITY"], ["SUSTAIN", "LEVEL"], ["OUTPUT", "Distortion"],
        ["GAIN", "VOLUME", "BASS", "TREBLE"], ["GAIN", "VOLUME", "BASS", "TREBLE"], ["GAIN", "TONE", "VOLUME"],
        ["DRIVE", "TONE", "LEVEL"], ["LEVEL", "TONE", "GAIN"], ["VOLUME", "DRIVE", "TONE"],
        ["DIST", "FILTER", "VOLUME"], ["DRIVE", "TONE", "LEVEL"], ["VOLUME", "TONE", "GAIN"],
        ["VOLUME", "TONE", "SUSTAIN"], ["GAIN BOOST", "VOLUME"], ["GAIN", "TONE", "OUTPUT"],
        ["VOLUME", "GAIN", "FILTER"]]
AMPS = ["Black Twin", "Tweed 5F1", "Bassguy", "Prince Rev", "Tweed 5F8", "Deluxe Rev", "Bdotone CL", "Voxy 30HW",
        "Mar J800", "Mar J45", "Mar Plexi100", "Jazz CL", "Mes Rector", "Mes MKIV Lead", "Mes MKV Red",
        "Mes LStar CL", "Cornfed M50", "ODS-100", "Spawn Qrod", "Soloist 100", "Fryman HB", "Fryman DS", "PVH 61",
        "Eddie 51", "2Rock J35", "Ampug SV Bass", "Ampug Port Bass", "FR Bass", "Mes 400 Bass", "Agula DP751"] + \
       [f"USER AMP {i:02}" for i in range(1, 11)]
CABS = ["Twin 212", "Tweed 110", "Bassguy 410", "Prince 112", "Tweed 210", "Deluxe 112", "Bdotone 212", "Voxy 212",
        "Mar G12M 412", "Mar Silver 212", "Mar V30 412", "Jazz 212", "Mes OS 412", "Mes Trad 412", "Mes V30 212",
        "Mes EV 112", "Cornfed 212", "ODS 212", "Spawn 412", "SLO 412", "Fryman 412", "Fryman 212", "PVH 412",
        "Eddie 412", "2Rock 212", "Ampug 810", "Ampug 410", "FR 210", "Mes 410", "Agula 810"] + \
       [f"USER IR {i:02}" for i in range(1, 11)]
MICS = ["E906", "M160", "MD421", "R121", "SM57", "SM58", "SM7B", "U47"]
POS = ["EDGE", "MIDDLE", "CENTER"]
NOTES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
PAGES = {0: "OD/FX", 1: "AMP", 2: "CAB", 6: "GATE", 7: "EQ"}
EQ6 = ["100Hz", "220Hz", "500Hz", "1.2kHz", "2.5kHz", "6.4kHz"]
EQ10 = ["GAIN", "31Hz", "62.5Hz", "125Hz", "250Hz", "500Hz", "1kHz", "2kHz", "4kHz", "8kHz", "16kHz", "VOLUME"]

MOD = ["Chorus-1", "Chorus-2", "Stereo Chorus", "Vibrato", "Detune", "Flanger", "Phaser 90", "Phaser 100",
       "Chorus+", "Uni Vibe", "Tremolo", "Chorus-3", "Harmony"]
KEYS = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"]
HARM = ["-8TH", "-6TH", "-5TH", "-4TH", "-3TH", "3TH", "4TH", "5TH", "6TH", "8TH"]
MOD_P = [["INTENSITY", "DEPTH", "RATE"], ["RATE", "DEPTH"], ["INTENSITY", "WIDTH", "RATE"], ["RATE", "DEPTH"],
         ["L", "MIX", "R"], ["LEVEL", "RATE", "WIDTH", "FEEDBACK"], ["SPEED"], ["INTENSITY", "SPEED"],
         ["SPEED", "WIDTH", ("MODE", ["CHORUS", "FLANGER"]), "INTENSITY"],
         ["SPEED", "VOLUME", "INTENSITY", ("MODE", ["CHORUS", "VIBE"])], ["RATE", "DEPTH"],
         ["RATE", "DEPTH", "TONE"], [("KEY", KEYS), ("MODE", ["MINOR", "MAJOR"]), ("HARM", HARM), "MIX"]]
PAGES[3] = "MOD"; PAGES[4] = "DELAY"; PAGES[5] = "REVERB"; PAGES[8] = "LOOP"; PAGES[9] = "DRUM"
RVB = ["Room", "Hall", "Plate", "Spring", "Shimmer", "Damp"]
RVB_P = [["DECAY", "TONE", "LEVEL"], ["DECAY", "TONE", "PRE DELAY (ms)", "LEVEL"], ["DECAY", "LEVEL"],
         ["DECAY", "LEVEL"], ["LEVEL", "DECAY", "SHIMMER"], ["LEVEL", "DEPTH"]]
DLY = ["Analog Delay", "Digital Delay", "Mod Delay", "Tape Delay", "Reverse Delay", "Pan Delay", "Physical Delay"]
DLY_P = [["TIME", "FEEDBACK", "LEVEL"]] * 2 + [["TIME", "FEEDBACK", "MOD", "LEVEL"]] + [["TIME", "FEEDBACK", "LEVEL"]] * 4

def param(module, models, plist, idx, i, v):
    names = plist[idx] if idx < len(plist) else []
    if i >= len(names):
        return f"{module:6} {models[idx] if idx < len(models) else idx} . P{i + 1}?? = {v}"
    n = names[i]
    if isinstance(n, tuple):
        n, opts = n
        return f"{module:6} {models[idx]} . {n} = {opts[v] if v < len(opts) else v}"
    return f"{module:6} {models[idx]} . {n} = {v}"

HEADS = {0x05: ("REVERB", RVB), 0x04: ("DELAY", DLY), 0x03: ("MOD", MOD), 0x00: ("OD/FX", OD), 0x01: ("AMP", AMPS), 0x02: ("CAB", CABS), 0x06: ("GATE", None), 0x07: ("EQ", ["6-band EQ", "10-band EQ"])}
state = {"od": 0, "eq": 0, "mod": 0, "dly": 0, "rvb": 0}

def head(cc, v):
    name, models = HEADS[cc]
    m = v & 0x3F
    mn = models[m] if models and m < len(models) else f"model {m}"
    if cc == 0x00:
        state["od"] = m
    if cc == 0x07:
        state["eq"] = m
    if cc == 0x03:
        state["mod"] = m
    if cc == 0x04:
        state["dly"] = m
    if cc == 0x05:
        state["rvb"] = m
    return f"{name:6} {'OFF' if v & 0x40 else 'ON '}  model {m:2} (app #{m + 1:02}) = {mn}"

def cc_label(cc, v):
    if cc in HEADS:
        return head(cc, v)
    if 0x0C <= cc <= 0x0F:
        names = OD_P[state["od"]] if state["od"] < len(OD_P) else []
        i = cc - 0x0C
        return f"OD/FX  {OD[state['od']]} . {names[i] if i < len(names) else f'P{i + 1}?'} = {v}"
    amp = {0x12: "GAIN", 0x13: "BASS", 0x14: "MIDDLE", 0x15: "TREBLE", 0x16: "PRESENCE", 0x17: "MASTER"}
    if cc in amp:
        return f"AMP    {amp[cc]} = {v}"
    if cc == 0x1A: return f"CAB    MIC = {MICS[v] if v < len(MICS) else v}"
    if cc == 0x1B: return f"CAB    POSITION = {POS[v] if v < len(POS) else v}"
    if cc == 0x1C: return f"CAB    LOW CUT raw = {v}"
    if cc == 0x1D: return f"CAB    HIGH CUT raw = {v}"
    if cc == 0x1E: return f"CAB    VOLUME = {v}"
    if 0x36 <= cc <= 0x39:
        return param("REVERB", RVB, RVB_P, state["rvb"], cc - 0x36, v)
    if 0x30 <= cc <= 0x33:
        return param("DELAY", DLY, DLY_P, state["dly"], cc - 0x30, v)
    if 0x2C <= cc <= 0x2F:
        return param("MOD", MOD, MOD_P, state["mod"], cc - 0x2C, v)
    if 0x20 <= cc <= 0x2B:
        names = EQ10 if state["eq"] == 1 else EQ6
        i = cc - 0x20
        return f"EQ     {names[i] if i < len(names) else f'band {i}?'} raw = {v} ({v - 50:+d})"
    if cc == 0x08: return f"GATE   THRESHOLD = {v}"
    if cc == 0x09: return f"GATE   DECAY = {v}"
    if cc == 0x3A: return f"VOLUME preset VOL = {v}"
    if cc == 0x3B: return f"PAGE   pedal shows {PAGES.get(v, f'module {v}?')}"
    if cc == 0x3F: return f"DRUM   {'PLAYING' if v else 'stopped'}"
    if cc == 0x40:
        st = ["Pop", "Rock", "Metal", "Disco", "Funk", "Blues", "Hip Hop", "Jazz", "Latin", "Metro"]
        return f"DRUM   pattern {v} = {st[v // 10] if v < 100 else '?'} {v % 10 + 1:02}"
    if cc == 0x41: return f"DRUM   level = {v}"
    if cc == 0x42: return f"LOOPER REC level = {v}"
    if cc == 0x43: return f"LOOPER PLAYBACK level = {v}"
    if cc == 0x49: return f"LOOPER LOOP SYNC = {['Off', 'On'][v] if v < 2 else v}"
    if cc == 0x4A: return f"LOOPER LOOP MODE = {['NR', 'AR'][v] if v < 2 else v}"
    if cc == 0x44:
        st = {0: "empty/cleared", 1: "RECORDING", 2: "playing", 3: "OVERDUBBING", 4: "stopped"}.get(v & 0x0F, f"state {v & 15}?")
        fl = (" [overdub layer on top: undo available]" if v & 0x10 else "") + (" [base layer only]" if v & 0x20 else "")
        return f"LOOPER {st}{fl}"
    if cc == 0x45: return f"TUNER  note {'detected' if v else 'released'}"
    if cc == 0x46: return f"TUNER  note = {NOTES[v % 12]}"
    if cc == 0x48: return f"TUNER  cents raw = {v} ({v - 50:+d} from 0x32)"
    return f"??     CC 0x{cc:02X} = {v} (0x{v:02X})"

SYX = {0x00: "hello/version", 0x09: "IR slot info", 0x0B: "preset", 0x0C: "current preset", 0x0D: "module link",
       0x12: "preset name", 0x14: "system settings", 0x15: "current state", 0x16: "delete IR", 0x18: "loop settings",
       0x19: "drum", 0x1B: "bluetooth", 0x6F: "tuner/mode", 0x7E: "NOTIFY"}
DIRS = {0: "read", 1: "WRITE", 2: "reply", 3: "write-ack"}

def syx_label(b):
    if b[1:4] != bytes([0x00, 0x21, 0x7A]):
        return f"??     SysEx (other mfr) {b.hex(' ').upper()}"
    if len(b) == 6:
        return f"SYSEX  hello/version {'request' if b[4] == 0 else 'reply'}"
    if len(b) > 6 and b[4] == 0x10:
        txt = bytes(x for x in b[5:-1] if 32 <= x < 127).decode()
        return f"SYSEX  version reply: {txt}"
    if len(b) < 8 or b[4] != 0x70:
        return f"??     SysEx {b.hex(' ').upper()}"
    cmd, d = b[5], b[6]
    if cmd == 0x7E:
        if b[7] == 0x19 and len(b) >= 16:
            return f"SYSEX  NOTIFY drum/tempo: BPM = {(b[13] << 7) | b[14]}  (raw {b[8:-1].hex(' ').upper()})"
        return f"SYSEX  NOTIFY: device says 0x{b[7]:02X} ({SYX.get(b[7], '?')}) changed"
    body = b[7:-1]
    s = body[:24].hex(" ").upper() + (" ..." if len(body) > 24 else "")
    return f"SYSEX  {SYX.get(cmd, f'cmd 0x{cmd:02X}??'):16} {DIRS.get(d, f'dir {d}?'):9} [{len(body)}] {s}"

def decode(direction, b):
    if not b: return None
    s = b[0]
    if s == 0xF0: return syx_label(b)
    if s & 0xF0 == 0xC0: return (f"[ch{(s & 15) + 1}] " if s & 15 else "") + f"PRESET -> P{b[1] + 1:02} (program change / index {b[1]})"
    if s & 0xF0 == 0xB0 and len(b) >= 3:
        ch = (s & 0x0F) + 1
        if ch == 2 and b[1] == 0x44:
            return "[ch2] LOOPER button: " + {1: "REC/PLAY/DUB", 2: "STOP", 4: "CLEAR", 8: "UNDO/REDO"}.get(b[2], f"0x{b[2]:02X}?")
        return (f"[ch{ch}] " if ch != 1 else "") + cc_label(b[1], b[2])
    return f"??     {b.hex(' ').upper()}"

COLORS = {"TX": "\033[33m", "RX": "\033[36m", "MON": "\033[36m"}
partial = {}

def handle(line):
    line = line.rstrip("\n")
    if line.startswith("# MARK"):
        txt = line.split(" ", 3)[3] if line.count(" ") >= 3 else line
        print(f"\n\033[1;35m===== {txt} =====\033[0m", flush=True)
        return
    if not line or line.startswith("#"): return
    p = line.split(" ")
    if len(p) < 3 or p[1] == "INFO": return
    d = p[1]
    data = bytes(int(x, 16) for x in line[line.index("]") + 1:].split())
    buf = partial.get(d, b"") + data
    out = []
    while buf:
        if buf[0] == 0xF0:
            e = buf.find(b"\xF7")
            if e < 0: break
            out.append(buf[:e + 1]); buf = buf[e + 1:]
        else:
            i = 1
            while i < len(buf) and buf[i] < 0x80: i += 1
            out.append(buf[:i]); buf = buf[i:]
    partial[d] = buf
    who = {"TX": "APP->PEDAL", "RX": "PEDAL->APP", "MON": "PEDAL"}.get(d, d)
    for m in out:
        lbl = decode(d, m)
        if lbl:
            t = time.strftime("%H:%M:%S", time.localtime(int(p[0]) / 1000))
            print(f"{COLORS.get(d, '')}{t} {who:10}\033[0m {lbl}", flush=True)

print(f"decoding {LOG}  (Ctrl-C to quit)")
f = open(LOG, errors="replace")
if not FROM_START:
    f.seek(0, 2)
while True:
    line = f.readline()
    if not line:
        time.sleep(0.05); continue
    handle(line)
