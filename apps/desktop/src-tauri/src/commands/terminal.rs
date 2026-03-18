use crate::dto::parse_terminal_id;
use crate::DesktopShellState;
use ecode_desktop_app::{TerminalState, UiAction};
use tauri::State;

#[tauri::command]
pub fn terminal_list(state: State<'_, DesktopShellState>) -> Vec<TerminalState> {
    state.app.state().terminals.read().unwrap().clone()
}

#[tauri::command]
pub fn terminal_open(state: State<'_, DesktopShellState>) -> Result<(), String> {
    state
        .app
        .send(UiAction::OpenTerminal)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn terminal_write(
    terminal_id: String,
    input: String,
    state: State<'_, DesktopShellState>,
) -> Result<(), String> {
    state
        .app
        .send(UiAction::SendTerminalInput {
            terminal_id: parse_terminal_id(&terminal_id)?,
            input,
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn terminal_resize(
    terminal_id: String,
    cols: u16,
    rows: u16,
    state: State<'_, DesktopShellState>,
) -> Result<(), String> {
    state
        .app
        .send(UiAction::ResizeTerminal {
            terminal_id: parse_terminal_id(&terminal_id)?,
            cols,
            rows,
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn terminal_close(
    terminal_id: String,
    state: State<'_, DesktopShellState>,
) -> Result<(), String> {
    state
        .app
        .send(UiAction::CloseTerminal(parse_terminal_id(&terminal_id)?))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn terminal_clear(
    terminal_id: String,
    state: State<'_, DesktopShellState>,
) -> Result<(), String> {
    state
        .app
        .send(UiAction::ClearTerminal(parse_terminal_id(&terminal_id)?))
        .map_err(|error| error.to_string())
}
