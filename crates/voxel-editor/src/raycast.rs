//! Amanatides & Woo DDA voxel raycast.
//! Prefer the surface the ray reaches (first solid entered from empty / outside).
//! Empty volume fallback uses the *far* cell along the ray inside the bounds.

use glam::Vec3;
use voxel_core::{IVec3, MvRotation, NodeId, VoxelModel, WorldInstance};

#[derive(Debug, Clone, Copy)]
pub struct Hit {
    /// Solid voxel under the cursor (or far empty cell when no solid).
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Adjacent empty cell for placement (toward camera / previous along ray).
    pub place_x: i32,
    pub place_y: i32,
    pub place_z: i32,
    /// Ray parameter at hit (world-comparable after instance transform).
    pub t: f32,
    pub from_solid: bool,
    pub transform_id: Option<NodeId>,
    pub model_id: Option<usize>,
}

pub fn raycast(model: &VoxelModel, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<Hit> {
    raycast_model(model, origin, dir, max_dist, None, None)
}

pub fn raycast_world(
    instances: &[WorldInstance],
    models: &[VoxelModel],
    origin: Vec3,
    dir: Vec3,
    max_dist: f32,
) -> Option<Hit> {
    let dir = dir.normalize_or_zero();
    if dir.length_squared() < 1e-8 {
        return None;
    }
    let mut best: Option<Hit> = None;
    for inst in instances.iter().filter(|i| !i.hidden) {
        let Some(model) = models.get(inst.model_id) else {
            continue;
        };
        let local_origin = world_point_to_local(origin, inst.translation, inst.rotation);
        let local_dir = rotate_dir(dir, inst.rotation, true);
        if let Some(mut hit) = raycast_model(
            model,
            local_origin,
            local_dir,
            max_dist,
            Some(inst.transform_id),
            Some(inst.model_id),
        ) {
            // Convert local t to approximate world distance along the view ray.
            let world_hit = local_to_world_f(
                Vec3::new(hit.x as f32 + 0.5, hit.y as f32 + 0.5, hit.z as f32 + 0.5),
                inst.translation,
                inst.rotation,
            );
            hit.t = (world_hit - origin).dot(dir).max(0.0);
            hit.transform_id = Some(inst.transform_id);
            hit.model_id = Some(inst.model_id);

            let take = match &best {
                None => true,
                // Prefer any solid over empty-volume fallback; among same kind, nearest along ray.
                Some(b) => match (hit.from_solid, b.from_solid) {
                    (true, false) => true,
                    (false, true) => false,
                    _ => hit.t < b.t,
                },
            };
            if take {
                best = Some(hit);
            }
        }
    }
    best
}

fn local_to_world_f(local: Vec3, t: IVec3, r: MvRotation) -> Vec3 {
    let m = r.to_matrix();
    let rx = m[0][0] as f32 * local.x + m[0][1] as f32 * local.y + m[0][2] as f32 * local.z;
    let ry = m[1][0] as f32 * local.x + m[1][1] as f32 * local.y + m[1][2] as f32 * local.z;
    let rz = m[2][0] as f32 * local.x + m[2][1] as f32 * local.y + m[2][2] as f32 * local.z;
    Vec3::new(rx + t.x as f32, ry + t.y as f32, rz + t.z as f32)
}

fn world_point_to_local(world: Vec3, t: IVec3, r: MvRotation) -> Vec3 {
    let p = Vec3::new(world.x - t.x as f32, world.y - t.y as f32, world.z - t.z as f32);
    let m = r.to_matrix();
    Vec3::new(
        m[0][0] as f32 * p.x + m[1][0] as f32 * p.y + m[2][0] as f32 * p.z,
        m[0][1] as f32 * p.x + m[1][1] as f32 * p.y + m[2][1] as f32 * p.z,
        m[0][2] as f32 * p.x + m[1][2] as f32 * p.y + m[2][2] as f32 * p.z,
    )
}

fn rotate_dir(dir: Vec3, r: MvRotation, inverse: bool) -> Vec3 {
    let m = r.to_matrix();
    if inverse {
        Vec3::new(
            m[0][0] as f32 * dir.x + m[1][0] as f32 * dir.y + m[2][0] as f32 * dir.z,
            m[0][1] as f32 * dir.x + m[1][1] as f32 * dir.y + m[2][1] as f32 * dir.z,
            m[0][2] as f32 * dir.x + m[1][2] as f32 * dir.y + m[2][2] as f32 * dir.z,
        )
        .normalize_or_zero()
    } else {
        Vec3::new(
            m[0][0] as f32 * dir.x + m[0][1] as f32 * dir.y + m[0][2] as f32 * dir.z,
            m[1][0] as f32 * dir.x + m[1][1] as f32 * dir.y + m[1][2] as f32 * dir.z,
            m[2][0] as f32 * dir.x + m[2][1] as f32 * dir.y + m[2][2] as f32 * dir.z,
        )
        .normalize_or_zero()
    }
}

fn raycast_model(
    model: &VoxelModel,
    origin: Vec3,
    dir: Vec3,
    max_dist: f32,
    transform_id: Option<NodeId>,
    model_id: Option<usize>,
) -> Option<Hit> {
    let dir = dir.normalize_or_zero();
    if dir.length_squared() < 1e-8 {
        return None;
    }

    if let Some(mut hit) = raycast_voxels(model, origin, dir, max_dist) {
        hit.transform_id = transform_id;
        hit.model_id = model_id;
        return Some(hit);
    }

    let mut hit = raycast_far_volume(model, origin, dir, max_dist)?;
    hit.transform_id = transform_id;
    hit.model_id = model_id;
    Some(hit)
}

/// First solid entered from empty/outside along the ray (skips solids at ray origin).
fn raycast_voxels(model: &VoxelModel, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<Hit> {
    let (sx, sy, sz) = model.size();
    let mut t = 0.0_f32;

    let mut x = origin.x.floor() as i32;
    let mut y = origin.y.floor() as i32;
    let mut z = origin.z.floor() as i32;

    let step_x = if dir.x >= 0.0 { 1 } else { -1 };
    let step_y = if dir.y >= 0.0 { 1 } else { -1 };
    let step_z = if dir.z >= 0.0 { 1 } else { -1 };

    let t_delta_x = inv_delta(dir.x);
    let t_delta_y = inv_delta(dir.y);
    let t_delta_z = inv_delta(dir.z);

    let mut t_max_x = initial_t_max(origin.x, dir.x, x);
    let mut t_max_y = initial_t_max(origin.y, dir.y, y);
    let mut t_max_z = initial_t_max(origin.z, dir.z, z);

    let mut prev = (x, y, z);
    // If we start inside a solid, ignore until we leave solids so hover isn't stuck on the near blob.
    let mut seen_empty = !in_bounds(sx, sy, sz, x, y, z)
        || model.get_or_empty(x, y, z) == 0;

    for _ in 0..1024 {
        if t > max_dist {
            break;
        }

        let inside = in_bounds(sx, sy, sz, x, y, z);
        if inside {
            let solid = model.get_or_empty(x, y, z) != 0;
            if solid {
                if seen_empty {
                    // Entered a solid from empty / outside — this is the surface under the cursor.
                    return Some(Hit {
                        x,
                        y,
                        z,
                        place_x: prev.0,
                        place_y: prev.1,
                        place_z: prev.2,
                        t,
                        from_solid: true,
                        transform_id: None,
                        model_id: None,
                    });
                }
            } else {
                seen_empty = true;
            }
        } else {
            seen_empty = true;
            if (x < -2
                || y < -2
                || z < -2
                || x > sx as i32 + 1
                || y > sy as i32 + 1
                || z > sz as i32 + 1)
                && t > 0.01
            {
                break;
            }
        }

        prev = (x, y, z);
        if t_max_x < t_max_y {
            if t_max_x < t_max_z {
                t = t_max_x;
                t_max_x += t_delta_x;
                x += step_x;
            } else {
                t = t_max_z;
                t_max_z += t_delta_z;
                z += step_z;
            }
        } else if t_max_y < t_max_z {
            t = t_max_y;
            t_max_y += t_delta_y;
            y += step_y;
        } else {
            t = t_max_z;
            t_max_z += t_delta_z;
            z += step_z;
        }
    }
    None
}

/// No solid along ray: place on the *farthest* empty cell still inside the volume.
fn raycast_far_volume(model: &VoxelModel, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<Hit> {
    let (sx, sy, sz) = model.size();
    let min = Vec3::ZERO;
    let max = Vec3::new(sx as f32, sy as f32, sz as f32);

    let (t_enter, t_exit) = ray_aabb(origin, dir, min, max)?;
    // Use far side of the volume (clamp to max_dist).
    let t_hit = t_exit.min(max_dist);
    if t_hit < 0.0 || t_enter > max_dist {
        return None;
    }
    // Step slightly back from the exit face so we stay inside.
    let t_inside = (t_hit - 1e-3).max(t_enter + 1e-3);
    let p = origin + dir * t_inside;
    let x = p.x.floor() as i32;
    let y = p.y.floor() as i32;
    let z = p.z.floor() as i32;
    if !in_bounds(sx, sy, sz, x, y, z) {
        return None;
    }
    Some(Hit {
        x,
        y,
        z,
        place_x: x,
        place_y: y,
        place_z: z,
        t: t_inside,
        from_solid: false,
        transform_id: None,
        model_id: None,
    })
}

fn in_bounds(sx: u32, sy: u32, sz: u32, x: i32, y: i32, z: i32) -> bool {
    x >= 0 && y >= 0 && z >= 0 && x < sx as i32 && y < sy as i32 && z < sz as i32
}

fn inv_delta(d: f32) -> f32 {
    if d.abs() < 1e-8 {
        f32::INFINITY
    } else {
        (1.0 / d).abs()
    }
}

fn initial_t_max(o: f32, d: f32, cell: i32) -> f32 {
    if d.abs() < 1e-8 {
        f32::INFINITY
    } else {
        let next = if d > 0.0 {
            cell as f32 + 1.0
        } else {
            cell as f32
        };
        (next - o) / d
    }
}

fn ray_aabb(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<(f32, f32)> {
    let mut tmin = f32::NEG_INFINITY;
    let mut tmax = f32::INFINITY;
    for i in 0..3 {
        let o = origin[i];
        let d = dir[i];
        let mn = min[i];
        let mx = max[i];
        if d.abs() < 1e-8 {
            if o < mn || o > mx {
                return None;
            }
        } else {
            let mut t1 = (mn - o) / d;
            let mut t2 = (mx - o) / d;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            tmin = tmin.max(t1);
            tmax = tmax.min(t2);
            if tmin > tmax {
                return None;
            }
        }
    }
    // Ray starts inside: t_enter = 0
    let t_enter = tmin.max(0.0);
    if tmax < t_enter {
        return None;
    }
    Some((t_enter, tmax))
}
