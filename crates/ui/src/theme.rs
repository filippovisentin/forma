//! Theme system with Light / Dark modes. Inspired by modern CAD interfaces.
//! Uses system font (Segoe UI / Inter / San Francisco) with scalable UI.

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke};

/// Theme mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
}

/// Theme colors for both modes
#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    // Primary surfaces
    pub panel: Color32,    // Main panel background
    pub strip: Color32,    // Toolbar/strip background
    pub window: Color32,   // Window/dialog background
    pub surface: Color32,  // Elevated surfaces (cards, menus)
    pub hover: Color32,    // Hover state
    pub active: Color32,   // Active/pressed state
    pub selected: Color32, // Selected items

    // Text
    pub text: Color32,           // Primary text
    pub text_weak: Color32,      // Secondary/disabled text
    pub text_on_accent: Color32, // Text on accent backgrounds

    // Accent (brand color)
    pub accent: Color32,        // Primary accent (blue)
    pub accent_bg: Color32,     // Accent background (subtle)
    pub accent_hover: Color32,  // Accent hover
    pub accent_active: Color32, // Accent pressed

    // Borders & dividers
    pub border: Color32,        // Default borders
    pub border_strong: Color32, // Strong borders (focus)

    // Semantic colors
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub info: Color32,

    // Viewport
    pub viewport_bg: Color32,
    pub grid_major: Color32,
    pub grid_minor: Color32,
    pub crosshair: Color32,
}

impl ThemeColors {
    /// Light theme (Rhino 8 inspired)
    pub fn light() -> Self {
        Self {
            panel: Color32::from_rgb(244, 244, 246),
            strip: Color32::from_rgb(232, 233, 236),
            window: Color32::from_rgb(250, 250, 252),
            surface: Color32::WHITE,
            hover: Color32::from_rgb(225, 237, 250),
            active: Color32::from_rgb(198, 220, 244),
            selected: Color32::from_rgb(204, 228, 247),

            text: Color32::from_rgb(20, 20, 22),
            text_weak: Color32::from_rgb(100, 100, 105),
            text_on_accent: Color32::WHITE,

            accent: Color32::from_rgb(0, 98, 192),
            accent_bg: Color32::from_rgb(204, 228, 247),
            accent_hover: Color32::from_rgb(0, 115, 210),
            accent_active: Color32::from_rgb(0, 82, 165),

            border: Color32::from_rgb(200, 200, 208),
            border_strong: Color32::from_rgb(0, 98, 192),

            success: Color32::from_rgb(24, 140, 60),
            warning: Color32::from_rgb(200, 130, 0),
            error: Color32::from_rgb(210, 50, 45),
            info: Color32::from_rgb(0, 98, 192),

            viewport_bg: Color32::from_rgb(145, 145, 155),
            grid_major: Color32::from_rgba_premultiplied(80, 80, 90, 180),
            grid_minor: Color32::from_rgba_premultiplied(100, 100, 110, 80),
            crosshair: Color32::from_rgb(255, 255, 255),
        }
    }

    /// Dark theme (modern CAD dark mode)
    pub fn dark() -> Self {
        Self {
            panel: Color32::from_rgb(28, 28, 30),
            strip: Color32::from_rgb(36, 36, 38),
            window: Color32::from_rgb(22, 22, 24),
            surface: Color32::from_rgb(38, 38, 42),
            hover: Color32::from_rgb(48, 52, 58),
            active: Color32::from_rgb(60, 70, 85),
            selected: Color32::from_rgb(10, 50, 90),

            text: Color32::from_rgb(235, 235, 240),
            text_weak: Color32::from_rgb(140, 140, 150),
            text_on_accent: Color32::WHITE,

            accent: Color32::from_rgb(70, 150, 230),
            accent_bg: Color32::from_rgb(20, 60, 110),
            accent_hover: Color32::from_rgb(90, 165, 240),
            accent_active: Color32::from_rgb(50, 130, 210),

            border: Color32::from_rgb(60, 60, 70),
            border_strong: Color32::from_rgb(70, 150, 230),

            success: Color32::from_rgb(48, 180, 90),
            warning: Color32::from_rgb(255, 170, 0),
            error: Color32::from_rgb(255, 80, 70),
            info: Color32::from_rgb(70, 150, 230),

            viewport_bg: Color32::from_rgb(40, 40, 45),
            grid_major: Color32::from_rgba_premultiplied(120, 120, 130, 200),
            grid_minor: Color32::from_rgba_premultiplied(90, 90, 100, 80),
            crosshair: Color32::from_rgb(200, 200, 210),
        }
    }

    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Self::light(),
            ThemeMode::Dark => Self::dark(),
        }
    }
}

/// Spacing scale
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Spacing {
    pub xs: f32,  // 2
    pub sm: f32,  // 4
    pub md: f32,  // 8
    pub lg: f32,  // 12
    pub xl: f32,  // 16
    pub xxl: f32, // 24
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            xs: 2.0,
            sm: 4.0,
            md: 8.0,
            lg: 12.0,
            xl: 16.0,
            xxl: 24.0,
        }
    }
}

