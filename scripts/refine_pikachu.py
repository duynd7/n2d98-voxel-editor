#!/usr/bin/env python3
"""Second pass: rounder Pikachu silhouette and clearer face."""

from __future__ import annotations

import sys

from mcp_client import VoxelMcp, default_paths
from generate_pikachu import K, PK, R, W, Y, YS, Pika


def refine(p: Pika) -> None:
    # Round the head by re-sphering, then recarve face.
    p.sphere(15, 16, 22, 7, Y)
    p.sphere(15, 17, 22, 6, Y)
    p.sphere(15, 20, 21, 3, Y)

    # Softer head corners (erase cube corners)
    for x, y, z in (
        (8, 9, 16),
        (8, 9, 28),
        (8, 23, 16),
        (22, 9, 16),
        (22, 9, 28),
        (22, 23, 16),
        (22, 23, 28),
        (8, 23, 28),
    ):
        p.sphere(x, y, z, 1, 0)

    # Cheeks sit on the sides, slightly forward
    p.sphere(8, 20, 21, 2, R)
    p.sphere(22, 20, 21, 2, R)

    # Larger oval eyes
    p.box(11, 21, 23, 13, 23, 27, K)
    p.box(17, 21, 23, 19, 23, 27, K)
    p.voxel(12, 23, 27, W)
    p.voxel(18, 23, 27, W)
    p.voxel(11, 23, 26, W)
    p.voxel(17, 23, 26, W)

    # Nose + mouth
    p.voxel(15, 23, 20, K)
    p.voxel(14, 23, 20, K)
    p.voxel(16, 23, 20, K)
    p.voxel(13, 23, 18, K)
    p.voxel(14, 23, 17, K)
    p.voxel(15, 23, 17, K)
    p.voxel(16, 23, 17, K)
    p.voxel(17, 23, 18, K)
    p.voxel(15, 23, 16, R)

    # Pointier ear tips
    p.sphere(7, 14, 39, 1, K)
    p.sphere(6, 14, 38, 0, K)
    p.sphere(23, 14, 39, 1, K)
    p.sphere(24, 14, 38, 0, K)
    p.voxel(7, 14, 39, K)
    p.voxel(23, 14, 39, K)

    # Keep inner ear pink
    p.box(11, 16, 29, 12, 16, 33, PK)
    p.box(18, 16, 29, 19, 16, 35, PK)

    # Chin / neck blend
    p.box(13, 15, 15, 17, 18, 16, Y)
    p.box(13, 19, 16, 17, 21, 17, YS)


def main() -> int:
    binary, project = default_paths()
    mcp = VoxelMcp(binary, project, 32)
    try:
        mcp.initialize()
        p = Pika(mcp)
        refine(p)
        scene = mcp.tool("get_scene_info")
        print("scene", scene)
        print(mcp.tool("save_vox", path=project))
    finally:
        mcp.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
