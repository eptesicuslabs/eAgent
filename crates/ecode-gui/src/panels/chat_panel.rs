//! Chat panel — transcript, thread controls, composer, welcome state.

use crate::state::{AppState, UiState};
use crate::theme::{R_LG, R_MD, R_SM, R_XL, SP_LG, SP_MD, SP_SM, SP_XL, SP_XXL, SP_XS, colors};
use crate::widgets::markdown::render_markdown;
use crate::widgets::shared::{badge, field_label, pill, status_chip, StatusSeverity};
use ecode_desktop_app::UiAction;
use ecode_contracts::orchestration::{
    ApprovalKind, InteractionMode, MessageRole, PendingApproval, PendingUserInput, RuntimeMode,
    ThreadState, TurnStatus,
};
use ecode_contracts::provider::ProviderKind;
use ecode_contracts::provider_runtime::{ProviderRuntimeEventKind, ProviderSessionStatus};
use std::sync::Arc;
use tokio::sync::mpsc;

pub fn show(
    ui: &mut egui::Ui,
    state: &Arc<AppState>,
    ui_state: &mut UiState,
    action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    let thread_id = *state.current_thread.read().unwrap();

    match thread_id {
        None => show_welcome(ui, state, action_tx),
        Some(thread_id) => {
            let thread = state
                .read_model
                .read()
                .unwrap()
                .threads
                .get(&thread_id)
                .cloned();

            match thread {
                None => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(SP_XL);
                        ui.label(
                            egui::RichText::new("Thread not found")
                                .color(colors::TEXT_MUTED),
                        );
                    });
                }
                Some(thread) => {
                    let draft = ui_state.thread_draft_mut(thread_id);
                    ui.vertical(|ui| {
                        ui.add_space(SP_SM);
                        show_thread_header(ui, state, &thread, action_tx);
                        ui.add_space(SP_SM);

                        // Transcript — take all height except composer (~100px)
                        let composer_height = 80.0;
                        let available = (ui.available_height() - composer_height).max(100.0);
                        egui::ScrollArea::vertical()
                            .max_height(available)
                            .stick_to_bottom(draft.chat_scroll_to_bottom)
                            .show(ui, |ui| {
                                ui.add_space(SP_SM);
                                show_transcript(ui, &thread, action_tx);
                                ui.add_space(SP_SM);
                            });

                        // Subtle separator
                        ui.separator();
                        ui.add_space(SP_XS);
                        show_composer(ui, state, &thread, draft, action_tx);
                    });
                }
            }
        }
    }
}

// ─── Welcome / Empty State ────────────────────────────────────────────────────

