//! Left sidebar — thread list and management.

use crate::state::{AppState, SidebarFilter, UiState};
use crate::theme::{R_MD, R_SM, SP_LG, SP_SM, SP_XS, colors};
use crate::widgets::shared::segmented_control;
use ecode_desktop_app::UiAction;
use ecode_contracts::orchestration::TurnStatus;
use std::sync::Arc;
use tokio::sync::mpsc;

pub fn show(
    ui: &mut egui::Ui,
    state: &Arc<AppState>,
    ui_state: &mut UiState,
    action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, SP_XS);

        // ── New Thread — primary action ───────────────────────────────────────
        ui.add_space(SP_SM);
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("+ New Thread")
                        .strong()
                        .size(13.0)
                        .color(colors::ACCENT),
                )
                .fill(colors::SURFACE_2)
                .stroke(egui::Stroke::new(1.0, colors::BORDER_DEFAULT))
                .corner_radius(egui::CornerRadius::same(R_MD))
                .min_size(egui::vec2(ui.available_width(), 34.0)),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            let title = format!("Thread {}", chrono::Local::now().format("%H:%M"));
            let _ = action_tx.send(UiAction::CreateThread { name: title });
        }

        ui.add_space(SP_XS);

        // ── Search ─────────────────────────────────────────────────────────────
        egui::Frame::new()
            .fill(colors::SURFACE_1)
            .stroke(egui::Stroke::new(1.0, colors::BORDER_SUBTLE))
            .corner_radius(egui::CornerRadius::same(R_SM))
            .inner_margin(egui::Margin::symmetric(SP_SM as i8, (SP_XS + 2.0) as i8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("⌕")
                            .size(13.0)
                            .color(colors::TEXT_DISABLED),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut ui_state.sidebar_search)
                            .hint_text("Search threads…")
                            .desired_width(f32::INFINITY)
                            .frame(false),
                    );
                });
            });

        ui.add_space(SP_XS);

        // ── Filter tabs ────────────────────────────────────────────────────────
        // Map SidebarFilter to an index so SegmentedControl can work with Copy types.
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum FilterIdx {
            All,
            Active,
            Done,
        }
        let mut idx = match ui_state.sidebar_filter {
            SidebarFilter::All => FilterIdx::All,
            SidebarFilter::Active => FilterIdx::Active,
            SidebarFilter::Done => FilterIdx::Done,
        };
        let options: &[(&str, FilterIdx)] =
            &[("All", FilterIdx::All), ("Active", FilterIdx::Active), ("Done", FilterIdx::Done)];
        if segmented_control(ui, egui::Id::new("sidebar-filter"), options, &mut idx) {
            ui_state.sidebar_filter = match idx {
                FilterIdx::All => SidebarFilter::All,
                FilterIdx::Active => SidebarFilter::Active,
                FilterIdx::Done => SidebarFilter::Done,
            };
        }

        ui.add_space(SP_SM);

        // ── Thread list ────────────────────────────────────────────────────────
        let model = state.read_model.read().unwrap();
        let current_thread = *state.current_thread.read().unwrap();
        let search_lower = ui_state.sidebar_search.to_lowercase();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, SP_XS - 2.0);
            let mut threads: Vec<_> = model.threads.iter().collect();
            threads.sort_by(|a, b| b.1.created_at.cmp(&a.1.created_at));

            for (thread_id, thread_state) in &threads {
                // Search filter
                if !search_lower.is_empty()
                    && !thread_state.name.to_lowercase().contains(&search_lower)
                {
                    continue;
                }

                // Status filter
                let is_active = thread_state.turns.iter().any(|t| {
                    matches!(
                        t.status,
                        TurnStatus::Requested | TurnStatus::Running | TurnStatus::Waiting
                    )
                });
                match ui_state.sidebar_filter {
                    SidebarFilter::Active if !is_active => continue,
                    SidebarFilter::Done if is_active => continue,
                    _ => {}
                }

                let is_selected = current_thread.as_ref() == Some(thread_id);

                // Thread item: left accent bar + name + meta row
                let _item_id = egui::Id::new(("sidebar-thread", thread_id));
                let response = ui
                    .scope(|ui| {
                        // Background frame for the item
                        let rect = ui.available_rect_before_wrap();
                        let item_height = 48.0;
                        let (item_rect, item_resp) = ui.allocate_exact_size(
                            egui::vec2(rect.width(), item_height),
                            egui::Sense::click(),
                        );

                        if ui.is_rect_visible(item_rect) {
                            let fill = if is_selected {
                                colors::SURFACE_2
                            } else if item_resp.hovered() {
                                colors::SURFACE_1
                            } else {
                                egui::Color32::TRANSPARENT
                            };
                            let rounding = egui::CornerRadius::same(R_SM);
                            let painter = ui.painter();
                            painter.rect_filled(item_rect, rounding, fill);

                            // Left accent bar for the selected item
                            if is_selected {
                                painter.rect_filled(
                                    egui::Rect::from_min_size(
                                        item_rect.min,
                                        egui::vec2(3.0, item_height),
                                    ),
                                    egui::CornerRadius::same(2),
                                    colors::ACCENT,
                                );
                            }

                            // Thread name
                            let name_rect = egui::Rect::from_min_size(
                                item_rect.min + egui::vec2(SP_SM + 2.0, SP_XS + 2.0),
                                egui::vec2(item_rect.width() - SP_LG, 22.0),
                            );
                            painter.text(
                                name_rect.min,
                                egui::Align2::LEFT_TOP,
                                &thread_state.name,
                                egui::FontId::new(13.0, egui::FontFamily::Proportional),
                                if is_selected {
                                    colors::TEXT_PRIMARY
                                } else {
                                    colors::TEXT_SECONDARY
                                },
                            );

                            // Meta row: timestamp
                            let ts = thread_state
                                .updated_at
                                .with_timezone(&chrono::Local)
                                .format("%H:%M")
                                .to_string();
                            let status_text = if is_active { "●  ".to_string() + &ts } else { ts };
                            let status_color =
                                if is_active { colors::STATUS_RUNNING_TEXT } else { colors::TEXT_DISABLED };
                            painter.text(
                                item_rect.min + egui::vec2(SP_SM + 2.0, 28.0),
                                egui::Align2::LEFT_TOP,
                                status_text,
                                egui::FontId::new(11.0, egui::FontFamily::Proportional),
                                status_color,
                            );
                        }

                        item_resp
                    })
                    .inner;

                if response.clicked() {
                    let _ = action_tx.send(UiAction::SelectThread(**thread_id));
                }

                // Context menu
                response.context_menu(|ui| {
                    if ui.button("Rename…").clicked() {
                        let _ = action_tx.send(UiAction::RenameThread {
                            id: **thread_id,
                            name: format!("{} (renamed)", thread_state.name),
                        });
                        ui.close_menu();
                    }
                    if ui
                        .add(egui::Button::new("Delete").fill(colors::ACTION_DESTRUCTIVE_FILL))
                        .clicked()
                    {
                        let _ = action_tx.send(UiAction::DeleteThread(**thread_id));
                        ui.close_menu();
                    }
                });
            }
        });
    });
}
