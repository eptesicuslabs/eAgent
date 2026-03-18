//! Settings modal — unified dark card layout for all provider sections.

use crate::state::{AppState, UiState};
use crate::theme::{R_LG, R_SM, SP_LG, SP_MD, SP_SM, SP_XS, colors};
use crate::widgets::shared::{elevated_card, field_label, primary_button, section_heading};
use ecode_desktop_app::UiAction;
use std::sync::Arc;
use tokio::sync::mpsc;

pub fn show(
    ctx: &egui::Context,
    state: &Arc<AppState>,
    ui_state: &mut UiState,
    action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    let mut open = ui_state.settings_open;

    egui::Window::new("Settings")
        .open(&mut open)
        .fixed_size([720.0, 580.0])
        .collapsible(false)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, SP_MD);

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, SP_MD);

                // ── General section ────────────────────────────────────────
                show_general_section(ui, state, action_tx);

                // ── Codex section ──────────────────────────────────────────
                show_codex_section(ui, state, action_tx);

                // ── llama.cpp section ──────────────────────────────────────
                show_llamacpp_section(ui, state, action_tx);

                // ── Debug / Status ─────────────────────────────────────────
                show_debug_section(ui, state);

                // ── Save button ────────────────────────────────────────────
                ui.add_space(SP_XS);
                if primary_button(ui, "Save & Apply").clicked() {
                    let _ = action_tx.send(UiAction::SaveSettings);
                }
                ui.add_space(SP_MD);
            });
        });

    ui_state.settings_open = open;
}

// ─── General ──────────────────────────────────────────────────────────────────

fn show_general_section(
    ui: &mut egui::Ui,
    _state: &Arc<AppState>,
    _action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    elevated_card(ui, |ui| {
        section_heading(ui, "General");
        ui.add_space(SP_SM);

        // Theme (dark-only for now)
        ui.horizontal(|ui| {
            field_label(ui, "Theme");
            ui.add_space(SP_XS);
            pill(ui, "Dark", colors::TEXT_MUTED, colors::SURFACE_3);
        });

        ui.add_space(SP_XS);

        // Font size (future: persist to config)
        ui.horizontal(|ui| {
            field_label(ui, "Font size");
            ui.add_space(SP_XS);
            ui.label(
                egui::RichText::new("14px (Body)")
                    .small()
                    .color(colors::TEXT_MUTED),
            );
        });
    });
}

use crate::widgets::shared::pill;

// ─── Codex ────────────────────────────────────────────────────────────────────

fn show_codex_section(
    ui: &mut egui::Ui,
    state: &Arc<AppState>,
    action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    let cfg = state.config.read().unwrap().clone();
    let available = *state.codex_available.read().unwrap();
    let version = state.codex_version.read().unwrap().clone();
    let models = state.codex_models.read().unwrap().clone();

    elevated_card(ui, |ui| {
        // Header row
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(SP_SM, 0.0);
            section_heading(ui, "Codex");
            if available {
                pill(ui, "Connected", colors::STATUS_RUNNING_TEXT, colors::STATUS_RUNNING_BG);
            } else {
                pill(ui, "Not found", colors::STATUS_ERROR_TEXT, colors::STATUS_ERROR_BG);
            }
        });

        ui.add_space(SP_SM);

        // Binary path
        let mut codex_path = cfg.codex.binary_path.clone();
        ui.vertical(|ui| {
            field_label(ui, "Binary path");
            ui.add_space(SP_XS - 2.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut codex_path)
                        .hint_text("e.g. C:\\Users\\you\\.cargo\\bin\\codex.exe")
                        .desired_width(380.0),
                );
                let browse_btn = egui::Button::new(
                    egui::RichText::new("Browse…").small().color(colors::TEXT_SECONDARY),
                )
                .fill(colors::SURFACE_3)
                .stroke(egui::Stroke::new(1.0, colors::BORDER_DEFAULT))
                .corner_radius(egui::CornerRadius::same(R_SM));
                if ui.add(browse_btn).clicked()
                    && let Some(path) = rfd::FileDialog::new().pick_file()
                {
                    codex_path = path.to_string_lossy().to_string();
                }
            });
        });

        // CODEX_HOME
        let mut home = cfg.codex.home_dir.clone();
        ui.add_space(SP_SM);
        ui.vertical(|ui| {
            field_label(ui, "CODEX_HOME");
            ui.add_space(SP_XS - 2.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut home)
                        .hint_text("Optional — defaults to ~/.codex")
                        .desired_width(380.0),
                );
                let browse_btn = egui::Button::new(
                    egui::RichText::new("Browse…").small().color(colors::TEXT_SECONDARY),
                )
                .fill(colors::SURFACE_3)
                .stroke(egui::Stroke::new(1.0, colors::BORDER_DEFAULT))
                .corner_radius(egui::CornerRadius::same(R_SM));
                if ui.add(browse_btn).clicked()
                    && let Some(path) = rfd::FileDialog::new().pick_folder()
                {
                    home = path.to_string_lossy().to_string();
                }
            });
        });

        // Preferred model + model pill list
        if !models.is_empty() {
            ui.add_space(SP_SM);
            ui.vertical(|ui| {
                field_label(ui, "Available models");
                ui.add_space(SP_XS - 2.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(SP_XS, SP_XS);
                    for model in &models {
                        pill(ui, model, colors::TEXT_SECONDARY, colors::SURFACE_2);
                    }
                });
            });
        }

        ui.add_space(SP_SM);

        // Resolved binary info
        if let Some(ref ver) = version {
            ui.label(
                egui::RichText::new(format!("Codex v{}", ver))
                    .small()
                    .color(colors::TEXT_DISABLED),
            );
        }

        // Apply changes
        let new_path_opt = if codex_path.is_empty() { None } else { Some(codex_path.clone()) };
        let new_home_opt = if home.is_empty() { None } else { Some(home.clone()) };
        
        if codex_path != cfg.codex.binary_path || home != cfg.codex.home_dir {
            let _ = action_tx.send(UiAction::UpdateCodexConfig {
                binary_path: new_path_opt,
                codex_home: new_home_opt,
            });
        }
    });
}

