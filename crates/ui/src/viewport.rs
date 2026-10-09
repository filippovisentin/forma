//! A viewport: camera, construction plane and screen ↔ world conversions.

use eframe::egui::{Pos2, Rect};
use forma_geom::{Plane, Point3, Vec3};
use forma_render::glam::DVec3;
use forma_render::{Camera, DisplayMode, StandardView, View};

pub fn to_d(p: Point3) -> DVec3 {
    DVec3::new(p.x, p.y, p.z)
}

pub fn to_p(v: DVec3) -> Point3 {
    Point3::new(v.x, v.y, v.z)
}

pub fn to_v(v: DVec3) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

pub struct Viewport {
    pub kind: StandardView,
    pub camera: Camera,
    pub view: Option<View>,
    pub texture: Option<eframe::egui::TextureId>,
    /// Screen rectangle of the image (points), from the last frame.
    pub rect: Rect,
    /// Pixel size of the render target.
    pub px: (u32, u32),
    pub dirty: bool,
    pub mode: DisplayMode,
}

impl Viewport {
    pub fn new(kind: StandardView) -> Self {
        Viewport {
            kind,
            camera: Camera::view(kind),
            view: None,
            texture: None,
            rect: Rect::NOTHING,
            px: (1, 1),
            dirty: true,
            // Rhino-like: drafting views in wireframe, the perspective shaded.
            mode: if kind == StandardView::Perspective {
                DisplayMode::Shaded
            } else {
                DisplayMode::Wireframe
            },
        }
    }

    pub fn name(&self) -> &'static str {
        match self.kind {
            StandardView::Perspective => "Perspective",
            StandardView::Top => "Top",
            StandardView::Front => "Front",
            StandardView::Right => "Right",
        }
    }

    /// Rhino's construction plane for this view.
    pub fn cplane(&self) -> Plane {
        match self.kind {
            StandardView::Front => Plane::FRONT,
            StandardView::Right => Plane::RIGHT,
            _ => Plane::TOP,
        }
    }

    pub fn is_parallel(&self) -> bool {
        self.camera.ortho
    }

    pub fn aspect(&self) -> f64 {
        (self.rect.width() / self.rect.height().max(1.0)) as f64
    }

    fn ndc(&self, pos: Pos2) -> (f64, f64) {
        let x = (pos.x - self.rect.left()) / self.rect.width().max(1.0);
        let y = (pos.y - self.rect.top()) / self.rect.height().max(1.0);
        (x as f64 * 2.0 - 1.0, 1.0 - y as f64 * 2.0)
    }

    /// World ray through a screen position.
    pub fn ray(&self, pos: Pos2, origin: DVec3) -> (Point3, Vec3) {
        let (x, y) = self.ndc(pos);
        let (o, d) = self.camera.ray(x, y, self.aspect());
        (to_p(o + origin), to_v(d))
    }

    /// Screen position of a world point (None when behind the camera).
    pub fn to_screen(&self, p: Point3, origin: DVec3) -> Option<Pos2> {
        let (x, y) = self.camera.project(to_d(p) - origin, self.aspect())?;
        Some(Pos2::new(
            self.rect.left() + ((x + 1.0) / 2.0) as f32 * self.rect.width(),
            self.rect.top() + ((1.0 - y) / 2.0) as f32 * self.rect.height(),
        ))
    }

    /// Point on the construction plane under the cursor. When the plane is seen
    /// edge-on, falls back to the plane through the camera target facing the viewer.
    pub fn cplane_point(&self, pos: Pos2, origin: DVec3) -> Point3 {
        let (o, d) = self.ray(pos, origin);
        let cp = self.cplane();
        if d.dot(cp.z).abs() > 0.02 {
            if let Some(p) = cp.intersect_line(o, d) {
                return p;
            }
        }
        let target = to_p(self.camera.target + origin);
        let facing = Plane::from_normal(target, to_v(self.camera.back()));
        facing.intersect_line(o, d).unwrap_or(target)
    }
}

/// Closest point on the line `a + t·dir` to the ray `o + s·d` (for heights).
pub fn closest_on_line(a: Point3, dir: Vec3, o: Point3, d: Vec3) -> Point3 {
    let n = dir.normalized().unwrap_or(Vec3::Z);
    let d = d.normalized().unwrap_or(Vec3::Z);
    let w0 = a - o;
    let b = n.dot(d);
    let denom = 1.0 - b * b;
    if denom.abs() < 1e-9 {
        return a;
    }
    let t = (b * d.dot(w0) - n.dot(w0)) / denom;
    a + n * t
}
