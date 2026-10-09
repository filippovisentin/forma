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

#[allow(clippy::too_many_lines)]
fn draw(g: &Pen<'_>, name: &str) {
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
        }
    }
}