fn show_welcome(
    ui: &mut egui::Ui,
    state: &Arc<AppState>,
    action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(SP_XXL + SP_SM);

        egui::Frame::new()
            .fill(colors::SURFACE_1)
            .stroke(egui::Stroke::new(1.0, colors::BORDER_DEFAULT))
            .corner_radius(egui::CornerRadius::same(R_XL))
            .inner_margin(egui::Margin::same(SP_XL as i8))
            .show(ui, |ui| {
                ui.set_max_width(580.0);

                // Mini wordmark above headline
                ui.label(
                    egui::RichText::new("eCode")
                        .size(12.0)
                        .strong()
                        .color(colors::TEXT_MUTED),
                );
                ui.add_space(SP_SM);

                ui.label(
                    egui::RichText::new(
                        "A local-first command center for autonomous coding.",
                    )
                    .size(24.0)
                    .strong()
                    .color(colors::TEXT_PRIMARY),
                );
                ui.add_space(SP_XS);
                ui.label(
                    egui::RichText::new(
                        "Open a project, pick a model, and keep the full transcript, approvals, and terminal in one place.",
                    )
                    .size(14.0)
                    .color(colors::TEXT_MUTED),
                );

                ui.add_space(SP_MD);

                // Codex unavailable warning
                if !*state.codex_available.read().unwrap() {
                    egui::Frame::new()
                        .fill(colors::STATUS_ERROR_BG)
                        .corner_radius(egui::CornerRadius::same(R_SM))
                        .inner_margin(egui::Margin::same(SP_SM as i8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Codex CLI not found.")
                                        .small()
                                        .color(colors::STATUS_ERROR_TEXT),
                                );
                                ui.add_space(SP_XS);
                                ui.label(
                                    egui::RichText::new("Configure the binary path in Settings.")
                                        .small()
                                        .color(colors::TEXT_MUTED),
                                );
                            });
                        });
                    ui.add_space(SP_MD);
                }

                // Capability badges
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(SP_XS, SP_XS);
                    badge(ui, "Codex app-server");
                    badge(ui, "Live model catalog");
                    badge(ui, "Rust desktop shell");
                    badge(ui, "Local llama.cpp");
                });

                ui.add_space(SP_LG);

                // Primary CTA
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Start a new thread")
                                .size(14.0)
                                .strong()
                                .color(colors::ACTION_PRIMARY_TEXT),
                        )
                        .fill(colors::ACTION_PRIMARY_FILL)
                        .corner_radius(egui::CornerRadius::same(R_MD))
                        .min_size(egui::vec2(200.0, 40.0)),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    let name = format!("Chat {}", chrono::Local::now().format("%H:%M"));
                    let _ = action_tx.send(UiAction::CreateThread { name });
                }

                ui.add_space(SP_LG);

                // Quick-start suggestions
                ui.label(
                    egui::RichText::new("Try asking…")
                        .small()
                        .color(colors::TEXT_DISABLED),
                );
                ui.add_space(SP_XS);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(SP_XS, SP_XS);
                    let suggestions = [
                        "Review this PR",
                        "Fix the failing test",
                        "Explain this module",
                        "Refactor auth",
                    ];
                    for suggestion in &suggestions {
                        let btn = egui::Button::new(
                            egui::RichText::new(*suggestion)
                                .small()
                                .color(colors::TEXT_SECONDARY),
                        )
                        .fill(colors::SURFACE_2)
                        .stroke(egui::Stroke::new(1.0, colors::BORDER_DEFAULT))
                        .corner_radius(egui::CornerRadius::same(R_SM));
                        if ui.add(btn).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            // Create thread and pre-fill composer
                            let name = format!("Chat {}", chrono::Local::now().format("%H:%M"));
                            let _ = action_tx.send(UiAction::CreateThread { name });
                            // Note: the suggestion text will appear as a hint in the composer on next frame.
                        }
                    }
                });
            });
    });
}

// ─── Thread Header ─────────────────────────────────────────────────────────────

