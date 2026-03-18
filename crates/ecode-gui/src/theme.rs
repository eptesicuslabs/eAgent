//! Theme configuration — fonts, colors, and spacing for eCode.

use egui::{Color32, FontDefinitions, FontFamily, FontId, TextStyle, Visuals};

// ─── Spacing Constants ─────────────────────────────────────────────────────────
// 4-px base unit grid.
pub const SP_XS: f32 = 4.0;
pub const SP_SM: f32 = 8.0;
pub const SP_MD: f32 = 12.0;
pub const SP_LG: f32 = 16.0;
pub const SP_XL: f32 = 24.0;
pub const SP_XXL: f32 = 32.0;

// ─── Corner Radii ─────────────────────────────────────────────────────────────
pub const R_SM: u8 = 8;    // inner card elements, code blocks
pub const R_MD: u8 = 12;   // message bubbles, inputs
pub const R_LG: u8 = 16;   // surface cards
pub const R_XL: u8 = 20;   // elevated cards (settings sections)
pub const R_PILL: u8 = 255; // pills / chips

// ─── Semantic Colors ─────────────────────────────────────────────────────────
/// Central color palette. Use these constants everywhere; never hardcode RGB values
/// directly in panel files.
pub mod colors {
    use egui::Color32;

    // Surfaces — darkest → lightest
    /// Deepest background, panel fills.
    pub const SURFACE_0: Color32 = Color32::from_rgb(9, 9, 11);
    /// Card backgrounds.
    pub const SURFACE_1: Color32 = Color32::from_rgb(17, 17, 19);
    /// Elevated cards, inputs, hover backgrounds.
    pub const SURFACE_2: Color32 = Color32::from_rgb(24, 24, 27);
    /// Active toggle background, strong hover.
    pub const SURFACE_3: Color32 = Color32::from_rgb(36, 36, 41);

    // Borders
    /// Subtle panel separators.
    pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(28, 28, 32);
    /// Card borders, input borders.
    pub const BORDER_DEFAULT: Color32 = Color32::from_rgb(44, 44, 50);
    /// Focus rings, hover borders.
    pub const BORDER_STRONG: Color32 = Color32::from_rgb(70, 70, 79);

    // Text hierarchy
    /// Headlines, active labels.
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(245, 245, 244);
    /// Body text.
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(212, 212, 216);
    /// Captions, field labels.
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(161, 161, 170);
    /// Disabled, placeholder.
    pub const TEXT_DISABLED: Color32 = Color32::from_rgb(113, 113, 122);

    // Accent
    /// Sky blue — selection bars, primary action text.
    pub const ACCENT: Color32 = Color32::from_rgb(125, 211, 252);
    /// Accent at low opacity for selection fills.
    pub const ACCENT_DIM: Color32 = Color32::from_rgb(18, 44, 56);

    // Status — (text, background) pairs
    pub const STATUS_READY_TEXT: Color32 = Color32::from_rgb(125, 211, 252);
    pub const STATUS_READY_BG: Color32 = Color32::from_rgb(12, 37, 54);

    pub const STATUS_RUNNING_TEXT: Color32 = Color32::from_rgb(110, 231, 183);
    pub const STATUS_RUNNING_BG: Color32 = Color32::from_rgb(15, 52, 38);

    pub const STATUS_WAITING_TEXT: Color32 = Color32::from_rgb(253, 186, 116);
    pub const STATUS_WAITING_BG: Color32 = Color32::from_rgb(56, 35, 12);

    pub const STATUS_ERROR_TEXT: Color32 = Color32::from_rgb(248, 113, 113);
    pub const STATUS_ERROR_BG: Color32 = Color32::from_rgb(69, 25, 25);

    pub const STATUS_OFFLINE_TEXT: Color32 = Color32::from_rgb(161, 161, 170);
    pub const STATUS_OFFLINE_BG: Color32 = Color32::from_rgb(39, 39, 42);

    pub const STATUS_STARTING_TEXT: Color32 = Color32::from_rgb(250, 204, 21);
    pub const STATUS_STARTING_BG: Color32 = Color32::from_rgb(57, 46, 14);

    // Semantic action colors
    pub const ACTION_DESTRUCTIVE_FILL: Color32 = Color32::from_rgb(185, 28, 28);
    pub const ACTION_APPROVE_FILL: Color32 = Color32::from_rgb(21, 128, 61);
    pub const ACTION_PRIMARY_FILL: Color32 = Color32::from_rgb(245, 245, 244);
    pub const ACTION_PRIMARY_TEXT: Color32 = Color32::from_rgb(12, 12, 16);
}

// ─── Theme Entry Point ──────────────────────────────────────────────────────

/// Configure the eCode theme — fonts, visuals, and spacing.
pub fn configure_theme(ctx: &egui::Context) {
    configure_fonts(ctx);
    configure_visuals(ctx);
    configure_spacing(ctx);
}

fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_insert_with(|| vec!["Hack".to_string()]);

    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::new(17.0, FontFamily::Proportional));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(13.0, FontFamily::Monospace),
    );
    style
        .text_styles
        .insert(TextStyle::Button, FontId::new(13.0, FontFamily::Proportional));
    // Small → "Caption" usage: field labels, timestamps, muted details.
    style
        .text_styles
        .insert(TextStyle::Small, FontId::new(11.0, FontFamily::Proportional));
    ctx.set_style(style);
}

fn configure_visuals(ctx: &egui::Context) {
    use colors::*;

    let mut visuals = Visuals::dark();

    // Windows and panels
    visuals.window_corner_radius = egui::CornerRadius::same(R_XL);
    visuals.panel_fill = SURFACE_0;
    visuals.window_fill = SURFACE_1;
    visuals.extreme_bg_color = Color32::from_rgb(7, 7, 9);
    visuals.faint_bg_color = SURFACE_2;

    // Widget layers
    visuals.widgets.noninteractive.bg_fill = SURFACE_2;
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, BORDER_SUBTLE);
    visuals.widgets.inactive.bg_fill = SURFACE_2;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER_DEFAULT);
    visuals.widgets.hovered.bg_fill = SURFACE_3;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, BORDER_STRONG);
    visuals.widgets.active.bg_fill = ACTION_PRIMARY_FILL;
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACTION_PRIMARY_FILL);

    // Text
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_MUTED);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, ACTION_PRIMARY_TEXT);

    // Selection
    visuals.selection.bg_fill = ACCENT_DIM;
    visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;

    ctx.set_visuals(visuals);
}

fn configure_spacing(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(SP_SM, SP_XS);
    style.spacing.button_padding = egui::vec2(SP_MD, SP_SM - 2.0);
    style.spacing.window_margin = egui::Margin::same(SP_MD as i8);
    ctx.set_style(style);
}
