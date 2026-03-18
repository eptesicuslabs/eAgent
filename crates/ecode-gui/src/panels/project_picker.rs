//! Project picker modal.

use crate::state::{AppState, UiState};
use crate::theme::{R_SM, SP_MD, SP_SM, SP_XS, colors};
use crate::widgets::shared::primary_button;
use ecode_desktop_app::UiAction;
use std::sync::Arc;
use tokio::sync::mpsc;

pub fn show(
    ctx: &egui::Context,
    state: &Arc<AppState>,
    ui_state: &mut UiState,
    action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    let mut open = ui_state.project_picker_open;

    egui::Window::new("Open Project")
        .open(&mut open)
        .fixed_size([480.0, 400.0])
        .collapsible(false)
        .show(ctx, |ui| {
            let current = state.current_project.read().unwrap().clone();
            let recent = state.recent_projects.read().unwrap().clone();

            ui.spacing_mut().item_spacing = egui::vec2(0.0, SP_MD);

            // ── Browse — primary action always at top ─────────────────────
            if primary_button(ui, "Browse for project folder…").clicked()
                && let Some(path) = rfd::FileDialog::new().pick_folder()
            {
                let path_str = path.to_string_lossy().to_string();
                let _ = action_tx.send(UiAction::OpenProject(path_str.clone()));
                ui_state.project_picker_open = false;
            }

            ui.add_space(SP_SM);

            // ── Current project ────────────────────────────────────────────
            if let Some(ref cur) = current {
                ui.label(
                    egui::RichText::new("CURRENT")
                        .small()
                        .strong()
                        .color(colors::TEXT_MUTED),
                );
                current_project_card(ui, cur, true, || {});
            }

            // ── Recent projects ────────────────────────────────────────────
            if !recent.is_empty() {
                ui.add_space(SP_XS);
                ui.label(
                    egui::RichText::new("RECENT")
                        .small()
                        .strong()
                        .color(colors::TEXT_MUTED),
                );
                ui.add_space(SP_XS);

                egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, SP_XS - 2.0);
                    for path in &recent {
                        let is_current = current.as_ref().map(|c| c == path).unwrap_or(false);
                        let action_tx_ = action_tx.clone();
                        let path_ = path.clone();
                        let mut clicked = false;
                        current_project_card(ui, path, is_current, || { clicked = true; });
                        if clicked {
                            let _ = action_tx_.send(UiAction::OpenProject(path_));
                            ui_state.project_picker_open = false;
                        }
                    }
                });
            }
        });

    ui_state.project_picker_open = open;
}

fn current_project_card(
    ui: &mut egui::Ui,
    path: &str,
    is_current: bool,
    _on_click: impl FnOnce(),
) {
    let project_name = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    let display_path = if path.len() > 60 {
        format!("…{}", &path[path.len() - 57..])
    } else {
        path.to_string()
    };

    egui::Frame::new()
        .fill(colors::SURFACE_1)
        .stroke(egui::Stroke::new(
            1.0,
            if is_current { colors::ACCENT } else { colors::BORDER_SUBTLE },
        ))
        .corner_radius(egui::CornerRadius::same(R_SM))
        .inner_margin(egui::Margin::same(SP_SM as i8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if is_current {
                    ui.label(egui::RichText::new("[ACTIVE]").small().color(colors::ACCENT));
                    ui.add_space(SP_XS);
                }
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(&project_name)
                            .strong()
                            .size(13.0)
                            .color(colors::TEXT_PRIMARY),
                    );
                    ui.label(
                        egui::RichText::new(&display_path)
                            .small()
                            .monospace()
                            .color(colors::TEXT_DISABLED),
                    );
                });
            });
        });
}
