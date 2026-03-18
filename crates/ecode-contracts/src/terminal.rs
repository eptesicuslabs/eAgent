//! Terminal types.

use crate::ids::TerminalId;
use serde::{Deserialize, Serialize};

/// Terminal event sent from the backend to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminalEvent {
    /// Terminal output data (raw bytes as string).
    Output {
        terminal_id: TerminalId,
        data: String,
    },
    /// Terminal exited.
    Exited {
        terminal_id: TerminalId,
        exit_code: Option<u32>,
    },
    /// Terminal resized.
    Resized {
        terminal_id: TerminalId,
        cols: u16,
        rows: u16,
    },
}

/// Configuration for spawning a terminal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// Working directory for the terminal.
    pub cwd: String,
    /// Shell override (None = use system default).
    pub shell: Option<String>,
    /// Initial columns.
    pub cols: u16,
    /// Initial rows.
    pub rows: u16,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            cwd: String::new(),
            shell: None,
            cols: 120,
            rows: 30,
        }
    }
}
