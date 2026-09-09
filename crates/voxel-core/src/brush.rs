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
    Line,
    Plane,
    Circle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirrorAxes {
    pub x: bool,
    pub y: bool,
    pub z: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
        BrushKind::Voxel | BrushKind::Line => {
            set_if_changed(model, center.x, center.y, center.z, opts.color)
        }
        BrushKind::Erase => set_if_changed(model, center.x, center.y, center.z, 0),
        BrushKind::Box => fill_box(model, center, opts.radius, opts.color),
        BrushKind::Sphere => fill_sphere(model, center, opts.radius, opts.color),
        BrushKind::FloodFill => flood_fill(model, center, opts.color),
        BrushKind::Circle => fill_circle(model, center, opts.radius, 'z', opts.color),
        BrushKind::Plane => {
            let r = opts.radius as i32;
            fill_plane_rect(
                model,
                IVec3::new(center.x - r, center.y - r, center.z),
                IVec3::new(center.x + r, center.y + r, center.z),
                'z',
                opts.color,
            )
        }
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

/// 3D Bresenham line, inclusive of both endpoints. Out-of-bounds cells are skipped.
pub fn fill_line(
    model: &mut VoxelModel,
    a: IVec3,
    b: IVec3,
    color: ColorIndex,
) -> Result<usize> {
    let dx = (b.x - a.x).abs();
    let dy = (b.y - a.y).abs();
    let dz = (b.z - a.z).abs();
    let sx = if a.x < b.x { 1 } else { -1 };
    let sy = if a.y < b.y { 1 } else { -1 };
    let sz = if a.z < b.z { 1 } else { -1 };
    let dm = dx.max(dy).max(dz);
    let mut x = a.x;
    let mut y = a.y;
    let mut z = a.z;
    let mut x_err = dm / 2;
    let mut y_err = dm / 2;
    let mut z_err = dm / 2;
    let mut n = set_if_changed(model, x, y, z, color)?;
    for _ in 0..dm {
        x_err -= dx;
        if x_err < 0 {
            x_err += dm;
            x += sx;
        }
        y_err -= dy;
        if y_err < 0 {
            y_err += dm;
            y += sy;
        }
        z_err -= dz;
        if z_err < 0 {
            z_err += dm;
            z += sz;
        }
        n += set_if_changed(model, x, y, z, color)?;
    }
    Ok(n)
}

/// Filled disk on the plane whose normal is `axis` (`'x'|'y'|'z'`).
pub fn fill_circle(
    model: &mut VoxelModel,
    center: IVec3,
    radius: u32,
    axis: char,
    color: ColorIndex,
) -> Result<usize> {
    let r = radius as i32;
    let r2 = (radius as i64) * (radius as i64);
    let mut n = 0;
    match axis {
        'x' | 'X' => {
            for z in (center.z - r)..=(center.z + r) {
                for y in (center.y - r)..=(center.y + r) {
                    let dy = (y - center.y) as i64;
                    let dz = (z - center.z) as i64;
                    if dy * dy + dz * dz <= r2 {
                        n += set_if_changed(model, center.x, y, z, color)?;
                    }
                }
            }
        }
        'y' | 'Y' => {
            for z in (center.z - r)..=(center.z + r) {
                for x in (center.x - r)..=(center.x + r) {
                    let dx = (x - center.x) as i64;
                    let dz = (z - center.z) as i64;
                    if dx * dx + dz * dz <= r2 {
                        n += set_if_changed(model, x, center.y, z, color)?;
                    }
                }
            }
        }
        'z' | 'Z' => {
            for y in (center.y - r)..=(center.y + r) {
                for x in (center.x - r)..=(center.x + r) {
                    let dx = (x - center.x) as i64;
                    let dy = (y - center.y) as i64;
                    if dx * dx + dy * dy <= r2 {
                        n += set_if_changed(model, x, y, center.z, color)?;
                    }
                }
            }
        }
        _ => {}
    }
    Ok(n)
}

/// Axis-aligned rectangle on a constant-`axis` plane.
///
/// The plane coordinate is **`a`'s component on `axis`** (not min of a/b).
/// The other two coordinates span the inclusive AABB of `a` and `b`.
/// Example: `axis == 'z'` fills all x,y between `a` and `b` at `z = a.z`.
pub fn fill_plane_rect(
    model: &mut VoxelModel,
    a: IVec3,
    b: IVec3,
    axis: char,
    color: ColorIndex,
) -> Result<usize> {
    let mut n = 0;
    match axis {
        'x' | 'X' => {
            let x = a.x;
            let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
            let (z0, z1) = (a.z.min(b.z), a.z.max(b.z));
            for z in z0..=z1 {
                for y in y0..=y1 {
                    n += set_if_changed(model, x, y, z, color)?;
                }
            }
        }
        'y' | 'Y' => {
            let y = a.y;
            let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
            let (z0, z1) = (a.z.min(b.z), a.z.max(b.z));
            for z in z0..=z1 {
                for x in x0..=x1 {
                    n += set_if_changed(model, x, y, z, color)?;
                }
            }
        }
        'z' | 'Z' => {
            let z = a.z;
            let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
            let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
            for y in y0..=y1 {
                for x in x0..=x1 {
                    n += set_if_changed(model, x, y, z, color)?;
                }
            }
        }
        _ => {}
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_line_includes_both_ends() {
        let mut model = VoxelModel::new(16, 16, 16).unwrap();
        let n = fill_line(
            &mut model,
            IVec3::new(1, 2, 3),
            IVec3::new(7, 2, 3),
            4,
        )
        .unwrap();
        assert!(n >= 2);
        assert_eq!(model.get(1, 2, 3).unwrap(), 4);
        assert_eq!(model.get(7, 2, 3).unwrap(), 4);
        for x in 1..=7 {
            assert_eq!(model.get(x, 2, 3).unwrap(), 4);
        }
    }

    #[test]
    fn fill_circle_z_stays_on_plane() {
        let mut model = VoxelModel::new(16, 16, 16).unwrap();
        fill_circle(&mut model, IVec3::new(8, 8, 5), 3, 'z', 2).unwrap();
        assert!(model.voxel_count() > 1);
        assert_eq!(model.get(8, 8, 5).unwrap(), 2);
        for (x, y, z, c) in model.iter_occupied() {
            assert_eq!(z, 5, "circle voxel off plane at ({x},{y},{z})");
            assert_eq!(c, 2);
        }
        assert_eq!(model.get_or_empty(8, 8, 4), 0);
        assert_eq!(model.get_or_empty(8, 8, 6), 0);
    }
}
