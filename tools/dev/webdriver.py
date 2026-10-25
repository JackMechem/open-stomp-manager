"""Drive the real app through WebDriver: click, drag, read the page, take screenshots.

Linux only: WebKitGTK ships `WebKitWebDriver`, which starts the app with automation turned on
(TAURI_WEBVIEW_AUTOMATION) and talks to its webview. Standard library only.

    from webdriver import App
    with App(["--replay", "captures/logs/01-main-session.log"]) as app:
        app.click(".preset:nth-child(3) button")
        app.screenshot("target/shots/main.png")

The app is the debug build (`cargo build`), which loads the frontend from Vite's dev server, so
`pnpm dev` must be running in app/ (shot.py starts it when it isn't).

`App(..., headless=(1440, 900))` runs it on an off-screen X server (Xvfb, in the Nix dev shell) at that size,
so nothing shows on screen and screenshots don't depend on the window manager.
"""

import base64
import json
import os
import shutil
import socket
import subprocess
import time
import urllib.error
import urllib.request

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
APP = os.path.join(ROOT, "target", "debug", "open-stomp-manager")
# W3C element reference key
ELEMENT = "element-6066-11e4-a52e-4f735466cecf"


def _free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class WebDriverError(RuntimeError):
    pass


class App:
    def __init__(self, args=(), env=None, binary=APP, headless=None):
        self.args = list(args)
        self.env = dict(os.environ, TAURI_WEBVIEW_AUTOMATION="true", **(env or {}))
        self.binary = binary
        self.headless = headless
        self.xvfb = None
        self.driver = None
        self.session = None

    # ── lifecycle ──

    def __enter__(self):
        if self.headless:
            w, h = self.headless
            display = f":{_free_port() % 400 + 100}"
            self.xvfb = subprocess.Popen(["Xvfb", display, "-screen", "0", f"{w}x{h}x24", "-nolisten", "tcp"])
            time.sleep(0.5)
            self.env.update(DISPLAY=display, GDK_BACKEND="x11")
            self.env.pop("WAYLAND_DISPLAY", None)
        driver = shutil.which("WebKitWebDriver")
        if not driver:
            raise WebDriverError("WebKitWebDriver not found (it comes with WebKitGTK; run inside `nix develop`)")
        self.port = _free_port()
        self.driver = subprocess.Popen(
            [driver, f"--port={self.port}"], env=self.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        deadline = time.time() + 10
        while True:
            try:
                self._call("GET", "/status")
                break
            except OSError:
                if time.time() > deadline:
                    raise
                time.sleep(0.1)
        caps = {"capabilities": {"alwaysMatch": {"webkitgtk:browserOptions": {"binary": self.binary, "args": self.args}}}}
        self.session = self._call("POST", "/session", caps)["sessionId"]
        self.wait_for("#root > .app")
        return self

    def __exit__(self, *exc):
        try:
            if self.session:
                self._call("DELETE", f"/session/{self.session}")
        except Exception:
            pass
        if self.driver:
            self.driver.terminate()
            self.driver.wait(5)
        if self.xvfb:
            self.xvfb.terminate()
            self.xvfb.wait(5)

    # ── protocol ──

    def _call(self, method, path, body=None):
        data = None if body is None else json.dumps(body).encode()
        req = urllib.request.Request(
            f"http://127.0.0.1:{self.port}{path}", data=data, method=method, headers={"Content-Type": "application/json"}
        )
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.loads(r.read() or b"{}").get("value")
        except urllib.error.HTTPError as e:
            raise WebDriverError(f"{method} {path}: {e.read().decode(errors='replace')}") from None

    def _s(self, path, method="POST", body=None):
        return self._call(method, f"/session/{self.session}{path}", {} if body is None and method == "POST" else body)

    # ── helpers ──

    def js(self, script, *args):
        """Run `script` (a function body; use `return`) in the page."""
        return self._s("/execute/sync", body={"script": script, "args": list(args)})

    def find(self, css):
        return self._s("/element", body={"using": "css selector", "value": css})[ELEMENT]

    def find_all(self, css):
        return [e[ELEMENT] for e in self._s("/elements", body={"using": "css selector", "value": css})]

    def wait_for(self, css, timeout=20):
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self.js("return !!document.querySelector(arguments[0])", css):
                return
            time.sleep(0.1)
        raise WebDriverError(f"timed out waiting for {css}")

    def wait_until(self, script, timeout=20, *args):
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self.js(script, *args):
                return
            time.sleep(0.1)
        raise WebDriverError(f"timed out waiting for: {script}")

    def click(self, css):
        self._s(f"/element/{self.find(css)}/click")

    def text(self, css):
        return self._s(f"/element/{self.find(css)}/text", "GET")

    def type(self, css, text, clear=True):
        el = self.find(css)
        if clear:
            self._s(f"/element/{el}/clear")
        self._s(f"/element/{el}/value", body={"text": text})

    def drag(self, css, dx, dy, steps=10):
        """Press on the element's centre, move by (dx, dy) in `steps`, release."""
        el = self.find(css)
        moves = [{"type": "pointerMove", "origin": {ELEMENT: el}, "x": 0, "y": 0}, {"type": "pointerDown", "button": 0}]
        for i in range(1, steps + 1):
            moves.append({"type": "pointerMove", "origin": "pointer", "x": dx / steps, "y": dy / steps, "duration": 16})
        moves.append({"type": "pointerUp", "button": 0})
        self._s("/actions", body={"actions": [{"type": "pointer", "id": "mouse", "parameters": {"pointerType": "mouse"}, "actions": moves}]})
        self._s("/actions", "DELETE")

    def resize(self, width, height):
        self._s("/window/rect", body={"width": width, "height": height})

    def screenshot(self, path):
        png = base64.b64decode(self._s("/screenshot", "GET"))
        os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
        with open(path, "wb") as f:
            f.write(png)
        return path

    def section(self, name):
        """A section of the pedal state as the frontend has it (see app/src/store.ts)."""
        return self.js("return window.__stomp?.sections?.[arguments[0]] ?? null", name)
