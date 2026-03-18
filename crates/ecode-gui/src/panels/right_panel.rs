//! Right panel — Plan view, Changes (diff) view, Git view.

use crate::state::{AppState, RightPanelTab, UiState};
use crate::theme::{R_SM, SP_MD, SP_SM, SP_XS, colors};
use crate::widgets::shared::segmented_control;
use ecode_contracts::provider_runtime::ProviderRuntimeEventKind;
use std::sync::Arc;

pub fn show(ui: &mut egui::Ui, state: &Arc<AppState>, ui_state: &mut UiState) {
    ui.vertical(|ui| {
        ui.add_space(SP_SM);

        // ── Segmented tab selector ──────────────────────────────────────────
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Tab {
            Plan,
            Changes,
            Git,
        }
        let mut tab = match ui_state.right_panel_tab {
            RightPanelTab::Plan => Tab::Plan,
            RightPanelTab::Diff => Tab::Changes,
            RightPanelTab::Git => Tab::Git,
        };
        let options: &[(&str, Tab)] = &[
            ("Plan", Tab::Plan),
            ("Changes", Tab::Changes),
            ("Git", Tab::Git),
        ];
        if segmented_control(ui, egui::Id::new("right-panel-tab"), options, &mut tab) {
            ui_state.right_panel_tab = match tab {
                Tab::Plan => RightPanelTab::Plan,
                Tab::Changes => RightPanelTab::Diff,
                Tab::Git => RightPanelTab::Git,
            };
        }

        ui.add_space(SP_SM);

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, SP_XS);
            match tab {
                Tab::Plan => show_plan(ui, state),
                Tab::Changes => show_changes(ui, state),
                Tab::Git => show_git(ui, state),
            }
        });
    });
}

// ─── Plan View ────────────────────────────────────────────────────────────────

fn show_plan(ui: &mut egui::Ui, state: &Arc<AppState>) {
    let thread_id = *state.current_thread.read().unwrap();
    let Some(tid) = thread_id else {
        empty_state(ui, "Select a thread to view its execution plan.");
        return;
    };

    let model = state.read_model.read().unwrap();
    let Some(thread) = model.threads.get(&tid) else {
        empty_state(ui, "Thread not found.");
        return;
    };

    if thread.turns.is_empty() {
        empty_state(ui, "No turns yet. Send a message to start.");
        return;
    }

    let mut turns: Vec<_> = thread.turns.iter().collect();
    turns.sort_by(|a, b| a.started_at.cmp(&b.started_at));

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, SP_XS - 2.0);

        for (i, turn) in turns.iter().enumerate() {
            egui::Frame::new()
                .fill(colors::SURFACE_1)
                .stroke(egui::Stroke::new(1.0, colors::BORDER_SUBTLE))
                .corner_radius(egui::CornerRadius::same(R_SM))
                .inner_margin(egui::Margin::same(SP_SM as i8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Status icon
                        let (icon, color) = match &turn.status {
                            ecode_contracts::orchestration::TurnStatus::Completed => ("OK", colors::STATUS_RUNNING_TEXT),
                            ecode_contracts::orchestration::TurnStatus::Running => ("RUN", colors::STATUS_WAITING_TEXT),
                            ecode_contracts::orchestration::TurnStatus::Waiting => ("WAIT", colors::STATUS_WAITING_TEXT),
                            ecode_contracts::orchestration::TurnStatus::Failed => ("FAIL", colors::STATUS_ERROR_TEXT),
                            ecode_contracts::orchestration::TurnStatus::Interrupted => ("INT", colors::TEXT_DISABLED),
                            ecode_contracts::orchestration::TurnStatus::Requested => ("...", colors::TEXT_MUTED),
                        };
                        ui.label(egui::RichText::new(icon).small().color(color));
                        ui.add_space(SP_XS);
                        ui.label(
                            egui::RichText::new(format!("Step {}", i + 1))
                                .small()
                                .strong()
                                .color(colors::TEXT_MUTED),
                        );
                    });

                    let preview = if turn.input.len() > 80 {
                        format!("{}…", &turn.input[..80])
                    } else {
                        turn.input.clone()
                    };
                    ui.label(
                        egui::RichText::new(preview)
                            .small()
                            .color(colors::TEXT_SECONDARY),
                    );

                    // Tool activity count
                    let tool_count = thread.runtime_events.iter()
                        .filter(|e| {
                            e.turn_id == Some(turn.id)
                                && e.event_type == ProviderRuntimeEventKind::ToolCompleted
                        })
                        .count();
                    if tool_count > 0 {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} tool call{}",
                                tool_count,
                                if tool_count == 1 { "" } else { "s" }
                            ))
                            .small()
                            .color(colors::TEXT_DISABLED),
                        );
                    }
                });
        }
    });
}

