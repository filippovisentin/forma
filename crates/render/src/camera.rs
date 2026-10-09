//! Orbit camera, Z-up like Rhino.

use glam::{DMat4, DVec3, Mat4};

/// Standard Rhino-style views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardView {
    Perspective,
    Top,
    Front,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Point the camera orbits around, in scene coordinates (already offset by the
    /// scene origin, see `Scene::origin`).
    pub target: DVec3,
    pub distance: f64,
    /// Azimuth around +Z, radians (0 = looking from +X).
    pub yaw: f64,
    /// Elevation, radians (positive = from above).
    pub pitch: f64,
    /// Vertical field of view, radians.
    pub fov_y: f64,
    /// Parallel projection (Top/Front/Right views).
    pub ortho: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self::view(StandardView::Perspective)
    }
}

impl Camera {
    pub fn view(v: StandardView) -> Self {
        let (yaw, pitch, ortho) = match v {
            StandardView::Perspective => ((-60f64).to_radians(), 30f64.to_radians(), false),
            StandardView::Top => ((-90f64).to_radians(), 89.999f64.to_radians(), true),
            StandardView::Front => ((-90f64).to_radians(), 0.0, true),
            StandardView::Right => (0.0, 0.0, true),
        };
        Camera {
            target: DVec3::ZERO,
            distance: 1000.0,
            yaw,
            pitch,
            fov_y: 40f64.to_radians(),
            ortho,
        }
    }

    /// Unit vector from the target towards the eye.
    pub fn back(&self) -> DVec3 {
        DVec3::new(
            self.pitch.cos() * self.yaw.cos(),
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
        )
    }

    pub fn eye(&self) -> DVec3 {
        self.target + self.back() * self.distance
    }

    fn up(&self) -> DVec3 {
        if self.pitch.abs() > 89.0f64.to_radians() {
            // Looking straight down/up: keep +Y up on screen.
            DVec3::new(-self.yaw.cos(), -self.yaw.sin(), 0.0) * self.pitch.signum()
        } else {
            DVec3::Z
        }
    }

    /// Half the visible height at the target distance.
    pub fn half_height(&self) -> f64 {
        self.distance * (self.fov_y / 2.0).tan()
    }

    pub fn view_proj(&self, aspect: f64) -> Mat4 {
        self.view_proj_f64(aspect).as_mat4()
    }

    /// Double-precision view-projection (scene coordinates → clip space), for picking.
    pub fn view_proj_f64(&self, aspect: f64) -> DMat4 {
        let view = DMat4::look_at_rh(self.eye(), self.target, self.up());
        let near = (self.distance * 1e-3).max(1e-3);
        let far = self.distance * 100.0 + 1.0;
        let proj = if self.ortho {
            let h = self.half_height();
            let w = h * aspect;
            DMat4::orthographic_rh(-w, w, -h, h, -far, far)
        } else {
            DMat4::perspective_rh(self.fov_y, aspect, near, far)
        };
        proj * view
    }

    /// Ray through a point in normalised device coordinates (x, y in −1..1),
    /// in scene coordinates: (origin, unit direction).
    pub fn ray(&self, ndc_x: f64, ndc_y: f64, aspect: f64) -> (DVec3, DVec3) {
        let inv = self.view_proj_f64(aspect).inverse();
        let near = inv.project_point3(DVec3::new(ndc_x, ndc_y, 0.0));
        let far = inv.project_point3(DVec3::new(ndc_x, ndc_y, 1.0));
        let dir = (far - near).normalize_or_zero();
        (
            near,
            if dir == DVec3::ZERO {
                -self.back()
            } else {
                dir
            },
        )
    }

    /// Normalised device coordinates of a scene point and whether it is in front of
    /// the camera.
    pub fn project(&self, p: DVec3, aspect: f64) -> Option<(f64, f64)> {
        let clip = self.view_proj_f64(aspect) * p.extend(1.0);
        if clip.w <= 1e-9 {
            return None;
        }
        Some((clip.x / clip.w, clip.y / clip.w))
    }

    /// Zoom by `factor` keeping the scene point `anchor` fixed on screen.
    pub fn zoom_at(&mut self, factor: f64, anchor: DVec3) {
        let new_distance = (self.distance * factor).max(1e-3);
        let k = new_distance / self.distance;
        // Move the target towards the anchor projected on the target plane.
        let n = self.back();
        let a = anchor - n * (anchor - self.target).dot(n);
        self.target = a + (self.target - a) * k;
        self.distance = new_distance;
    }

