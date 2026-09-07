//! Orbit camera for the 3D viewport (Z-up, MagicaVoxel-like).

use glam::{Mat4, Vec2, Vec3};

#[derive(Debug, Clone)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub fov_y: f32,
}

impl OrbitCamera {
    pub fn for_volume(sx: u32, sy: u32, sz: u32) -> Self {
        let target = Vec3::new(sx as f32 * 0.5, sy as f32 * 0.5, sz as f32 * 0.5);
        let distance = (sx.max(sy).max(sz) as f32) * 2.2 + 4.0;
        Self {
            target,
            yaw: 0.7,
            pitch: 0.55,
            distance,
            fov_y: 50f32.to_radians(),
        }
    }

    pub fn eye(&self) -> Vec3 {
        let cp = self.pitch.cos();
        let sp = self.pitch.sin();
        let cy = self.yaw.cos();
        let sy = self.yaw.sin();
        self.target
            + Vec3::new(
                self.distance * cp * sy,
                -self.distance * cp * cy,
                self.distance * sp,
            )
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Z)
    }

    pub fn proj_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, aspect.max(0.01), 0.1, 2000.0)
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.proj_matrix(aspect) * self.view_matrix()
    }

    pub fn orbit(&mut self, delta: Vec2) {
        self.yaw += delta.x * 0.01;
        self.pitch = (self.pitch + delta.y * 0.01).clamp(-1.45, 1.45);
    }

    pub fn pan(&mut self, delta: Vec2, viewport_h: f32) {
        let eye = self.eye();
        let forward = (self.target - eye).normalize_or_zero();
        let right = forward.cross(Vec3::Z).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        let scale = self.distance / viewport_h.max(1.0);
        self.target += (-right * delta.x + up * delta.y) * scale;
    }

    pub fn zoom(&mut self, scroll: f32) {
        self.distance = (self.distance * (1.0 - scroll * 0.08)).clamp(2.0, 800.0);
    }

    /// `local` = pointer position relative to viewport min (points).
    /// `size` = viewport size (points).
    pub fn screen_ray(&self, local: Vec2, size: Vec2) -> (Vec3, Vec3) {
        let ndc_x = (local.x / size.x.max(1.0)) * 2.0 - 1.0;
        let ndc_y = 1.0 - (local.y / size.y.max(1.0)) * 2.0;
        let inv = self.view_proj(size.x / size.y.max(1.0)).inverse();
        let near = inv.project_point3(Vec3::new(ndc_x, ndc_y, -1.0));
        let far = inv.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));
        let dir = (far - near).normalize_or_zero();
        (near, dir)
    }
}