fn show_thread_header(
    ui: &mut egui::Ui,
    state: &Arc<AppState>,
    thread: &ThreadState,
    action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    let mut settings = thread.settings.clone();
    let original = settings.clone();
    let status = thread
        .session
        .as_ref()
        .map(|s| s.status)
        .unwrap_or(ProviderSessionStatus::Stopped);
    let codex_models = state.codex_models.read().unwrap().clone();
    let current_project = state.current_project.read().unwrap().clone();

    egui::Frame::new()
        .fill(colors::SURFACE_1)
        .stroke(egui::Stroke::new(1.0, colors::BORDER_SUBTLE))
        .corner_radius(egui::CornerRadius::same(R_LG))
        .inner_margin(egui::Margin::symmetric(SP_MD as i8, SP_SM as i8))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(SP_SM, SP_XS);

            // Row 1: project context + thread name + status pill
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    // Project breadcrumb
                    if let Some(ref path) = current_project
                        && let Some(name) = std::path::Path::new(path).file_name()
                    {
                        ui.label(
                            egui::RichText::new(name.to_string_lossy())
                                .small()
                                .color(colors::TEXT_DISABLED),
                        );
                    }
                    // Thread name
                    ui.label(
                        egui::RichText::new(&thread.name)
                            .size(15.0)
                            .strong()
                            .color(colors::TEXT_PRIMARY),
                    );
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    let severity = match status {
                        ProviderSessionStatus::Starting => StatusSeverity::Starting,
                        ProviderSessionStatus::Ready => StatusSeverity::Ready,
                        ProviderSessionStatus::Running => StatusSeverity::Running,
                        ProviderSessionStatus::Waiting => StatusSeverity::Waiting,
                        ProviderSessionStatus::Stopped => StatusSeverity::Offline,
                        ProviderSessionStatus::Error => StatusSeverity::Error,
                    };
                    let label = match status {
                        ProviderSessionStatus::Starting => "Starting",
                        ProviderSessionStatus::Ready => "Ready",
                        ProviderSessionStatus::Running => "Running",
                        ProviderSessionStatus::Waiting => "Waiting",
                        ProviderSessionStatus::Stopped => "Stopped",
                        ProviderSessionStatus::Error => "Error",
                    };
                    status_chip(ui, label, severity);
                });
            });

            ui.add_space(SP_XS);

            // Row 2: compact inline control strip
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(SP_LG, SP_XS);

                // Provider
                ui.vertical(|ui| {
                    field_label(ui, "Provider");
                    egui::ComboBox::from_id_salt(("provider", thread.id))
                        .width(120.0)
                        .selected_text(match settings.provider {
                            ProviderKind::Codex => "Codex",
                            ProviderKind::LlamaCpp => "llama.cpp",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut settings.provider, ProviderKind::Codex, "Codex");
                            ui.selectable_value(
                                &mut settings.provider,
                                ProviderKind::LlamaCpp,
                                "llama.cpp",
                            );
                        });
                    // Auto-reset model when provider changes
                    if settings.provider != original.provider {
                        match settings.provider {
                            ProviderKind::Codex => {
                                settings.model = state.preferred_codex_model();
                                settings.local_agent_web_search_enabled = false;
                            }
                            ProviderKind::LlamaCpp => {
                                settings.model =
                                    state.config.read().unwrap().llama_cpp.default_model.clone();
                            }
                        }
                    }
                });

                // Model
                match settings.provider {
                    ProviderKind::Codex => {
                        ui.vertical(|ui| {
                            field_label(ui, "Model");
                            egui::ComboBox::from_id_salt(("model", thread.id))
                                .width(190.0)
                                .selected_text(&settings.model)
                                .show_ui(ui, |ui| {
                                    if !codex_models.iter().any(|m| m == &settings.model) {
                                        let current = settings.model.clone();
                                        ui.selectable_value(
                                            &mut settings.model,
                                            current.clone(),
                                            format!("{} (current)", current),
                                        );
                                    }
                                    for m in &codex_models {
                                        ui.selectable_value(&mut settings.model, m.clone(), m);
                                    }
                                });
                        });
                    }
                    ProviderKind::LlamaCpp => {
                        ui.vertical(|ui| {
                            field_label(ui, "Model");
                            ui.add_sized(
                                [190.0, 24.0],
                                egui::TextEdit::singleline(&mut settings.model),
                            );
                        });
                    }
                }

                // Access
                ui.vertical(|ui| {
                    field_label(ui, "Access");
                    egui::ComboBox::from_id_salt(("runtime", thread.id))
                        .width(155.0)
                        .selected_text(match settings.runtime_mode {
                            RuntimeMode::ApprovalRequired => "Approval Required",
                            RuntimeMode::FullAccess => "Full Access",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut settings.runtime_mode,
                                RuntimeMode::ApprovalRequired,
                                "Approval Required",
                            );
                            ui.selectable_value(
                                &mut settings.runtime_mode,
                                RuntimeMode::FullAccess,
                                "Full Access",
                            );
                        });
                });

                // Mode
                ui.vertical(|ui| {
                    field_label(ui, "Mode");
                    egui::ComboBox::from_id_salt(("interaction", thread.id))
                        .width(110.0)
                        .selected_text(match settings.interaction_mode {
                            InteractionMode::Chat => "Chat",
                            InteractionMode::Plan => "Plan",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut settings.interaction_mode,
                                InteractionMode::Chat,
                                "Chat",
                            );
                            ui.selectable_value(
                                &mut settings.interaction_mode,
                                InteractionMode::Plan,
                                "Plan",
                            );
                        });
                });

                // Web Search toggle (llama.cpp only)
                if settings.provider == ProviderKind::LlamaCpp {
                    ui.vertical(|ui| {
                        field_label(ui, "Tools");
                        let web_on = settings.local_agent_web_search_enabled;
                        let web_btn = egui::Button::new(
                            egui::RichText::new("Web")
                                .small()
                                .color(if web_on { colors::ACCENT } else { colors::TEXT_DISABLED }),
                        )
                        .fill(if web_on { colors::ACCENT_DIM } else { colors::SURFACE_2 })
                        .stroke(egui::Stroke::new(
                            1.0,
                            if web_on { colors::ACCENT } else { colors::BORDER_DEFAULT },
                        ))
                        .corner_radius(egui::CornerRadius::same(R_SM))
                        .min_size(egui::vec2(0.0, 24.0));
                        if ui.add(web_btn).clicked() {
                            settings.local_agent_web_search_enabled = !web_on;
                        }
                    });
                }
            });

            if settings != thread.settings {
                let _ = action_tx.send(UiAction::UpdateCurrentThreadSettings(settings));
            }
        });
}

