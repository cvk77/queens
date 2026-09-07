#!/usr/bin/env python3
"""Draws the placeholder app icon: a bigger version of the game's own queen
token (see queens_app/src/theme.rs's QUEEN_* colours), on the app's own
background colour. Not run by CI — this only regenerates icon_1024.png, which
build_dmg.sh's `sips`/`iconutil` step turns into AppIcon.icns from a committed
copy. Needs Pillow (`pip install pillow`).
"""

from pathlib import Path

from PIL import Image, ImageDraw

SIZE = 1024

# queens_app/src/theme.rs
BACKGROUND = (0x16, 0x18, 0x1F)
QUEEN_BODY = (0x14, 0x16, 0x1C)
QUEEN_RING = (0xF6, 0xF7, 0xFB)
QUEEN_CROWN = (0xFB, 0xFC, 0xFE)
QUEEN_GEM = (0xF4, 0xB7, 0x40)


def rounded_square(draw: ImageDraw.ImageDraw, size: int, fill, radius_ratio: float = 0.22) -> None:
    radius = int(size * radius_ratio)
    draw.rounded_rectangle([0, 0, size - 1, size - 1], radius=radius, fill=fill)


def crown_shape(
    cx: float, cy: float, width: float, base_y: float, peak_y: float, valley_y: float, spikes: int = 3
) -> tuple[list[tuple[float, float]], list[tuple[float, float]]]:
    """A zigzag crown outline (`spikes` peaks across `width`, sitting on a flat
    base), and the peak points separately, for a gem to sit on each."""
    left = cx - width / 2
    step = width / (spikes * 2)
    outline = [(left, base_y)]
    peaks = []
    for i in range(spikes * 2):
        x = left + step * (i + 1)
        if i % 2 == 0:
            peaks.append((x, peak_y))
            outline.append((x, peak_y))
        else:
            outline.append((x, valley_y))
    outline.append((left + width, base_y))
    return outline, peaks


def draw_icon(path: Path) -> None:
    img = Image.new("RGB", (SIZE, SIZE), BACKGROUND)
    draw = ImageDraw.Draw(img)
    rounded_square(draw, SIZE, BACKGROUND)

    cx = cy = SIZE / 2
    disc_r = SIZE * 0.34
    draw.ellipse(
        [cx - disc_r, cy - disc_r, cx + disc_r, cy + disc_r],
        fill=QUEEN_BODY,
        outline=QUEEN_RING,
        width=int(SIZE * 0.014),
    )

    crown_width = disc_r * 1.2
    base_y = cy + disc_r * 0.30
    peak_y = cy - disc_r * 0.55
    valley_y = cy - disc_r * 0.05
    outline, peaks = crown_shape(cx, cy, crown_width, base_y, peak_y, valley_y)
    draw.polygon(outline, fill=QUEEN_CROWN)

    # Reaches above the valleys' own height, so the diagonal edge from a
    # valley down to the base corner is backed solid rather than left as a
    # notch cut into the silhouette; a few pixels wider and taller than the
    # polygon everywhere else, so anti-aliasing rounding can never leave a seam.
    overlap = SIZE * 0.004
    draw.rectangle(
        [
            cx - crown_width / 2 - overlap,
            valley_y - overlap,
            cx + crown_width / 2 + overlap,
            base_y + overlap,
        ],
        fill=QUEEN_CROWN,
    )

    gem_r = disc_r * 0.055
    for gx, gy in peaks:
        draw.ellipse([gx - gem_r, gy - gem_r, gx + gem_r, gy + gem_r], fill=QUEEN_GEM)

    img.save(path)


if __name__ == "__main__":
    out = Path(__file__).parent / "icon_1024.png"
    draw_icon(out)
    print(f"wrote {out}")
