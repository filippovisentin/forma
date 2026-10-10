//! What is painted over the rendered viewports: point objects, tool previews,
//! the cursor with its osnap tag and live measurement, SmartTrack lines, the
//! gumball, face push / pull, the selection window, axes and the view title.
//! Cursor markers and labels appear only in the viewport under the mouse;
//! previews (rubber bands, gumball) in every view, like Rhino.

use crate::tools::{Measure, Tool};
use crate::viewport::{to_p, Projector, Viewport};
use crate::{gumball, theme, FormaApp, Hover};
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke, StrokeKind};
use forma_geom::{Point3, Vec3, Xform};
use forma_render::DisplayMode;

const INK: Color32 = Color32::from_rgb(20, 20, 24);
const YELLOW: Color32 = Color32::from_rgb(255, 200, 0);

/// Look of a text label.
#[derive(Clone, Copy)]
enum Label {
    /// Pale yellow tag (osnap names, drag values), like Rhino's tooltips.
    Tag,
    /// White box (live measurements next to the cursor).
    Measure,
}

/// Text in a small box with its top-left corner at `at`.
fn label_box(p: &Painter, at: Pos2, text: String, size: f32, style: Label) {
    let galley = p.layout_no_wrap(text, egui::FontId::proportional(size), Color32::BLACK);
    let (fill, border, pad, radius) = match style {
        Label::Tag => (
            Color32::from_rgb(255, 255, 225),
            Color32::from_gray(90),
            egui::vec2(4.0, 2.0),
            2.0,
        ),
        Label::Measure => (
            Color32::from_rgba_unmultiplied(255, 255, 255, 230),
            Color32::from_gray(120),
            egui::vec2(5.0, 2.5),
            3.0,
        ),
    };
    let bg = Rect::from_min_size(at, galley.size() + pad * 2.0);
    p.rect_filled(bg, radius, fill);
    p.rect_stroke(bg, radius, Stroke::new(1.0, border), StrokeKind::Inside);
    p.galley(at + pad, galley, Color32::BLACK);
}

/// Live measurement next to a screen point (used by grip drags).
pub(crate) fn measure_tag(p: &Painter, at: Pos2, text: String) {
    label_box(p, at + egui::vec2(12.0, 8.0), text, 13.0, Label::Measure);
}

/// Osnap marker: a white square, with the snap name when `label` is given.
fn snap_marker(p: &Painter, s: Pos2, label: Option<&str>) {
    let r = Rect::from_center_size(s, egui::vec2(10.0, 10.0));
    p.rect_stroke(r, 0.0, Stroke::new(1.5, Color32::WHITE), StrokeKind::Middle);
    if let Some(l) = label {
        label_box(
            p,
            s + egui::vec2(10.0, -22.0),
            l.to_string(),
            12.0,
            Label::Tag,
        );
    }
}

/// Segment between two world points, when both are in front of the camera.
fn seg(p: &Painter, pr: &Projector, a: Point3, b: Point3, s: Stroke) {
    if let (Some(sa), Some(sb)) = (pr.to_screen(a), pr.to_screen(b)) {
        p.line_segment([sa, sb], s);
    }
}

impl FormaApp {
    pub(crate) fn draw_overlays(&self, painter: &egui::Painter, vi: usize) {
        let vp = &self.viewports[vi];
        let p = painter.with_clip_rect(vp.rect);
        let pr = vp.projector(self.origin());
        self.draw_point_objects(&p, &pr);
        self.draw_annotations(&p, &pr);
        self.draw_grips(&p, &pr);
        if let (Some(t), Some(h)) = (self.tool.as_ref(), self.hover.as_ref()) {
            self.draw_tool(&p, &pr, vi, t, h);
        }
        self.draw_face(&p, vp, &pr, vi);
        self.draw_gumball(&p, vp, &pr, vi);
        self.draw_window(&p, vi);
        self.draw_axes(&p, vp);
        self.draw_title(&p, vp, vi);
    }

    /// Point objects: small squares, like Rhino's point display.
    fn draw_point_objects(&self, p: &Painter, pr: &Projector) {
        let doc = self.engine.doc();
        for o in doc.objects() {
            let forma_doc::Geometry::Point(q) = &o.geometry else {
                continue;
            };
            if !doc.is_visible(o) {
                continue;
            }
            if let Some(sp) = pr.to_screen(*q) {
                let c = if self.engine.ctx.selection.contains(&o.id) {
                    YELLOW
                } else {
                    let [r, g, b] = doc.display_color(o);
                    Color32::from_rgb(r, g, b)
                };
                let r = Rect::from_center_size(sp, egui::vec2(6.0, 6.0));
                p.rect_filled(r, 0.0, c);
                p.rect_stroke(r, 0.0, Stroke::new(1.0, INK), StrokeKind::Outside);
            }
        }
    }

