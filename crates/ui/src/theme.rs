//! Light look modelled on Rhino 8 for Windows: light grey chrome, white inputs,
//! blue selection, Segoe UI when available.

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke};

/// Panel background.
pub const PANEL: Color32 = Color32::from_rgb(240, 240, 240);
/// Slightly darker strip (toolbars, status bar).
pub const STRIP: Color32 = Color32::from_rgb(232, 232, 232);
pub const TEXT: Color32 = Color32::from_rgb(28, 28, 30);
pub const WEAK: Color32 = Color32::from_rgb(105, 105, 110);
pub const ACCENT: Color32 = Color32::from_rgb(0, 103, 192);
pub const ACCENT_BG: Color32 = Color32::from_rgb(204, 228, 247);
pub const BORDER: Color32 = Color32::from_rgb(200, 200, 204);

pub fn apply(ctx: &egui::Context) {
    load_system_font(ctx);
    let mut v = egui::Visuals::light();
    v.panel_fill = PANEL;
    v.window_fill = Color32::from_rgb(250, 250, 250);
    v.extreme_bg_color = Color32::WHITE;
    v.faint_bg_color = Color32::from_rgb(246, 246, 246);
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = ACCENT_BG;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(4);
    v.menu_corner_radius = CornerRadius::same(3);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = PANEL;
    w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    w.inactive.bg_fill = Color32::from_rgb(253, 253, 253);
    w.inactive.weak_bg_fill = Color32::from_rgb(236, 236, 236);
    w.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(208, 208, 212));
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.hovered.bg_fill = ACCENT_BG;
    w.hovered.weak_bg_fill = Color32::from_rgb(222, 236, 249);
    w.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(120, 174, 229));
    w.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    w.active.bg_fill = Color32::from_rgb(188, 220, 244);
    w.active.weak_bg_fill = Color32::from_rgb(188, 220, 244);
    w.active.bg_stroke = Stroke::new(1.0, ACCENT);
    w.active.fg_stroke = Stroke::new(1.0, TEXT);
    w.open.weak_bg_fill = Color32::from_rgb(222, 236, 249);
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
