//! Top bar — brand mark, project selector, panel toggles, status.

use crate::state::{AppState, UiState};
use crate::theme::{R_SM, SP_MD, SP_SM, SP_XS, colors};
use crate::widgets::shared::{inline_sep, status_chip, StatusSeverity};
use ecode_desktop_app::UiAction;
use std::sync::Arc;
use tokio::sync::mpsc;

pub fn show(
    ui: &mut egui::Ui,
    state: &Arc<AppState>,
    ui_state: &mut UiState,
    action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    let project_label = state
        .current_project
        .read()
        .unwrap()
        .as_ref()
        .and_then(|p| {
            std::path::Path::new(p)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "No project".to_string());

    let running = state
        .read_model
        .read()
        .unwrap()
        .threads
        .values()
        .any(|t| t.active_turn.is_some());
    let available = *state.codex_available.read().unwrap();

    // Top bar is structural chrome — no card frame, no corner radius.
    // A bottom border is rendered by eframe's TopBottomPanel automatically.
    egui::Frame::new()
        .fill(colors::SURFACE_0)
        .inner_margin(egui::Margin::symmetric(SP_MD as i8, SP_SM as i8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(SP_SM, 0.0);

                // ── Brand mark ────────────────────────────────────────────
                // Compact 2-char wordmark; no subtitle.
                ui.label(
                    egui::RichText::new("eCode")
                        .strong()
                        .size(15.0)
                        .color(colors::TEXT_PRIMARY),
                );

                inline_sep(ui);

                // ── Project selector ──────────────────────────────────────
                let project_btn = egui::Button::new(
                    egui::RichText::new(&project_label)
                        .size(13.0)
                        .color(colors::TEXT_SECONDARY),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::new(1.0, colors::BORDER_DEFAULT))
                .corner_radius(egui::CornerRadius::same(R_SM))
                .min_size(egui::vec2(0.0, 28.0));
                if ui.add(project_btn).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    ui_state.project_picker_open = true;
                }

                inline_sep(ui);

                // ── Panel toggles — compact segmented group ───────────────
                panel_toggle(ui, "Threads", &mut ui_state.sidebar_visible);
                panel_toggle_with_action(
                    ui,
                    "Terminal",
                    &mut ui_state.terminal_visible,
                    || { let _ = action_tx.send(UiAction::OpenTerminal); },
                );
                panel_toggle(ui, "Plan", &mut ui_state.right_panel_visible);

                // ── Right-aligned: status pill + settings ─────────────────
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Settings button
                    let settings_btn = egui::Button::new(
                        egui::RichText::new("Settings")
                            .size(13.0)
                            .color(colors::TEXT_SECONDARY),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::new(1.0, colors::BORDER_DEFAULT))
                    .corner_radius(egui::CornerRadius::same(R_SM))
                    .min_size(egui::vec2(0.0, 28.0));
                    if ui.add(settings_btn).clicked() {
                        ui_state.settings_open = true;
                    }

                    ui.add_space(SP_XS);
                    inline_sep(ui);
                    ui.add_space(SP_XS);

                    // Global status pill
                    let (label, severity) = if running {
                        ("Running", StatusSeverity::Running)
                    } else if available {
                        ("Ready", StatusSeverity::Ready)
                    } else {
                        ("Offline", StatusSeverity::Offline)
                    };
                    status_chip(ui, label, severity);
                });
            });
        });
}

/// A pill-style toggle button for panel visibility.
fn panel_toggle(ui: &mut egui::Ui, label: &str, value: &mut bool) {
    let active = *value;
    let btn = egui::Button::new(
        egui::RichText::new(label)
            .size(12.0)
            .strong()
            .color(if active { colors::TEXT_PRIMARY } else { colors::TEXT_MUTED }),
    )
    .fill(if active { colors::SURFACE_3 } else { egui::Color32::TRANSPARENT })
    .stroke(egui::Stroke::new(
        1.0,
        if active { colors::BORDER_DEFAULT } else { egui::Color32::TRANSPARENT },
    ))
    .corner_radius(egui::CornerRadius::same(R_SM))
    .min_size(egui::vec2(0.0, 28.0));

    if ui.add(btn).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        *value = !*value;
    }
}

/// Same as `panel_toggle` but fires an action when toggled on.
fn panel_toggle_with_action(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut bool,
    on_enable: impl FnOnce(),
) {
    let active = *value;
    let btn = egui::Button::new(
        egui::RichText::new(label)
            .size(12.0)
            .strong()
            .color(if active { colors::TEXT_PRIMARY } else { colors::TEXT_MUTED }),
    )
    .fill(if active { colors::SURFACE_3 } else { egui::Color32::TRANSPARENT })
    .stroke(egui::Stroke::new(
        1.0,
        if active { colors::BORDER_DEFAULT } else { egui::Color32::TRANSPARENT },
    ))
    .corner_radius(egui::CornerRadius::same(R_SM))
    .min_size(egui::vec2(0.0, 28.0));

    if ui.add(btn).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        *value = !*value;
        if *value {
            on_enable();
        }
    }
}