// ─── Transcript ────────────────────────────────────────────────────────────────

fn show_transcript(
    ui: &mut egui::Ui,
    thread: &ThreadState,
    action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    let mut turns: Vec<_> = thread.turns.iter().collect();
    turns.sort_by(|a, b| a.started_at.cmp(&b.started_at));

    for turn in &turns {
        // User message
        show_message_bubble(ui, "You", &turn.input, true);

        // Assistant / system messages
        for msg in &turn.messages {
            match msg.role {
                MessageRole::Assistant => {
                    show_assistant_bubble(ui, &msg.content);
                }
                MessageRole::System => {
                    show_system_bubble(ui, &msg.content);
                }
                MessageRole::User => {}
            }
        }

        // Tool-call activity blocks from runtime events for this turn
        for event in thread.runtime_events.iter().filter(|e| e.turn_id == Some(turn.id)) {
            show_tool_activity_block(ui, event);
        }

        // Inline turn status indicator
        match turn.status {
            TurnStatus::Requested | TurnStatus::Running => {
                show_running_state(ui);
            }
            TurnStatus::Waiting => {
                show_waiting_state(ui, "Waiting for approval or user response");
            }
            TurnStatus::Failed => {
                show_turn_error(ui, &thread.errors);
            }
            TurnStatus::Interrupted => {
                show_interrupted_state(ui);
            }
            TurnStatus::Completed => {}
        }
    }

    // Pending approvals
    for approval in thread.pending_approvals.values() {
        show_approval_request(ui, approval, action_tx);
    }

    // Pending user input requests
    for input in thread.pending_inputs.values() {
        show_user_input_request(ui, input, action_tx);
    }
}

// ─── Message Bubbles ──────────────────────────────────────────────────────────

fn show_message_bubble(ui: &mut egui::Ui, sender: &str, text: &str, is_user: bool) {
    let (bg, border, _label_color) = if is_user {
        (colors::SURFACE_2, colors::BORDER_DEFAULT, colors::TEXT_PRIMARY)
    } else {
        (colors::SURFACE_1, colors::BORDER_SUBTLE, colors::TEXT_SECONDARY)
    };

    egui::Frame::new()
        .fill(bg)
        .stroke(egui::Stroke::new(1.0, border))
        .corner_radius(egui::CornerRadius::same(R_MD))
        .inner_margin(egui::Margin::same(SP_MD as i8))
        .outer_margin(egui::Margin::symmetric(0, SP_XS as i8))
        .show(ui, |ui| {
            // Compact sender + timestamp row
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(SP_XS, 0.0);
                ui.label(
                    egui::RichText::new(sender)
                        .size(11.0)
                        .strong()
                        .color(if is_user { colors::TEXT_PRIMARY } else { colors::TEXT_MUTED }),
                );
            });
            ui.add_space(SP_XS - 2.0);
            ui.label(
                egui::RichText::new(text)
                    .size(14.0)
                    .color(colors::TEXT_SECONDARY),
            );
        });
}

fn show_assistant_bubble(ui: &mut egui::Ui, content: &str) {
    egui::Frame::new()
        .fill(colors::SURFACE_1)
        .stroke(egui::Stroke::new(1.0, colors::BORDER_SUBTLE))
        .corner_radius(egui::CornerRadius::same(R_MD))
        .inner_margin(egui::Margin::same(SP_MD as i8))
        .outer_margin(egui::Margin::symmetric(0, SP_XS as i8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Assistant")
                        .size(11.0)
                        .strong()
                        .color(colors::TEXT_MUTED),
                );
            });
            ui.add_space(SP_XS - 2.0);
            // Use the markdown renderer for assistant content
            render_markdown(ui, content);
        });
}

fn show_system_bubble(ui: &mut egui::Ui, content: &str) {
    // System messages render as a subtle muted label with no frame
    ui.label(
        egui::RichText::new(content)
            .small()
            .italics()
            .color(colors::TEXT_DISABLED),
    );
}

// ─── Tool Activity Blocks ─────────────────────────────────────────────────────