// ─── llama.cpp ───────────────────────────────────────────────────────────────

fn show_llamacpp_section(
    ui: &mut egui::Ui,
    state: &Arc<AppState>,
    action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    let cfg = state.config.read().unwrap().clone();
    let llama = cfg.llama_cpp.clone();

    elevated_card(ui, |ui| {
        section_heading(ui, "llama.cpp");
        ui.add_space(SP_SM);

        let mut binary = llama.llama_server_binary_path.clone();
        let mut model = llama.model_path.clone();
        let mut host = llama.host.clone();
        let mut port: i32 = llama.port as i32;
        let mut ctx: i32 = llama.ctx_size as i32;
        let mut threads: i32 = llama.threads as i32;

        // Binary path
        labeled_text_row(ui, "Binary path", &mut binary, "Path to llama-server binary", true);
        ui.add_space(SP_XS);
        labeled_text_row(ui, "Default model", &mut model, "Path to .gguf model file", true);
        ui.add_space(SP_XS);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(SP_LG, 0.0);
            small_int_field(ui, "Host", &mut llama.host.clone(), &mut host, 130.0);
            small_int_field_i(ui, "Port", &mut port, 80.0);
            small_int_field_i(ui, "Context", &mut ctx, 90.0);
            small_int_field_i(ui, "Threads", &mut threads, 80.0);
        });

        // Dispatch config updates if modified
        let changed = binary != llama.llama_server_binary_path
            || model != llama.model_path
            || host != llama.host
            || port != llama.port as i32
            || ctx != llama.ctx_size as i32
            || threads != llama.threads as i32;

        if changed {
            let _ = action_tx.send(UiAction::UpdateLlamaCppConfig {
                binary_path: if binary.is_empty() { None } else { Some(binary) },
                default_model: model,
                host: Some(host),
                port: Some(port as u16),
                context_size: Some(ctx as u32),
                threads: Some(threads as u32),
            });
        }
    });
}

fn labeled_text_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    hint: &str,
    browse: bool,
) {
    ui.vertical(|ui| {
        field_label(ui, label);
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(value)
                    .hint_text(hint)
                    .desired_width(380.0),
            );
            if browse {
                let btn = egui::Button::new(
                    egui::RichText::new("Browse…").small().color(colors::TEXT_SECONDARY),
                )
                .fill(colors::SURFACE_3)
                .stroke(egui::Stroke::new(1.0, colors::BORDER_DEFAULT))
                .corner_radius(egui::CornerRadius::same(R_SM));
                if ui.add(btn).clicked()
                    && let Some(path) = rfd::FileDialog::new().pick_file()
                {
                    *value = path.to_string_lossy().to_string();
                }
            }
        });
    });
}

fn small_int_field(ui: &mut egui::Ui, label: &str, _raw: &mut String, value: &mut String, width: f32) {
    ui.vertical(|ui| {
        field_label(ui, label);
        ui.add_space(2.0);
        ui.add_sized([width, 24.0], egui::TextEdit::singleline(value));
    });
}

fn small_int_field_i(ui: &mut egui::Ui, label: &str, value: &mut i32, width: f32) {
    ui.vertical(|ui| {
        field_label(ui, label);
        ui.add_space(2.0);
        ui.add_sized([width, 24.0], egui::DragValue::new(value).range(1..=65535));
    });
}

// ─── Debug / Status Section ───────────────────────────────────────────────────

fn show_debug_section(ui: &mut egui::Ui, state: &Arc<AppState>) {
    let errors = state.errors.read().unwrap().clone();
    let config_path = state.config_path.read().unwrap().clone();

    egui::Frame::new()
        .fill(colors::SURFACE_1)
        .stroke(egui::Stroke::new(1.0, colors::BORDER_SUBTLE))
        .corner_radius(egui::CornerRadius::same(R_LG))
        .inner_margin(egui::Margin::same(SP_MD as i8))
        .show(ui, |ui| {
            section_heading(ui, "Debug info");
            ui.add_space(SP_SM);

            if let Some(ref path) = config_path {
                ui.horizontal(|ui| {
                    field_label(ui, "Config");
                    ui.add_space(SP_XS);
                    ui.label(
                        egui::RichText::new(path.display().to_string())
                            .small()
                            .monospace()
                            .color(colors::TEXT_DISABLED),
                    );
                });
            }

            if !errors.is_empty() {
                ui.add_space(SP_XS);
                for err in &errors {
                    ui.label(
                        egui::RichText::new(format!("⚠ {}", err))
                            .small()
                            .color(colors::STATUS_ERROR_TEXT),
                    );
                }
            }
        });
}
