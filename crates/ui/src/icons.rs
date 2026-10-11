//! Toolbar icons drawn with the egui painter on a 24×24 design grid. Original
//! drawings (no image assets): dark ink, blue points and handles, pale blue
//! surfaces, in the spirit of a light CAD toolbar.

use crate::tools::ToolKind;
use eframe::egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};
use std::f32::consts::{PI, TAU};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Tool(ToolKind),
    /// Anything else, by command name (New, Open, Hide, SelAll, …).
    Named(&'static str),
}

const BLUE: Color32 = Color32::from_rgb(0, 110, 210);
const FILL: Color32 = Color32::from_rgb(178, 208, 238);
const FILL2: Color32 = Color32::from_rgb(132, 176, 222);
const RED: Color32 = Color32::from_rgb(210, 50, 45);
const GREEN: Color32 = Color32::from_rgb(40, 150, 60);
const GOLD: Color32 = Color32::from_rgb(235, 175, 30);
/// The AI (Ask) colour.
pub const VIOLET: Color32 = Color32::from_rgb(124, 77, 255);

struct Pen<'a> {
    p: &'a Painter,
    r: Rect,
    ink: Color32,
}

impl Pen<'_> {
    fn q(&self, x: f32, y: f32) -> Pos2 {
        let s = self.r.width().min(self.r.height()) / 24.0;
        let o = self.r.center() - vec2(12.0 * s, 12.0 * s);
        pos2(o.x + x * s, o.y + y * s)
    }
    fn s(&self) -> f32 {
        self.r.width().min(self.r.height()) / 24.0
    }
    fn line(&self, pts: &[(f32, f32)], w: f32, c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|(x, y)| self.q(*x, *y)).collect();
        self.p.add(Shape::line(v, Stroke::new(w, c)));
    }
    fn ink(&self, pts: &[(f32, f32)]) {
        self.line(pts, 1.6, self.ink);
    }
    fn thin(&self, pts: &[(f32, f32)]) {
        self.line(pts, 1.0, self.ink);
    }
    fn dashed(&self, a: (f32, f32), b: (f32, f32)) {
        self.p.extend(Shape::dashed_line(
            &[self.q(a.0, a.1), self.q(b.0, b.1)],
            Stroke::new(1.0, self.ink),
            2.5,
            2.0,
        ));
    }
    fn dot(&self, x: f32, y: f32) {
        let c = self.q(x, y);
        let r = 1.9 * self.s().max(0.8);
        self.p.circle_filled(c, r, Color32::WHITE);
        self.p.circle_stroke(c, r, Stroke::new(1.2, BLUE));
    }
    fn solid_dot(&self, x: f32, y: f32, c: Color32) {
        self.p
            .circle_filled(self.q(x, y), 1.8 * self.s().max(0.8), c);
    }
    fn poly(&self, pts: &[(f32, f32)], fill: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|(x, y)| self.q(*x, *y)).collect();
        self.p
            .add(Shape::convex_polygon(v, fill, Stroke::new(1.2, self.ink)));
    }
    fn arc(&self, c: (f32, f32), rx: f32, ry: f32, a0: f32, a1: f32) -> Vec<(f32, f32)> {
        (0..=24)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / 24.0;
                (c.0 + rx * a.cos(), c.1 - ry * a.sin())
            })
            .collect()
    }
    fn arrow(&self, a: (f32, f32), b: (f32, f32), c: Color32) {
        self.line(&[a, b], 1.6, c);
        let (pa, pb) = (self.q(a.0, a.1), self.q(b.0, b.1));
        let d = (pb - pa).normalized();
        let n = vec2(-d.y, d.x);
        let h = 3.6 * self.s();
        self.p.add(Shape::convex_polygon(
            vec![
                pb + d * 1.0,
                pb - d * h + n * h * 0.6,
                pb - d * h - n * h * 0.6,
            ],
            c,
            Stroke::NONE,
        ));
    }
    fn circle(&self, cx: f32, cy: f32, r: f32) {
        self.ink(&self.arc((cx, cy), r, r, 0.0, TAU));
    }
    fn disc(&self, cx: f32, cy: f32, r: f32, fill: Color32) {
        let c = self.q(cx, cy);
        self.p.circle_filled(c, r * self.s(), fill);
        self.p
            .circle_stroke(c, r * self.s(), Stroke::new(1.2, self.ink));
    }
    /// Points of the cubic Bézier through control points `c`.
    fn bez(&self, c: [(f32, f32); 4]) -> Vec<(f32, f32)> {
        (0..=24)
            .map(|i| {
                let t = i as f32 / 24.0;
                let u = 1.0 - t;
                let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                (
                    c.iter().zip(w).map(|(p, k)| p.0 * k).sum(),
                    c.iter().zip(w).map(|(p, k)| p.1 * k).sum(),
                )
            })
            .collect()
    }
    fn text(&self, x: f32, y: f32, t: &str, size: f32, c: Color32) {
        self.p.text(
            self.q(x, y),
            eframe::egui::Align2::CENTER_CENTER,
            t,
            eframe::egui::FontId::proportional(size * self.s()),
            c,
        );
    }
    fn dashed_rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.dashed((x0, y0), (x1, y0));
        self.dashed((x1, y0), (x1, y1));
        self.dashed((x1, y1), (x0, y1));
        self.dashed((x0, y1), (x0, y0));
    }
    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.ink(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]);
    }
    fn cube(&self, x0: f32, y0: f32, w: f32, d: f32, fill: Color32) {
        // Front face, top and side.
        self.poly(
            &[(x0, y0), (x0 + w, y0), (x0 + w, y0 + w), (x0, y0 + w)],
            fill,
        );
        self.poly(
            &[
                (x0, y0),
                (x0 + d, y0 - d),
                (x0 + w + d, y0 - d),
                (x0 + w, y0),
            ],
            Color32::from_rgb(214, 230, 246),
        );
        self.poly(
            &[
                (x0 + w, y0),
                (x0 + w + d, y0 - d),
                (x0 + w + d, y0 + w - d),
                (x0 + w, y0 + w),
            ],
            FILL2,
        );
    }
}

/// Paint `icon` inside `r`; `ink` is the stroke colour (from the theme).
pub fn paint(p: &Painter, r: Rect, icon: Icon, ink: Color32) {
    let name = match icon {
        Icon::Tool(k) => k.name(),
        Icon::Named(n) => n,
    };
    let pen = Pen { p, r, ink };
    draw(&pen, name);
}

/// Whether `name` has a drawing (not the initials fallback); remembered.
pub fn has_drawing(name: &str) -> bool {
    thread_local! {
        static KNOWN: std::cell::RefCell<std::collections::HashMap<String, bool>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }
    if let Some(b) = KNOWN.with(|k| k.borrow().get(name).copied()) {
        return b;
    }
    let b = probe(name);
    KNOWN.with(|k| k.borrow_mut().insert(name.to_string(), b));
    b
}

fn probe(name: &str) -> bool {
    let ctx = eframe::egui::Context::default();
    // Fonts are loaded by the first frame (the fallback draws text).
    let mut out = ctx.run_ui(Default::default(), |_| {});
    out.textures_delta.clear();
    let p = Painter::new(
        ctx,
        eframe::egui::LayerId::background(),
        Rect::from_min_size(pos2(0.0, 0.0), vec2(24.0, 24.0)),
    );
    let pen = Pen {
        p: &p,
        r: Rect::from_min_size(pos2(0.0, 0.0), vec2(24.0, 24.0)),
        ink: Color32::BLACK,
    };
    draw(&pen, name)
}

