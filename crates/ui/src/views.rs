//! The viewport area: 2×2 layout (or one maximized view), offscreen
//! rendering into textures, view tabs and the title drop-down menu.

use crate::menus::SET_VIEWS;
use crate::viewport::Viewport;
use crate::{perf, theme, Act, FormaApp};
use eframe::egui::{self, Color32, Key, PointerButton, Rect};
use eframe::egui_wgpu;
use forma_render::{grid_plane_for, DisplayMode};

impl FormaApp {
    fn render_viewports(&mut self, frame: &eframe::Frame, ppp: f32) {
        let Some(rs) = frame.wgpu_render_state() else {
            return;
        };
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        let visible: Vec<usize> = match self.maximized {
            Some(m) => vec![m],
            None => (0..self.viewports.len()).collect(),
        };
        for i in visible {
            let vp = &mut self.viewports[i];
            if !vp.rect.is_positive() {
                continue;
            }
            let px = (
                (vp.rect.width() * ppp).round().max(1.0) as u32,
                (vp.rect.height() * ppp).round().max(1.0) as u32,
            );
            if !(vp.dirty || px != vp.px || vp.texture.is_none()) {
                continue;
            }
            let _t = perf::span_min("render view", 1.0);
            let view = vp.view.get_or_insert_with(|| renderer.new_view(&rs.device));
            let tex = renderer.render(
                &rs.device,
                &rs.queue,
                view,
                px,
                &vp.camera,
                grid_plane_for(vp.kind),
                vp.mode,
            );
            let mut egui_renderer = rs.renderer.write();
            match vp.texture {
                Some(id) => egui_renderer.update_egui_texture_from_wgpu_texture(
                    &rs.device,
                    tex,
                    egui_wgpu::wgpu::FilterMode::Linear,
                    id,
                ),
                None => {
                    vp.texture = Some(egui_renderer.register_native_texture(
                        &rs.device,
                        tex,
                        egui_wgpu::wgpu::FilterMode::Linear,
                    ))
                }
            }
            vp.px = px;
            vp.dirty = false;
        }
    }

    /// Screen rectangle of a viewport's title (click: menu, double-click: maximize).
    pub(crate) fn title_rect(vp: &Viewport) -> Rect {
        let w = 12.0 + vp.name().len() as f32 * 7.6 + 16.0;
        Rect::from_min_size(
            vp.rect.left_top() + egui::vec2(4.0, 3.0),
            egui::vec2(w, 20.0),
        )
    }

