//! Forma desktop UI (egui). Thin by design: every modelling action goes through
//! `forma-engine` commands; this crate only shows the viewport, panels and the
//! command line.

use eframe::egui;
use eframe::egui_wgpu;
use forma_engine::Engine;
use forma_render::{Camera, Renderer, Scene, StandardView};

/// Start the desktop app, optionally opening a file.
pub fn run(open: Option<String>) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Forma")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([640.0, 400.0])
            .with_drag_and_drop(true),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "Forma",
        options,
        Box::new(move |cc| {
            let mut app = FormaApp::new(cc);
            if let Some(path) = open {
                app.run_command(&format!("Open {path}"));
            }
            Ok(Box::new(app))
        }),
    )
}

struct FormaApp {
    engine: Engine,
    renderer: Option<Renderer>,
    texture: Option<egui::TextureId>,
    camera: Camera,
    scene_triangles: usize,
    scene_min: forma_render::glam::DVec3,
    scene_max: forma_render::glam::DVec3,
    scene_dirty: bool,
    view_dirty: bool,
    fit_pending: bool,
    last_size: (u32, u32),
    command: String,
    log: Vec<(bool, String)>,
    title: String,
}

impl FormaApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let renderer = cc
            .wgpu_render_state
            .as_ref()
            .map(|rs| Renderer::new(&rs.device));
        let mut app = FormaApp {
            engine: Engine::new(),
            renderer,
            texture: None,
            camera: Camera::default(),
            scene_triangles: 0,
            scene_min: forma_render::glam::DVec3::splat(-500.0),
            scene_max: forma_render::glam::DVec3::splat(500.0),
            scene_dirty: true,
            view_dirty: true,
            fit_pending: true,
            last_size: (0, 0),
            command: String::new(),
            log: Vec::new(),
            title: String::new(),
        };
        app.log(
            true,
            "Welcome to Forma. Open a .3dm with Ctrl+O or drop it on the window. Type Help for commands.",
        );
        app
    }

    fn log(&mut self, ok: bool, msg: impl Into<String>) {
        self.log.push((ok, msg.into()));
        if self.log.len() > 200 {
            self.log.remove(0);
        }
    }

    fn run_command(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        let lower = line.to_lowercase();
        let is_view = matches!(
            lower.as_str(),
            "ze" | "zoom extents"
                | "zoomextents"
                | "top"
                | "front"
                | "right"
                | "perspective"
                | "persp"
        );
        if is_view {
            self.log(true, format!("Command: {line}"));
        }
        match lower.as_str() {
            "help" | "?" => {
                let list: Vec<String> = self
                    .engine
                    .command_list()
                    .into_iter()
                    .map(|(_, h)| h.to_string())
                    .collect();
                self.log(true, format!("Command: {line}"));
                for h in list {
                    self.log(true, format!("  {h}"));
                }
                self.log(
                    true,
                    "  ZE — zoom extents · Top/Front/Right/Perspective — standard views",
                );
                return;
            }
            "ze" | "zoom extents" | "zoomextents" => {
                self.fit_pending = true;
                return;
            }
            "top" => return self.set_view(StandardView::Top),
            "front" => return self.set_view(StandardView::Front),
            "right" => return self.set_view(StandardView::Right),
            "perspective" | "persp" => return self.set_view(StandardView::Perspective),
            _ => {}
        }
        let path_before = self.engine.doc().path.clone();
        match self.engine.run_line(line) {
            Ok(msg) => {
                self.log(true, format!("Command: {line}"));
                if !msg.is_empty() {
                    self.log(true, msg);
                }
                self.scene_dirty = true;
                if self.engine.doc().path != path_before {
                    self.fit_pending = true;
                }
            }
            Err(e) => {
                self.log(true, format!("Command: {line}"));
                self.log(false, format!("{e}"));
            }
        }
    }

    fn set_view(&mut self, v: StandardView) {
        let target = self.camera.target;
        self.camera = Camera::view(v);
        self.camera.target = target;
        self.fit_pending = true;
    }

    fn open_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("Rhino 3D model", &["3dm"])
            .pick_file()
        {
            self.run_command(&format!("Open {}", p.display()));
        }
    }

    fn ui_menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open…  (Ctrl+O)").clicked() {
                    ui.close();
                    self.open_dialog();
                }
                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("View", |ui| {
                for (label, v) in [
                    ("Perspective", StandardView::Perspective),
                    ("Top", StandardView::Top),
                    ("Front", StandardView::Front),
                    ("Right", StandardView::Right),
                ] {
                    if ui.button(label).clicked() {
                        ui.close();
                        self.set_view(v);
                    }
                }
                ui.separator();
                if ui.button("Zoom Extents  (ZE)").clicked() {
                    ui.close();
                    self.fit_pending = true;
                }
                if let Some(r) = &mut self.renderer {
                    if ui.checkbox(&mut r.show_grid, "Grid").changed() {
                        self.view_dirty = true;
                    }
                }
            });
            ui.separator();
            for (label, v) in [
                ("Perspective", StandardView::Perspective),
                ("Top", StandardView::Top),
                ("Front", StandardView::Front),
                ("Right", StandardView::Right),
            ] {
                if ui.small_button(label).clicked() {
                    self.set_view(v);
                }
            }
            if ui.small_button("Zoom Extents").clicked() {
                self.fit_pending = true;
            }
        });
    }

    fn ui_layers(&mut self, ui: &mut egui::Ui) {
        ui.heading("Layers");
        ui.separator();
        let mut counts = vec![0usize; self.engine.doc().layers.len()];
        for o in self.engine.doc().objects() {
            counts[o.layer.0] += 1;
        }
        let mut changed = false;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, layer) in self.engine.ctx.doc.layers.iter_mut().enumerate() {
                let depth = layer.name.matches("::").count();
                let short = layer
                    .name
                    .rsplit("::")
                    .next()
                    .unwrap_or(&layer.name)
                    .to_string();
                ui.horizontal(|ui| {
                    ui.add_space(depth as f32 * 14.0);
                    if ui.checkbox(&mut layer.visible, "").changed() {
                        changed = true;
                    }
                    let [r, g, b] = layer.color;
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 2.0, egui::Color32::from_rgb(r, g, b));
                    ui.label(short)
                        .on_hover_text(format!("{} — {} objects", layer.name, counts[i]));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.weak(counts[i].to_string());
                    });
                });
            }
        });
        if changed {
            self.scene_dirty = true;
        }
    }

    fn ui_command_line(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .max_height(84.0)
            .stick_to_bottom(true)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (ok, line) in &self.log {
                    if *ok {
                        ui.monospace(line);
                    } else {
                        ui.colored_label(ui.visuals().error_fg_color, line);
                    }
                }
            });
        ui.horizontal(|ui| {
            ui.monospace("Command:");
            let edit = egui::TextEdit::singleline(&mut self.command)
                .font(egui::TextStyle::Monospace)
                .hint_text("Line 0,0,0 @100,0 · Open file.3dm · ZE · Top · Help")
                .desired_width(f32::INFINITY);
            let r = ui.add(edit);
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let line = std::mem::take(&mut self.command);
                self.run_command(&line);
                r.request_focus();
            }
        });
    }

    fn ui_status(&self, ui: &mut egui::Ui) {
        let doc = self.engine.doc();
        ui.horizontal(|ui| {
            ui.weak(format!("Units: {}", doc.units.abbreviation()));
            ui.separator();
            ui.weak(format!("{} objects", doc.len()));
            ui.separator();
            ui.weak(format!("{} triangles", self.scene_triangles));
            ui.separator();
            ui.weak("Right-drag: orbit · Shift+right / middle-drag: pan · Wheel: zoom");
        });
    }

    fn ui_viewport(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let ppp = ui.ctx().pixels_per_point();
        let px = (
            (rect.width() * ppp).round().max(1.0) as u32,
            (rect.height() * ppp).round().max(1.0) as u32,
        );

        // Navigation (Rhino-like).
        let shift = ui.input(|i| i.modifiers.shift);
        let delta = response.drag_delta();
        if response.dragged_by(egui::PointerButton::Middle)
            || (shift && response.dragged_by(egui::PointerButton::Secondary))
        {
            self.camera.pan(
                delta.x as f64 * ppp as f64,
                delta.y as f64 * ppp as f64,
                px.1 as f64,
            );
            self.view_dirty = true;
        } else if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Primary)
        {
            self.camera.orbit(delta.x as f64, delta.y as f64);
            self.view_dirty = true;
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera.zoom((-scroll as f64 * 0.0025).exp());
                self.view_dirty = true;
            }
        }
        if response.double_clicked_by(egui::PointerButton::Middle) {
            self.fit_pending = true;
        }

        let Some(rs) = frame.wgpu_render_state() else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No GPU renderer available",
                egui::FontId::proportional(16.0),
                egui::Color32::WHITE,
            );
            return;
        };
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        if self.scene_dirty {
            let scene = Scene::from_document(self.engine.doc());
            self.scene_triangles = scene.triangle_count();
            if !scene.is_empty() {
                self.scene_min = scene.min;
                self.scene_max = scene.max;
            }
            // Keep the camera on the same model point if the scene origin moved.
            renderer.set_scene(&rs.device, &scene);
            self.scene_dirty = false;
            self.view_dirty = true;
        }
        if self.fit_pending {
            self.camera
                .fit(self.scene_min, self.scene_max, px.0 as f64 / px.1 as f64);
            self.fit_pending = false;
            self.view_dirty = true;
        }
        if self.view_dirty || px != self.last_size || self.texture.is_none() {
            let view = renderer.render(&rs.device, &rs.queue, px, &self.camera);
            let mut egui_renderer = rs.renderer.write();
            match self.texture {
                Some(id) => egui_renderer.update_egui_texture_from_wgpu_texture(
                    &rs.device,
                    view,
                    wgpu_filter(),
                    id,
                ),
                None => {
                    self.texture =
                        Some(egui_renderer.register_native_texture(&rs.device, view, wgpu_filter()))
                }
            }
            self.view_dirty = false;
            self.last_size = px;
        }
        if let Some(id) = self.texture {
            ui.painter().image(
                id,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        if self.engine.doc().is_empty() {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Open a Rhino .3dm file (Ctrl+O) or drag it here",
                egui::FontId::proportional(18.0),
                egui::Color32::from_gray(235),
            );
        }
    }
}

fn wgpu_filter() -> egui_wgpu::wgpu::FilterMode {
    egui_wgpu::wgpu::FilterMode::Linear
}

impl eframe::App for FormaApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Files dropped on the window.
        let dropped: Vec<String> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().display().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        });
        if let Some(p) = dropped.into_iter().next() {
            self.run_command(&format!("Open {p}"));
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.open_dialog();
        }

        let title = match &self.engine.doc().path {
            Some(p) => format!(
                "Forma — {}",
                std::path::Path::new(p)
                    .file_name()
                    .map_or(p.clone(), |n| n.to_string_lossy().into_owned())
            ),
            None => "Forma".to_string(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        egui::Panel::top("menu").show(ui, |ui| self.ui_menu(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.ui_status(ui));
        egui::Panel::bottom("command")
            .resizable(true)
            .show(ui, |ui| self.ui_command_line(ui));
        egui::Panel::left("layers")
            .resizable(true)
            .default_size(230.0)
            .show(ui, |ui| self.ui_layers(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.ui_viewport(ui, frame));
    }
}
