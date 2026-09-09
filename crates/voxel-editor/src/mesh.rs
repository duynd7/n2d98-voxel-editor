//! Face-culled colored cube mesh for solid voxels (+ world transforms).

use voxel_core::{local_to_world, IVec3, MaterialTable, MvRotation, Palette, VoxelModel};

/// Packed vertex: pos (12) + normal (12) + rgba (16) + emit/metal/rough (12) = 52 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
    pub emit: f32,
    pub metal: f32,
    pub rough: f32,
}

const _: () = assert!(std::mem::size_of::<Vertex>() == 52);

impl Vertex {
    fn new(
        pos: [f32; 3],
        normal: [f32; 3],
        color: [f32; 4],
        emit: f32,
        metal: f32,
        rough: f32,
    ) -> Self {
        Self {
            pos,
            normal,
            color,
            emit,
            metal,
            rough,
        }
    }

    pub fn as_bytes(vertices: &[Vertex]) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                vertices.as_ptr() as *const u8,
                std::mem::size_of_val(vertices),
            )
        }
    }
}

#[derive(Clone)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    /// Drawn first: depth write on, blend off.
    pub opaque_indices: Vec<u32>,
    /// Drawn after opaque: alpha blend, depth write off.
    pub transparent_indices: Vec<u32>,
}

