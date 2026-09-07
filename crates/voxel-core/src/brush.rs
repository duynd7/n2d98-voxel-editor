use crate::{ColorIndex, IVec3, Result, VoxelModel};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrushKind {
    Voxel,
    Box,
    Sphere,
    Erase,
    FloodFill,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirrorAxes {
    pub x: bool,
    pub y: bool,
    pub z: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BrushOptions {
    pub kind: BrushKind,
    pub color: ColorIndex,
    /// Radius for sphere / half-extent for box around center.
    pub radius: u32,
    pub mirror: MirrorAxes,
}

impl Default for BrushOptions {
    fn default() -> Self {
        Self {
            kind: BrushKind::Voxel,
            color: 1,
            radius: 0,
            mirror: MirrorAxes::default(),
        }
    }
}

/// Apply a brush at `center`. Returns number of voxels changed.
pub fn apply_brush(model: &mut VoxelModel, center: IVec3, opts: BrushOptions) -> Result<usize> {
    let centers = mirrored_points(model, center, opts.mirror);
    let mut changed = 0usize;
    for c in centers {
        changed += apply_once(model, c, opts)?;
    }
    Ok(changed)
}

fn mirrored_points(model: &VoxelModel, p: IVec3, m: MirrorAxes) -> Vec<IVec3> {
    let (sx, sy, sz) = model.size();
    let mut pts = vec![p];
    if m.x {
        pts = pts
            .into_iter()
            .flat_map(|q| [q, IVec3::new((sx as i32 - 1) - q.x, q.y, q.z)])
            .collect();
    }
    if m.y {
        pts = pts
            .into_iter()
            .flat_map(|q| [q, IVec3::new(q.x, (sy as i32 - 1) - q.y, q.z)])
            .collect();
    }
    if m.z {
        pts = pts
            .into_iter()
            .flat_map(|q| [q, IVec3::new(q.x, q.y, (sz as i32 - 1) - q.z)])
            .collect();
    }
    pts.sort_by_key(|v| (v.x, v.y, v.z));
    pts.dedup();
    pts
}

fn apply_once(model: &mut VoxelModel, center: IVec3, opts: BrushOptions) -> Result<usize> {
    match opts.kind {
        BrushKind::Voxel => set_if_changed(model, center.x, center.y, center.z, opts.color),
        BrushKind::Erase => set_if_changed(model, center.x, center.y, center.z, 0),
        BrushKind::Box => fill_box(model, center, opts.radius, opts.color),
        BrushKind::Sphere => fill_sphere(model, center, opts.radius, opts.color),
        BrushKind::FloodFill => flood_fill(model, center, opts.color),
    }
}

fn set_if_changed(
    model: &mut VoxelModel,
    x: i32,
    y: i32,
    z: i32,
    color: ColorIndex,
) -> Result<usize> {
    if !model.in_bounds(x, y, z) {
        return Ok(0);
    }
    let prev = model.get(x, y, z)?;
    if prev == color {
        return Ok(0);
    }
    model.set(x, y, z, color)?;
    Ok(1)
}

pub fn fill_box(
    model: &mut VoxelModel,
    center: IVec3,
    radius: u32,
    color: ColorIndex,
) -> Result<usize> {
    let r = radius as i32;
    let mut n = 0;
    for z in (center.z - r)..=(center.z + r) {
        for y in (center.y - r)..=(center.y + r) {
            for x in (center.x - r)..=(center.x + r) {
                n += set_if_changed(model, x, y, z, color)?;
            }
        }
    }
    Ok(n)
}

pub fn fill_box_region(
    model: &mut VoxelModel,
    min: IVec3,
    max: IVec3,
    color: ColorIndex,
) -> Result<usize> {
    let (x0, x1) = (min.x.min(max.x), min.x.max(max.x));
    let (y0, y1) = (min.y.min(max.y), min.y.max(max.y));
    let (z0, z1) = (min.z.min(max.z), min.z.max(max.z));
    let mut n = 0;
    for z in z0..=z1 {
        for y in y0..=y1 {
            for x in x0..=x1 {
                n += set_if_changed(model, x, y, z, color)?;
            }
        }
    }
    Ok(n)
}

pub fn fill_sphere(
    model: &mut VoxelModel,
    center: IVec3,
    radius: u32,
    color: ColorIndex,
) -> Result<usize> {
    let r = radius as i32;
    let r2 = (radius as i64) * (radius as i64);
    let mut n = 0;
    for z in (center.z - r)..=(center.z + r) {
        for y in (center.y - r)..=(center.y + r) {
            for x in (center.x - r)..=(center.x + r) {
                let dx = (x - center.x) as i64;
                let dy = (y - center.y) as i64;
                let dz = (z - center.z) as i64;
                if dx * dx + dy * dy + dz * dz <= r2 {
                    n += set_if_changed(model, x, y, z, color)?;
                }
            }
        }
    }
    Ok(n)
}

pub fn flood_fill(model: &mut VoxelModel, start: IVec3, color: ColorIndex) -> Result<usize> {
    if !model.in_bounds(start.x, start.y, start.z) {
        return Ok(0);
    }
    let target = model.get(start.x, start.y, start.z)?;
    if target == color {
        return Ok(0);
    }
    let mut stack = vec![start];
    let mut n = 0;
    while let Some(p) = stack.pop() {
        if !model.in_bounds(p.x, p.y, p.z) {
            continue;
        }
        if model.get(p.x, p.y, p.z)? != target {
            continue;
        }
        model.set(p.x, p.y, p.z, color)?;
        n += 1;
        stack.push(IVec3::new(p.x + 1, p.y, p.z));
        stack.push(IVec3::new(p.x - 1, p.y, p.z));
        stack.push(IVec3::new(p.x, p.y + 1, p.z));
        stack.push(IVec3::new(p.x, p.y - 1, p.z));
        stack.push(IVec3::new(p.x, p.y, p.z + 1));
        stack.push(IVec3::new(p.x, p.y, p.z - 1));
    }
    Ok(n)
}

/// MagicaVoxel-style axis mirror of the whole volume.
pub fn mirror_model(model: &mut VoxelModel, axis: char) -> Result<()> {
    let (sx, sy, sz) = model.size();
    let mut clone = model.clone();
    match axis {
        'x' | 'X' => {
            for z in 0..sz as i32 {
                for y in 0..sy as i32 {
                    for x in 0..sx as i32 {
                        let c = model.get(x, y, z)?;
                        clone.set((sx as i32 - 1) - x, y, z, c)?;
                    }
                }
            }
        }
        'y' | 'Y' => {
            for z in 0..sz as i32 {
                for y in 0..sy as i32 {
                    for x in 0..sx as i32 {
                        let c = model.get(x, y, z)?;
                        clone.set(x, (sy as i32 - 1) - y, z, c)?;
                    }
                }
            }
        }
        'z' | 'Z' => {
            for z in 0..sz as i32 {
                for y in 0..sy as i32 {
                    for x in 0..sx as i32 {
                        let c = model.get(x, y, z)?;
                        clone.set(x, y, (sz as i32 - 1) - z, c)?;
                    }
                }
            }
        }
        _ => return Ok(()),
    }
    *model = clone;
    Ok(())
}
