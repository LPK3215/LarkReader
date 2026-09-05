#!/usr/bin/env python3
"""
Generate the LarkReader app icon.

Design: rounded-square blue badge with a white stylized open-book / "L" mark.
Output: a 1024x1024 PNG that can be fed into `npx tauri icon` to produce
src-tauri/icons/* for all platforms.
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter


def find_repo_root(start: Path) -> Path:
    """Walk up from `start` until a repo anchor (.git or Cargo.toml) is found.

    Decoupled from the script's own depth so it works whether the script lives
    at <root>/scripts/ or <root>/docs/scripts/.
    """
    cur = start.resolve()
    for parent in [cur, *cur.parents]:
        if (parent / ".git").is_dir() or (parent / "Cargo.toml").is_file():
            return parent
    return cur


ROOT = find_repo_root(Path(__file__))
OUT = ROOT / "tmp" / "lark-reader-1024.png"

W = 1024
PAD = 40
R = 180  # corner radius of the outer badge

# Feishu/Lark primary blue to a bright cyan accent
BLUE = (22, 93, 255)      # #165DFF
CYAN = (0, 214, 185)      # #00D6B9
WHITE = (255, 255, 255)
SHADOW = (0, 0, 0)


def rounded_rectangle_mask(size: int, radius: int) -> Image.Image:
    """Return a single-channel mask for a rounded rectangle."""
    mask = Image.new("L", (size, size), 0)
    d = ImageDraw.Draw(mask)
    d.rounded_rectangle((0, 0, size - 1, size - 1), radius=radius, fill=255)
    return mask


def draw_gradient_background(img: Image.Image) -> None:
    """Draw a top-left to bottom-right linear-ish gradient."""
    pixels = img.load()
    for y in range(W):
        for x in range(W):
            # Normalized diagonal t
            t = (x + y) / (2 * (W - 1))
            t = max(0.0, min(1.0, t))
            r = int(BLUE[0] + (CYAN[0] - BLUE[0]) * t)
            g = int(BLUE[1] + (CYAN[1] - BLUE[1]) * t)
            b = int(BLUE[2] + (CYAN[2] - BLUE[2]) * t)
            pixels[x, y] = (r, g, b, 255)


def draw_rounded_badge(canvas: Image.Image) -> Image.Image:
    """Return a new image containing only the rounded badge content."""
    badge = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    draw_gradient_background(badge)

    mask = rounded_rectangle_mask(W, R)
    masked = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    masked.paste(badge, (0, 0), mask)

    # Soft drop shadow for depth
    shadow = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    shadow_draw = ImageDraw.Draw(shadow)
    shadow_draw.rounded_rectangle(
        (PAD, PAD, W - PAD, W - PAD), radius=R - 10, fill=(*SHADOW, 60)
    )
    shadow = shadow.filter(ImageFilter.GaussianBlur(radius=20))

    canvas.alpha_composite(shadow, (0, 0))
    canvas.alpha_composite(masked, (0, 0))
    return canvas


def draw_book_mark(draw: ImageDraw.ImageDraw) -> None:
    """Draw a minimal open-book badge mark in the center."""
    cx, cy = W // 2, W // 2

    # Main book body — a large rounded rectangle
    book_w, book_h = 400, 512
    book_r = 56
    book_x = cx - book_w // 2
    book_y = cy - book_h // 2
    draw.rounded_rectangle(
        (book_x, book_y, book_x + book_w, book_y + book_h),
        radius=book_r,
        fill=WHITE,
    )

    # Page fold / spine curve (light blue accent)
    fold_x1 = book_x + 160
    fold_y1 = book_y + 20
    fold_x2 = book_x + 240
    fold_y2 = book_y + book_h - 20
    draw.line([(fold_x1, fold_y1), (fold_x2, fold_y2)], fill=(235, 245, 255), width=14)

    # Folded top-right corner triangle
    corner_size = 110
    corner_x = book_x + book_w
    corner_y = book_y
    draw.polygon(
        [
            (corner_x, corner_y),
            (corner_x, corner_y + corner_size),
            (corner_x - corner_size, corner_y),
        ],
        fill=(230, 240, 255),
    )


def main() -> int:
    OUT.parent.mkdir(parents=True, exist_ok=True)

    canvas = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    draw_rounded_badge(canvas)

    draw = ImageDraw.Draw(canvas)
    draw_book_mark(draw)

    # Slight inner sheen at top-left
    sheen = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    sheen_draw = ImageDraw.Draw(sheen)
    sheen_draw.ellipse((-W // 3, -W // 3, W * 2 // 3, W // 4), fill=(255, 255, 255, 20))
    canvas.alpha_composite(sheen)

    canvas.save(OUT, "PNG")
    print(f"Saved {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
