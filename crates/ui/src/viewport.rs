//! A viewport: camera, construction plane and screen ↔ world conversions.

use eframe::egui::{Pos2, Rect};
use forma_geom::{Plane, Point3, Vec3};
use forma_render::glam::{DMat4, DVec3, DVec4};
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
            StandardView::Bottom => "Bottom",
            StandardView::Back => "Back",
            StandardView::Left => "Left",
        }
    }

    /// Rhino's construction plane for this view.
    pub fn cplane(&self) -> Plane {
        match self.kind {
            StandardView::Front => Plane::FRONT,
            StandardView::Right => Plane::RIGHT,
            // Opposite views: same planes as Top / Front / Right, facing the
            // other way (heights grow towards the viewer, like Rhino).
            StandardView::Bottom => Plane {
                origin: Point3::ORIGIN,
                x: Vec3::X,
                y: Vec3::new(0.0, -1.0, 0.0),
                z: Vec3::new(0.0, 0.0, -1.0),
            },
            StandardView::Back => Plane {
                origin: Point3::ORIGIN,
                x: Vec3::new(-1.0, 0.0, 0.0),
                y: Vec3::Z,
                z: Vec3::Y,
            },
            StandardView::Left => Plane {
                origin: Point3::ORIGIN,
                x: Vec3::new(0.0, -1.0, 0.0),
                y: Vec3::Z,
                z: Vec3::new(-1.0, 0.0, 0.0),
            },
            StandardView::Top | StandardView::Perspective => Plane::TOP,
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

    /// Zoom so the screen rectangle `r` fills the view (Rhino's Zoom Window).
    pub fn zoom_window(&mut self, r: Rect) {
        if r.width() < 2.0 || r.height() < 2.0 || !self.rect.is_positive() {
            return;
        }
        let (x, y) = self.ndc(r.center());
        let (o, d) = self.camera.ray(x, y, self.aspect());
        // Where the ray through the window centre meets the target plane.
        let n = self.camera.back();
        let denom = d.dot(n);
        let centre = if denom.abs() > 1e-12 {
            o + d * ((self.camera.target - o).dot(n) / denom)
        } else {
            self.camera.target
        };
        let k = f64::from((r.width() / self.rect.width()).max(r.height() / self.rect.height()));
        self.camera.target = centre;
        self.camera.zoom(k.clamp(1e-6, 1.0));
        self.dirty = true;
    }

    /// World ray through a screen position.
    pub fn ray(&self, pos: Pos2, origin: DVec3) -> (Point3, Vec3) {
        let (x, y) = self.ndc(pos);
        let (o, d) = self.camera.ray(x, y, self.aspect());
        (to_p(o + origin), to_v(d))
    }

    /// Screen position of a world point (None when behind the camera). For many
    /// points use [`Viewport::projector`], which builds the matrix only once.
    pub fn to_screen(&self, p: Point3, origin: DVec3) -> Option<Pos2> {
        self.projector(origin).to_screen(p)
    }

    /// World → screen mapping of this view, for projecting many points.
    pub fn projector(&self, origin: DVec3) -> Projector {
        Projector {
            m: self.camera.view_proj_f64(self.aspect()),
            rect: self.rect,
            origin,
        }
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

/// A frozen world → screen transform (camera matrix, viewport rectangle and
/// scene origin), cheap to apply to many points.
#[derive(Clone)]
pub struct Projector {
    m: DMat4,
    rect: Rect,
    origin: DVec3,
}

impl Projector {
    /// Screen position of a world point (None when behind the camera).
    pub fn to_screen(&self, p: Point3) -> Option<Pos2> {
        let o = self.origin;
        let clip = self.m * DVec4::new(p.x - o.x, p.y - o.y, p.z - o.z, 1.0);
        if clip.w <= 1e-9 {
            return None;
        }
        let (x, y) = (clip.x / clip.w, clip.y / clip.w);
        Some(Pos2::new(
            self.rect.left() + ((x + 1.0) / 2.0) as f32 * self.rect.width(),
            self.rect.top() + ((1.0 - y) / 2.0) as f32 * self.rect.height(),
        ))
    }

    /// Screen rectangle of a world box, or `None` when part of it is behind the
    /// camera (then nothing can be culled).
    pub fn screen_box(&self, min: Point3, max: Point3) -> Option<Rect> {
        let mut r = Rect::NOTHING;
        for i in 0..8 {
            let c = Point3::new(
                if i & 1 == 0 { min.x } else { max.x },
                if i & 2 == 0 { min.y } else { max.y },
                if i & 4 == 0 { min.z } else { max.z },
            );
            r.extend_with(self.to_screen(c)?);
        }
        Some(r)
    }

    /// Could anything inside the world box be within `radius` of `pos` on screen?
    pub fn box_near(&self, min: Point3, max: Point3, pos: Pos2, radius: f32) -> bool {
        self.screen_box(min, max)
            .is_none_or(|r| r.expand(radius).contains(pos))
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