// ─── Changes View ─────────────────────────────────────────────────────────────

fn show_changes(ui: &mut egui::Ui, state: &Arc<AppState>) {
    let thread_id = *state.current_thread.read().unwrap();
    let Some(tid) = thread_id else {
        empty_state(ui, "Select a thread to view file changes.");
        return;
    };

    let model = state.read_model.read().unwrap();
    let Some(thread) = model.threads.get(&tid) else {
        empty_state(ui, "Thread not found.");
        return;
    };

    // Collect FileChange events
    let changes: Vec<_> = thread
        .runtime_events
        .iter()
        .filter(|e| e.event_type == ProviderRuntimeEventKind::ToolCompleted)
        .filter(|e| e.item_id.as_deref().map(|id| id.contains("write") || id.contains("file")).unwrap_or(false))
        .collect();

    if changes.is_empty() {
        empty_state(ui, "No file changes recorded in this session.");
        return;
    }

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, SP_XS - 2.0);
        for event in &changes {
            egui::Frame::new()
                .fill(colors::SURFACE_1)
                .stroke(egui::Stroke::new(1.0, colors::BORDER_SUBTLE))
                .corner_radius(egui::CornerRadius::same(R_SM))
                .inner_margin(egui::Margin::same(SP_SM as i8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("M").small().color(colors::STATUS_STARTING_TEXT));
                        ui.add_space(SP_XS);
                        let path = event.item_id.as_deref().unwrap_or("unknown");
                        ui.label(
                            egui::RichText::new(path)
                                .small()
                                .monospace()
                                .color(colors::TEXT_SECONDARY),
                        );
                    });
                });
        }
    });
}

// ─── Git View ─────────────────────────────────────────────────────────────────

fn show_git(ui: &mut egui::Ui, state: &Arc<AppState>) {
    let project = state.current_project.read().unwrap().clone();
    let Some(path) = project else {
        empty_state(ui, "Open a project to view Git information.");
        return;
    };

    match ecode_core::git::GitManager::open(&path) {
        Err(_) => {
            empty_state(ui, "Not a Git repository.");
        }
        Ok(git) => {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, SP_XS);

                // Branch
                if let Ok(Some(branch)) = git.current_branch() {
                    egui::Frame::new()
                        .fill(colors::SURFACE_1)
                        .corner_radius(egui::CornerRadius::same(R_SM))
                        .inner_margin(egui::Margin::same(SP_SM as i8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(SP_XS, 0.0);
                                ui.label(
                                    egui::RichText::new("Branch")
                                        .small()
                                        .color(colors::TEXT_MUTED),
                                );
                                ui.label(
                                    egui::RichText::new(&branch)
                                        .small()
                                        .strong()
                                        .color(colors::TEXT_PRIMARY),
                                );
                            });
                        });
                }

                ui.add_space(SP_XS);

                // File status
                match git.status() {
                    Ok(statuses) if statuses.is_empty() => {
                        ui.label(
                            egui::RichText::new("Working tree clean")
                                .small()
                                .color(colors::STATUS_RUNNING_TEXT),
                        );
                    }
                    Ok(statuses) => {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                            for file in &statuses {
                                let (icon, color) = match file.status {
                                    ecode_contracts::git::FileStatusKind::New => ("A", colors::STATUS_RUNNING_TEXT),
                                    ecode_contracts::git::FileStatusKind::Modified => ("M", colors::STATUS_STARTING_TEXT),
                                    ecode_contracts::git::FileStatusKind::Deleted => ("D", colors::STATUS_ERROR_TEXT),
                                    ecode_contracts::git::FileStatusKind::Renamed => ("R", colors::ACCENT),
                                    _ => ("?", colors::TEXT_DISABLED),
                                };
                                egui::Frame::new()
                                    .fill(colors::SURFACE_1)
                                    .corner_radius(egui::CornerRadius::same(R_SM))
                                    .inner_margin(egui::Margin::symmetric(SP_SM as i8, (SP_XS - 2.0) as i8))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing = egui::vec2(SP_SM, 0.0);
                                            ui.label(
                                                egui::RichText::new(icon).small().strong().color(color),
                                            );
                                            ui.label(
                                                egui::RichText::new(&file.path)
                                                    .small()
                                                    .monospace()
                                                    .color(colors::TEXT_SECONDARY),
                                            );
                                        });
                                    });
                            }
                        });
                    }
                    Err(_) => {
                        empty_state(ui, "Failed to read repository status.");
                    }
                }
            });
        }
    }
}

// ─── Empty State Helper ───────────────────────────────────────────────────────

fn empty_state(ui: &mut egui::Ui, msg: &str) {
    ui.add_space(SP_MD);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(msg).small().color(colors::TEXT_DISABLED));
    });
}
