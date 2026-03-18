//! Main application shell — eframe integration around the shared desktop runtime.

use crate::panels;
use crate::state::{AppState, UiState};
use ecode_desktop_app::{AppRuntime, UiAction};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

/// The main eCode application.
pub struct ECodeApp {
    state: Arc<AppState>,
    ui: UiState,
    action_tx: mpsc::UnboundedSender<UiAction>,
    _app_runtime: AppRuntime,
    _runtime: Runtime,
}

impl ECodeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, runtime: Runtime) -> Self {
        let ctx = cc.egui_ctx.clone();
        let app_runtime = AppRuntime::spawn_with_notifier(&runtime, move || {
            ctx.request_repaint();
        });
        let state = app_runtime.state();
        let action_tx = app_runtime.action_tx();

        Self {
            state,
            ui: UiState::default(),
            action_tx,
            _app_runtime: app_runtime,
            _runtime: runtime,
        }
    }
}

impl eframe::App for ECodeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let errors = self.state.drain_errors();
        for err in &errors {
            tracing::warn!("UI error: {}", err);
        }

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            panels::top_bar::show(ui, &self.state, &mut self.ui, &self.action_tx);
        });

        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(24.0)
            .show(ctx, |ui| {
                panels::status_bar::show(ui, &self.state);
            });

        if self.ui.terminal_visible {
            egui::TopBottomPanel::bottom("terminal_panel")
                .resizable(true)
                .default_height(self.ui.terminal_height)
                .show(ctx, |ui| {
                    panels::terminal_panel::show(ui, &self.state, &mut self.ui, &self.action_tx);
                });
        }

        if self.ui.sidebar_visible {
            egui::SidePanel::left("sidebar")
                .resizable(true)
                .default_width(self.ui.sidebar_width)
                .min_width(220.0)
                .show(ctx, |ui| {
                    panels::sidebar::show(ui, &self.state, &mut self.ui, &self.action_tx);
                });
        }

        if self.ui.right_panel_visible {
            egui::SidePanel::right("right_panel")
                .resizable(true)
                .default_width(self.ui.right_panel_width)
                .min_width(260.0)
                .show(ctx, |ui| {
                    panels::right_panel::show(ui, &self.state, &mut self.ui);
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            panels::chat_panel::show(ui, &self.state, &mut self.ui, &self.action_tx);
        });

        if self.ui.settings_open {
            panels::settings::show(ctx, &self.state, &mut self.ui, &self.action_tx);
        }

        if self.ui.project_picker_open {
            panels::project_picker::show(ctx, &self.state, &mut self.ui, &self.action_tx);
        }
    }
}
