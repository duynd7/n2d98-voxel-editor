#!/usr/bin/env python3
"""Generate a standing Pikachu in the voxel editor via voxel-mcp-server."""

from __future__ import annotations

import sys

from mcp_client import VoxelMcp, default_paths

# Palette indices
Y = 1  # body yellow
YS = 2  # yellow shadow
K = 3  # black
R = 4  # cheek / tongue red
W = 5  # white
BR = 6  # brown (tail base, back)
CR = 7  # cream belly
PK = 8  # inner ear
DB = 9  # dark brown


class Pika:
    def __init__(self, mcp: VoxelMcp) -> None:
        self.mcp = mcp

    def palette(self) -> None:
        colors = {
            Y: (255, 213, 53),
            YS: (232, 168, 20),
            K: (28, 24, 22),
            R: (228, 52, 40),
            W: (255, 255, 255),
            BR: (168, 92, 32),
            CR: (255, 241, 186),
            PK: (242, 148, 164),
            DB: (92, 48, 20),
        }
        for idx, (r, g, b) in colors.items():
            self.mcp.tool("set_palette_color", index=idx, r=r, g=g, b=b, a=255)
            print(f"  palette {idx} = ({r},{g},{b})")

    def box(self, x0, y0, z0, x1, y1, z1, color) -> None:
        self.mcp.tool(
            "fill_box",
            min_x=x0,
            min_y=y0,
            min_z=z0,
            max_x=x1,
            max_y=y1,
            max_z=z1,
            color=color,
        )

    def sphere(self, x, y, z, radius, color) -> None:
        self.mcp.tool("fill_sphere", x=x, y=y, z=z, radius=radius, color=color)

    def voxel(self, x, y, z, color) -> None:
        self.mcp.tool("set_voxel", x=x, y=y, z=z, color=color)

    def line(self, x0, y0, z0, x1, y1, z1, r0, r1, color_at) -> None:
        steps = max(abs(x1 - x0), abs(y1 - y0), abs(z1 - z0), 1)
        for i in range(steps + 1):
            t = i / steps
            x = round(x0 + (x1 - x0) * t)
            y = round(y0 + (y1 - y0) * t)
            z = round(z0 + (z1 - z0) * t)
            r = max(0, round(r0 + (r1 - r0) * t))
            self.sphere(x, y, z, r, color_at(t))

    def build(self) -> None:
        self.mcp.tool("set_edit_mode", mode="model")
        self.mcp.tool("resize_model", size_x=32, size_y=32, size_z=40)
        self.mcp.tool("clear_model")
        print("palette")
        self.palette()

        print("body")
        # Torso + belly. Face +Y, Z up, X right.
        self.sphere(15, 16, 11, 6, Y)
        self.sphere(15, 16, 9, 5, Y)
        self.sphere(15, 18, 10, 4, CR)  # cream belly toward camera
        # Back shading
        self.box(12, 11, 8, 18, 13, 14, YS)
        # Two back stripes
        self.box(13, 11, 11, 17, 12, 12, BR)
        self.box(13, 11, 8, 17, 12, 9, BR)

        print("legs")
        self.box(10, 14, 1, 14, 19, 7, Y)
        self.box(16, 14, 1, 20, 19, 7, Y)
        # Feet
        self.box(9, 13, 1, 14, 20, 3, Y)
        self.box(16, 13, 1, 21, 20, 3, Y)
        # Toe pads (brown)
        self.box(10, 19, 1, 13, 20, 1, BR)
        self.box(17, 19, 1, 20, 20, 1, BR)
        # Inner-leg carve
        self.box(14, 16, 1, 16, 18, 4, 0)

        print("arms")
        self.sphere(8, 17, 11, 3, Y)
        self.sphere(22, 17, 11, 3, Y)
        self.box(6, 16, 9, 9, 19, 12, Y)
        self.box(21, 16, 9, 24, 19, 12, Y)
        # Hands slightly forward
        self.sphere(7, 19, 10, 2, Y)
        self.sphere(23, 19, 10, 2, Y)

        print("head")
        self.sphere(15, 16, 22, 7, Y)
        self.sphere(15, 17, 21, 6, Y)
        self.sphere(15, 20, 21, 4, Y)  # snout
        # Cheek volume (yellow first, red on top)
        self.sphere(9, 19, 21, 3, Y)
        self.sphere(21, 19, 21, 3, Y)

        print("ears")
        # Long ears, angled out and slightly back, black tips ~1/3
        self.line(11, 16, 27, 7, 14, 39, 2, 1, lambda t: K if t > 0.62 else Y)
        self.line(19, 16, 27, 23, 14, 39, 2, 1, lambda t: K if t > 0.62 else Y)
        # Inner-ear pink (inward faces, lower 2/3)
        self.line(12, 16, 28, 9, 15, 35, 1, 0, lambda t: PK)
        self.line(18, 16, 28, 21, 15, 35, 1, 0, lambda t: PK)

        print("cheeks eyes mouth")
        self.sphere(8, 21, 21, 2, R)
        self.sphere(22, 21, 21, 2, R)

        # Oval black eyes on the front of the head
        self.box(11, 21, 23, 13, 23, 26, K)
        self.box(17, 21, 23, 19, 23, 26, K)
        self.box(11, 22, 24, 13, 23, 25, K)
        self.box(17, 22, 24, 19, 23, 25, K)
        # Highlights
        self.voxel(12, 23, 26, W)
        self.voxel(18, 23, 26, W)
        self.voxel(12, 22, 26, W)
        self.voxel(18, 22, 26, W)

        # Nose
        self.voxel(15, 23, 20, K)
        self.voxel(15, 23, 21, K)
        self.voxel(14, 23, 20, K)
        self.voxel(16, 23, 20, K)

        # Smile + tongue
        self.voxel(13, 23, 18, K)
        self.voxel(14, 23, 17, K)
        self.voxel(15, 23, 17, K)
        self.voxel(16, 23, 17, K)
        self.voxel(17, 23, 18, K)
        self.voxel(15, 23, 16, R)

        print("tail")
        # Brown rectangular base on the lower back
        self.box(14, 8, 7, 18, 12, 13, BR)
        self.box(15, 9, 6, 17, 11, 8, DB)
        # Lightning bolt — back and up, slightly to +X
        self.box(13, 6, 11, 17, 10, 16, Y)
        self.box(11, 4, 14, 19, 8, 17, Y)  # wide middle
        self.box(15, 3, 16, 18, 7, 21, Y)
        self.box(16, 2, 20, 22, 6, 24, Y)  # next zag
        self.box(19, 2, 23, 24, 5, 31, Y)  # rising tip
        self.box(21, 2, 29, 25, 4, 35, Y)
        # Shadow on underside of tail
        self.box(13, 6, 11, 17, 7, 14, YS)

        print("polish")
        # Crown / forehead flatten a little so ears sit on a surface
        self.box(13, 14, 28, 17, 18, 28, Y)
        # Chin shadow
        self.box(13, 19, 16, 17, 21, 17, YS)
        # Ear roots
        self.sphere(11, 16, 27, 2, Y)
        self.sphere(19, 16, 27, 2, Y)


def main() -> int:
    binary, project = default_paths()
    print(f"MCP {binary}")
    print(f"project {project}")
    mcp = VoxelMcp(binary, project, 32)
    try:
        mcp.initialize()
        Pika(mcp).build()
        scene = mcp.tool("get_scene_info")
        print("scene", scene)
        save = mcp.tool("save_vox", path=project)
        print("save", save)
    finally:
        mcp.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