impl MeshData {
    pub fn append(&mut self, other: MeshData) {
        let base = self.vertices.len() as u32;
        self.vertices.extend(other.vertices);
        self.opaque_indices
            .extend(other.opaque_indices.into_iter().map(|i| i + base));
        self.transparent_indices
            .extend(other.transparent_indices.into_iter().map(|i| i + base));
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

pub fn build_mesh(model: &VoxelModel, palette: &Palette, materials: &MaterialTable) -> MeshData {
    build_mesh_with_transform(
        model,
        palette,
        materials,
        IVec3::new(0, 0, 0),
        MvRotation::IDENTITY,
    )
}

pub fn build_mesh_with_transform(
    model: &VoxelModel,
    palette: &Palette,
    materials: &MaterialTable,
    translation: IVec3,
    rotation: MvRotation,
) -> MeshData {
    let (sx, sy, sz) = model.size();
    let mut vertices = Vec::new();
    let mut opaque_indices = Vec::new();
    let mut transparent_indices = Vec::new();
    let m = rotation.to_matrix();

    for z in 0..sz as i32 {
        for y in 0..sy as i32 {
            for x in 0..sx as i32 {
                let idx = model.get_or_empty(x, y, z);
                if idx == 0 {
                    continue;
                }
                let c = palette.get(idx);
                let mat = materials.get(idx);
                // Raw albedo; emit is a separate unlit add in the shader (do not pre-boost RGB).
                let rgba = mat.viewport_rgba(c);
                let color = [
                    c.r as f32 / 255.0,
                    c.g as f32 / 255.0,
                    c.b as f32 / 255.0,
                    rgba[3],
                ];
                let emit = mat.emit_amount();
                let metal = mat.metal_amount();
                let rough = mat.rough.clamp(0.0, 1.0);
                let transparent = mat.is_transparent() && color[3] < 0.999;

                for (normal, corners, (dx, dy, dz)) in FACES {
                    if model.get_or_empty(x + dx, y + dy, z + dz) != 0 {
                        continue;
                    }
                    let n_local = IVec3::new(normal[0] as i32, normal[1] as i32, normal[2] as i32);
                    let n_world = rotation.rotate_vec(n_local);
                    let n = [n_world.x as f32, n_world.y as f32, n_world.z as f32];
                    let start = vertices.len() as u32;
                    let origin = local_to_world(IVec3::new(x, y, z), translation, rotation);
                    for corner in corners {
                        let lx = corner[0];
                        let ly = corner[1];
                        let lz = corner[2];
                        let rx = m[0][0] as f32 * lx + m[0][1] as f32 * ly + m[0][2] as f32 * lz;
                        let ry = m[1][0] as f32 * lx + m[1][1] as f32 * ly + m[1][2] as f32 * lz;
                        let rz = m[2][0] as f32 * lx + m[2][1] as f32 * ly + m[2][2] as f32 * lz;
                        vertices.push(Vertex::new(
                            [
                                origin.x as f32 + rx,
                                origin.y as f32 + ry,
                                origin.z as f32 + rz,
                            ],
                            n,
                            color,
                            emit,
                            metal,
                            rough,
                        ));
                    }
                    let dest = if transparent {
                        &mut transparent_indices
                    } else {
                        &mut opaque_indices
                    };
                    dest.extend_from_slice(&[
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

    MeshData {
        vertices,
        opaque_indices,
        transparent_indices,
    }
}

pub fn build_world_mesh(
    instances: &[(&VoxelModel, IVec3, MvRotation)],
    palette: &Palette,
    materials: &MaterialTable,
) -> MeshData {
    let mut mesh = MeshData {
        vertices: Vec::new(),
        opaque_indices: Vec::new(),
        transparent_indices: Vec::new(),
    };
    for (model, t, r) in instances {
        mesh.append(build_mesh_with_transform(
            model, palette, materials, *t, *r,
        ));
    }
    mesh
}

pub fn build_bounds_lines_aabb(min: [f32; 3], max: [f32; 3]) -> Vec<[f32; 3]> {
    let (x0, y0, z0) = (min[0], min[1], min[2]);
    let (x1, y1, z1) = (max[0], max[1], max[2]);
    let corners = [
        [x0, y0, z0],
        [x1, y0, z0],
        [x1, y1, z0],
        [x0, y1, z0],
        [x0, y0, z1],
        [x1, y0, z1],
        [x1, y1, z1],
        [x0, y1, z1],
    ];
    let edges = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    let mut out = Vec::with_capacity(24);
    for (a, b) in edges {
        out.push(corners[a]);
        out.push(corners[b]);
    }
    out
}

pub fn build_bounds_lines(sx: u32, sy: u32, sz: u32) -> Vec<[f32; 3]> {
    build_bounds_lines_aabb([0.0, 0.0, 0.0], [sx as f32, sy as f32, sz as f32])
}

/// Ground grid on Z = `z` covering [0,sx] × [0,sy], minor every 1, major every `major`.
pub fn build_ground_grid(sx: u32, sy: u32, z: f32, major: u32) -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
    let mut minor = Vec::new();
    let mut majors = Vec::new();
    let major = major.max(1);
    for x in 0..=sx {
        let xf = x as f32;
        let line = [[xf, 0.0, z], [xf, sy as f32, z]];
        if x % major == 0 {
            majors.extend_from_slice(&line);
        } else {
            minor.extend_from_slice(&line);
        }
    }
    for y in 0..=sy {
        let yf = y as f32;
        let line = [[0.0, yf, z], [sx as f32, yf, z]];
        if y % major == 0 {
            majors.extend_from_slice(&line);
        } else {
            minor.extend_from_slice(&line);
        }
    }
    (minor, majors)
}

/// Axis guides from origin (RGB = XYZ), length `len`.
pub fn build_axis_gizmo(len: f32) -> Vec<([f32; 3], [f32; 3], [f32; 3])> {
    // returns (a, b, color) as separate — caller builds colored lines differently
    // We'll return packed: for simplicity return three segments with colors encoded by returning RGB lines via mesh helper
    vec![
        ([0.0, 0.0, 0.0], [len, 0.0, 0.0], [0.92, 0.28, 0.28]),
        ([0.0, 0.0, 0.0], [0.0, len, 0.0], [0.32, 0.78, 0.38]),
        ([0.0, 0.0, 0.0], [0.0, 0.0, len], [0.30, 0.55, 0.95]),
    ]
}

/// Unit wire cube at integer cell (x,y,z) with inset to reduce z-fight.
pub fn build_wire_cube(x: i32, y: i32, z: i32, inset: f32) -> Vec<[f32; 3]> {
    let x0 = x as f32 + inset;
    let y0 = y as f32 + inset;
    let z0 = z as f32 + inset;
    let x1 = x as f32 + 1.0 - inset;
    let y1 = y as f32 + 1.0 - inset;
    let z1 = z as f32 + 1.0 - inset;
    build_bounds_lines_aabb([x0, y0, z0], [x1, y1, z1])
}

/// Solid ghost cube (slightly inset) for translucent overlay.
pub fn build_ghost_cube_mesh(x: i32, y: i32, z: i32, rgba: [f32; 3], inset: f32) -> MeshData {
    let x0 = x as f32 + inset;
    let y0 = y as f32 + inset;
    let z0 = z as f32 + inset;
    let x1 = x as f32 + 1.0 - inset;
    let y1 = y as f32 + 1.0 - inset;
    let z1 = z as f32 + 1.0 - inset;
    // 6 faces, two tris each
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        (
            [1.0, 0.0, 0.0],
            [
                [x1, y0, z0],
                [x1, y1, z0],
                [x1, y1, z1],
                [x1, y0, z1],
            ],
        ),
        (
            [-1.0, 0.0, 0.0],
            [
                [x0, y0, z1],
                [x0, y1, z1],
                [x0, y1, z0],
                [x0, y0, z0],
            ],
        ),
        (
            [0.0, 1.0, 0.0],
            [
                [x0, y1, z0],
                [x0, y1, z1],
                [x1, y1, z1],
                [x1, y1, z0],
            ],
        ),
        (
            [0.0, -1.0, 0.0],
            [
                [x1, y0, z0],
                [x1, y0, z1],
                [x0, y0, z1],
                [x0, y0, z0],
            ],
        ),
        (
            [0.0, 0.0, 1.0],
            [
                [x0, y0, z1],
                [x1, y0, z1],
                [x1, y1, z1],
                [x0, y1, z1],
            ],
        ),
        (
            [0.0, 0.0, -1.0],
            [
                [x0, y1, z0],
                [x1, y1, z0],
                [x1, y0, z0],
                [x0, y0, z0],
            ],
        ),
    ];
    let mut vertices = Vec::new();
    let mut opaque_indices = Vec::new();
    let color = [rgba[0], rgba[1], rgba[2], 1.0];
    for (n, corners) in faces {
        let start = vertices.len() as u32;
        for c in corners {
            vertices.push(Vertex::new(c, n, color, 0.0, 0.0, 1.0));
        }
        opaque_indices.extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
    }
    MeshData {
        vertices,
        opaque_indices,
        transparent_indices: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxel_core::{Material, MaterialKind, VoxelModel};

    fn one_voxel(color: u8) -> VoxelModel {
        let mut model = VoxelModel::new(2, 2, 2).unwrap();
        model.set(0, 0, 0, color).unwrap();
        model
    }

    #[test]
    fn glass_faces_are_transparent() {
        let model = one_voxel(1);
        let palette = Palette::default();
        let mut materials = MaterialTable::default();
        let mut glass = Material::default();
        glass.kind = MaterialKind::Glass;
        glass.weight = 1.0;
        materials.set(1, glass).unwrap();
        let mesh = build_mesh(&model, &palette, &materials);
        assert!(mesh.opaque_indices.is_empty());
        assert!(!mesh.transparent_indices.is_empty());
        let v = mesh.vertices[0];
        let expected_a = (1.0 - 1.0 * 0.65) * (palette.get(1).a as f32 / 255.0);
        assert!((v.color[3] - expected_a).abs() < 1e-5);
        assert_eq!(v.emit, 0.0);
        assert_eq!(v.metal, 0.0);
    }

    #[test]
    fn emit_is_opaque_unlit_add() {
        let model = one_voxel(1);
        let palette = Palette::default();
        let mut materials = MaterialTable::default();
        let mut emit = Material::default();
        emit.kind = MaterialKind::Emit;
        emit.weight = 1.0;
        emit.flux = 2.0;
        materials.set(1, emit).unwrap();
        let mesh = build_mesh(&model, &palette, &materials);
        assert!(mesh.transparent_indices.is_empty());
        assert!(!mesh.opaque_indices.is_empty());
        let v = mesh.vertices[0];
        let c = palette.get(1);
        assert!((v.color[0] - c.r as f32 / 255.0).abs() < 1e-5);
        assert_eq!(v.color[3], 1.0);
        assert!((v.emit - (1.0 * (1.0 + 2.0 * 0.35))).abs() < 1e-5);
    }

    #[test]
    fn metal_passes_spec_attributes() {
        let model = one_voxel(1);
        let palette = Palette::default();
        let mut materials = MaterialTable::default();
        let mut metal = Material::default();
        metal.kind = MaterialKind::Metal;
        metal.weight = 0.8;
        metal.rough = 0.2;
        materials.set(1, metal).unwrap();
        let mesh = build_mesh(&model, &palette, &materials);
        let v = mesh.vertices[0];
        assert!((v.metal - 0.8).abs() < 1e-5);
        assert!((v.rough - 0.2).abs() < 1e-5);
        assert_eq!(v.color[3], 1.0);
        assert!(mesh.transparent_indices.is_empty());
    }
}
