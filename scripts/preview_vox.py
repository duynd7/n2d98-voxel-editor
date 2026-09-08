#!/usr/bin/env python3
"""Render orthographic PNG previews of a MagicaVoxel .vox file."""

from __future__ import annotations

import struct
import sys
from pathlib import Path

from PIL import Image


def read_vox(path: Path):
    data = path.read_bytes()
    assert data[:4] == b"VOX "
    offset = 8
    cid, content_size, children_size = struct.unpack_from("<4sII", data, offset)
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


def render(voxels, size, palette, view: str, scale: int = 8) -> Image.Image:
    sx, sy, sz = size
    occupied = {(x, y, z): c for x, y, z, c in voxels}
    if view == "front":  # +Y: X right, Z up
        w, h = sx, sz
        def key(x, y, z):
            return y  # closer = larger y
        def proj(x, y, z):
            return x, (sz - 1 - z)
    elif view == "side":  # +X: Y right-ish as depth, show Y as x, Z up
        w, h = sy, sz
        def key(x, y, z):
            return x
        def proj(x, y, z):
            return y, (sz - 1 - z)
    elif view == "top":
        w, h = sx, sy
        def key(x, y, z):
            return z
        def proj(x, y, z):
            return x, (sy - 1 - y)
    else:
        raise ValueError(view)

    depth = [[-1] * w for _ in range(h)]
    color = [[(18, 18, 22, 255)] * w for _ in range(h)]
    for (x, y, z), c in occupied.items():
        px, py = proj(x, y, z)
        if not (0 <= px < w and 0 <= py < h):
            continue
        d = key(x, y, z)
        if d >= depth[py][px]:
            depth[py][px] = d
            r, g, b, a = palette[c]
            # simple depth shade
            shade = 0.72 + 0.28 * (d / max(sx, sy, sz))
            color[py][px] = (
                min(255, int(r * shade)),
                min(255, int(g * shade)),
                min(255, int(b * shade)),
                255,
            )

    img = Image.new("RGBA", (w, h))
    pix = img.load()
    for py in range(h):
        for px in range(w):
            pix[px, py] = color[py][px]
    return img.resize((w * scale, h * scale), Image.NEAREST)


def main() -> int:
    vox = Path(sys.argv[1])
    out_dir = Path(sys.argv[2])
    out_dir.mkdir(parents=True, exist_ok=True)
    size, voxels, palette = read_vox(vox)
    print("size", size, "voxels", len(voxels))
    for view in ("front", "side", "top"):
        img = render(voxels, size, palette, view)
        dest = out_dir / f"pikachu_{view}.png"
        img.save(dest)
        print("wrote", dest)
    return 0


if __name__ == "__main__":
    sys.exit(main())
