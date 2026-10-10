//! View → Capture View to Image (Rhino's ViewCaptureToFile): a PNG of the
//! active viewport as shown on screen, texts, dimensions and points included
//! (cursor, gumball and tool previews are left out).

use crate::{FormaApp, LogKind};
use eframe::egui;

/// A capture on its way: file, viewport, and whether the screenshot was asked.
pub(crate) struct Capture {
    path: String,
    viewport: usize,
    requested: bool,
}

/// Write RGBA8 pixels as a PNG file.
pub(crate) fn write_png(path: &str, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut wr| wr.write_image_data(rgba))
        .map_err(|e| format!("{path}: {e}"))
}

impl FormaApp {
    /// Ask where to save, then capture the active viewport on the next frame.
    pub(crate) fn capture_view(&mut self, path: Option<String>) {
        let path = match path {
            Some(p) => p,
            None => {
                let mut dialog = rfd::FileDialog::new()
                    .add_filter("PNG image", &["png"])
                    .set_file_name(format!("{}.png", self.viewports[self.active].name()));
                if let Some(dir) = self.start_dir() {
                    dialog = dialog.set_directory(dir);
                }
                let Some(p) = dialog.save_file() else {
                    return;
                };
                let mut p = p;
                if p.extension().is_none() {
                    p.set_extension("png");
                }
                p.display().to_string()
            }
        };
        self.capture = Some(Capture {
            path,
            viewport: self.active,
            requested: false,
        });
        self.dirty_all();
    }

    /// True while a capture is pending: overlays that do not belong in the
    /// picture are skipped.
    pub(crate) fn capturing(&self) -> bool {
        self.capture.is_some()
    }

    /// Request the screenshot, and save it when it arrives.
    pub(crate) fn poll_capture(&mut self, ctx: &egui::Context) {
        let Some(c) = self.capture.as_mut() else {
            return;
        };
        if !c.requested {
            c.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            ctx.request_repaint();
            return;
        }
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        let Some(image) = image else {
            ctx.request_repaint();
            return;
        };
        let c = self.capture.take().expect("pending");
        let rect = self.viewports[c.viewport].rect;
        let ppp = ctx.pixels_per_point();
        let full = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(image.size[0] as f32, image.size[1] as f32) / ppp,
        );
        let rect = rect.intersect(full).shrink(2.0);
        if !rect.is_positive() {
            self.log(LogKind::Error, "capture: the view is not visible");
            return;
        }
        let part = image.region(&rect, Some(ppp));
        let rgba: Vec<u8> = part.pixels.iter().flat_map(|p| p.to_array()).collect();
        let (w, h) = (part.size[0] as u32, part.size[1] as u32);
        match write_png(&c.path, w, h, &rgba) {
            Ok(()) => self.log(LogKind::Normal, format!("saved {} ({w} × {h} px)", c.path)),
            Err(e) => self.log(LogKind::Error, e),
        }
        self.dirty_all();
    }
}