    /// Text of text objects and dimensions (drawn in their plane, scaled with
    /// the view) and text dots (screen-sized labels). Their lines are rendered
    /// by `forma-render`.
    fn draw_annotations(&self, p: &Painter, pr: &Projector) {
        let doc = self.engine.doc();
        for o in doc.objects() {
            if !o.geometry.is_annotation() || !doc.is_visible(o) {
                continue;
            }
            let Some(l) = o.geometry.annotation_label() else {
                continue;
            };
            let color = if self.engine.ctx.selection.contains(&o.id) {
                YELLOW
            } else {
                let [r, g, b] = doc.display_color(o);
                Color32::from_rgb(r, g, b)
            };
            let Some(sp) = pr.to_screen(l.position) else {
                continue;
            };
            if matches!(&o.geometry, forma_doc::Geometry::Text(t) if t.dot) {
                let galley =
                    p.layout_no_wrap(l.text, egui::FontId::proportional(12.0), Color32::WHITE);
                let r = Rect::from_center_size(sp, galley.size() + egui::vec2(8.0, 4.0));
                p.rect_filled(r, 3.0, color);
                p.rect_stroke(r, 3.0, Stroke::new(1.0, INK), StrokeKind::Outside);
                let text = if color == YELLOW { INK } else { Color32::WHITE };
                p.galley(r.min + egui::vec2(4.0, 2.0), galley, text);
                continue;
            }
            let (Some(sx), Some(sy)) = (
                pr.to_screen(l.position + l.x * l.height),
                pr.to_screen(l.position + l.y * l.height),
            ) else {
                continue;
            };
            // Screen size of the text height; nothing when the text is tiny or
            // its plane is seen edge-on (Rhino shows it as a line then).
            let (vx, vy) = (sx - sp, sy - sp);
            let h_px = vy.length();
            let area = (vx.x * vy.y - vx.y * vy.x).abs();
            if h_px < 3.0 || area < 0.2 * vx.length().max(h_px).powi(2) {
                continue;
            }
            let ux = (sx - sp).normalized();
            let ux = if ux.x.is_finite() && ux.length() > 0.5 {
                ux
            } else {
                egui::vec2(1.0, 0.0)
            };
            // Text upright on screen: never upside down.
            let ux = if ux.x < -1e-3 { -ux } else { ux };
            let angle = ux.y.atan2(ux.x);
            // Cap height ≈ 0.7 × font size.
            let size = (h_px / 0.7).min(400.0);
            let galley = p.layout_no_wrap(l.text, egui::FontId::proportional(size), color);
            let down = egui::vec2(-ux.y, ux.x);
            let (w, gh) = (galley.size().x, galley.size().y);
            let mut top_left = sp - down * (gh * 0.85);
            if l.centered {
                top_left -= ux * (w / 2.0);
            }
            p.add(egui::epaint::TextShape::new(top_left, galley, color).with_angle(angle));
        }
    }

    /// Tool preview everywhere; cursor, osnap tag, SmartTrack and the live
    /// measurement only in the view under the mouse.
    fn draw_tool(&self, p: &Painter, pr: &Projector, vi: usize, t: &Tool, h: &Hover) {
        let st = Stroke::new(1.3, INK);
        for [a, b] in t.preview(h.point) {
            seg(p, pr, a, b, st);
        }
        if h.viewport != vi {
            return;
        }
        // SmartTrack lines and points.
        let track = Stroke::new(1.0, Color32::WHITE);
        for [a, b] in &h.tracks {
            if let (Some(sa), Some(sb)) = (pr.to_screen(*a), pr.to_screen(*b)) {
                p.extend(egui::Shape::dashed_line(&[sa, sb], track, 4.0, 3.0));
            }
        }
        for q in &self.track_points {
            if let Some(sq) = pr.to_screen(*q) {
                p.line_segment(
                    [sq - egui::vec2(4.0, 0.0), sq + egui::vec2(4.0, 0.0)],
                    track,
                );
                p.line_segment(
                    [sq - egui::vec2(0.0, 4.0), sq + egui::vec2(0.0, 4.0)],
                    track,
                );
            }
        }
        let Some(s) = pr.to_screen(h.point) else {
            return;
        };
        // Live measurement next to the cursor (Rhino's dynamic readout).
        let text: Vec<String> = t
            .measure(h.point)
            .into_iter()
            .map(|m| match m {
                Measure::Len("", v) => self.fmt_len(v),
                Measure::Len(l, v) => format!("{l} {}", self.fmt_len(v)),
                Measure::Angle(a) => format!("∠ {a:.1}°"),
                Measure::Factor(f) => format!("× {f:.3}"),
            })
            .collect();
        if !text.is_empty() {
            label_box(
                p,
                s + egui::vec2(14.0, 10.0),
                text.join("   "),
                12.5,
                Label::Measure,
            );
        }
        if !t.selecting {
            p.circle_filled(s, 3.0, INK);
        }
        if let Some(k) = h.snap {
            snap_marker(p, s, Some(k.label()));
        }
    }

