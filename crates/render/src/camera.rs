//! Orbit camera. Mouse deltas come from the host; the camera never talks to a window system.
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
    /// Target and distance that frame the whole model, for `reset` and `preset`.
    pub home: (Vec3, f32),
}

impl Orbit {
    /// Frame a bounding box from the front.
    pub fn fit(min: Vec3, max: Vec3) -> Self {
        let target = (min + max) * 0.5;
        let radius = ((max - min).length() * 0.5).max(1e-4);
        let fov_y = 30f32.to_radians();
        let distance = radius / (fov_y * 0.5).sin() * 1.05;
        Orbit { target, yaw: 0.0, pitch: 0.05, distance, fov_y, radius, home: (target, distance) }
    }

    /// Turn by a mouse delta in pixels.
    pub fn rotate(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.008;
        self.pitch = (self.pitch + dy * 0.008).clamp(-1.5, 1.5);
    }

    /// Move closer (positive) or farther (negative), in wheel steps.
    pub fn zoom(&mut self, steps: f32) {
        self.distance = (self.distance * (1.0 - steps * 0.1)).clamp(self.radius * 0.2, self.radius * 20.0);
    }

    /// Slide the target by a mouse delta in pixels.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let (right, up, _) = self.basis();
        let k = self.distance * 0.0015;
        self.target += (-right * dx + up * dy) * k;
    }

    /// Back to the front view that frames the whole model.
    pub fn reset(&mut self) {
        self.preset(crate::thumb::View::Front);
    }

    /// One of the thumbnail views, framing the whole model.
    pub fn preset(&mut self, view: crate::thumb::View) {
        (self.yaw, self.pitch) = view.angles();
        (self.target, self.distance) = self.home;
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
