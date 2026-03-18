//! Terminal panel — tabbed terminal management.

use crate::state::{AppState, UiState};
use crate::theme::{R_SM, SP_SM, SP_XS, colors};
use ecode_desktop_app::UiAction;
use std::sync::Arc;
use tokio::sync::mpsc;

pub fn show(
    ui: &mut egui::Ui,
    state: &Arc<AppState>,
    ui_state: &mut UiState,
    action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    let terminals = state.terminals.read().unwrap().clone();

    ui.vertical(|ui| {
        // ── Tab bar + New Terminal button ──────────────────────────────────
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(SP_XS, 0.0);

            for (idx, term) in terminals.iter().enumerate() {
                let is_active = ui_state.active_terminal.as_ref() == Some(&term.id);
                let tab_btn = egui::Button::new(
                    egui::RichText::new(format!("Terminal {}", idx + 1))
                        .size(12.0)
                        .strong()
                        .color(if is_active { colors::TEXT_PRIMARY } else { colors::TEXT_MUTED }),
                )
                .fill(if is_active { colors::SURFACE_3 } else { egui::Color32::TRANSPARENT })
                .stroke(egui::Stroke::new(
                    1.0,
                    if is_active { colors::BORDER_DEFAULT } else { egui::Color32::TRANSPARENT },
                ))
                .corner_radius(egui::CornerRadius::same(R_SM))
                .min_size(egui::vec2(0.0, 26.0));

                if ui.add(tab_btn).clicked() && !is_active {
                    ui_state.active_terminal = Some(term.id);
                }
            }

            // "+ New" at the end
            let new_btn = egui::Button::new(
                egui::RichText::new("+").size(14.0).color(colors::TEXT_MUTED),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(1.0, colors::BORDER_SUBTLE))
            .corner_radius(egui::CornerRadius::same(R_SM))
            .min_size(egui::vec2(28.0, 26.0));
            if ui.add(new_btn).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                let _ = action_tx.send(UiAction::OpenTerminal);
            }
        });

        ui.add_space(SP_XS);
        ui.separator();

        // ── Active terminal content ────────────────────────────────────────
        let active_id = ui_state.active_terminal.or_else(|| terminals.first().map(|t| t.id));

        if let Some(term_id) = active_id {
            if let Some(term) = terminals.iter().find(|t| t.id == term_id) {
                // Ensure active_terminal is set
                ui_state.active_terminal = Some(term_id);

                // Buffer — fill available height minus input row (28px)
                let buffer_height = (ui.available_height() - 36.0).max(40.0);

                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .max_height(buffer_height)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut term.buffer.as_str())
                                .font(egui::TextStyle::Monospace)
                                .desired_rows(20)
                                .desired_width(f32::INFINITY)
                                .interactive(false),
                        );
                    });

                // Input row — persisted per terminal via UiState
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(SP_XS, 0.0);
                    ui.label(
                        egui::RichText::new("$")
                            .monospace()
                            .size(13.0)
                            .color(colors::ACCENT),
                    );

                    let input = ui_state.terminal_inputs.entry(term_id).or_default();
                    let response = ui.add_sized(
                        [ui.available_width(), 22.0],
                        egui::TextEdit::singleline(input)
                            .font(egui::TextStyle::Monospace)
                            .hint_text("Enter command…")
                            .frame(false),
                    );

                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let cmd = std::mem::take(input);
                        if !cmd.trim().is_empty() {
                            let _ = action_tx.send(UiAction::SendTerminalInput {
                                terminal_id: term_id,
                                input: cmd,
                            });
                        }
                        response.request_focus();
                    }
                });
            }
        } else {
            // No terminals yet
            ui.vertical_centered(|ui| {
                ui.add_space(SP_SM);
                ui.label(
                    egui::RichText::new("No terminals open. Click + to start one.")
                        .small()
                        .color(colors::TEXT_DISABLED),
                );
            });
        }
    });
}