    /// The picked face, its push / pull preview and arrow.
    fn draw_face(&self, p: &Painter, vp: &Viewport, pr: &Projector, vi: usize) {
        let Some(f) = &self.face_sel else { return };
        for [a, b] in &f.outline {
            seg(p, pr, *a, *b, Stroke::new(3.0, YELLOW));
        }
        let drag = self.face_drag.as_ref();
        let dist = drag.map_or(0.0, |d| d.distance);
        let here = drag.is_some_and(|d| d.viewport == vi);
        if dist.abs() > 1e-9 {
            let v = f.normal * dist;
            let st = Stroke::new(1.3, INK);
            for [a, b] in &f.outline {
                seg(p, pr, *a + v, *b + v, st);
                seg(p, pr, *a, *a + v, st);
            }
            if let Some(s) = pr.to_screen(f.center + v).filter(|_| here) {
                let what = if dist > 0.0 { "Pull" } else { "Push" };
                let text = format!("{what} {}", self.fmt_len(dist.abs()));
                label_box(p, s + egui::vec2(12.0, -26.0), text, 13.0, Label::Tag);
            }
        }
        if let Some((q, k)) = drag.and_then(|d| d.snapped).filter(|_| here) {
            if let Some(sq) = pr.to_screen(q) {
                snap_marker(p, sq, Some(k.label()));
            }
        }
        if self.tool.is_none() {
            let moved = f.center + f.normal * dist;
            if let Some((base, tip)) = gumball::face_handle(vp, moved, f.normal, self.origin()) {
                gumball::draw_face_handle(p, base, tip, self.face_hot || self.face_typed);
            }
        }
    }

    /// The gumball, or the preview of a gumball drag.
    fn draw_gumball(&self, p: &Painter, vp: &Viewport, pr: &Projector, vi: usize) {
        let origin = self.origin();
        let Some(d) = &self.gumball_drag else {
            if self.tool.is_none() {
                if let Some(l) = self
                    .gumball_center()
                    .and_then(|c| gumball::layout(vp, c, origin))
                {
                    let hot = self
                        .gumball_hot
                        .filter(|(v, _)| *v == vi)
                        .map(|(_, h)| h)
                        .or(self.gumball_typed.map(|g| g.0));
                    gumball::draw(p, vp, &l, origin, hot);
                }
            }
            return;
        };
        let here = d.viewport == vi;
        if let Some((q, k)) = d.snapped.filter(|_| here) {
            if let Some(sq) = pr.to_screen(q) {
                snap_marker(p, sq, Some(k.label()));
            }
        }
        let Some(m) = d.motion else { return };
        let x: Xform = m.xform(d.center);
        let st = Stroke::new(1.2, INK);
        let extrude = matches!(m, gumball::Motion::Extrude(..));
        // Solids grow from one face: no moved copy of the whole object.
        let push_pull = extrude && !d.faces.is_empty();
        if !push_pull {
            for [a, b] in &d.skeleton {
                seg(p, pr, x.point(*a), x.point(*b), st);
            }
        }
        if let gumball::Motion::Extrude(h, ax) = m {
            let v = ax * h;
            for outline in &d.faces {
                for [a, b] in outline {
                    seg(p, pr, *a + v, *b + v, Stroke::new(1.6, YELLOW));
                    seg(p, pr, *a, *a + v, st);
                }
            }
        }
        if extrude && !push_pull {
            // Side edges of the extrusion.
            let step = (d.skeleton.len() / 24).max(1);
            for [a, _] in d.skeleton.iter().step_by(step) {
                seg(p, pr, *a, x.point(*a), st);
            }
        }
        if here {
            if let Some(s) = pr.to_screen(x.point(d.center)) {
                let text = match m {
                    gumball::Motion::Translate(v) => self.fmt_len(v.length()),
                    gumball::Motion::Rotate(a, _) => format!("{a:.1}°"),
                    gumball::Motion::Extrude(h, _) => format!("Extrude {}", self.fmt_len(h)),
                };
                label_box(p, s + egui::vec2(12.0, -26.0), text, 13.0, Label::Tag);
            }
        }
    }