    /// Fit a box (scene coordinates) in view, keeping the viewing direction.
    pub fn fit(&mut self, min: DVec3, max: DVec3, aspect: f64) {
        let center = (min + max) / 2.0;
        let radius = ((max - min).length() / 2.0).max(1e-6);
        let vertical = self.fov_y / 2.0;
        let horizontal = (vertical.tan() * aspect.max(1e-6)).atan();
        let half_fov = vertical.min(horizontal);
        self.target = center;
        self.distance = radius / half_fov.sin() * 1.05;
        // Tighten: project the 8 corners and scale until the box fills ~90% of the view.
        // In perspective the eye must stay outside the bounding sphere, otherwise
        // nearby faces get clipped and fill the view.
        let aspect = aspect.max(1e-6);
        let min_distance = radius * 1.1;
        for _ in 0..4 {
            let vp = self.view_proj_f64(aspect);
            let mut ext: f64 = 0.0;
            let mut behind = false;
            for i in 0..8 {
                let c = DVec3::new(
                    if i & 1 == 0 { min.x } else { max.x },
                    if i & 2 == 0 { min.y } else { max.y },
                    if i & 4 == 0 { min.z } else { max.z },
                );
                let clip = vp * c.extend(1.0);
                if clip.w <= 1e-9 {
                    behind = true;
                    break;
                }
                ext = ext
                    .max((clip.x / clip.w).abs())
                    .max((clip.y / clip.w).abs());
            }
            if behind {
                self.distance *= 1.5;
                continue;
            }
            if !(ext.is_finite() && ext > 1e-6) {
                break;
            }
            let k = ext / 0.9;
            if self.ortho {
                self.distance *= k;
            } else {
                self.distance = (self.distance * (1.0 + (k - 1.0) * 0.8)).max(min_distance);
            }
        }
    }

    /// Orbit by screen-space pixels (Rhino: right-drag in perspective).
    pub fn orbit(&mut self, dx: f64, dy: f64) {
        self.yaw -= dx * 0.008;
        self.pitch =
            (self.pitch + dy * 0.008).clamp((-89.999f64).to_radians(), 89.999f64.to_radians());
        self.ortho = false;
    }

    /// Pan by screen-space pixels for a viewport `height_px` tall.
    pub fn pan(&mut self, dx: f64, dy: f64, height_px: f64) {
        let per_px = 2.0 * self.half_height() / height_px.max(1.0);
        let fwd = -self.back();
        let right = fwd.cross(self.up()).normalize_or_zero();
        let up = right.cross(fwd).normalize_or_zero();
        self.target += (-right * dx + up * dy) * per_px;
    }

    /// Zoom by a factor (< 1 zooms in).
    pub fn zoom(&mut self, factor: f64) {
        self.distance = (self.distance * factor).max(1e-3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_puts_box_in_front_of_the_eye() {
        let mut c = Camera::default();
        c.fit(
            DVec3::new(-10.0, -10.0, 0.0),
            DVec3::new(10.0, 10.0, 5.0),
            1.5,
        );
        let vp = c.view_proj(1.5);
        let p = vp.project_point3(glam::Vec3::new(0.0, 0.0, 2.5));
        assert!(p.x.abs() < 1e-3 && p.y.abs() < 1e-3);
        for corner in [
            glam::Vec3::new(10.0, 10.0, 5.0),
            glam::Vec3::new(-10.0, -10.0, 0.0),
        ] {
            let q = vp.project_point3(corner);
            assert!(q.x.abs() <= 1.0 && q.y.abs() <= 1.0, "{q:?}");
        }
    }

    #[test]
    fn top_view_looks_down_with_y_up() {
        let c = Camera::view(StandardView::Top);
        let vp = c.view_proj(1.0);
        let up = vp.project_point3(glam::Vec3::new(0.0, 100.0, 0.0));
        let right = vp.project_point3(glam::Vec3::new(100.0, 0.0, 0.0));
        assert!(up.y > 0.0 && up.x.abs() < 1e-3, "{up:?}");
        assert!(right.x > 0.0 && right.y.abs() < 1e-3, "{right:?}");
    }
}
