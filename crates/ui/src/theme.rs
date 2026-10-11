//! Two looks for the window chrome (menus, panels, command line, status bar),
//! switched with View ▸ Interface or the `Interface` command and kept in the
//! settings:
//! - Classic: light, modelled on Rhino 8 for Windows (light grey chrome, white
//!   inputs, blue selection).
//! - Graphite: dark chrome, warm grey panels and a gold accent.
//!
//! Graphite also darkens the viewports (`forma_render::set_dark_viewports`), where
//! black objects are drawn light grey.
//! Segoe UI is used when available.

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Classic,
    Graphite,
}

impl Look {
    #[cfg(test)]
    pub const ALL: [Look; 2] = [Look::Classic, Look::Graphite];

    pub fn name(self) -> &'static str {
        match self {
            Look::Classic => "Classic",
            Look::Graphite => "Graphite",
        }
    }

    /// `classic` / `graphite` (any case); also `light` / `dark`.
    pub fn parse(s: &str) -> Option<Look> {
        match s.trim().to_ascii_lowercase().as_str() {
            "classic" | "light" | "original" => Some(Look::Classic),
            "graphite" | "dark" | "new" => Some(Look::Graphite),
            _ => None,
        }
    }

    fn palette(self) -> &'static Palette {
        match self {
            Look::Classic => &CLASSIC,
            Look::Graphite => &GRAPHITE,
        }
    }
}

/// Colours of one look.
struct Palette {
    /// Panel background.
    panel: Color32,
    /// Slightly different strip (toolbars, status bar).
    strip: Color32,
    text: Color32,
    weak: Color32,
    accent: Color32,
    accent_bg: Color32,
    border: Color32,
    /// Text fields, the command history, the selected tab.
    input: Color32,
    /// Menu bar.
    menu: Color32,
    /// Between and around the viewports.
    gap: Color32,
    /// Hovered row or button.
    hover: Color32,
    hover_stroke: Color32,
    error: Color32,
    window: Color32,
    faint: Color32,
    button: Color32,
    button_weak: Color32,
    button_stroke: Color32,
    active: Color32,
}

const CLASSIC: Palette = Palette {
    panel: Color32::from_rgb(240, 240, 240),
    strip: Color32::from_rgb(232, 232, 232),
    text: Color32::from_rgb(28, 28, 30),
    weak: Color32::from_rgb(105, 105, 110),
    accent: Color32::from_rgb(0, 103, 192),
    accent_bg: Color32::from_rgb(204, 228, 247),
    border: Color32::from_rgb(200, 200, 204),
    input: Color32::WHITE,
    menu: Color32::WHITE,
    gap: Color32::from_gray(150),
    hover: Color32::from_rgb(229, 241, 251),
    hover_stroke: Color32::from_rgb(160, 200, 236),
    error: Color32::from_rgb(190, 30, 30),
    window: Color32::from_rgb(250, 250, 250),
    faint: Color32::from_rgb(246, 246, 246),
    button: Color32::from_rgb(253, 253, 253),
    button_weak: Color32::from_rgb(236, 236, 236),
    button_stroke: Color32::from_rgb(208, 208, 212),
    active: Color32::from_rgb(188, 220, 244),
};

const GRAPHITE: Palette = Palette {
    panel: Color32::from_rgb(32, 34, 37),
    strip: Color32::from_rgb(29, 31, 34),
    text: Color32::from_rgb(233, 231, 226),
    weak: Color32::from_rgb(163, 167, 173),
    accent: Color32::from_rgb(210, 168, 106),
    accent_bg: Color32::from_rgb(58, 52, 42),
    border: Color32::from_rgb(52, 56, 61),
    input: Color32::from_rgb(17, 18, 20),
    menu: Color32::from_rgb(29, 31, 34),
    gap: Color32::from_rgb(16, 17, 19),
    hover: Color32::from_rgb(44, 48, 54),
    hover_stroke: Color32::from_rgb(74, 80, 88),
    error: Color32::from_rgb(240, 120, 110),
    window: Color32::from_rgb(32, 34, 37),
    faint: Color32::from_rgb(38, 40, 44),
    button: Color32::from_rgb(42, 45, 49),
    button_weak: Color32::from_rgb(38, 41, 45),
    button_stroke: Color32::from_rgb(58, 63, 69),
    active: Color32::from_rgb(70, 62, 48),
};

/// The look in use (`true` = Graphite). Read by the colour functions below.
static GRAPHITE_ON: AtomicBool = AtomicBool::new(false);

pub fn look() -> Look {
    if GRAPHITE_ON.load(Ordering::Relaxed) {
        Look::Graphite
    } else {
        Look::Classic
    }
}