    /// Window (solid, blue) or crossing (dashed, green) selection rectangle.
    fn draw_window(&self, p: &Painter, vi: usize) {
        let (Some(d), Some(h)) = (self.drag.as_ref(), self.hover.as_ref()) else {
            return;
        };
        if d.viewport != vi {
            return;
        }
        let r = Rect::from_two_pos(d.start, h.pos);
        let crossing = h.pos.x < d.start.x;
        let (fill, stroke) = if crossing {
            (
                Color32::from_rgba_unmultiplied(80, 200, 80, 30),
                Stroke::new(1.0, Color32::from_rgb(30, 110, 30)),
            )
        } else {
            (
                Color32::from_rgba_unmultiplied(80, 120, 220, 30),
                Stroke::new(1.0, Color32::from_rgb(30, 50, 140)),
            )
        };
        p.rect_filled(r, 0.0, fill);
        if crossing {
            for e in [
                [r.left_top(), r.right_top()],
                [r.right_top(), r.right_bottom()],
                [r.right_bottom(), r.left_bottom()],
                [r.left_bottom(), r.left_top()],
            ] {
                p.extend(egui::Shape::dashed_line(&e, stroke, 5.0, 4.0));
            }
        } else {
            p.rect_stroke(r, 0.0, stroke, StrokeKind::Inside);
        }
    }

    /// World axes icon, lower left (Rhino shows one in every view).
    fn draw_axes(&self, p: &Painter, vp: &Viewport) {
        let origin = self.origin();
        let pr = vp.projector(origin);
        let corner = vp.rect.left_bottom() + egui::vec2(28.0, -28.0);
        let t = to_p(vp.camera.target + origin);
        let Some(c0) = pr.to_screen(t) else { return };
        for (axis, color, label) in [
            (Vec3::X, Color32::from_rgb(200, 40, 40), "x"),
            (Vec3::Y, Color32::from_rgb(30, 140, 40), "y"),
            (Vec3::Z, Color32::from_rgb(40, 70, 200), "z"),
        ] {
            let Some(c1) = pr.to_screen(t + axis * (vp.camera.distance * 0.05)) else {
                continue;
            };
            let d = c1 - c0;
            if d.length() < 1e-3 {
                continue;
            }
            let len = 18.0 * (d.length() / (vp.rect.height() * 0.05 + 1e-3)).min(1.0);
            if len < 3.0 {
                continue;
            }
            let dir = d.normalized();
            let tip = corner + dir * len;
            p.line_segment([corner, tip], Stroke::new(1.8, color));
            p.text(
                tip + dir * 7.0,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(11.0),
                color,
            );
        }
    }

    /// Title with the drop-down arrow, display mode and frame, Rhino style.
    fn draw_title(&self, p: &Painter, vp: &Viewport, vi: usize) {
        let active = vi == self.active;
        let tr = Self::title_rect(vp);
        if active {
            p.rect_filled(tr, 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, 150));
        }
        p.text(
            tr.left_center() + egui::vec2(6.0, 0.0),
            egui::Align2::LEFT_CENTER,
            format!("{} ▾", vp.name()),
            egui::FontId::proportional(if active { 13.5 } else { 13.0 }),
            if active {
                Color32::BLACK
            } else {
                Color32::from_gray(45)
            },
        );
        if vp.mode != DisplayMode::Wireframe || active {
            p.text(
                vp.rect.right_top() + egui::vec2(-8.0, 6.0),
                egui::Align2::RIGHT_TOP,
                vp.mode.name(),
                egui::FontId::proportional(11.5),
                Color32::from_gray(55),
            );
        }
        let (w, c) = if active {
            (2.0, theme::ACCENT)
        } else {
            (1.0, Color32::from_gray(110))
        };
        p.rect_stroke(vp.rect, 0.0, Stroke::new(w, c), StrokeKind::Inside);
    }
}
