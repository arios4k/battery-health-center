#!/usr/bin/env python3
"""Cut the full-page captures (docs/manual/img/<page>-full-<style>.png) into one image per card
for the user manual, hide the battery serial number, and remove the full-page files."""
import pathlib, sys
from PIL import Image, ImageDraw

IMG = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "docs/manual/img")
# name: (page, (left, top, right, bottom)) in the 960 px wide full-page capture.
CROPS = {
    "overview-ring": ("overview", (12, 8, 478, 310)),
    "overview-power-source": ("overview", (12, 316, 478, 480)),
    "overview-health": ("overview", (482, 8, 948, 292)),
    "overview-recommendations": ("overview", (482, 298, 948, 545)),
    "health-capacity": ("health", (12, 8, 478, 456)),
    "health-trend": ("health", (482, 8, 948, 490)),
    "health-usage": ("health", (12, 464, 478, 870)),
    "health-information": ("health", (482, 498, 948, 904)),
    "health-calibration": ("health", (12, 878, 478, 1100)),
    "charging-status": ("charging", (100, 8, 860, 186)),
    "charging-limit": ("charging", (100, 192, 860, 742)),
    "charging-schedule": ("charging", (100, 748, 860, 1312)),
    "charging-behavior": ("charging", (100, 1318, 860, 1490)),
    "power-mode": ("power", (100, 8, 860, 178)),
    "power-automatic": ("power", (100, 184, 860, 616)),
    "power-saving": ("power", (100, 624, 860, 1040)),
    "power-buttons": ("power", (100, 1046, 860, 1332)),
    "power-top-bar": ("power", (100, 1338, 860, 1502)),
    "usage-battery-level": ("usage", (12, 8, 478, 400)),
    "usage-background": ("usage", (12, 408, 478, 544)),
    "usage-sleep": ("usage", (12, 550, 478, 1004)),
    "usage-significant": ("usage", (482, 8, 948, 380)),
    "usage-apps": ("usage", (482, 386, 948, 788)),
}
# Rows to hide: page, box that covers the value.
HIDE = {"health": [(780, 712, 930, 740)]}

for style in ("light", "dark"):
    full = {}
    for page in ("overview", "health", "charging", "power", "usage"):
        path = IMG / f"{page}-full-{style}.png"
        full[page] = Image.open(path).convert("RGB")
        draw = ImageDraw.Draw(full[page])
        for box in HIDE.get(page, []):
            draw.rectangle(box, fill=full[page].getpixel((box[0] - 40, box[1] + 10)))
            draw.text((box[0] + 40, box[1] + 6), "••••", fill=(128, 128, 128))
    for name, (page, box) in CROPS.items():
        full[page].crop(box).save(IMG / f"{name}-{style}.png", optimize=True)
    for page in full:
        (IMG / f"{page}-full-{style}.png").unlink()
