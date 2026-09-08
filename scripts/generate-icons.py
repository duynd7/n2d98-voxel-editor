#!/usr/bin/env python3
"""Generate macOS .icns, Windows .ico, and PNG sizes from assets/icons/icon-1024.png."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

os.environ["COPYFILE_DISABLE"] = "1"

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "assets" / "icons" / "icon-1024.png"
PNG_DIR = ROOT / "assets" / "icons" / "png"
ICNS = ROOT / "assets" / "icons" / "macos" / "AppIcon.icns"
ICO = ROOT / "assets" / "icons" / "windows" / "icon.ico"
LINUX_DIR = ROOT / "assets" / "icons" / "linux" / "hicolor"
WINDOW_ICON = ROOT / "crates" / "voxel-editor" / "assets" / "icon.png"

PNG_SIZES = (16, 24, 32, 48, 64, 128, 256, 512, 1024)
ICO_SIZES = ((16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256))
LINUX_SIZES = (16, 24, 32, 48, 64, 128, 256, 512)
ICONSET_FILES = {
    "icon_16x16.png": 16,
    "icon_16x16@2x.png": 32,
    "icon_32x32.png": 32,
    "icon_32x32@2x.png": 64,
    "icon_128x128.png": 128,
    "icon_128x128@2x.png": 256,
    "icon_256x256.png": 256,
    "icon_256x256@2x.png": 512,
    "icon_512x512.png": 512,
    "icon_512x512@2x.png": 1024,
}


def resized(src: Image.Image, size: int) -> Image.Image:
    return src.resize((size, size), Image.Resampling.LANCZOS)


def main() -> int:
    if not SRC.exists():
        print(f"missing master icon: {SRC}", file=sys.stderr)
        return 1

    src = Image.open(SRC).convert("RGBA")
    if src.size != (1024, 1024):
        src = resized(src, 1024)

    PNG_DIR.mkdir(parents=True, exist_ok=True)
    pngs: dict[int, Image.Image] = {}
    for size in PNG_SIZES:
        img = resized(src, size)
        pngs[size] = img
        img.save(PNG_DIR / f"icon_{size}.png", format="PNG")

    WINDOW_ICON.parent.mkdir(parents=True, exist_ok=True)
    pngs[256].save(WINDOW_ICON, format="PNG")

    ICO.parent.mkdir(parents=True, exist_ok=True)
    src.save(ICO, format="ICO", sizes=list(ICO_SIZES))

    for size in LINUX_SIZES:
        dest = LINUX_DIR / f"{size}x{size}" / "apps"
        dest.mkdir(parents=True, exist_ok=True)
        pngs[size].save(dest / "n2d98-voxel-editor.png", format="PNG")

    # iconutil rejects AppleDouble `._*` files that external volumes inject.
    with tempfile.TemporaryDirectory(prefix="n2d98-iconset-") as tmp:
        iconset = Path(tmp) / "AppIcon.iconset"
        iconset.mkdir()
        for name, size in ICONSET_FILES.items():
            pngs[size].save(iconset / name, format="PNG")
        icns_tmp = Path(tmp) / "AppIcon.icns"
        subprocess.run(
            ["iconutil", "-c", "icns", "-o", str(icns_tmp), str(iconset)],
            check=True,
        )
        ICNS.parent.mkdir(parents=True, exist_ok=True)
        ICNS.write_bytes(icns_tmp.read_bytes())

    print(f"png:    {PNG_DIR}")
    print(f"icns:   {ICNS} ({ICNS.stat().st_size} bytes)")
    print(f"ico:    {ICO} ({ICO.stat().st_size} bytes)")
    print(f"window: {WINDOW_ICON}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