/// Border radius scale
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Radius {
    pub none: CornerRadius,
    pub sm: CornerRadius,
    pub md: CornerRadius,
    pub lg: CornerRadius,
    pub full: CornerRadius,
}

impl Default for Radius {
    fn default() -> Self {
        Self {
            none: CornerRadius::ZERO,
            sm: CornerRadius::same(2),
            md: CornerRadius::same(4),
            lg: CornerRadius::same(8),
            full: CornerRadius::same(255),
        }
    }
}

/// Apply theme to egui context
pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    let colors = ThemeColors::for_mode(mode);
    let spacing = Spacing::default();
    let radius = Radius::default();

    load_system_font(ctx);

    let mut v = match mode {
        ThemeMode::Light => egui::Visuals::light(),
        ThemeMode::Dark => egui::Visuals::dark(),
    };

    // Panel & window
    v.panel_fill = colors.panel;
    v.window_fill = colors.window;
    v.extreme_bg_color = colors.surface;
    v.faint_bg_color = colors.strip;

    // Text
    v.override_text_color = Some(colors.text);
    v.hyperlink_color = colors.accent;

    // Selection
    v.selection.bg_fill = colors.selected;
    v.selection.stroke = Stroke::new(1.0, colors.accent);

    // Window
    v.window_stroke = Stroke::new(1.0, colors.border);
    v.window_corner_radius = radius.md;
    v.menu_corner_radius = radius.sm;
    v.popup_shadow = egui::epaint::Shadow {
        offset: [0, 2],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(80),
    };

    // Widgets
    let w = &mut v.widgets;

    // Non-interactive (labels, etc.)
    w.noninteractive.bg_fill = colors.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, colors.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, colors.text);
    w.noninteractive.corner_radius = radius.sm;

    // Inactive (default button state)
    w.inactive.bg_fill = colors.surface;
    w.inactive.weak_bg_fill = colors.strip;
    w.inactive.bg_stroke = Stroke::new(1.0, colors.border);
    w.inactive.fg_stroke = Stroke::new(1.0, colors.text);
    w.inactive.corner_radius = radius.sm;

    // Hovered
    w.hovered.bg_fill = colors.hover;
    w.hovered.weak_bg_fill = colors.hover;
    w.hovered.bg_stroke = Stroke::new(1.0, colors.accent);
    w.hovered.fg_stroke = Stroke::new(1.0, colors.text);
    w.hovered.corner_radius = radius.sm;

    // Active (pressed)
    w.active.bg_fill = colors.active;
    w.active.weak_bg_fill = colors.active;
    w.active.bg_stroke = Stroke::new(1.0, colors.accent);
    w.active.fg_stroke = Stroke::new(1.0, colors.text);
    w.active.corner_radius = radius.sm;

    // Open (dropdown open)
    w.open.weak_bg_fill = colors.hover;
    w.open.corner_radius = radius.sm;

    ctx.set_visuals(v);

    // Global style
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(spacing.md, spacing.sm);
        s.spacing.button_padding = egui::vec2(spacing.md, spacing.xs);
        s.spacing.menu_margin = egui::Margin::symmetric(spacing.sm as i8, spacing.sm as i8);
        s.spacing.combo_width = 180.0;
        s.spacing.text_edit_width = 180.0;
        s.spacing.indent = spacing.lg;
        s.spacing.window_margin = egui::Margin::symmetric(spacing.md as i8, spacing.md as i8);

        // Text styles
        s.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(20.0, FontFamily::Proportional),
        );
        s.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(13.5, FontFamily::Proportional),
        );
        s.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(13.0, FontFamily::Proportional),
        );
        s.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(12.5, FontFamily::Monospace),
        );
        s.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::new(11.0, FontFamily::Proportional),
        );

        // Animation
        s.animation_time = 0.15;
    });

    // Store theme mode in memory for persistence
    ctx.data_mut(|d| d.insert_persisted(egui::Id::new("theme_mode"), mode));
}

/// Load system UI font
fn load_system_font(ctx: &egui::Context) {
    let candidates = [
        r"C:\Windows\Fonts\segoeui.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
    ];
    let mono_candidates = [
        r"C:\Windows\Fonts\consola.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/System/Library/Fonts/Menlo.ttc",
    ];

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
    if let Some(bytes) = mono_candidates.iter().find_map(|p| std::fs::read(p).ok()) {
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

/// Get current theme mode from context
pub fn current_mode(ctx: &egui::Context) -> ThemeMode {
    ctx.data_mut(|d| d.get_persisted(egui::Id::new("theme_mode")))
        .unwrap_or_default()
}

/// Toggle theme mode
#[allow(dead_code)]
pub fn toggle_mode(ctx: &egui::Context) {
    let mode = current_mode(ctx);
    let new_mode = match mode {
        ThemeMode::Light => ThemeMode::Dark,
        ThemeMode::Dark => ThemeMode::Light,
    };
    apply(ctx, new_mode);
}
