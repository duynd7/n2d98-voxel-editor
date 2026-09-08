//! Face-culled voxel mesh in MagicaVoxel space, then converted to glTF Y-up.

use voxel_core::{local_to_world, IVec3, MvRotation, Palette, VoxelModel};

pub struct CpuMesh {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub translation: [f32; 3],
}

impl CpuMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}

const FACES: [([f32; 3], [[f32; 3]; 4], (i32, i32, i32)); 6] = [
    (
        [1.0, 0.0, 0.0],
        [
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [1.0, 0.0, 1.0],
        ],
        (1, 0, 0),
    ),
    (
        [-1.0, 0.0, 0.0],
        [
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
        ],
        (-1, 0, 0),
    ),
    (
        [0.0, 1.0, 0.0],
        [
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
        ],
        (0, 1, 0),
    ),
    (
        [0.0, -1.0, 0.0],
        [
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
        ],
        (0, -1, 0),
    ),
    (
        [0.0, 0.0, 1.0],
        [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
        (0, 0, 1),
    ),
    (
        [0.0, 0.0, -1.0],
        [
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ],
        (0, 0, -1),
    ),
];

/// MagicaVoxel Z-up, Y-forward → glTF/Godot Y-up, -Z-forward.
pub fn z_up_to_y_up(p: [f32; 3]) -> [f32; 3] {
    [p[0], p[2], -p[1]]
}

pub fn build_local_mesh(
    name: impl Into<String>,
    model: &VoxelModel,
    palette: &Palette,
    translation: IVec3,
    rotation: MvRotation,
) -> CpuMesh {
    let (sx, sy, sz) = model.size();
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    let m = rotation.to_matrix();

    for z in 0..sz as i32 {
        for y in 0..sy as i32 {
            for x in 0..sx as i32 {
                let idx = model.get_or_empty(x, y, z);
                if idx == 0 {
                    continue;
                }
                let c = palette.get(idx);
                let color = [
                    c.r as f32 / 255.0,
                    c.g as f32 / 255.0,
                    c.b as f32 / 255.0,
                    c.a as f32 / 255.0,
                ];

                for (normal, corners, (dx, dy, dz)) in FACES {
                    if model.get_or_empty(x + dx, y + dy, z + dz) != 0 {
                        continue;
                    }
                    let n_local = IVec3::new(normal[0] as i32, normal[1] as i32, normal[2] as i32);
                    let n_world = rotation.rotate_vec(n_local);
                    let n = z_up_to_y_up([n_world.x as f32, n_world.y as f32, n_world.z as f32]);
                    let start = positions.len() as u32;
                    let origin = local_to_world(IVec3::new(x, y, z), IVec3::new(0, 0, 0), rotation);
                    for corner in corners {
                        let lx = corner[0];
                        let ly = corner[1];
                        let lz = corner[2];
                        let rx = m[0][0] as f32 * lx + m[0][1] as f32 * ly + m[0][2] as f32 * lz;
                        let ry = m[1][0] as f32 * lx + m[1][1] as f32 * ly + m[1][2] as f32 * lz;
                        let rz = m[2][0] as f32 * lx + m[2][1] as f32 * ly + m[2][2] as f32 * lz;
                        positions.push(z_up_to_y_up([
                            origin.x as f32 + rx,
                            origin.y as f32 + ry,
                            origin.z as f32 + rz,
                        ]));
                        normals.push(n);
                        colors.push(color);
                    }
                    indices.extend_from_slice(&[
                        start,
                        start + 1,
                        start + 2,
                        start,
                        start + 2,
                        start + 3,
                    ]);
                }
            }
        }
    }

    CpuMesh {
        name: name.into(),
        positions,
        normals,
        colors,
        indices,
        translation: z_up_to_y_up([
            translation.x as f32,
            translation.y as f32,
            translation.z as f32,
        ]),
    }
}
