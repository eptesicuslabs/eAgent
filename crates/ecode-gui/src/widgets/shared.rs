//! Shared UI widgets used across all panels.
//!
//! Import with `use crate::widgets::shared::*;`

use crate::theme::{R_MD, R_PILL, R_SM, R_XL, SP_LG, SP_SM, SP_XS, colors};

// ─── Status Chip ─────────────────────────────────────────────────────────────

/// Severity level for a status chip.
#[derive(Debug, Clone, Copy)]
pub enum StatusSeverity {
    Ready,
    Running,
    Waiting,
    Error,
    Offline,
    Starting,
}

/// Render a compact status pill with the appropriate semantic colors.
pub fn status_chip(ui: &mut egui::Ui, text: &str, severity: StatusSeverity) {
    let (text_color, fill) = match severity {
        StatusSeverity::Ready => (colors::STATUS_READY_TEXT, colors::STATUS_READY_BG),
        StatusSeverity::Running => (colors::STATUS_RUNNING_TEXT, colors::STATUS_RUNNING_BG),
        StatusSeverity::Waiting => (colors::STATUS_WAITING_TEXT, colors::STATUS_WAITING_BG),
        StatusSeverity::Error => (colors::STATUS_ERROR_TEXT, colors::STATUS_ERROR_BG),
        StatusSeverity::Offline => (colors::STATUS_OFFLINE_TEXT, colors::STATUS_OFFLINE_BG),
        StatusSeverity::Starting => (colors::STATUS_STARTING_TEXT, colors::STATUS_STARTING_BG),
    };
    pill(ui, text, text_color, fill);
}

/// Render a plain pill (non-semantic, caller specifies colors).
pub fn pill(ui: &mut egui::Ui, text: &str, text_color: egui::Color32, fill: egui::Color32) {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(R_PILL))
        .inner_margin(egui::Margin::symmetric(SP_SM as i8, (SP_XS + 2.0) as i8))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).small().strong().color(text_color));
        });
}

/// Render a plain badge pill (gray, for info-only tags).
pub fn badge(ui: &mut egui::Ui, text: &str) {
    pill(ui, text, colors::TEXT_SECONDARY, colors::SURFACE_2);
}

// ─── Segmented Control ────────────────────────────────────────────────────────

/// A compact segmented control / toggle group.  
/// `options` is a slice of `(label, value)`.  
/// `current` is the currently selected value (must implement `PartialEq + Copy`).  
/// Returns `true` if the selection changed.
pub fn segmented_control<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    id: egui::Id,
    options: &[(&str, T)],
    current: &mut T,
) -> bool {
    let mut changed = false;

    // Outer track frame
    egui::Frame::new()
        .fill(colors::SURFACE_0)
        .stroke(egui::Stroke::new(1.0, colors::BORDER_SUBTLE))
        .corner_radius(egui::CornerRadius::same(R_MD))
        .inner_margin(egui::Margin::same(SP_XS as i8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(SP_XS - 2.0, 0.0);
                for (i, (label, value)) in options.iter().enumerate() {
                    let is_active = current == value;
                    let btn = egui::Button::new(
                        egui::RichText::new(*label)
                            .size(12.0)
                            .strong()
                            .color(if is_active {
                                colors::TEXT_PRIMARY
                            } else {
                                colors::TEXT_MUTED
                            }),
                    )
                    .fill(if is_active {
                        colors::SURFACE_3
                    } else {
                        egui::Color32::TRANSPARENT
                    })
                    .stroke(if is_active {
                        egui::Stroke::new(1.0, colors::BORDER_DEFAULT)
                    } else {
                        egui::Stroke::NONE
                    })
                    .corner_radius(egui::CornerRadius::same(R_SM))
                    .min_size(egui::vec2(0.0, 26.0));

                    let resp = ui.add(btn).on_hover_cursor(egui::CursorIcon::PointingHand);
                    if resp.clicked() && !is_active {
                        *current = *value;
                        changed = true;
                    }

                    // Subtle vertical divider between segments (not after the last)
                    if i < options.len() - 1 && !is_active {
                        // only paint if neither this nor the next is active
                        let next_active = options
                            .get(i + 1)
                            .is_some_and(|(_, v)| current == v);
                        if !next_active {
                            let rect = ui.available_rect_before_wrap();
                            let x = rect.left();
                            let painter = ui.painter();
                            painter.line_segment(
                                [
                                    egui::pos2(x, rect.top() + SP_XS),
                                    egui::pos2(x, rect.bottom() - SP_XS),
                                ],
                                egui::Stroke::new(1.0, colors::BORDER_SUBTLE),
                            );
                        }
                    }

                    let _ = id; // id may be used in future for focus/keyboard nav
                }
            });
        });

    changed
}

// ─── Card Frames ─────────────────────────────────────────────────────────────

/// An elevated card frame — `surface_2` fill, default border, r=XL.
pub fn elevated_card(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(colors::SURFACE_2)
        .stroke(egui::Stroke::new(1.0, colors::BORDER_DEFAULT))
        .corner_radius(egui::CornerRadius::same(R_XL))
        .inner_margin(egui::Margin::same(SP_LG as i8))
        .show(ui, add_contents);
}

// ─── Provider + Model Chip ───────────────────────────────────────────────────

// ─── Section Heading ─────────────────────────────────────────────────────────

/// Consistent settings / panel section heading.
pub fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .strong()
            .size(13.0)
            .color(colors::TEXT_MUTED),
    );
}

// ─── Field Label ─────────────────────────────────────────────────────────────

/// Muted small label used above combo boxes and text fields.
pub fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .small()
            .color(colors::TEXT_MUTED),
    );
}

// ─── Labeled Combo ────────────────────────────────────────────────────────────

// ─── Primary Button ──────────────────────────────────────────────────────────

/// A full-width primary action button (inverted: white fill, dark text).
pub fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .strong()
                .size(13.0)
                .color(colors::ACTION_PRIMARY_TEXT),
        )
        .fill(colors::ACTION_PRIMARY_FILL)
        .corner_radius(egui::CornerRadius::same(R_MD))
        .min_size(egui::vec2(ui.available_width(), 36.0)),
    )
}

// ─── Inline Separator ────────────────────────────────────────────────────────

/// Render a subtle vertical separator for inline horizontal layouts.
pub fn inline_sep(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, 16.0), egui::Sense::hover());
    ui.painter().line_segment(
        [rect.center_top(), rect.center_bottom()],
        egui::Stroke::new(1.0, colors::BORDER_SUBTLE),
    );
}