/// Draw `name`; false when it fell back to initials.
#[allow(clippy::too_many_lines)]
fn draw(g: &Pen<'_>, name: &str) -> bool {
    match name {
        "Point" => {
            g.thin(&[(6.0, 12.0), (18.0, 12.0)]);
            g.thin(&[(12.0, 6.0), (12.0, 18.0)]);
            g.solid_dot(12.0, 12.0, BLUE);
        }
        "Line" => {
            g.ink(&[(4.0, 19.0), (20.0, 5.0)]);
            g.dot(4.0, 19.0);
            g.dot(20.0, 5.0);
        }
        "Polyline" => {
            let pts = [(3.0, 19.0), (8.0, 6.0), (15.0, 17.0), (21.0, 5.0)];
            g.ink(&pts);
            for (x, y) in pts {
                g.dot(x, y);
            }
        }
        "Curve" => {
            let cps = [(3.0, 19.0), (6.0, 4.0), (16.0, 20.0), (21.0, 5.0)];
            g.dashed(cps[0], cps[1]);
            g.dashed(cps[1], cps[2]);
            g.dashed(cps[2], cps[3]);
            let b: Vec<(f32, f32)> = (0..=24)
                .map(|i| {
                    let t = i as f32 / 24.0;
                    let u = 1.0 - t;
                    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                    (
                        cps.iter().zip(w).map(|(c, k)| c.0 * k).sum(),
                        cps.iter().zip(w).map(|(c, k)| c.1 * k).sum(),
                    )
                })
                .collect();
            g.ink(&b);
            for (x, y) in cps {
                g.dot(x, y);
            }
        }
        "InterpCrv" => {
            let b: Vec<(f32, f32)> = (0..=24)
                .map(|i| {
                    let t = i as f32 / 24.0;
                    (3.0 + 18.0 * t, 12.0 - 7.0 * (t * TAU * 0.9).sin())
                })
                .collect();
            g.ink(&b);
            for k in [0usize, 7, 15, 24] {
                g.dot(b[k].0, b[k].1);
            }
        }
        "Rectangle" => {
            g.rect(3.0, 6.0, 21.0, 18.0);
            g.dot(3.0, 18.0);
            g.dot(21.0, 6.0);
        }
        "Circle" => {
            g.ink(&g.arc((12.0, 12.0), 8.5, 8.5, 0.0, TAU));
            g.solid_dot(12.0, 12.0, BLUE);
            g.thin(&[(12.0, 12.0), (20.5, 12.0)]);
        }
        "Arc" => {
            g.ink(&g.arc((12.0, 17.0), 9.0, 9.0, 0.0, PI));
            g.solid_dot(12.0, 17.0, BLUE);
            g.dot(21.0, 17.0);
            g.dot(3.0, 17.0);
        }
        "Ellipse" => {
            g.ink(&g.arc((12.0, 12.0), 10.0, 6.0, 0.0, TAU));
            g.thin(&[(2.0, 12.0), (22.0, 12.0)]);
            g.solid_dot(12.0, 12.0, BLUE);
        }
        "Polygon" => {
            let pts: Vec<(f32, f32)> = (0..=6)
                .map(|k| {
                    let a = PI / 2.0 + k as f32 * TAU / 6.0;
                    (12.0 + 9.0 * a.cos(), 12.0 - 9.0 * a.sin())
                })
                .collect();
            g.ink(&pts);
            g.solid_dot(12.0, 12.0, BLUE);
        }
        "Box" => g.cube(3.0, 9.0, 12.0, 6.0, FILL),
        "Cylinder" => {
            g.poly(&[(5.0, 7.0), (19.0, 7.0), (19.0, 18.0), (5.0, 18.0)], FILL);
            let top = g.arc((12.0, 7.0), 7.0, 2.6, 0.0, TAU);
            g.poly(&top, Color32::from_rgb(214, 230, 246));
            g.ink(&g.arc((12.0, 18.0), 7.0, 2.6, PI, TAU));
        }
        "Sphere" => {
            let c = g.q(12.0, 12.0);
            let rr = 9.0 * g.s();
            g.p.circle_filled(c, rr, FILL);
            g.p.circle_filled(
                c - vec2(2.5, 2.5) * g.s(),
                rr * 0.35,
                Color32::from_rgb(225, 238, 250),
            );
            g.p.circle_stroke(c, rr, Stroke::new(1.2, g.ink));
            g.thin(&g.arc((12.0, 12.0), 9.0, 3.0, PI, TAU));
        }
        "ExtrudeCrv" => {
            g.poly(
                &[(4.0, 18.0), (16.0, 18.0), (20.0, 14.0), (8.0, 14.0)],
                FILL,
            );
            g.dashed((4.0, 18.0), (4.0, 8.0));
            g.dashed((16.0, 18.0), (16.0, 8.0));
            g.arrow((12.0, 16.0), (12.0, 3.0), GREEN);
        }
        "ExtrudeSrf" => {
            g.cube(4.0, 10.0, 11.0, 5.0, FILL);
            g.arrow((19.0, 20.0), (19.0, 3.0), GREEN);
        }
        "Revolve" => {
            g.dashed((12.0, 2.0), (12.0, 22.0));
            g.ink(&[(12.0, 5.0), (17.0, 8.0), (16.0, 14.0), (19.0, 19.0)]);
            g.line(
                &g.arc((12.0, 19.0), 7.0, 2.4, PI * 1.1, TAU * 0.98),
                1.6,
                BLUE,
            );
        }
        "Sweep1" => {
            g.line(&g.arc((4.0, 22.0), 16.0, 16.0, 0.0, PI / 2.0), 1.6, BLUE);
            g.poly(&[(1.5, 6.5), (6.5, 6.5), (6.5, 11.5), (1.5, 11.5)], FILL);
            g.poly(
                &[(17.5, 19.5), (22.5, 19.5), (22.5, 24.0), (17.5, 24.0)],
                FILL,
            );
        }
        "PlanarSrf" => {
            g.poly(&[(3.0, 17.0), (15.0, 17.0), (21.0, 7.0), (9.0, 7.0)], FILL);
        }
        "Loft" => {
            g.poly(&[(3.0, 19.0), (9.0, 5.0), (21.0, 5.0), (15.0, 19.0)], FILL);
            g.line(&[(3.0, 19.0), (15.0, 19.0)], 2.0, BLUE);
            g.line(&[(9.0, 5.0), (21.0, 5.0)], 2.0, BLUE);
        }
        "Cap" => {
            g.poly(&[(5.0, 8.0), (19.0, 8.0), (19.0, 19.0), (5.0, 19.0)], FILL);
            g.poly(&g.arc((12.0, 8.0), 7.0, 2.6, 0.0, TAU), GOLD);
        }
        "Move" => {
            g.arrow((12.0, 12.0), (12.0, 2.5), g.ink);
            g.arrow((12.0, 12.0), (12.0, 21.5), g.ink);
            g.arrow((12.0, 12.0), (2.5, 12.0), g.ink);
            g.arrow((12.0, 12.0), (21.5, 12.0), g.ink);
        }
        "Copy" => {
            g.poly(
                &[(3.0, 10.0), (13.0, 10.0), (13.0, 20.0), (3.0, 20.0)],
                Color32::WHITE,
            );
            g.poly(
                &[(10.0, 4.0), (20.0, 4.0), (20.0, 14.0), (10.0, 14.0)],
                FILL,
            );
        }
        "Rotate" => {
            let a = g.arc((12.0, 12.0), 8.0, 8.0, 0.4, TAU * 0.8);
            g.ink(&a);
            let n = a.len();
            g.arrow(a[n - 3], a[n - 1], g.ink);
            g.solid_dot(12.0, 12.0, BLUE);
        }
        "Scale" => {
            g.rect(3.0, 13.0, 11.0, 21.0);
            g.poly(
                &[(3.0, 4.0), (20.0, 4.0), (20.0, 21.0), (3.0, 21.0)],
                Color32::TRANSPARENT,
            );
            g.arrow((9.0, 15.0), (18.0, 6.0), GREEN);
        }
        "Scale1D" => {
            g.rect(3.0, 8.0, 10.0, 16.0);
            g.dashed((10.0, 8.0), (21.0, 8.0));
            g.dashed((10.0, 16.0), (21.0, 16.0));
            g.arrow((10.0, 12.0), (21.0, 12.0), GREEN);
        }
        "Scale2D" => {
            g.rect(3.0, 11.0, 12.0, 20.0);
            g.dashed((3.0, 4.0), (20.0, 4.0));
            g.dashed((20.0, 4.0), (20.0, 20.0));
            g.arrow((10.0, 14.0), (19.0, 5.0), GREEN);
        }
        "Mirror" => {
            g.dashed((12.0, 2.0), (12.0, 22.0));
            g.poly(&[(10.0, 5.0), (10.0, 19.0), (3.0, 19.0)], FILL);
            g.poly(&[(14.0, 5.0), (21.0, 19.0), (14.0, 19.0)], Color32::WHITE);
        }
        "Orient" => {
            g.ink(&[(3.0, 20.0), (10.0, 20.0)]);
            g.dot(3.0, 20.0);
            g.dot(10.0, 20.0);
            g.line(&[(13.0, 11.0), (20.0, 4.0)], 1.6, BLUE);
            g.dot(13.0, 11.0);
            g.dot(20.0, 4.0);
            g.dashed((7.0, 18.0), (14.0, 9.0));
        }
        "ArrayLinear" => {
            for k in 0..3 {
                let x = 2.0 + k as f32 * 7.5;
                g.poly(
                    &[(x, 9.0), (x + 5.0, 9.0), (x + 5.0, 15.0), (x, 15.0)],
                    FILL,
                );
            }
        }
        "ArrayPolar" | "Array" => {
            g.thin(&g.arc((12.0, 12.0), 8.0, 8.0, 0.0, TAU));
            for k in 0..6 {
                let a = k as f32 * TAU / 6.0;
                let (x, y) = (12.0 + 8.0 * a.cos(), 12.0 - 8.0 * a.sin());
                g.poly(
                    &[
                        (x - 2.0, y - 2.0),
                        (x + 2.0, y - 2.0),
                        (x + 2.0, y + 2.0),
                        (x - 2.0, y + 2.0),
                    ],
                    FILL,
                );
            }
        }
        "Offset" => {
            g.thin(&g.arc((3.0, 21.0), 9.0, 9.0, 0.0, PI / 2.0));
            g.ink(&g.arc((3.0, 21.0), 17.0, 17.0, 0.0, PI / 2.0));
            g.arrow((10.0, 15.0), (14.0, 11.0), GREEN);
        }
        "Trim" => {
            g.ink(&[(12.0, 3.0), (12.0, 21.0)]);
            g.ink(&[(3.0, 12.0), (12.0, 12.0)]);
            g.dashed((12.0, 12.0), (21.0, 12.0));
            g.line(&[(16.0, 9.0), (20.0, 15.0)], 1.4, RED);
            g.line(&[(16.0, 15.0), (20.0, 9.0)], 1.4, RED);
        }
        "Split" => {
            g.ink(&[(12.0, 3.0), (12.0, 21.0)]);
            g.ink(&[(3.0, 12.0), (10.5, 12.0)]);
            g.line(&[(13.5, 12.0), (21.0, 12.0)], 1.6, BLUE);
        }
        "Extend" => {
            g.ink(&[(20.0, 3.0), (20.0, 21.0)]);
            g.ink(&[(3.0, 12.0), (10.0, 12.0)]);
            g.arrow((10.0, 12.0), (19.0, 12.0), GREEN);
        }
        "Fillet" => {
            g.ink(&[(4.0, 21.0), (4.0, 13.0)]);
            g.ink(&[(12.0, 5.0), (21.0, 5.0)]);
            g.line(&g.arc((12.0, 13.0), 8.0, 8.0, PI / 2.0, PI), 1.8, BLUE);
            g.dashed((4.0, 13.0), (4.0, 5.0));
            g.dashed((4.0, 5.0), (12.0, 5.0));
        }
        "Chamfer" => {
            g.ink(&[(4.0, 21.0), (4.0, 12.0)]);
            g.ink(&[(13.0, 5.0), (21.0, 5.0)]);
            g.line(&[(4.0, 12.0), (13.0, 5.0)], 1.8, BLUE);
            g.dashed((4.0, 12.0), (4.0, 5.0));
            g.dashed((4.0, 5.0), (13.0, 5.0));
        }
        "FilletCorners" => {
            let mut pts = Vec::new();
            for (c, a0) in [
                ((17.0, 7.0), 0.0),
                ((7.0, 7.0), PI / 2.0),
                ((7.0, 17.0), PI),
                ((17.0, 17.0), PI * 1.5),
            ] {
                pts.extend(g.arc(c, 4.0, 4.0, a0, a0 + PI / 2.0));
            }
            pts.push(pts[0]);
            g.ink(&pts);
        }
        "Join" => {
            g.ink(&[(3.0, 19.0), (12.0, 12.0)]);
            g.line(&[(12.0, 12.0), (21.0, 5.0)], 1.6, BLUE);
            g.solid_dot(12.0, 12.0, GOLD);
        }
        "Explode" => {
            for (dx, dy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                g.arrow(
                    (12.0 + dx * 2.5, 12.0 + dy * 2.5),
                    (12.0 + dx * 9.0, 12.0 + dy * 9.0),
                    g.ink,
                );
            }
            g.solid_dot(12.0, 12.0, GOLD);
        }
        "Flip" | "Dir" => {
            g.ink(&g.arc((12.0, 16.0), 9.0, 9.0, 0.0, PI));
            g.arrow((12.0, 7.0), (6.0, 7.0), BLUE);
            g.arrow((16.0, 15.0), (16.0, 22.0), RED);
        }
        "ProjectToCPlane" => {
            g.poly(
                &[(2.0, 19.0), (16.0, 19.0), (22.0, 14.0), (8.0, 14.0)],
                FILL,
            );
            g.ink(&g.arc((12.0, 8.0), 6.0, 3.0, 0.0, PI));
            g.arrow((12.0, 9.0), (12.0, 16.0), GREEN);
        }
        "Intersect" => {
            g.ink(&[(3.0, 19.0), (21.0, 5.0)]);
            g.ink(&g.arc((12.0, 12.0), 8.0, 8.0, 0.0, TAU));
            g.solid_dot(5.7, 16.9, RED);
            g.solid_dot(18.3, 7.1, RED);
        }
        "Distance" => {
            g.ink(&[(3.0, 16.0), (21.0, 16.0)]);
            g.thin(&[(3.0, 12.0), (3.0, 20.0)]);
            g.thin(&[(21.0, 12.0), (21.0, 20.0)]);
            g.arrow((12.0, 9.0), (4.0, 9.0), BLUE);
            g.arrow((12.0, 9.0), (20.0, 9.0), BLUE);
        }
        "Length" => {
            g.ink(&g.arc((12.0, 18.0), 9.0, 9.0, 0.0, PI));
            g.line(&[(3.0, 21.0), (21.0, 21.0)], 1.0, BLUE);
        }
        "Area" => {
            g.poly(&[(4.0, 6.0), (20.0, 6.0), (20.0, 18.0), (4.0, 18.0)], FILL);
            g.line(&[(7.0, 15.0), (12.0, 9.0), (17.0, 15.0)], 1.0, BLUE);
        }
        "Volume" => g.cube(4.0, 10.0, 11.0, 5.0, FILL2),
        "BoundingBox" => {
            g.dashed((3.0, 5.0), (21.0, 5.0));
            g.dashed((21.0, 5.0), (21.0, 19.0));
            g.dashed((21.0, 19.0), (3.0, 19.0));
            g.dashed((3.0, 19.0), (3.0, 5.0));
            g.p.circle_filled(g.q(12.0, 12.0), 5.0 * g.s(), FILL);
        }
        "What" | "Help" => {
            g.p.circle_stroke(g.q(12.0, 12.0), 9.0 * g.s(), Stroke::new(1.4, BLUE));
            g.p.text(
                g.q(12.0, 12.5),
                eframe::egui::Align2::CENTER_CENTER,
                if name == "Help" { "?" } else { "i" },
                eframe::egui::FontId::proportional(13.0 * g.s()),
                BLUE,
            );
        }
        "MatchProperties" => {
            g.poly(&[(3.0, 4.0), (10.0, 4.0), (10.0, 11.0), (3.0, 11.0)], GOLD);
            g.poly(
                &[(14.0, 13.0), (21.0, 13.0), (21.0, 20.0), (14.0, 20.0)],
                GOLD,
            );
            g.arrow((10.0, 9.0), (15.0, 14.0), g.ink);
        }
        "Hide" => {
            let eye = g.arc((12.0, 12.0), 9.0, 5.0, 0.0, TAU);
            g.ink(&eye);
            g.p.circle_filled(g.q(12.0, 12.0), 2.5 * g.s(), g.ink);
            g.line(&[(4.0, 20.0), (20.0, 4.0)], 1.8, RED);
        }
        "Show" | "Unisolate" => {
            g.ink(&g.arc((12.0, 12.0), 9.0, 5.0, 0.0, TAU));
            g.p.circle_filled(g.q(12.0, 12.0), 3.0 * g.s(), BLUE);
        }
        "Isolate" => {
            g.poly(&[(8.0, 8.0), (16.0, 8.0), (16.0, 16.0), (8.0, 16.0)], FILL);
            g.dashed((3.0, 3.0), (21.0, 3.0));
            g.dashed((3.0, 21.0), (21.0, 21.0));
        }
        "Lock" | "Unlock" => {
            g.poly(
                &[(6.0, 11.0), (18.0, 11.0), (18.0, 20.0), (6.0, 20.0)],
                GOLD,
            );
            let a = if name == "Lock" {
                g.arc((12.0, 11.0), 4.5, 5.0, 0.0, PI)
            } else {
                g.arc((16.5, 9.0), 4.5, 5.0, 0.2, PI)
            };
            g.ink(&a);
        }
        "Group" | "Ungroup" | "SelGroup" => {
            g.poly(&[(4.0, 6.0), (11.0, 6.0), (11.0, 13.0), (4.0, 13.0)], FILL);
            g.p.circle_filled(g.q(16.0, 15.0), 4.0 * g.s(), FILL);
            if name == "Ungroup" {
                g.line(&[(3.0, 21.0), (21.0, 3.0)], 1.4, RED);
            } else {
                g.dashed((2.0, 3.0), (22.0, 3.0));
                g.dashed((22.0, 3.0), (22.0, 21.0));
                g.dashed((22.0, 21.0), (2.0, 21.0));
                g.dashed((2.0, 21.0), (2.0, 3.0));
            }
        }
        "New" => {
            g.poly(
                &[
                    (5.0, 3.0),
                    (15.0, 3.0),
                    (19.0, 7.0),
                    (19.0, 21.0),
                    (5.0, 21.0),
                ],
                Color32::WHITE,
            );
            g.thin(&[(15.0, 3.0), (15.0, 7.0), (19.0, 7.0)]);
        }
        "Open" => {
            g.poly(
                &[
                    (3.0, 6.0),
                    (9.0, 6.0),
                    (11.0, 8.0),
                    (20.0, 8.0),
                    (20.0, 19.0),
                    (3.0, 19.0),
                ],
                GOLD,
            );
            g.poly(
                &[(3.0, 19.0), (6.0, 11.0), (22.0, 11.0), (20.0, 19.0)],
                Color32::from_rgb(250, 210, 90),
            );
        }
        "Save" => {
            g.poly(
                &[
                    (4.0, 4.0),
                    (18.0, 4.0),
                    (20.0, 6.0),
                    (20.0, 20.0),
                    (4.0, 20.0),
                ],
                BLUE,
            );
            g.poly(
                &[(8.0, 4.0), (16.0, 4.0), (16.0, 9.0), (8.0, 9.0)],
                Color32::WHITE,
            );
            g.poly(
                &[(7.0, 13.0), (17.0, 13.0), (17.0, 20.0), (7.0, 20.0)],
                Color32::WHITE,
            );
        }
        "Undo" | "Redo" => {
            let flip = if name == "Undo" { 1.0 } else { -1.0 };
            let a: Vec<(f32, f32)> = g
                .arc((12.0, 15.0), 7.0, 6.0, 0.0, PI)
                .into_iter()
                .map(|(x, y)| (12.0 + (x - 12.0) * flip, y))
                .collect();
            g.ink(&a);
            let e = *a.last().expect("pts");
            g.arrow((e.0, e.1 - 0.5), (e.0, e.1 + 4.5), g.ink);
        }
        "Delete" => {
            g.line(&[(5.0, 5.0), (19.0, 19.0)], 2.4, RED);
            g.line(&[(19.0, 5.0), (5.0, 19.0)], 2.4, RED);
        }
        "ZoomExtents" | "ZoomSelected" => {
            g.p.circle_filled(g.q(10.0, 10.0), 6.5 * g.s(), Color32::WHITE);
            g.p.circle_stroke(g.q(10.0, 10.0), 6.5 * g.s(), Stroke::new(1.6, g.ink));
            g.line(&[(15.0, 15.0), (21.0, 21.0)], 2.6, g.ink);
            if name == "ZoomSelected" {
                g.p.circle_filled(g.q(10.0, 10.0), 2.4 * g.s(), GOLD);
            } else {
                g.thin(&[(7.0, 10.0), (13.0, 10.0)]);
                g.thin(&[(10.0, 7.0), (10.0, 13.0)]);
            }
        }
        "SelAll" | "SelNone" | "Invert" | "SelLast" | "SelCrv" | "SelMesh" | "SelPt" => {
            g.dashed((3.0, 3.0), (21.0, 3.0));
            g.dashed((21.0, 3.0), (21.0, 21.0));
            g.dashed((21.0, 21.0), (3.0, 21.0));
            g.dashed((3.0, 21.0), (3.0, 3.0));
            match name {
                "SelAll" => {
                    g.p.circle_filled(g.q(9.0, 10.0), 3.0 * g.s(), GOLD);
                    g.p.circle_filled(g.q(15.0, 15.0), 3.0 * g.s(), GOLD);
                }
                "SelNone" => {
                    g.p.circle_stroke(g.q(9.0, 10.0), 3.0 * g.s(), Stroke::new(1.0, g.ink));
                    g.p.circle_stroke(g.q(15.0, 15.0), 3.0 * g.s(), Stroke::new(1.0, g.ink));
                }
                "Invert" => {
                    g.p.circle_filled(g.q(9.0, 10.0), 3.0 * g.s(), GOLD);
                    g.p.circle_stroke(g.q(15.0, 15.0), 3.0 * g.s(), Stroke::new(1.0, g.ink));
                }
                "SelLast" => g.arrow((7.0, 17.0), (17.0, 7.0), GOLD),
                "SelCrv" => g.line(
                    &[(6.0, 16.0), (10.0, 8.0), (14.0, 15.0), (18.0, 7.0)],
                    1.8,
                    GOLD,
                ),
                "SelMesh" => g.poly(&[(7.0, 8.0), (17.0, 8.0), (17.0, 16.0), (7.0, 16.0)], GOLD),
                _ => g.solid_dot(12.0, 12.0, GOLD),
            }
        }
        "Paste" | "CopyToClipboard" | "Cut" => {
            g.poly(
                &[(5.0, 5.0), (19.0, 5.0), (19.0, 21.0), (5.0, 21.0)],
                Color32::from_rgb(250, 230, 180),
            );
            g.poly(
                &[(9.0, 3.0), (15.0, 3.0), (15.0, 7.0), (9.0, 7.0)],
                Color32::WHITE,
            );
            match name {
                "Cut" => {
                    g.line(&[(8.0, 11.0), (16.0, 18.0)], 1.4, RED);
                    g.line(&[(16.0, 11.0), (8.0, 18.0)], 1.4, RED);
                }
                "Paste" => g.arrow((12.0, 10.0), (12.0, 19.0), GREEN),
                _ => g.poly(
                    &[(8.0, 11.0), (16.0, 11.0), (16.0, 18.0), (8.0, 18.0)],
                    FILL,
                ),
            }
        }
        "Layer" => {
            for (k, c) in [
                (0.0, FILL),
                (4.0, GOLD),
                (8.0, Color32::from_rgb(170, 220, 170)),
            ] {
                g.poly(
                    &[
                        (3.0, 8.0 + k),
                        (13.0, 4.0 + k),
                        (21.0, 8.0 + k),
                        (11.0, 12.0 + k),
                    ],
                    c,
                );
            }
        }
        "Top" | "Front" | "Right" | "Perspective" => {
            g.cube(5.0, 10.0, 10.0, 5.0, Color32::WHITE);
            let face: [(f32, f32); 4] = match name {
                "Top" => [(5.0, 10.0), (10.0, 5.0), (20.0, 5.0), (15.0, 10.0)],
                "Front" => [(5.0, 10.0), (15.0, 10.0), (15.0, 20.0), (5.0, 20.0)],
                "Right" => [(15.0, 10.0), (20.0, 5.0), (20.0, 15.0), (15.0, 20.0)],
                _ => [(10.0, 5.0), (20.0, 5.0), (20.0, 15.0), (10.0, 15.0)],
            };
            if name == "Perspective" {
                g.dashed(face[0], face[1]);
                g.dashed(face[1], face[2]);
                g.dashed(face[2], face[3]);
                g.dashed(face[3], face[0]);
            } else {
                g.poly(&face, FILL2);
            }
        }
        "Back" | "Bottom" | "Left" => {
            // The hidden face of the cube, shaded and dashed like a hidden edge.
            g.cube(5.0, 10.0, 10.0, 5.0, Color32::WHITE);
            let face: [(f32, f32); 4] = match name {
                "Back" => [(10.0, 5.0), (20.0, 5.0), (20.0, 15.0), (10.0, 15.0)],
                "Bottom" => [(5.0, 20.0), (10.0, 15.0), (20.0, 15.0), (15.0, 20.0)],
                _ => [(5.0, 10.0), (10.0, 5.0), (10.0, 15.0), (5.0, 20.0)],
            };
            let pts: Vec<Pos2> = face.iter().map(|(x, y)| g.q(*x, *y)).collect();
            g.p.add(Shape::convex_polygon(
                pts,
                Color32::from_rgba_unmultiplied(132, 176, 222, 150),
                Stroke::NONE,
            ));
            for k in 0..4 {
                g.dashed(face[k], face[(k + 1) % 4]);
            }
        }
        "Ask" => {
            // AI: a large four-pointed spark with a small one, violet and gold.
            let star = |cx: f32, cy: f32, r: f32, inner: f32, fill: Color32| {
                let pts: Vec<Pos2> = (0..8)
                    .map(|i| {
                        let a = i as f32 * PI / 4.0 - PI / 2.0;
                        let rr = if i % 2 == 0 { r } else { inner };
                        g.q(cx + rr * a.cos(), cy + rr * a.sin())
                    })
                    .collect();
                // Concave: each tip is a triangle on the inner square.
                for k in (0..8).step_by(2) {
                    g.p.add(Shape::convex_polygon(
                        vec![pts[k], pts[(k + 1) % 8], pts[(k + 7) % 8]],
                        fill,
                        Stroke::NONE,
                    ));
                }
                g.p.add(Shape::convex_polygon(
                    vec![pts[1], pts[3], pts[5], pts[7]],
                    fill,
                    Stroke::NONE,
                ));
            };
            star(10.5, 13.5, 9.0, 2.6, VIOLET);
            star(19.0, 5.0, 4.0, 1.2, GOLD);
            g.p.circle_filled(g.q(4.5, 4.5), 1.4 * g.s(), GOLD);
        }
        "Wireframe" | "Shaded" | "Ghosted" | "XRay" => {
            let fill = match name {
                "Shaded" => FILL,
                "Ghosted" => Color32::from_rgba_unmultiplied(132, 176, 222, 90),
                _ => Color32::TRANSPARENT,
            };
            let c = g.q(12.0, 12.0);
            let rr = 8.5 * g.s();
            g.p.circle_filled(c, rr, fill);
            g.p.circle_stroke(c, rr, Stroke::new(1.2, g.ink));
            g.thin(&g.arc((12.0, 12.0), 8.5, 3.0, 0.0, TAU));
            if name != "Shaded" {
                g.thin(&g.arc((12.0, 12.0), 3.0, 8.5, 0.0, TAU));
            }
        }
        "Booleans" | "BooleanUnion" | "BooleanDifference" | "BooleanIntersection" => {
            g.p.circle_filled(g.q(9.0, 12.0), 6.5 * g.s(), FILL);
            g.p.circle_filled(g.q(15.0, 12.0), 6.5 * g.s(), FILL2);
            g.p.circle_stroke(g.q(9.0, 12.0), 6.5 * g.s(), Stroke::new(1.0, g.ink));
            g.p.circle_stroke(g.q(15.0, 12.0), 6.5 * g.s(), Stroke::new(1.0, g.ink));
        }
        _ if more(g, name) => {}
        _ => {
            // Fallback: initials in a rounded box.
            g.p.rect_stroke(
                g.r.shrink(g.r.width() * 0.15),
                3.0,
                Stroke::new(1.0, g.ink),
                eframe::egui::StrokeKind::Middle,
            );
            let initials: String = name.chars().filter(|c| c.is_uppercase()).take(2).collect();
            g.p.text(
                g.r.center(),
                eframe::egui::Align2::CENTER_CENTER,
                initials,
                eframe::egui::FontId::proportional(10.0 * g.s()),
                g.ink,
            );
            return false;
        }
    }
    true
}