fn show_tool_activity_block(
    ui: &mut egui::Ui,
    event: &ecode_contracts::provider_runtime::ProviderRuntimeEvent,
) {
    let (icon, text_color) = match event.event_type {
        ProviderRuntimeEventKind::ToolStarted => ("START", colors::STATUS_WAITING_TEXT),
        ProviderRuntimeEventKind::ToolCompleted => ("DONE", colors::STATUS_RUNNING_TEXT),
        ProviderRuntimeEventKind::RuntimeError => ("FAIL", colors::STATUS_ERROR_TEXT),
        _ => return, // skip non-tool events in transcript
    };

    egui::Frame::new()
        .fill(colors::SURFACE_0)
        .stroke(egui::Stroke::new(1.0, colors::BORDER_SUBTLE))
        .corner_radius(egui::CornerRadius::same(R_SM))
        .inner_margin(egui::Margin::symmetric(SP_SM as i8, (SP_XS + 2.0) as i8))
        .outer_margin(egui::Margin::symmetric(0, 2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(SP_XS, 0.0);
                ui.label(
                    egui::RichText::new(icon)
                        .small()
                        .color(text_color),
                );
                let summary = event.summary.as_deref().unwrap_or("Tool activity");
                ui.label(
                    egui::RichText::new(summary)
                        .small()
                        .monospace()
                        .color(colors::TEXT_MUTED),
                );
                // Show item_id (tool name) if different from summary
                if let Some(ref item) = event.item_id
                    && Some(item.as_str()) != event.summary.as_deref()
                {
                    ui.label(
                        egui::RichText::new(format!("· {}", item))
                            .small()
                            .color(colors::TEXT_DISABLED),
                    );
                }
            });
        });
}

// ─── Inline Turn Status Cards ─────────────────────────────────────────────────

fn show_running_state(ui: &mut egui::Ui) {
    ui.add_space(SP_XS);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(SP_SM, 0.0);
        ui.spinner();
        ui.label(
            egui::RichText::new("Running…")
                .small()
                .color(colors::TEXT_MUTED),
        );
    });
    ui.add_space(SP_XS);
}

fn show_waiting_state(ui: &mut egui::Ui, msg: &str) {
    egui::Frame::new()
        .fill(colors::STATUS_WAITING_BG)
        .corner_radius(egui::CornerRadius::same(R_SM))
        .inner_margin(egui::Margin::symmetric(SP_SM as i8, SP_XS as i8))
        .outer_margin(egui::Margin::symmetric(0, SP_XS as i8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(SP_XS, 0.0);
                ui.label(
                    egui::RichText::new("PAUSE")
                        .small()
                        .color(colors::STATUS_WAITING_TEXT),
                );
                ui.label(
                    egui::RichText::new(msg)
                        .small()
                        .color(colors::STATUS_WAITING_TEXT),
                );
            });
        });
}

fn show_turn_error(ui: &mut egui::Ui, errors: &[ecode_contracts::orchestration::ThreadError]) {
    let msg = errors
        .last()
        .map(|e| e.message.as_str())
        .unwrap_or("Turn failed");
    egui::Frame::new()
        .fill(colors::STATUS_ERROR_BG)
        .corner_radius(egui::CornerRadius::same(R_SM))
        .inner_margin(egui::Margin::symmetric(SP_SM as i8, SP_XS as i8))
        .outer_margin(egui::Margin::symmetric(0, SP_XS as i8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(SP_XS, 0.0);
                ui.label(
                    egui::RichText::new("!")
                        .small()
                        .color(colors::STATUS_ERROR_TEXT),
                );
                ui.label(
                    egui::RichText::new(msg)
                        .small()
                        .color(colors::STATUS_ERROR_TEXT),
                );
            });
        });
}

fn show_interrupted_state(ui: &mut egui::Ui) {
    ui.label(
        egui::RichText::new("— Interrupted")
            .small()
            .italics()
            .color(colors::TEXT_DISABLED),
    );
}

// ─── Approval Cards ───────────────────────────────────────────────────────────

