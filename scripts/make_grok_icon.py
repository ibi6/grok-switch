"""Generate a Grok-inspired app icon master PNG for Tauri + UI brand mark."""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parents[1]
ICONS = ROOT / "src-tauri" / "icons"
ASSETS = ROOT / "src" / "assets"


def make_icon(size: int = 1024) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Rounded near-black tile (Grok / xAI app aesthetic)
    radius = int(size * 0.22)
    draw.rounded_rectangle((0, 0, size - 1, size - 1), radius=radius, fill=(10, 10, 12, 255))

    # Soft center glow
    glow = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    gd = ImageDraw.Draw(glow)
    cx = cy = size // 2
    for i in range(int(size * 0.34), 0, -3):
        alpha = int(22 * (i / (size * 0.34)))
        gd.ellipse((cx - i, cy - i, cx + i, cy + i), fill=(88, 96, 255, alpha))
    glow = glow.filter(ImageFilter.GaussianBlur(radius=size * 0.02))
    img = Image.alpha_composite(img, glow)
    draw = ImageDraw.Draw(img)

    s = size / 1024.0

    # Outer ring
    ring_r = int(300 * s)
    ring_w = max(10, int(20 * s))
    draw.ellipse(
        (cx - ring_r, cy - ring_r, cx + ring_r, cy + ring_r),
        outline=(255, 255, 255, 255),
        width=ring_w,
    )

    # Solid white core
    core_r = int(215 * s)
    draw.ellipse(
        (cx - core_r, cy - core_r, cx + core_r, cy + core_r),
        fill=(255, 255, 255, 255),
    )

    # Abstract cutouts (stylized Grok mark)
    cut = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    cd = ImageDraw.Draw(cut)
    bg = (10, 10, 12, 255)

    # Large offset void
    vr = int(155 * s)
    ox, oy = int(-36 * s), int(-28 * s)
    cd.ellipse((cx + ox - vr, cy + oy - vr, cx + ox + vr, cy + oy + vr), fill=bg)

    # Small lower-right void
    vr2 = int(50 * s)
    ox2, oy2 = int(78 * s), int(58 * s)
    cd.ellipse((cx + ox2 - vr2, cy + oy2 - vr2, cx + ox2 + vr2, cy + oy2 + vr2), fill=bg)

    img = Image.alpha_composite(img, cut)
    draw = ImageDraw.Draw(img)

    # Accent sparkle
    ar = int(30 * s)
    ax, ay = int(100 * s), int(-112 * s)
    draw.ellipse((cx + ax - ar, cy + ay - ar, cx + ax + ar, cy + ay + ar), fill=(255, 255, 255, 255))

    return img


def main() -> None:
    ICONS.mkdir(parents=True, exist_ok=True)
    ASSETS.mkdir(parents=True, exist_ok=True)

    master = make_icon(1024)
    master_path = ICONS / "app-icon.png"
    master.save(master_path, "PNG")

    master.resize((256, 256), Image.Resampling.LANCZOS).save(ASSETS / "grok-icon.png", "PNG")
    master.resize((64, 64), Image.Resampling.LANCZOS).save(ASSETS / "grok-icon-64.png", "PNG")
    master.resize((32, 32), Image.Resampling.LANCZOS).save(ASSETS / "grok-icon-32.png", "PNG")

    print(f"wrote {master_path}")
    print(f"wrote {ASSETS / 'grok-icon.png'}")


if __name__ == "__main__":
    main()
