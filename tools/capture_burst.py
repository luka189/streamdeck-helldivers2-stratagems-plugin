"""Save N full-screen frames, one per interval, into a folder."""
import sys
import time
from pathlib import Path

from PIL import ImageGrab

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
n, interval = int(sys.argv[2]), float(sys.argv[3])
for i in range(n):
    ImageGrab.grab().save(out / f"{i:02d}.png")
    print("saved", i, flush=True)
    time.sleep(interval)
