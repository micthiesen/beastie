#!/usr/bin/env python3
"""Print logical (320x180) coordinates of Mop's head in a window screenshot, by its yellow."""
import sys
from PIL import Image
img = Image.open(sys.argv[1]).convert("RGB")
w, h = img.size
small = img.resize((w // 4, h // 4))
sw, sh = small.size
pts = []
px = small.load()
title = int(sh * 28 / 572 * 2 / 2)  # skip title bar region proportionally
for y in range(int(sh * 0.08), int(sh * 0.82)):
    for x in range(sw):
        r, g, b = px[x, y]
        if r > 190 and 140 < g < 215 and b < 90 and r - b > 120:
            pts.append((x, y))
if not pts:
    sys.exit(1)
# densest cluster: use median as a robust center (head is the largest yellow mass)
xs = sorted(p[0] for p in pts); ys = sorted(p[1] for p in pts)
cx, cy = xs[len(xs) // 2], ys[len(ys) // 2]
# map screenshot pixels to logical UI units: window has 28pt title bar, screenshot is 2x
lx = cx * 4 / 2 * 320 / (w / 2)
ly = (cy * 4 / 2 - 28) * 180 / (h / 2 - 28)
print(int(lx), int(ly))