fn p() -> &'static Palette {
    look().palette()
}

pub fn panel() -> Color32 {
    p().panel
}
pub fn strip() -> Color32 {
    p().strip
}
pub fn text() -> Color32 {
    p().text
}
pub fn weak() -> Color32 {
    p().weak
}
pub fn accent() -> Color32 {
    p().accent
}
pub fn accent_bg() -> Color32 {
    p().accent_bg
}
pub fn border() -> Color32 {
    p().border
}
pub fn input() -> Color32 {
    p().input
}
pub fn menu() -> Color32 {
    p().menu
}
pub fn gap() -> Color32 {
    p().gap
}
pub fn hover() -> Color32 {
    p().hover
}
pub fn hover_stroke() -> Color32 {
    p().hover_stroke
}
pub fn error() -> Color32 {
    p().error
}

/// Fonts and the starting look; call once.
pub fn init(ctx: &egui::Context, look: Look) {
    load_system_font(ctx);
    set_look(ctx, look);
}

/// Switch the chrome to `look`.
pub fn set_look(ctx: &egui::Context, look: Look) {
    GRAPHITE_ON.store(look == Look::Graphite, Ordering::Relaxed);
    forma_render::set_dark_viewports(look == Look::Graphite);
    let c = look.palette();
    let mut v = if look == Look::Graphite {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = c.panel;
    v.window_fill = c.window;
    v.extreme_bg_color = c.input;
    v.faint_bg_color = c.faint;
    v.override_text_color = Some(c.text);
    v.selection.bg_fill = c.accent_bg;
    v.selection.stroke = Stroke::new(1.0, c.accent);
    v.hyperlink_color = c.accent;
    v.window_stroke = Stroke::new(1.0, c.border);
    v.window_corner_radius = CornerRadius::same(4);
    v.menu_corner_radius = CornerRadius::same(3);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = c.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, c.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, c.text);
    w.inactive.bg_fill = c.button;
    w.inactive.weak_bg_fill = c.button_weak;
    w.inactive.bg_stroke = Stroke::new(1.0, c.button_stroke);
    w.inactive.fg_stroke = Stroke::new(1.0, c.text);
    w.hovered.bg_fill = c.accent_bg;
    w.hovered.weak_bg_fill = c.hover;
    w.hovered.bg_stroke = Stroke::new(1.0, c.hover_stroke);
    w.hovered.fg_stroke = Stroke::new(1.0, c.text);
    w.active.bg_fill = c.active;
    w.active.weak_bg_fill = c.active;
    w.active.bg_stroke = Stroke::new(1.0, c.accent);
    w.active.fg_stroke = Stroke::new(1.0, c.text);
    w.open.weak_bg_fill = c.hover;
    for s in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        s.corner_radius = CornerRadius::same(2);
    }
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(6.0, 2.0);
        s.spacing.menu_margin = egui::Margin::same(4);
        s.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(13.5, FontFamily::Proportional),
        );
        s.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(13.5, FontFamily::Proportional),
        );
        s.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(13.0, FontFamily::Monospace),
        );
        s.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::new(11.0, FontFamily::Proportional),
        );
    });
}

/// Use the system UI font (Segoe UI on Windows) when it is installed. Read at
/// runtime from the user's own system, never bundled.
fn load_system_font(ctx: &egui::Context) {
    let candidates = [
        r"C:\Windows\Fonts\segoeui.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ];
    let mono = [r"C:\Windows\Fonts\consola.ttf"];
    let mut fonts = FontDefinitions::default();
    let mut changed = false;
    if let Some(bytes) = candidates.iter().find_map(|p| std::fs::read(p).ok()) {
        fonts
            .font_data
            .insert("system-ui".into(), FontData::from_owned(bytes).into());
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "system-ui".into());
        changed = true;
    }
    if let Some(bytes) = mono.iter().find_map(|p| std::fs::read(p).ok()) {
        fonts
            .font_data
            .insert("system-mono".into(), FontData::from_owned(bytes).into());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "system-mono".into());
        changed = true;
    }
    if changed {
        ctx.set_fonts(fonts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_parse_by_name() {
        assert_eq!(Look::parse("Graphite"), Some(Look::Graphite));
        assert_eq!(Look::parse(" dark "), Some(Look::Graphite));
        assert_eq!(Look::parse("CLASSIC"), Some(Look::Classic));
        assert_eq!(Look::parse("original"), Some(Look::Classic));
        assert_eq!(Look::parse("neon"), None);
        for l in Look::ALL {
            assert_eq!(Look::parse(l.name()), Some(l));
        }
    }
}
