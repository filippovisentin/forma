//! Toolbar icons drawn with the egui painter (no image assets to license).

use crate::tools::ToolKind;
use eframe::egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};
use std::f32::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Tool(ToolKind),
    New,
    Open,
    Save,
    Undo,
    Redo,
    Delete,
    ZoomExtents,
    SelectAll,
}

fn ellipse(c: Pos2, rx: f32, ry: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    (0..=32)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / 32.0;
            pos2(c.x + rx * a.cos(), c.y - ry * a.sin())
        })
        .collect()
}

fn arrow(p: &Painter, from: Pos2, to: Pos2, s: Stroke) {
    p.line_segment([from, to], s);
    let d = (to - from).normalized();
    let n = vec2(-d.y, d.x);
    let h = 4.0;
    p.line_segment([to, to - d * h + n * h * 0.7], s);
    p.line_segment([to, to - d * h - n * h * 0.7], s);
}

/// Paint `icon` inside `r` with colour `c`.
pub fn paint(p: &Painter, r: Rect, icon: Icon, c: Color32) {
    let s = Stroke::new(1.6, c);
    let thin = Stroke::new(1.0, c);
    let m = r.shrink(r.width() * 0.2);
    let (l, t, rr, b) = (m.left(), m.top(), m.right(), m.bottom());
    let cx = m.center();
    let dot = |q: Pos2| p.circle_filled(q, 2.0, c);
    use ToolKind as K;
    match icon {
        Icon::Tool(K::Line) => {
            p.line_segment([pos2(l, b), pos2(rr, t)], s);
            dot(pos2(l, b));
            dot(pos2(rr, t));
        }
        Icon::Tool(K::Polyline) => {
            let pts = vec![
                pos2(l, b),
                pos2(l + m.width() * 0.3, t + 3.0),
                pos2(l + m.width() * 0.65, b - 3.0),
                pos2(rr, t),
            ];
            for q in &pts {
                dot(*q);
            }
            p.add(Shape::line(pts, s));
        }
        Icon::Tool(K::Rectangle) => {
            p.rect_stroke(
                m.shrink2(vec2(0.0, m.height() * 0.15)),
                0.0,
                s,
                StrokeKind::Middle,
            );
        }
        Icon::Tool(K::Circle) => {
            p.circle_stroke(cx, m.width() * 0.48, s);
            dot(cx);
        }
        Icon::Tool(K::Arc) => {
            p.add(Shape::line(
                ellipse(
                    pos2(cx.x, b),
                    m.width() * 0.5,
                    m.height() * 0.9,
                    0.0,
                    TAU / 2.0,
                ),
                s,
            ));
            dot(pos2(cx.x, b));
        }
        Icon::Tool(K::Box) => {
            let d = m.width() * 0.3;
            let front = Rect::from_min_max(pos2(l, t + d), pos2(rr - d, b));
            p.rect_stroke(front, 0.0, s, StrokeKind::Middle);
            p.line_segment([front.left_top(), front.left_top() + vec2(d, -d)], s);
            p.line_segment([front.right_top(), front.right_top() + vec2(d, -d)], s);
            p.line_segment(
                [front.right_bottom(), front.right_bottom() + vec2(d, -d)],
                s,
            );
            p.line_segment(
                [
                    front.left_top() + vec2(d, -d),
                    front.right_top() + vec2(d, -d),
                ],
                s,
            );
            p.line_segment(
                [
                    front.right_top() + vec2(d, -d),
                    front.right_bottom() + vec2(d, -d),
                ],
                s,
            );
        }
        Icon::Tool(K::Cylinder) => {
            let rx = m.width() * 0.45;
            let ry = m.height() * 0.15;
            p.add(Shape::line(
                ellipse(pos2(cx.x, t + ry), rx, ry, 0.0, TAU),
                s,
            ));
            p.add(Shape::line(
                ellipse(pos2(cx.x, b - ry), rx, ry, TAU / 2.0, TAU),
                s,
            ));
            p.line_segment([pos2(cx.x - rx, t + ry), pos2(cx.x - rx, b - ry)], s);
            p.line_segment([pos2(cx.x + rx, t + ry), pos2(cx.x + rx, b - ry)], s);
        }
        Icon::Tool(K::Sphere) => {
            let rad = m.width() * 0.48;
            p.circle_stroke(cx, rad, s);
            p.add(Shape::line(
                ellipse(cx, rad, rad * 0.3, TAU / 2.0, TAU),
                thin,
            ));
            p.add(Shape::line(
                ellipse(cx, rad * 0.3, rad, -TAU / 4.0, TAU / 4.0),
                thin,
            ));
        }
        Icon::Tool(K::Extrude) => {
            let base = Rect::from_min_max(pos2(l, b - m.height() * 0.3), pos2(rr - 4.0, b));
            p.rect_stroke(base, 0.0, thin, StrokeKind::Middle);
            arrow(
                p,
                pos2(cx.x - 2.0, b - m.height() * 0.15),
                pos2(cx.x - 2.0, t),
                s,
            );
        }
        Icon::Tool(K::Move) => {
            arrow(p, cx, pos2(cx.x, t), s);
            arrow(p, cx, pos2(cx.x, b), s);
            arrow(p, cx, pos2(l, cx.y), s);
            arrow(p, cx, pos2(rr, cx.y), s);
        }
        Icon::Tool(K::Copy) => {
            let a = Rect::from_min_size(pos2(l, t + 5.0), vec2(m.width() * 0.6, m.height() * 0.6));
            p.rect_stroke(a, 1.0, thin, StrokeKind::Middle);
            p.rect_stroke(
                a.translate(vec2(m.width() * 0.35, -m.height() * 0.3)),
                1.0,
                s,
                StrokeKind::Middle,
            );
        }
        Icon::Tool(K::Rotate) => {
            let pts = ellipse(cx, m.width() * 0.45, m.height() * 0.45, 0.3, TAU * 0.8);
            let end = *pts.last().expect("pts");
            let prev = pts[pts.len() - 3];
            p.add(Shape::line(pts, s));
            arrow(p, prev, end + (end - prev) * 0.5, s);
        }
        Icon::Tool(K::Scale) => {
            p.rect_stroke(
                Rect::from_min_max(pos2(l, cx.y), pos2(cx.x, b)),
                0.0,
                thin,
                StrokeKind::Middle,
            );
            p.rect_stroke(m, 0.0, s, StrokeKind::Middle);
            arrow(p, pos2(cx.x - 2.0, cx.y + 2.0), pos2(rr - 2.0, t + 2.0), s);
        }
        Icon::Tool(K::Mirror) => {
            for k in 0..5 {
                let y0 = t + k as f32 * m.height() / 5.0;
                p.line_segment([pos2(cx.x, y0), pos2(cx.x, y0 + m.height() / 10.0)], thin);
            }
            p.add(Shape::convex_polygon(
                vec![
                    pos2(cx.x - 3.0, t + 3.0),
                    pos2(cx.x - 3.0, b - 3.0),
                    pos2(l, b - 3.0),
                ],
                Color32::TRANSPARENT,
                s,
            ));
            p.add(Shape::convex_polygon(
                vec![
                    pos2(cx.x + 3.0, t + 3.0),
                    pos2(rr, b - 3.0),
                    pos2(cx.x + 3.0, b - 3.0),
                ],
                Color32::TRANSPARENT,
                s,
            ));
        }
        Icon::Tool(K::Offset) => {
            let a = ellipse(
                pos2(l, b),
                m.width() * 0.55,
                m.height() * 0.55,
                0.0,
                TAU / 4.0,
            );
            let o = ellipse(pos2(l, b), m.width(), m.height(), 0.0, TAU / 4.0);
            p.add(Shape::line(a, thin));
            p.add(Shape::line(o, s));
        }
        Icon::Tool(K::Trim) => {
            p.line_segment([pos2(cx.x, t), pos2(cx.x, b)], s);
            p.line_segment([pos2(l, cx.y), pos2(cx.x, cx.y)], s);
            p.add(Shape::dashed_line(
                &[pos2(cx.x, cx.y), pos2(rr, cx.y)],
                thin,
                2.0,
                2.0,
            ));
            p.line_segment([pos2(rr - 5.0, cx.y - 4.0), pos2(rr, cx.y + 4.0)], thin);
            p.line_segment([pos2(rr - 5.0, cx.y + 4.0), pos2(rr, cx.y - 4.0)], thin);
        }
        Icon::Tool(K::Extend) => {
            p.line_segment([pos2(rr, t), pos2(rr, b)], s);
            p.line_segment([pos2(l, cx.y), pos2(cx.x - 2.0, cx.y)], s);
            arrow(p, pos2(cx.x - 2.0, cx.y), pos2(rr - 1.0, cx.y), thin);
        }
        Icon::Tool(K::Fillet) => {
            p.line_segment([pos2(l, b), pos2(l, cx.y)], s);
            p.line_segment([pos2(cx.x, t), pos2(rr, t)], s);
            p.add(Shape::line(
                ellipse(pos2(cx.x, cx.y), cx.x - l, cx.y - t, TAU / 4.0, TAU / 2.0),
                s,
            ));
        }
        Icon::Tool(K::FilletCorners) => {
            let r0 = m.shrink2(vec2(0.0, m.height() * 0.12));
            p.rect_stroke(r0, 5.0, s, StrokeKind::Middle);
        }
        Icon::Tool(K::Join) => {
            p.line_segment([pos2(l, b), pos2(cx.x, cx.y)], s);
            p.line_segment([pos2(cx.x, cx.y), pos2(rr, t)], s);
            p.circle_stroke(cx, 3.0, thin);
        }
        Icon::Tool(K::Explode) => {
            for (dx, dy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                arrow(
                    p,
                    cx + vec2(dx, dy) * 3.0,
                    cx + vec2(dx * m.width() * 0.48, dy * m.height() * 0.48),
                    thin,
                );
            }
        }
        Icon::Tool(K::ArrayLinear) => {
            for k in 0..3 {
                let x0 = l + k as f32 * m.width() / 3.0;
                p.rect_stroke(
                    Rect::from_min_size(pos2(x0, cx.y - 3.0), vec2(m.width() / 4.0, 6.0)),
                    0.0,
                    s,
                    StrokeKind::Middle,
                );
            }
        }
        Icon::Tool(K::ArrayPolar) => {
            p.circle_stroke(cx, m.width() * 0.4, thin);
            for k in 0..6 {
                let a = k as f32 * TAU / 6.0;
                p.circle_filled(cx + vec2(a.cos(), a.sin()) * m.width() * 0.4, 2.3, c);
            }
        }
        Icon::New => {
            p.rect_stroke(
                m.shrink2(vec2(m.width() * 0.15, 0.0)),
                1.0,
                s,
                StrokeKind::Middle,
            );
        }
        Icon::Open => {
            p.add(Shape::line(
                vec![
                    pos2(l, b),
                    pos2(l, t + 3.0),
                    pos2(cx.x - 2.0, t + 3.0),
                    pos2(cx.x + 1.0, t + 6.0),
                    pos2(rr, t + 6.0),
                    pos2(rr, b),
                    pos2(l, b),
                ],
                s,
            ));
        }
        Icon::Save => {
            p.rect_stroke(m, 2.0, s, StrokeKind::Middle);
            p.rect_stroke(
                Rect::from_min_max(pos2(l + 4.0, t), pos2(rr - 4.0, t + m.height() * 0.35)),
                0.0,
                thin,
                StrokeKind::Middle,
            );
            p.rect_filled(
                Rect::from_min_max(
                    pos2(l + 5.0, b - m.height() * 0.35),
                    pos2(rr - 5.0, b - 2.0),
                ),
                0.0,
                c,
            );
        }
        Icon::Undo | Icon::Redo => {
            let flip = if icon == Icon::Undo { 1.0 } else { -1.0 };
            let pts = ellipse(
                pos2(cx.x, cx.y + 4.0),
                m.width() * 0.4,
                m.height() * 0.35,
                0.0,
                TAU / 2.0,
            );
            let pts: Vec<Pos2> = pts
                .into_iter()
                .map(|q| pos2(cx.x + (q.x - cx.x) * flip, q.y))
                .collect();
            let end = *pts.last().expect("pts");
            p.add(Shape::line(pts, s));
            arrow(p, end + vec2(0.0, -4.0), end + vec2(0.0, 2.0), s);
        }
        Icon::Delete => {
            p.line_segment([m.left_top(), m.right_bottom()], Stroke::new(2.0, c));
            p.line_segment([m.right_top(), m.left_bottom()], Stroke::new(2.0, c));
        }
        Icon::ZoomExtents => {
            p.circle_stroke(cx - vec2(2.0, 2.0), m.width() * 0.32, s);
            p.line_segment([cx + vec2(4.0, 4.0), m.right_bottom()], Stroke::new(2.2, c));
        }
        Icon::SelectAll => {
            for k in 0..4 {
                let y0 = t + k as f32 * m.height() / 4.0;
                p.line_segment([pos2(l, y0), pos2(l + 3.0, y0)], thin);
                p.line_segment([pos2(rr - 3.0, y0), pos2(rr, y0)], thin);
            }
            p.line_segment([pos2(l, t), pos2(rr, t)], thin);
            p.line_segment([pos2(l, b), pos2(rr, b)], thin);
            p.circle_filled(cx, 3.0, c);
        }
    }
}
