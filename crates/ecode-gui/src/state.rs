//! GUI-side view state.

pub use ecode_desktop_app::AppState;

use ecode_contracts::ids::{TerminalId, ThreadId};
use std::collections::HashMap;

/// UI-only per-thread draft state.
#[derive(Debug, Clone, Default)]
pub struct ThreadDraftUi {
    pub composer_text: String,
    pub chat_scroll_to_bottom: bool,
}

/// UI-side state (not shared with background tasks).
pub struct UiState {
    pub sidebar_width: f32,
    pub sidebar_visible: bool,
    pub terminal_visible: bool,
    pub terminal_height: f32,
    pub right_panel_visible: bool,
    pub right_panel_width: f32,
    pub right_panel_tab: RightPanelTab,
    pub settings_open: bool,
    pub project_picker_open: bool,
    pub sidebar_search: String,
    pub sidebar_filter: SidebarFilter,
    pub thread_drafts: HashMap<ThreadId, ThreadDraftUi>,
    pub active_terminal: Option<TerminalId>,
    pub terminal_inputs: HashMap<TerminalId, String>,
}

impl UiState {
    pub fn thread_draft_mut(&mut self, thread_id: ThreadId) -> &mut ThreadDraftUi {
        self.thread_drafts
            .entry(thread_id)
            .or_insert_with(|| ThreadDraftUi {
                composer_text: String::new(),
                chat_scroll_to_bottom: true,
            })
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            sidebar_width: 260.0,
            sidebar_visible: true,
            terminal_visible: false,
            terminal_height: 250.0,
            right_panel_visible: false,
            right_panel_width: 400.0,
            right_panel_tab: RightPanelTab::Plan,
            settings_open: false,
            project_picker_open: false,
            sidebar_search: String::new(),
            sidebar_filter: SidebarFilter::All,
            thread_drafts: HashMap::new(),
            active_terminal: None,
            terminal_inputs: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightPanelTab {
    Plan,
    Diff,
    Git,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarFilter {
    All,
    Active,
    Done,
}