fn show_approval_request(
    ui: &mut egui::Ui,
    approval: &PendingApproval,
    action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    let kind_label = match approval.kind {
        ApprovalKind::CommandExecution => "Command Execution",
        ApprovalKind::FileChange => "File Change",
        ApprovalKind::FileRead => "File Read",
    };

    // Format details — try to extract a meaningful summary from the JSON
    let detail_text = format_approval_details(&approval.details);

    egui::Frame::new()
        .fill(colors::STATUS_WAITING_BG)
        .stroke(egui::Stroke::new(1.0, colors::STATUS_WAITING_TEXT))
        .corner_radius(egui::CornerRadius::same(R_MD))
        .inner_margin(egui::Margin::same(SP_MD as i8))
        .outer_margin(egui::Margin::symmetric(0, SP_XS as i8))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(SP_SM, SP_XS);

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Approval Required")
                        .strong()
                        .size(13.0)
                        .color(colors::STATUS_WAITING_TEXT),
                );
                ui.add_space(SP_XS);
                pill(ui, kind_label, colors::STATUS_WAITING_TEXT, colors::STATUS_WAITING_BG);
            });

            if !detail_text.is_empty() {
                egui::Frame::new()
                    .fill(colors::SURFACE_0)
                    .corner_radius(egui::CornerRadius::same(R_SM))
                    .inner_margin(egui::Margin::same(SP_SM as i8))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(&detail_text)
                                .small()
                                .monospace()
                                .color(colors::TEXT_SECONDARY),
                        );
                    });
            }

            ui.add_space(SP_XS);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(SP_SM, 0.0);

                // Approve (green)
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Approve")
                                .strong()
                                .size(13.0)
                                .color(colors::ACTION_PRIMARY_TEXT),
                        )
                        .fill(colors::ACTION_APPROVE_FILL)
                        .corner_radius(egui::CornerRadius::same(R_SM))
                        .min_size(egui::vec2(86.0, 32.0)),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    let _ = action_tx.send(UiAction::Approve(approval.id));
                }

                // Deny (red)
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Deny")
                                .strong()
                                .size(13.0)
                                .color(colors::ACTION_PRIMARY_TEXT),
                        )
                        .fill(colors::ACTION_DESTRUCTIVE_FILL)
                        .corner_radius(egui::CornerRadius::same(R_SM))
                        .min_size(egui::vec2(86.0, 32.0)),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    let _ = action_tx.send(UiAction::Deny(approval.id));
                }
            });
        });
}

