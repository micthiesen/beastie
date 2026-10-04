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
# Densest cluster, not the median: the gold bell and the ball's stripe are yellow too, and the
# head is the largest solid yellow mass.
R = max(6, sw // 40)
def density(p):
    return sum(1 for q in pts if abs(q[0] - p[0]) <= R and abs(q[1] - p[1]) <= R)
seed = max(pts[:: max(1, len(pts) // 400)], key=density)
near = [q for q in pts if abs(q[0] - seed[0]) <= R * 2 and abs(q[1] - seed[1]) <= R * 2]
cx = sum(q[0] for q in near) / len(near); cy = sum(q[1] for q in near) / len(near)
# map screenshot pixels to logical UI units: window has 28pt title bar, screenshot is 2x
lx = cx * 4 / 2 * 320 / (w / 2)
ly = (cy * 4 / 2 - 28) * 180 / (h / 2 - 28)
print(int(lx), int(ly))
