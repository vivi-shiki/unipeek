use glam::{Mat4, Vec3};

/// Orbit camera around a target point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orbit {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub fov_y: f32,
    pub radius: f32,
}

impl Orbit {
    pub fn fit(min: Vec3, max: Vec3) -> Self {
        let target = (min + max) * 0.5;
        let radius = (max - min).length() * 0.5;
        let fov_y = 30f32.to_radians();
        Orbit { target, yaw: 0.0, pitch: 0.05, distance: radius / (fov_y * 0.5).sin() * 1.05, fov_y, radius }
    }

    pub fn rotate(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.008;
        self.pitch = (self.pitch + dy * 0.008).clamp(-1.5, 1.5);
    }

    pub fn zoom(&mut self, steps: f32) {
        self.distance = (self.distance * (1.0 - steps * 0.1)).clamp(self.radius * 0.2, self.radius * 20.0);
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        let (right, up, _) = self.basis();
        let k = self.distance * 0.0015;
        self.target += (-right * dx + up * dy) * k;
    }

    fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let dir = Vec3::new(self.yaw.sin() * self.pitch.cos(), self.pitch.sin(), self.yaw.cos() * self.pitch.cos());
        let right = Vec3::Y.cross(dir).normalize();
        (right, dir.cross(right), dir)
    }

    pub fn eye(&self) -> Vec3 {
        self.target + self.basis().2 * self.distance
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let near = (self.distance - self.radius * 2.0).max(self.radius * 0.01);
        let far = self.distance + self.radius * 4.0;
        Mat4::perspective_rh(self.fov_y, aspect, near, far) * Mat4::look_at_rh(self.eye(), self.target, Vec3::Y)
    }
}