fn format_approval_details(details: &serde_json::Value) -> String {
    use serde_json::Value;
    match details {
        Value::Object(map) => {
            // Try common fields in order of relevance
            for key in &["command", "cmd", "path", "file", "content"] {
                if let Some(v) = map.get(*key)
                    && let Value::String(s) = v
                {
                    return format!("{}: {}", key, s);
                }
            }
            // Fallback: render all keys
            map.iter()
                .filter_map(|(k, v)| {
                    if let Value::String(s) = v {
                        Some(format!("{}: {}", k, s))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn show_user_input_request(
    ui: &mut egui::Ui,
    input: &PendingUserInput,
    action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    egui::Frame::new()
        .fill(colors::STATUS_READY_BG)
        .stroke(egui::Stroke::new(1.0, colors::STATUS_READY_TEXT))
        .corner_radius(egui::CornerRadius::same(R_MD))
        .inner_margin(egui::Margin::same(SP_MD as i8))
        .outer_margin(egui::Margin::symmetric(0, SP_XS as i8))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(SP_SM, SP_XS);

            ui.label(
                egui::RichText::new("● Input Requested")
                    .strong()
                    .size(13.0)
                    .color(colors::STATUS_READY_TEXT),
            );

            if let Some(questions) = input.questions.as_ref() {
                let q_text = match questions {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                ui.label(egui::RichText::new(q_text).size(13.0).color(colors::TEXT_SECONDARY));
            }

            let id = egui::Id::new(("user-input", input.id));
            let mut response =
                ui.data_mut(|data| data.get_persisted::<String>(id).unwrap_or_default());

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(SP_SM, 0.0);
                egui::Frame::new()
                    .fill(colors::SURFACE_0)
                    .stroke(egui::Stroke::new(1.0, colors::BORDER_DEFAULT))
                    .corner_radius(egui::CornerRadius::same(R_SM))
                    .inner_margin(egui::Margin::symmetric(SP_SM as i8, SP_XS as i8))
                    .show(ui, |ui| {
                        ui.add_sized(
                            [280.0, 24.0],
                            egui::TextEdit::singleline(&mut response)
                                .frame(false)
                                .hint_text("Your response…"),
                        );
                    });

                if ui
                    .add_enabled(
                        !response.trim().is_empty(),
                        egui::Button::new(
                            egui::RichText::new("Send")
                                .strong()
                                .size(13.0)
                                .color(colors::ACTION_PRIMARY_TEXT),
                        )
                        .fill(colors::ACTION_PRIMARY_FILL)
                        .corner_radius(egui::CornerRadius::same(R_SM))
                        .min_size(egui::vec2(64.0, 32.0)),
                    )
                    .clicked()
                    && !response.trim().is_empty()
                {
                    let _ = action_tx.send(UiAction::UserInputResponse {
                        id: input.id,
                        response: response.clone(),
                    });
                    response.clear();
                }
            });

            ui.data_mut(|data| data.insert_persisted(id, response));
        });
}

// ─── Composer ─────────────────────────────────────────────────────────────────

fn show_composer(
    ui: &mut egui::Ui,
    state: &Arc<AppState>,
    thread: &ThreadState,
    draft: &mut crate::state::ThreadDraftUi,
    action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    let running = state.current_thread_busy();

    egui::Frame::new()
        .fill(colors::SURFACE_1)
        .stroke(egui::Stroke::new(1.0, colors::BORDER_DEFAULT))
        .corner_radius(egui::CornerRadius::same(R_XL))
        .inner_margin(egui::Margin::same(SP_MD as i8))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(SP_SM, SP_XS);

            // Context strip: current model + mode
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(SP_XS, 0.0);
                let provider_label = match thread.settings.provider {
                    ProviderKind::Codex => "Codex",
                    ProviderKind::LlamaCpp => "llama.cpp",
                };
                ui.label(
                    egui::RichText::new(provider_label)
                        .small()
                        .color(colors::TEXT_DISABLED),
                );
                ui.label(
                    egui::RichText::new("·")
                        .small()
                        .color(colors::TEXT_DISABLED),
                );
                ui.label(
                    egui::RichText::new(&thread.settings.model)
                        .small()
                        .monospace()
                        .color(colors::TEXT_DISABLED),
                );
                ui.label(
                    egui::RichText::new("·")
                        .small()
                        .color(colors::TEXT_DISABLED),
                );
                ui.label(
                    egui::RichText::new(match thread.settings.runtime_mode {
                        RuntimeMode::ApprovalRequired => "Approval Required",
                        RuntimeMode::FullAccess => "Full Access",
                    })
                    .small()
                    .color(colors::TEXT_DISABLED),
                );
            });

            // Input row
            ui.horizontal(|ui| {
                let btn_width = 86.0;
                let input_width = ui.available_width() - btn_width - SP_SM;

                let response = ui.add_sized(
                    [input_width, 40.0],
                    egui::TextEdit::multiline(&mut draft.composer_text)
                        .desired_rows(1)
                        .hint_text("Ask eCode to inspect, edit, review, or ship…")
                        .font(egui::TextStyle::Body),
                );

                let enter_pressed = response.lost_focus()
                    && ui.input(|i| {
                        i.key_pressed(egui::Key::Enter) && !i.modifiers.shift
                    });

                if running {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Stop")
                                    .strong()
                                    .color(colors::ACTION_PRIMARY_TEXT),
                            )
                            .fill(colors::ACTION_DESTRUCTIVE_FILL)
                            .corner_radius(egui::CornerRadius::same(R_MD))
                            .min_size(egui::vec2(btn_width, 40.0)),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        let _ = action_tx.send(UiAction::InterruptTurn);
                    }
                } else {
                    let can_send = !draft.composer_text.trim().is_empty();
                    if ui
                        .add_enabled(
                            can_send,
                            egui::Button::new(
                                egui::RichText::new("Send ⏎")
                                    .strong()
                                    .color(colors::ACTION_PRIMARY_TEXT),
                            )
                            .fill(colors::ACTION_PRIMARY_FILL)
                            .corner_radius(egui::CornerRadius::same(R_MD))
                            .min_size(egui::vec2(btn_width, 40.0)),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                        || (enter_pressed && can_send)
                    {
                        let msg = std::mem::take(&mut draft.composer_text);
                        let _ = action_tx.send(UiAction::SendMessage(msg));
                        draft.chat_scroll_to_bottom = true;
                    }
                }
            });
        });
}
