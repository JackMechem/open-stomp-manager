import os, signal, threading, time
from open_stomp import Device
d = Device(auto_connect=False, sync=False)
d.drain()
threading.Timer(0.5, lambda: os.kill(os.getpid(), signal.SIGINT)).start()
t = time.monotonic()
try:
    while True:
        d.recv()
except KeyboardInterrupt:
    print(f"interrupted after {time.monotonic()-t:.2f}s")
d.close()
