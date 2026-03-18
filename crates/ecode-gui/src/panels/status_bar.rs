//! Status bar — bottom chrome with status, activity, and diagnostics.

use crate::state::AppState;
use crate::theme::{SP_MD, SP_SM, SP_XS, colors};
use crate::widgets::shared::inline_sep;
use std::sync::Arc;

pub fn show(ui: &mut egui::Ui, state: &Arc<AppState>) {
    let msg = state.status_message.read().unwrap().clone();
    let thread_count = state.read_model.read().unwrap().threads.len();
    let codex_available = *state.codex_available.read().unwrap();
    let codex_version = state.codex_version.read().unwrap().clone();
    let errors = state.errors.read().unwrap().clone();
    let any_busy = state
        .read_model
        .read()
        .unwrap()
        .threads
        .values()
        .any(|t| t.active_turn.is_some());

    egui::Frame::new()
        .fill(colors::SURFACE_0)
        .inner_margin(egui::Margin::symmetric(SP_MD as i8, SP_XS as i8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(SP_SM, 0.0);

                // ── Left: status message + optional spinner ─────────────────
                if any_busy {
                    ui.spinner();
                }
                ui.label(
                    egui::RichText::new(&msg)
                        .small()
                        .color(if any_busy { colors::TEXT_SECONDARY } else { colors::TEXT_DISABLED }),
                );

                // ── Error badge ───────────────────────────────────────────
                if !errors.is_empty() {
                    ui.add_space(SP_XS);
                    inline_sep(ui);
                    ui.add_space(SP_XS);
                    ui.label(
                        egui::RichText::new(format!("⚠ {} error{}", errors.len(), if errors.len() == 1 { "" } else { "s" }))
                            .small()
                            .color(colors::STATUS_WAITING_TEXT),
                    );
                }

                // ── Right-aligned: thread count · Codex status ─────────────
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(SP_SM, 0.0);

                    // Codex connection status dot
                    let (dot, dot_color) = if codex_available {
                        ("●", colors::STATUS_RUNNING_TEXT)
                    } else {
                        ("○", colors::TEXT_DISABLED)
                    };
                    let version_suffix = codex_version
                        .as_deref()
                        .map(|v| format!(" v{}", v))
                        .unwrap_or_default();
                    ui.label(
                        egui::RichText::new(format!("{} Codex{}", dot, version_suffix))
                            .small()
                            .color(dot_color),
                    );

                    inline_sep(ui);

                    // Thread count
                    ui.label(
                        egui::RichText::new(format!(
                            "{} thread{}",
                            thread_count,
                            if thread_count == 1 { "" } else { "s" }
                        ))
                        .small()
                        .color(colors::TEXT_DISABLED),
                    );
                });
            });
        });
}