/// The menu commands' icons (the toolbar ones are in [`draw`]); false when
/// `name` has none.
#[allow(clippy::too_many_lines)]
fn more(g: &Pen<'_>, name: &str) -> bool {
    let surface = |g: &Pen<'_>| {
        g.poly(&[(3.0, 18.0), (15.0, 18.0), (21.0, 8.0), (9.0, 8.0)], FILL);
    };
    let mesh = |g: &Pen<'_>, seam: Option<Color32>| {
        g.poly(&[(3.0, 19.0), (21.0, 19.0), (21.0, 5.0), (3.0, 5.0)], FILL);
        for x in [9.0, 15.0] {
            g.thin(&[(x, 5.0), (x, 19.0)]);
        }
        g.thin(&[(3.0, 12.0), (21.0, 12.0)]);
        g.thin(&[(3.0, 19.0), (9.0, 12.0), (15.0, 5.0)]);
        g.thin(&[(9.0, 19.0), (21.0, 5.0)]);
        if let Some(c) = seam {
            g.line(&[(12.0, 3.0), (12.0, 21.0)], 2.0, c);
        }
    };
    let block = |g: &Pen<'_>| {
        g.rect(3.0, 5.0, 19.0, 19.0);
        g.poly(&[(5.0, 8.0), (11.0, 8.0), (11.0, 14.0), (5.0, 14.0)], FILL);
        g.disc(14.5, 13.0, 3.0, FILL2);
        g.solid_dot(3.0, 19.0, GOLD);
    };
    let sel_frame = |g: &Pen<'_>| g.dashed_rect(2.0, 2.0, 22.0, 22.0);
    let dim = |g: &Pen<'_>, a: (f32, f32), b: (f32, f32)| {
        g.arrow(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0), a, BLUE);
        g.arrow(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0), b, BLUE);
    };
    match name {
        // Curves
        "Lines" => {
            g.ink(&[(3.0, 18.0), (10.0, 6.0)]);
            g.ink(&[(12.0, 20.0), (21.0, 12.0)]);
            g.ink(&[(13.0, 4.0), (21.0, 6.0)]);
            for (x, y) in [(3.0, 18.0), (10.0, 6.0), (12.0, 20.0), (21.0, 12.0)] {
                g.dot(x, y);
            }
        }
        "LineBothSides" => {
            g.ink(&[(3.0, 19.0), (21.0, 5.0)]);
            g.solid_dot(12.0, 12.0, BLUE);
            g.dot(3.0, 19.0);
            g.dot(21.0, 5.0);
        }
        "LineVertical" => {
            g.dashed((3.0, 20.0), (21.0, 20.0));
            g.ink(&[(12.0, 20.0), (12.0, 4.0)]);
            g.dot(12.0, 20.0);
            g.dot(12.0, 4.0);
        }
        "LinePerpendicular" => {
            g.ink(&g.arc((12.0, 26.0), 14.0, 10.0, 0.4, PI - 0.4));
            g.line(&[(12.0, 16.0), (12.0, 3.0)], 1.6, BLUE);
            g.thin(&[(12.0, 13.0), (15.0, 13.0), (15.0, 16.0)]);
            g.dot(12.0, 3.0);
        }
        "LineBisector" => {
            g.ink(&[(21.0, 20.0), (3.0, 20.0), (15.0, 4.0)]);
            g.line(&[(3.0, 20.0), (21.0, 11.0)], 1.6, BLUE);
            g.thin(&g.arc((3.0, 20.0), 7.0, 7.0, 0.0, 0.93));
        }
        "Arc3Pt" | "ArcDirection" => {
            g.ink(&g.arc((12.0, 17.0), 9.0, 9.0, 0.0, PI));
            g.dot(3.0, 17.0);
            g.dot(21.0, 17.0);
            if name == "Arc3Pt" {
                g.dot(12.0, 8.0);
            } else {
                g.arrow((3.0, 17.0), (3.0, 6.0), GREEN);
            }
        }
        "Circle2Pt" | "Circle3Pt" | "CircleTTR" => {
            g.circle(12.0, 12.0, 8.0);
            match name {
                "Circle2Pt" => {
                    g.thin(&[(4.0, 12.0), (20.0, 12.0)]);
                    g.dot(4.0, 12.0);
                    g.dot(20.0, 12.0);
                }
                "Circle3Pt" => {
                    g.dot(4.0, 12.0);
                    g.dot(17.6, 6.3);
                    g.dot(15.0, 19.4);
                }
                _ => {
                    g.line(&[(1.0, 20.0), (23.0, 20.0)], 1.6, BLUE);
                    g.line(&[(4.0, 1.0), (4.0, 23.0)], 1.6, BLUE);
                    g.solid_dot(12.0, 20.0, GOLD);
                    g.solid_dot(4.0, 12.0, GOLD);
                }
            }
        }
        "Rectangle3Pt" => {
            g.ink(&[
                (3.0, 15.0),
                (14.0, 4.0),
                (21.0, 11.0),
                (10.0, 22.0),
                (3.0, 15.0),
            ]);
            g.dot(3.0, 15.0);
            g.dot(14.0, 4.0);
            g.dot(21.0, 11.0);
        }
        "RectangleCenter" => {
            g.rect(3.0, 6.0, 21.0, 18.0);
            g.solid_dot(12.0, 12.0, BLUE);
            g.dashed((12.0, 12.0), (21.0, 6.0));
            g.dot(21.0, 6.0);
        }
        "RoundedRectangle" => {
            let mut pts = Vec::new();
            for (c, a0) in [
                ((17.0, 10.0), 0.0),
                ((7.0, 10.0), PI / 2.0),
                ((7.0, 14.0), PI),
                ((17.0, 14.0), PI * 1.5),
            ] {
                pts.extend(g.arc(c, 4.0, 4.0, a0, a0 + PI / 2.0));
            }
            pts.push(pts[0]);
            g.ink(&pts);
        }
        "Slot" => {
            let mut pts = g.arc((16.0, 12.0), 5.0, 5.0, -PI / 2.0, PI / 2.0);
            pts.extend(g.arc((8.0, 12.0), 5.0, 5.0, PI / 2.0, PI * 1.5));
            pts.push(pts[0]);
            g.ink(&pts);
            g.solid_dot(8.0, 12.0, BLUE);
            g.solid_dot(16.0, 12.0, BLUE);
        }
        "Helix" => {
            let pts: Vec<(f32, f32)> = (0..=60)
                .map(|i| {
                    let t = i as f32 / 60.0;
                    (
                        12.0 + 7.0 * (t * TAU * 3.0).cos(),
                        20.0 - 16.0 * t + 2.0 * (t * TAU * 3.0).sin(),
                    )
                })
                .collect();
            g.ink(&pts);
            g.dashed((12.0, 2.0), (12.0, 22.0));
        }
        "Spiral" => {
            let pts: Vec<(f32, f32)> = (0..=60)
                .map(|i| {
                    let t = i as f32 / 60.0;
                    let a = t * TAU * 2.5;
                    (
                        12.0 + (1.0 + 8.5 * t) * a.cos(),
                        12.0 - (1.0 + 8.5 * t) * a.sin(),
                    )
                })
                .collect();
            g.ink(&pts);
        }
        "Convert" | "SimplifyCrv" => {
            let c = g.bez([(3.0, 18.0), (7.0, 2.0), (17.0, 22.0), (21.0, 6.0)]);
            g.thin(&c);
            let poly: Vec<(f32, f32)> = c.iter().step_by(6).copied().collect();
            g.line(&poly, 1.6, BLUE);
            for (x, y) in poly {
                g.dot(x, y);
            }
        }
        "CloseCrv" => {
            g.ink(&g.arc((12.0, 12.0), 8.5, 8.5, 0.5, TAU - 0.5));
            g.line(&g.arc((12.0, 12.0), 8.5, 8.5, -0.5, 0.5), 2.0, BLUE);
        }
        "ExtractPt" | "Divide" | "DivideByLength" => {
            if name == "ExtractPt" {
                let c = g.bez([(3.0, 18.0), (7.0, 4.0), (17.0, 22.0), (21.0, 6.0)]);
                g.ink(&c);
                for k in [0, 8, 16, 24] {
                    g.solid_dot(c[k].0, c[k].1 - 0.5, GOLD);
                }
            } else {
                g.ink(&[(2.0, 14.0), (22.0, 14.0)]);
                for k in 0..5 {
                    g.dot(2.0 + k as f32 * 5.0, 14.0);
                }
                if name == "DivideByLength" {
                    dim(g, (2.0, 7.0), (7.0, 7.0));
                }
            }
        }
        "Rebuild" => {
            let c = g.bez([(3.0, 18.0), (6.0, 4.0), (16.0, 20.0), (21.0, 6.0)]);
            g.ink(&c);
            for k in (0..=24).step_by(4) {
                g.dot(c[k].0, c[k].1);
            }
        }
        "Smooth" => {
            g.thin(&[
                (2.0, 14.0),
                (5.0, 8.0),
                (7.0, 16.0),
                (10.0, 7.0),
                (13.0, 17.0),
                (16.0, 8.0),
                (19.0, 15.0),
                (22.0, 10.0),
            ]);
            g.line(
                &g.bez([(2.0, 13.0), (8.0, 10.0), (15.0, 14.0), (22.0, 11.0)]),
                1.8,
                BLUE,
            );
        }
        "BlendCrv" => {
            g.ink(&[(2.0, 19.0), (9.0, 19.0)]);
            g.ink(&[(15.0, 5.0), (22.0, 5.0)]);
            g.line(
                &g.bez([(9.0, 19.0), (15.0, 19.0), (9.0, 5.0), (15.0, 5.0)]),
                1.6,
                BLUE,
            );
        }
        "CurveBoolean" => {
            g.rect(3.0, 4.0, 15.0, 16.0);
            g.rect(9.0, 9.0, 21.0, 21.0);
            g.poly(&[(9.0, 9.0), (15.0, 9.0), (15.0, 16.0), (9.0, 16.0)], GOLD);
        }
        "Contour" | "Section" => {
            g.cube(3.0, 9.0, 12.0, 6.0, FILL);
            if name == "Contour" {
                for y in [7.0, 12.0, 17.0] {
                    g.line(
                        &[(3.0, y + 2.0), (15.0, y + 2.0), (21.0, y - 4.0)],
                        1.4,
                        RED,
                    );
                }
            } else {
                g.line(&[(9.0, 6.0), (9.0, 21.0)], 1.8, RED);
                g.line(&[(9.0, 6.0), (15.0, 1.0), (15.0, 15.0)], 1.4, RED);
            }
        }
        "Project" | "Pull" => {
            if name == "Project" {
                g.poly(
                    &[(2.0, 20.0), (16.0, 20.0), (22.0, 14.0), (8.0, 14.0)],
                    FILL,
                );
            } else {
                g.poly(&g.arc((12.0, 26.0), 11.0, 9.0, 0.0, PI), FILL);
            }
            g.ink(&g.arc((12.0, 8.0), 6.0, 3.0, 0.0, PI));
            g.arrow((7.0, 8.0), (7.0, 17.0), GREEN);
            g.arrow((17.0, 8.0), (17.0, 17.0), GREEN);
        }
        "DupEdge" | "DupFaceBorder" | "DupBorder" => {
            if name == "DupBorder" {
                surface(g);
                g.line(
                    &[
                        (3.0, 18.0),
                        (15.0, 18.0),
                        (21.0, 8.0),
                        (9.0, 8.0),
                        (3.0, 18.0),
                    ],
                    2.0,
                    BLUE,
                );
            } else {
                g.cube(3.0, 9.0, 12.0, 6.0, FILL);
                if name == "DupEdge" {
                    g.line(&[(15.0, 9.0), (21.0, 3.0)], 2.2, BLUE);
                } else {
                    g.line(
                        &[
                            (3.0, 9.0),
                            (15.0, 9.0),
                            (15.0, 21.0),
                            (3.0, 21.0),
                            (3.0, 9.0),
                        ],
                        2.0,
                        BLUE,
                    );
                }
            }
        }
        "ExtractIsocurve" => {
            surface(g);
            g.line(&[(6.0, 13.0), (18.0, 13.0)], 2.0, BLUE);
        }
        "ClosestPt" | "EvaluatePt" => {
            let c = g.bez([(2.0, 18.0), (8.0, 10.0), (16.0, 22.0), (22.0, 12.0)]);
            g.ink(&c);
            if name == "ClosestPt" {
                g.solid_dot(12.0, 4.0, BLUE);
                g.dashed((12.0, 4.0), (12.0, 16.0));
                g.solid_dot(12.0, 16.0, RED);
            } else {
                g.solid_dot(c[14].0, c[14].1, RED);
                g.text(8.0, 6.0, "xyz", 7.0, BLUE);
            }
        }
        "SetPt" => {
            g.dashed((2.0, 18.0), (22.0, 18.0));
            for (x, y) in [(5.0, 6.0), (12.0, 10.0), (19.0, 4.0)] {
                g.arrow((x, y), (x, 16.5), GREEN);
                g.dot(x, y);
            }
        }
        "Dot" => {
            g.poly(&[(9.0, 6.0), (21.0, 6.0), (21.0, 13.0), (9.0, 13.0)], GOLD);
            g.solid_dot(5.0, 18.0, g.ink);
            g.thin(&[(5.0, 18.0), (9.0, 13.0)]);
        }
        // Surfaces
        "Plane" => {
            g.poly(&[(3.0, 18.0), (15.0, 18.0), (21.0, 8.0), (9.0, 8.0)], FILL);
            g.dot(3.0, 18.0);
            g.dot(21.0, 8.0);
        }
        "SrfPt" => {
            let pts = [(3.0, 19.0), (17.0, 21.0), (21.0, 6.0), (7.0, 4.0)];
            g.poly(&pts, FILL);
            for (x, y) in pts {
                g.dot(x, y);
            }
        }
        "EdgeSrf" => {
            g.poly(&[(3.0, 19.0), (17.0, 21.0), (21.0, 6.0), (7.0, 4.0)], FILL);
            g.line(
                &g.bez([(3.0, 19.0), (8.0, 23.0), (13.0, 18.0), (17.0, 21.0)]),
                2.0,
                BLUE,
            );
            g.line(
                &g.bez([(7.0, 4.0), (12.0, 1.0), (16.0, 8.0), (21.0, 6.0)]),
                2.0,
                BLUE,
            );
            g.line(&[(3.0, 19.0), (7.0, 4.0)], 2.0, BLUE);
            g.line(&[(17.0, 21.0), (21.0, 6.0)], 2.0, BLUE);
        }
        "Sweep2" => {
            g.poly(&[(3.0, 19.0), (6.0, 4.0), (21.0, 7.0), (17.0, 21.0)], FILL);
            g.line(&[(3.0, 19.0), (6.0, 4.0)], 2.0, BLUE);
            g.line(&[(17.0, 21.0), (21.0, 7.0)], 2.0, BLUE);
            g.ink(&g.bez([(4.0, 12.0), (8.0, 8.0), (14.0, 18.0), (19.0, 14.0)]));
        }
        "Pipe" | "Tube" => {
            let c = g.bez([(3.0, 19.0), (6.0, 4.0), (16.0, 20.0), (21.0, 5.0)]);
            g.line(&c, 7.0 * g.s(), g.ink);
            g.line(&c, 5.0 * g.s(), FILL);
            if name == "Tube" {
                g.line(&c, 1.8 * g.s(), Color32::WHITE);
            } else {
                g.line(&c, 1.0, BLUE);
            }
        }
        "ExtrudeCrvAlongCrv" => {
            g.poly(&[(3.0, 21.0), (9.0, 21.0), (9.0, 15.0), (3.0, 15.0)], FILL);
            g.line(
                &g.bez([(6.0, 18.0), (8.0, 6.0), (16.0, 12.0), (21.0, 3.0)]),
                1.8,
                BLUE,
            );
            g.dot(21.0, 3.0);
        }
        "ExtrudeCrvTapered" => {
            g.poly(&[(3.0, 20.0), (21.0, 20.0), (17.0, 6.0), (7.0, 6.0)], FILL);
            g.dashed((3.0, 20.0), (3.0, 6.0));
            g.dashed((21.0, 20.0), (21.0, 6.0));
        }
        "ExtrudeCrvToPoint" => {
            g.poly(&[(3.0, 20.0), (21.0, 20.0), (12.0, 3.0)], FILL);
            g.dot(12.0, 3.0);
        }
        "OffsetSrf" => {
            g.poly(
                &[(2.0, 21.0), (14.0, 21.0), (20.0, 13.0), (8.0, 13.0)],
                FILL,
            );
            g.poly(
                &[(4.0, 12.0), (16.0, 12.0), (22.0, 4.0), (10.0, 4.0)],
                FILL2,
            );
            g.arrow((11.0, 17.0), (13.0, 9.0), GREEN);
        }
        "Untrim" | "ShrinkTrimmedSrf" => {
            surface(g);
            if name == "Untrim" {
                g.dashed_rect(9.0, 11.0, 14.0, 15.0);
                g.p.circle_filled(g.q(11.5, 13.0), 1.5 * g.s(), GREEN);
            } else {
                g.dashed((1.0, 21.0), (23.0, 21.0));
                g.dashed((7.0, 5.0), (23.0, 5.0));
                g.arrow((22.0, 3.0), (19.0, 8.0), GREEN);
            }
        }
        "MergeAllCoplanarFaces" => {
            g.poly(&[(3.0, 18.0), (12.0, 18.0), (15.0, 8.0), (9.0, 8.0)], FILL);
            g.poly(
                &[(12.0, 18.0), (18.0, 18.0), (21.0, 8.0), (15.0, 8.0)],
                FILL,
            );
            g.line(&[(12.0, 18.0), (15.0, 8.0)], 1.4, RED);
            g.dashed((12.0, 18.0), (15.0, 8.0));
        }
        // Solids
        "Cone" | "TCone" | "Pyramid" => {
            if name == "Pyramid" {
                g.poly(&[(3.0, 18.0), (13.0, 21.0), (12.0, 3.0)], FILL);
                g.poly(&[(13.0, 21.0), (21.0, 16.0), (12.0, 3.0)], FILL2);
            } else {
                let top = if name == "Cone" { 0.1 } else { 3.5 };
                g.poly(
                    &[
                        (4.0, 18.0),
                        (20.0, 18.0),
                        (12.0 + top, 4.0),
                        (12.0 - top, 4.0),
                    ],
                    FILL,
                );
                g.ink(&g.arc((12.0, 18.0), 8.0, 2.6, PI, TAU));
                if name == "TCone" {
                    g.poly(
                        &g.arc((12.0, 4.0), 3.5, 1.2, 0.0, TAU),
                        Color32::from_rgb(214, 230, 246),
                    );
                }
            }
        }
        "Ellipsoid" => {
            g.poly(&g.arc((12.0, 12.0), 10.0, 6.5, 0.0, TAU), FILL);
            g.thin(&g.arc((12.0, 12.0), 10.0, 2.5, PI, TAU));
            g.thin(&g.arc((12.0, 12.0), 3.0, 6.5, -PI / 2.0, PI / 2.0));
        }
        "Torus" => {
            g.poly(&g.arc((12.0, 12.0), 10.0, 6.5, 0.0, TAU), FILL);
            g.poly(&g.arc((12.0, 11.5), 4.0, 2.0, 0.0, TAU), Color32::WHITE);
        }
        "Slab" => g.cube(2.0, 14.0, 14.0, 6.0, FILL),
        "Shell" => {
            g.cube(3.0, 9.0, 12.0, 6.0, FILL);
            g.poly(
                &[(5.0, 8.0), (9.0, 4.0), (19.0, 4.0), (15.0, 8.0)],
                Color32::WHITE,
            );
        }
        "CreateSolid" => {
            g.cube(3.0, 9.0, 12.0, 6.0, FILL);
            g.line(&[(15.0, 18.0), (18.0, 21.0), (23.0, 13.0)], 2.2, GREEN);
        }
        "FilletEdge" | "ChamferEdge" => {
            g.poly(
                &[(3.0, 10.0), (14.0, 10.0), (14.0, 21.0), (3.0, 21.0)],
                FILL,
            );
            g.poly(
                &[(3.0, 10.0), (9.0, 4.0), (20.0, 4.0), (14.0, 10.0)],
                Color32::from_rgb(214, 230, 246),
            );
            g.poly(
                &[(14.0, 10.0), (20.0, 4.0), (20.0, 15.0), (14.0, 21.0)],
                FILL2,
            );
            if name == "FilletEdge" {
                g.line(&[(13.0, 11.0), (19.0, 5.0)], 3.0, BLUE);
            } else {
                g.line(&[(12.0, 10.0), (18.0, 4.0)], 1.6, BLUE);
                g.line(&[(14.0, 12.0), (20.0, 6.0)], 1.6, BLUE);
            }
        }
        "BooleanSplit" => {
            g.disc(9.0, 12.0, 6.5, FILL);
            g.disc(15.0, 12.0, 6.5, FILL2);
            g.line(&[(12.0, 3.0), (12.0, 21.0)], 1.8, RED);
        }
        "Make2D" => {
            g.cube(2.0, 7.0, 8.0, 4.0, FILL);
            g.arrow((12.0, 12.0), (15.0, 12.0), GREEN);
            g.rect(16.0, 8.0, 22.0, 14.0);
            g.rect(16.0, 15.0, 22.0, 21.0);
        }
        // Meshes
        "Mesh" => mesh(g, None),
        "Weld" => mesh(g, Some(GOLD)),
        "Unweld" => mesh(g, Some(RED)),
        "MeshRepair" => {
            mesh(g, None);
            g.line(&[(10.0, 15.0), (14.0, 19.0), (22.0, 9.0)], 2.4, GREEN);
        }
        "ReduceMesh" => {
            g.poly(&[(3.0, 19.0), (21.0, 19.0), (21.0, 5.0), (3.0, 5.0)], FILL);
            g.thin(&[(3.0, 19.0), (21.0, 5.0)]);
            g.arrow((17.0, 10.0), (17.0, 17.0), GREEN);
        }
        "MeshToNURB" => {
            g.poly(
                &[(2.0, 20.0), (10.0, 20.0), (10.0, 12.0), (2.0, 12.0)],
                FILL,
            );
            g.thin(&[(2.0, 20.0), (10.0, 12.0)]);
            g.thin(&[(6.0, 12.0), (6.0, 20.0)]);
            g.arrow((9.0, 9.0), (14.0, 6.0), GREEN);
            g.poly(&g.arc((18.0, 10.0), 5.0, 5.0, 0.0, PI), FILL2);
        }
        "UnifyMeshNormals" => {
            surface(g);
            for x in [7.0, 12.0, 17.0] {
                g.arrow((x, 13.0), (x, 3.0), RED);
            }
        }
        // Transforms
        "Mirror3Pt" => {
            g.dashed((4.0, 22.0), (20.0, 2.0));
            g.poly(&[(4.0, 4.0), (11.0, 4.0), (4.0, 13.0)], FILL);
            g.poly(&[(20.0, 20.0), (13.0, 20.0), (20.0, 11.0)], Color32::WHITE);
            g.dot(4.0, 22.0);
            g.dot(20.0, 2.0);
        }
        "Orient3Pt" => {
            g.poly(&[(2.0, 20.0), (10.0, 20.0), (2.0, 13.0)], FILL);
            g.poly(&[(13.0, 9.0), (21.0, 3.0), (20.0, 12.0)], FILL2);
            for (x, y) in [(13.0, 9.0), (21.0, 3.0), (20.0, 12.0)] {
                g.dot(x, y);
            }
            g.dashed((8.0, 17.0), (14.0, 10.0));
        }
        "Rotate3D" => {
            g.dashed((3.0, 21.0), (21.0, 3.0));
            let a = g.arc((12.0, 12.0), 9.0, 4.0, 0.3, TAU * 0.85);
            g.ink(&a);
            let n = a.len();
            g.arrow(a[n - 3], a[n - 1], g.ink);
        }
        "ScaleNU" => {
            g.rect(3.0, 12.0, 9.0, 21.0);
            g.arrow((9.0, 16.5), (21.0, 16.5), GREEN);
            g.arrow((6.0, 12.0), (6.0, 7.0), GREEN);
            g.arrow((5.0, 16.5), (5.0, 16.0), GREEN);
        }
        "Shear" | "Taper" => {
            g.dashed_rect(5.0, 6.0, 17.0, 20.0);
            let pts = if name == "Shear" {
                [(5.0, 20.0), (17.0, 20.0), (22.0, 6.0), (10.0, 6.0)]
            } else {
                [(5.0, 20.0), (17.0, 20.0), (14.0, 6.0), (8.0, 6.0)]
            };
            g.poly(&pts, FILL);
        }
        "Bend" => {
            g.dashed((3.0, 18.0), (21.0, 18.0));
            g.line(&g.arc((12.0, 26.0), 11.0, 18.0, 0.35, PI - 0.35), 3.0, BLUE);
        }
        "Twist" => {
            for k in 0..5 {
                let t = k as f32 / 4.0;
                let a = t * PI;
                let w = 7.0 * a.cos();
                g.line(
                    &[(12.0 - w, 3.0 + 18.0 * t), (12.0 + w, 3.0 + 18.0 * t)],
                    1.6,
                    BLUE,
                );
            }
            g.dashed((12.0, 2.0), (12.0, 22.0));
        }
        "Stretch" => {
            g.ink(&[(3.0, 12.0), (14.0, 12.0), (14.0, 6.0)]);
            g.dashed_rect(10.0, 3.0, 22.0, 15.0);
            g.arrow((16.0, 19.0), (22.0, 19.0), GREEN);
        }
        "ArrayCrv" => {
            let c = g.bez([(2.0, 20.0), (6.0, 4.0), (16.0, 22.0), (22.0, 6.0)]);
            g.dashed(c[0], c[12]);
            g.thin(&c);
            for k in [2, 12, 22] {
                let (x, y) = c[k];
                g.poly(
                    &[
                        (x - 2.2, y - 2.2),
                        (x + 2.2, y - 2.2),
                        (x + 2.2, y + 2.2),
                        (x - 2.2, y + 2.2),
                    ],
                    FILL,
                );
            }
        }
        "Distribute" => {
            for x in [3.0, 10.5, 18.0] {
                g.poly(
                    &[(x, 9.0), (x + 3.0, 9.0), (x + 3.0, 15.0), (x, 15.0)],
                    FILL,
                );
            }
            dim(g, (6.0, 19.0), (10.5, 19.0));
            dim(g, (13.5, 19.0), (18.0, 19.0));
        }
        // Dimensions and annotations
        "Dim" | "DimRotated" | "DimAligned" => {
            let (a, b, off) = match name {
                "DimAligned" => ((4.0, 20.0), (18.0, 6.0), (-3.0, -3.0)),
                "DimRotated" => ((4.0, 16.0), (20.0, 10.0), (0.0, -6.0)),
                _ => ((4.0, 18.0), (20.0, 18.0), (0.0, -8.0)),
            };
            let (a2, b2) = ((a.0 + off.0, a.1 + off.1), (b.0 + off.0, b.1 + off.1));
            g.thin(&[a, a2]);
            g.thin(&[b, b2]);
            dim(g, a2, b2);
            g.dot(a.0, a.1);
            g.dot(b.0, b.1);
        }
        "DimAngle" | "Angle" => {
            g.ink(&[(21.0, 20.0), (3.0, 20.0), (16.0, 5.0)]);
            let a = g.arc((3.0, 20.0), 12.0, 12.0, 0.0, 0.85);
            g.line(&a, 1.4, BLUE);
            if name == "DimAngle" {
                g.arrow(a[3], a[0], BLUE);
                g.arrow(a[21], a[24], BLUE);
            }
        }
        "DimRadius" | "DimDiameter" | "Radius" => {
            g.circle(12.0, 12.0, 8.5);
            if name == "DimDiameter" {
                dim(g, (4.0, 18.0), (20.0, 6.0));
            } else {
                g.arrow((12.0, 12.0), (18.0, 6.0), BLUE);
                g.solid_dot(12.0, 12.0, BLUE);
            }
            if name == "Radius" {
                g.text(6.0, 6.0, "R", 8.0, BLUE);
            }
        }
        "Leader" => {
            g.arrow((14.0, 8.0), (4.0, 19.0), g.ink);
            g.ink(&[(14.0, 8.0), (21.0, 8.0)]);
            g.thin(&[(15.0, 4.0), (21.0, 4.0)]);
        }
        "Text" | "EditText" => {
            g.text(10.0, 12.0, "A", 18.0, g.ink);
            if name == "EditText" {
                g.line(&[(14.0, 21.0), (22.0, 13.0)], 3.0, GOLD);
                g.line(&[(13.0, 22.0), (14.0, 21.0)], 3.0, g.ink);
            }
        }
        "Hatch" | "HatchSolid" => {
            let fill = if name == "HatchSolid" {
                FILL2
            } else {
                Color32::WHITE
            };
            g.poly(&[(4.0, 4.0), (20.0, 4.0), (20.0, 20.0), (4.0, 20.0)], fill);
            if name == "Hatch" {
                for (a, b) in [
                    ((4.0, 10.0), (10.0, 4.0)),
                    ((4.0, 16.0), (16.0, 4.0)),
                    ((6.0, 20.0), (20.0, 6.0)),
                    ((12.0, 20.0), (20.0, 12.0)),
                ] {
                    g.line(&[a, b], 1.0, BLUE);
                }
            }
        }
        // Blocks, planes, layers
        "Block" => block(g),
        "Insert" => {
            block(g);
            g.arrow((21.0, 2.0), (16.0, 8.0), GREEN);
        }
        "ExplodeBlock" => {
            block(g);
            g.arrow((19.0, 19.0), (23.0, 23.0), g.ink);
            g.arrow((19.0, 5.0), (23.0, 1.0), g.ink);
        }
        "CPlane" | "CPlaneOrigin" | "CPlaneFace" => {
            if name == "CPlaneFace" {
                g.cube(3.0, 11.0, 10.0, 5.0, Color32::WHITE);
                g.poly(&[(3.0, 11.0), (8.0, 6.0), (18.0, 6.0), (13.0, 11.0)], FILL2);
                g.line(&[(8.0, 9.0), (16.0, 9.0)], 1.6, RED);
                g.line(&[(8.0, 9.0), (11.0, 6.5)], 1.6, GREEN);
            } else {
                g.poly(&[(2.0, 19.0), (16.0, 19.0), (22.0, 9.0), (8.0, 9.0)], FILL);
                g.thin(&[(9.0, 19.0), (15.0, 9.0)]);
                g.thin(&[(5.0, 14.0), (19.0, 14.0)]);
                g.line(&[(6.0, 17.0), (14.0, 17.0)], 1.6, RED);
                g.line(&[(6.0, 17.0), (9.5, 11.0)], 1.6, GREEN);
                g.line(&[(6.0, 17.0), (6.0, 6.0)], 1.6, BLUE);
                if name == "CPlaneOrigin" {
                    g.solid_dot(6.0, 17.0, GOLD);
                }
            }
        }
        "ClippingPlane" => {
            g.poly(
                &[(2.0, 20.0), (14.0, 20.0), (22.0, 10.0), (10.0, 10.0)],
                Color32::from_rgba_unmultiplied(210, 50, 45, 60),
            );
            g.line(
                &[
                    (2.0, 20.0),
                    (14.0, 20.0),
                    (22.0, 10.0),
                    (10.0, 10.0),
                    (2.0, 20.0),
                ],
                1.4,
                RED,
            );
            g.arrow((12.0, 15.0), (12.0, 3.0), RED);
        }
        "ChangeToCurrentLayer" => {
            for (k, c) in [(0.0, FILL), (6.0, GOLD)] {
                g.poly(
                    &[
                        (2.0, 11.0 + k),
                        (10.0, 7.0 + k),
                        (17.0, 11.0 + k),
                        (9.0, 15.0 + k),
                    ],
                    c,
                );
            }
            g.arrow((20.0, 3.0), (20.0, 15.0), GREEN);
        }
        // Selection
        "SelSrf" | "SelPolysrf" | "SelBoundary" => {
            sel_frame(g);
            match name {
                "SelSrf" => g.poly(&[(6.0, 16.0), (14.0, 16.0), (18.0, 8.0), (10.0, 8.0)], GOLD),
                "SelPolysrf" => g.cube(6.0, 11.0, 8.0, 4.0, GOLD),
                _ => {
                    g.line(
                        &[
                            (5.0, 8.0),
                            (12.0, 5.0),
                            (19.0, 10.0),
                            (16.0, 19.0),
                            (7.0, 17.0),
                            (5.0, 8.0),
                        ],
                        1.6,
                        BLUE,
                    );
                    g.solid_dot(11.0, 12.0, GOLD);
                }
            }
        }
        // Analysis
        "AreaCentroid" | "VolumeCentroid" => {
            if name == "AreaCentroid" {
                g.poly(&[(4.0, 6.0), (20.0, 6.0), (20.0, 18.0), (4.0, 18.0)], FILL);
                g.thin(&[(4.0, 6.0), (20.0, 18.0)]);
                g.thin(&[(20.0, 6.0), (4.0, 18.0)]);
                g.solid_dot(12.0, 12.0, RED);
            } else {
                g.cube(3.0, 9.0, 12.0, 6.0, FILL2);
                g.solid_dot(12.0, 12.0, RED);
            }
        }
        "Check" => {
            g.line(&[(4.0, 13.0), (10.0, 19.0), (20.0, 5.0)], 3.0, GREEN);
        }
        "Silhouette" => {
            g.disc(12.0, 12.0, 8.5, FILL);
            g.line(&g.arc((12.0, 12.0), 8.5, 8.5, 0.0, TAU), 2.2, BLUE);
        }
        // Scripts
        "RunPythonScript" => {
            g.poly(
                &[
                    (4.0, 3.0),
                    (15.0, 3.0),
                    (19.0, 7.0),
                    (19.0, 21.0),
                    (4.0, 21.0),
                ],
                Color32::WHITE,
            );
            for (k, w) in [(0.0, 8.0), (3.0, 6.0), (6.0, 9.0)] {
                g.thin(&[(7.0, 8.0 + k), (7.0 + w, 8.0 + k)]);
            }
            g.p.add(Shape::convex_polygon(
                vec![g.q(12.0, 14.0), g.q(22.0, 18.5), g.q(12.0, 23.0)],
                GREEN,
                Stroke::NONE,
            ));
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Commands the menus run by name (besides tools and `seq::ALL`).
    const MENU_COMMANDS: &[&str] = &[
        "Check",
        "CreateSolid",
        "ExplodeBlock",
        "MergeAllCoplanarFaces",
        "Mesh",
        "MeshRepair",
        "MeshToNURB",
        "ShrinkTrimmedSrf",
        "Silhouette",
        "SimplifyCrv",
        "Smooth",
        "Untrim",
        "New",
        "Undo",
        "Redo",
        "SelAll",
        "SelNone",
        "Invert",
        "SelLast",
        "SelCrv",
        "SelMesh",
        "SelPt",
        "SelSrf",
        "SelPolysrf",
        "Show",
        "Unlock",
        "ClippingPlane",
        "RunPythonScript",
        "Ask",
    ];

    /// Every toolbar button, sidebar button and menu command has a drawn icon.
    #[test]
    fn every_button_has_an_icon() {
        let mut missing = Vec::new();
        let mut names: Vec<&str> = Vec::new();
        for tab in 0..crate::toolbar::TABS.len() {
            for it in crate::toolbar::tab_items_for(tab).into_iter().flatten() {
                names.push(match it.icon {
                    Icon::Tool(k) => k.name(),
                    Icon::Named(n) => n,
                });
            }
        }
        for g in crate::toolbar::SIDEBAR {
            names.extend(g.iter().map(|k| k.name()));
        }
        // Menu entries: every described command and every selection command.
        names.extend(crate::tools::seq::ALL.iter().map(|s| s.name));
        names.extend(crate::tools::SELECTION_COMMANDS);
        names.extend(MENU_COMMANDS);
        names.sort_unstable();
        names.dedup();
        for n in names {
            if !has_drawing(n) {
                missing.push(n);
            }
        }
        assert!(missing.is_empty(), "no icon for: {missing:?}");
    }
}
