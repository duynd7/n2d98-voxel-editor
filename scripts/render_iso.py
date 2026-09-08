#!/usr/bin/env python3
"""Render isometric PNG previews of a MagicaVoxel .vox file for docs."""

from __future__ import annotations

import struct
import sys
from pathlib import Path

from PIL import Image, ImageDraw


def read_vox(path: Path):
    data = path.read_bytes()
    assert data[:4] == b"VOX "
    offset = 8
    _cid, _content_size, children_size = struct.unpack_from("<4sII", data, offset)
    offset += 12
    children_end = offset + children_size
    sizes = []
    voxels = []
    palette = [(0, 0, 0, 0)] * 256
    while offset < children_end:
        cid, content_size, children_size = struct.unpack_from("<4sII", data, offset)
        offset += 12
        content = data[offset : offset + content_size]
        after = offset + content_size + children_size
        if cid == b"SIZE":
            sizes.append(struct.unpack_from("<III", content))
        elif cid == b"XYZI":
            n = struct.unpack_from("<I", content)[0]
            chunk = []
            for i in range(n):
                x, y, z, c = content[4 + i * 4 : 8 + i * 4]
                chunk.append((x, y, z, c))
            voxels.append(chunk)
        elif cid == b"RGBA":
            for i in range(255):
                r, g, b, a = content[i * 4 : 4 + i * 4]
                palette[i + 1] = (r, g, b, a)
        offset = after
    sx, sy, sz = sizes[0]
    return (sx, sy, sz), voxels[0], palette


def shade(rgb, k: float):
    return tuple(max(0, min(255, int(c * k))) for c in rgb[:3])


def iso_project(x: int, y: int, z: int, s: int) -> tuple[int, int]:
    # 2:1 isometric, extra Z so tall models (ears, sun) do not sit on the roof
    px = (x - y) * (s // 2)
    py = (x + y) * (s // 4) - z * ((s * 3) // 4)
    return px, py


def render_iso(voxels, palette, scale: int = 12, pad: int = 24) -> Image.Image:
    occupied = {(x, y, z): palette[c][:3] for x, y, z, c in voxels if c}
    if not occupied:
        return Image.new("RGBA", (256, 256), (18, 18, 22, 255))

    pts = [iso_project(x, y, z, scale) for (x, y, z) in occupied]
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    min_x, max_x = min(xs), max(xs)
    min_y, max_y = min(ys), max(ys)
    w = max_x - min_x + scale + pad * 2
    h = max_y - min_y + scale + pad * 2
    ox = pad - min_x
    oy = pad - min_y + scale // 2

    img = Image.new("RGBA", (w, h), (16, 18, 24, 255))
    draw = ImageDraw.Draw(img)
    s = scale
    hw = s // 2
    hh = s // 4

    for x, y, z in sorted(occupied, key=lambda p: (p[0] + p[1], p[2], p[0])):
        rgb = occupied[(x, y, z)]
        px, py = iso_project(x, y, z, s)
        cx, cy = px + ox, py + oy
        top = shade(rgb, 1.08)
        right = shade(rgb, 0.72)
        left = shade(rgb, 0.88)
        # top diamond
        draw.polygon(
            [
                (cx, cy - s // 2),
                (cx + hw, cy - hh),
                (cx, cy),
                (cx - hw, cy - hh),
            ],
            fill=top,
        )
        # left face
        draw.polygon(
            [
                (cx - hw, cy - hh),
                (cx, cy),
                (cx, cy + s // 2),
                (cx - hw, cy + hh),
            ],
            fill=left,
        )
        # right face
        draw.polygon(
            [
                (cx + hw, cy - hh),
                (cx, cy),
                (cx, cy + s // 2),
                (cx + hw, cy + hh),
            ],
            fill=right,
        )
    return img


def main() -> int:
    vox = Path(sys.argv[1])
    dest = Path(sys.argv[2])
    dest.parent.mkdir(parents=True, exist_ok=True)
    _size, voxels, palette = read_vox(vox)
    img = render_iso(voxels, palette, scale=int(sys.argv[3]) if len(sys.argv) > 3 else 12)
    img.save(dest)
    print("wrote", dest, img.size)
    return 0


if __name__ == "__main__":
    sys.exit(main())