    pub(crate) fn ui_viewports(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let outer = ui.available_rect_before_wrap();
        let tabs_h = 24.0;
        let full = Rect::from_min_max(outer.min, egui::pos2(outer.max.x, outer.max.y - tabs_h));
        let gap = 3.0;
        let rects: Vec<(usize, Rect)> = match self.maximized {
            Some(m) => vec![(m, full)],
            None => {
                let w = (full.width() - gap) / 2.0;
                let h = (full.height() - gap) / 2.0;
                let at = |c: f32, r: f32| {
                    Rect::from_min_size(
                        full.left_top() + egui::vec2(c * (w + gap), r * (h + gap)),
                        egui::vec2(w, h),
                    )
                };
                vec![
                    (0, at(0.0, 0.0)),
                    (1, at(1.0, 0.0)),
                    (2, at(0.0, 1.0)),
                    (3, at(1.0, 1.0)),
                ]
            }
        };
        for (i, r) in &rects {
            self.viewports[*i].rect = *r;
        }
        if self.pending_fit && self.renderer.is_some() && self.seen_version != u64::MAX {
            self.fit(None);
            self.pending_fit = false;
        }
        let mut any_hover = false;
        for (i, r) in rects.clone() {
            let resp = ui.interact(
                r,
                egui::Id::new(("viewport", i)),
                egui::Sense::click_and_drag(),
            );
            if resp.hovered() {
                any_hover = true;
            }
            let title = Self::title_rect(&self.viewports[i]);
            let on_title = resp
                .interact_pointer_pos()
                .is_some_and(|p| title.contains(p));
            if on_title && resp.double_clicked_by(PointerButton::Primary) {
                self.active = i;
                self.title_menu = None;
                self.toggle_maximize();
                continue;
            }
            if on_title
                && (resp.clicked_by(PointerButton::Primary)
                    || resp.clicked_by(PointerButton::Secondary))
            {
                self.active = i;
                self.title_menu = Some((i, title.left_bottom()));
                continue;
            }
            self.viewport_input(ui, i, &resp);
        }
        if !any_hover && self.drag.is_none() {
            self.hover = None;
        }
        self.render_viewports(frame, ui.ctx().pixels_per_point());
        let painter = ui.painter();
        painter.rect_filled(outer, 0.0, Color32::from_gray(150));
        for (i, r) in &rects {
            if let Some(id) = self.viewports[*i].texture {
                painter.image(
                    id,
                    *r,
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        }
        for (i, _) in &rects {
            self.draw_overlays(painter, *i);
        }
        self.ui_title_menu(ui);
        // Viewport tabs under the views.
        let tabs = Rect::from_min_max(egui::pos2(outer.min.x, full.max.y), outer.max);
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(tabs.shrink2(egui::vec2(4.0, 2.0)))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        child.painter().rect_filled(tabs, 0.0, theme::STRIP);
        for i in [1usize, 0, 2, 3] {
            let name = self.viewports[i].name();
            let selected = self.active == i;
            if child.selectable_label(selected, name).clicked() {
                self.active = i;
                if self.maximized.is_some() {
                    self.maximized = Some(i);
                }
                self.dirty_all();
            }
        }
        if self.viewports.iter().any(|v| v.dirty) {
            ui.ctx().request_repaint();
        }
    }

    /// Drop-down menu of a viewport title (Rhino's "Top ▾").
    fn ui_title_menu(&mut self, ui: &mut egui::Ui) {
        let Some((vi, at)) = self.title_menu else {
            return;
        };
        let mut close = false;
        let mut act: Option<Act> = None;
        let area = egui::Area::new(egui::Id::new("viewport-title-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(at + egui::vec2(0.0, 2.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_min_width(170.0);
                    ui.label(egui::RichText::new("Set View").small().weak());
                    for (label, cmd) in SET_VIEWS {
                        if ui.selectable_label(false, label).clicked() {
                            act = Some(Act::Submit(cmd));
                            close = true;
                        }
                    }
                    ui.separator();
                    ui.label(egui::RichText::new("Display Mode").small().weak());
                    let current = self.viewports[vi].mode;
                    for m in DisplayMode::ALL {
                        if ui.selectable_label(current == m, m.name()).clicked() {
                            act = Some(Act::Mode(m));
                            close = true;
                        }
                    }
                    ui.separator();
                    let label = if self.maximized.is_some() {
                        "Restore Viewport Layout"
                    } else {
                        "Maximize"
                    };
                    if ui.selectable_label(false, label).clicked() {
                        act = Some(Act::Maximize);
                        close = true;
                    }
                    if ui.selectable_label(false, "Zoom Extents").clicked() {
                        act = Some(Act::Submit("ze"));
                        close = true;
                    }
                    if ui.selectable_label(false, "Zoom Selected").clicked() {
                        act = Some(Act::Submit("zs"));
                        close = true;
                    }
                });
            });
        let title = Self::title_rect(&self.viewports[vi]);
        let press = ui.input(|i| {
            i.pointer
                .any_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten()
        });
        let clicked_elsewhere =
            press.is_some_and(|p| !area.response.rect.contains(p) && !title.contains(p));
        if close || clicked_elsewhere || ui.input(|i| i.key_pressed(Key::Escape)) {
            self.title_menu = None;
        }
        if let Some(a) = act {
            self.active = vi;
            self.act(ui.ctx(), a);
        }
    }
}
