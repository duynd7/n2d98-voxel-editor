//! Editor visual theme — dark charcoal + teal accent (tool-like, not purple).

use eframe::egui::{self, Color32, Stroke, Visuals};

pub fn apply(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.window_fill = Color32::from_rgb(22, 24, 28);
    visuals.panel_fill = Color32::from_rgb(28, 31, 36);
    visuals.extreme_bg_color = Color32::from_rgb(16, 18, 21);
    visuals.faint_bg_color = Color32::from_rgb(36, 40, 46);
    visuals.code_bg_color = Color32::from_rgb(18, 20, 24);
    visuals.override_text_color = Some(Color32::from_rgb(220, 224, 230));
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(28, 31, 36);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 44, 52);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(52, 58, 68);
    visuals.widgets.active.bg_fill = Color32::from_rgb(45, 120, 130);
    visuals.widgets.open.bg_fill = Color32::from_rgb(40, 100, 110);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(56, 168, 176, 90);
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(180, 186, 196));
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(235, 238, 242));
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.window_stroke = Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 62));
    visuals.widgets.noninteractive.bg_stroke =
        Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 62));
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(58, 64, 74));
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.window_margin = egui::Margin::same(10);
    style.spacing.indent = 14.0;
    ctx.set_style(style);
}

pub const ACCENT: Color32 = Color32::from_rgb(64, 196, 180);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(40, 120, 112);
pub const DANGER: Color32 = Color32::from_rgb(220, 90, 90);
pub const MUTED: Color32 = Color32::from_rgb(130, 138, 150);
pub const PANEL_BG: Color32 = Color32::from_rgb(28, 31, 36);
