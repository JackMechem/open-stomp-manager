#!/usr/bin/env python3
"""Screenshot the app on a replayed capture (Linux; for the docs and visual checks).

    nix develop --command tools/dev/shot.py                     # -> target/shots/main.png
    nix develop --command tools/dev/shot.py log --dev log       # the activity log open
    nix develop --command tools/dev/shot.py import --dev import --size 1100x760

Builds the debug app, starts Vite if it isn't running, replays the capture fast for --secs seconds on
an off-screen display of --size, then saves a PNG of the window's contents.
"""

import argparse
import os
import socket
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from webdriver import ROOT, App  # noqa: E402

p = argparse.ArgumentParser()
p.add_argument("name", nargs="?", default="main")
p.add_argument("--dev", default="", help="STOMP_DEV flags, e.g. log,import")
p.add_argument("--secs", type=float, default=4)
p.add_argument("--speed", default="20")
p.add_argument("--size", default="1440x900")
p.add_argument("--replay", default=os.path.join(ROOT, "captures", "logs", "01-main-session.log"))
a = p.parse_args()

subprocess.run(["cargo", "build", "-q", "-p", "open-stomp-manager"], cwd=ROOT, check=True)
vite = None
with socket.socket() as s:
    if s.connect_ex(("127.0.0.1", 1420)) != 0:
        vite = subprocess.Popen(["pnpm", "dev"], cwd=os.path.join(ROOT, "app"), stdout=subprocess.DEVNULL)
        time.sleep(2)
try:
    w, h = (int(x) for x in a.size.split("x"))
    with App(["--replay", a.replay, "--speed", a.speed], env={"STOMP_DEV": a.dev}, headless=(w, h)) as app:
        app.resize(w, h)
        time.sleep(a.secs)
        print(app.screenshot(os.path.join(ROOT, "target", "shots", f"{a.name}.png")))
finally:
    if vite:
        vite.terminate()
