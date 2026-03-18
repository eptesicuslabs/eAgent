use crate::dto::{
    OrchestrationCommandInput, OrchestrationSnapshotPayload, parse_approval_id, parse_terminal_id,
    parse_thread_id,
};
use crate::DesktopShellState;
use ecode_desktop_app::UiAction;
use tauri::State;

#[tauri::command]
pub fn orchestration_get_snapshot(
    state: State<'_, DesktopShellState>,
) -> OrchestrationSnapshotPayload {
    OrchestrationSnapshotPayload::from_state(state.app.state().as_ref())
}

#[tauri::command]
pub fn orchestration_dispatch(
    command: OrchestrationCommandInput,
    state: State<'_, DesktopShellState>,
) -> Result<(), String> {
    let action = match command {
        OrchestrationCommandInput::CreateThread { name } => UiAction::CreateThread { name },
        OrchestrationCommandInput::SelectThread { thread_id } => {
            UiAction::SelectThread(parse_thread_id(&thread_id)?)
        }
        OrchestrationCommandInput::DeleteThread { thread_id } => {
            UiAction::DeleteThread(parse_thread_id(&thread_id)?)
        }
        OrchestrationCommandInput::RenameThread { thread_id, name } => UiAction::RenameThread {
            id: parse_thread_id(&thread_id)?,
            name,
        },
        OrchestrationCommandInput::SendMessage { message } => UiAction::SendMessage(message),
        OrchestrationCommandInput::InterruptTurn => UiAction::InterruptTurn,
        OrchestrationCommandInput::UpdateCurrentThreadSettings { settings } => {
            UiAction::UpdateCurrentThreadSettings(settings.into())
        }
        OrchestrationCommandInput::Approve { approval_id } => {
            UiAction::Approve(parse_approval_id(&approval_id)?)
        }
        OrchestrationCommandInput::Deny { approval_id } => {
            UiAction::Deny(parse_approval_id(&approval_id)?)
        }
        OrchestrationCommandInput::UserInputResponse {
            approval_id,
            response,
        } => UiAction::UserInputResponse {
            id: parse_approval_id(&approval_id)?,
            response,
        },
        OrchestrationCommandInput::OpenProject { path } => UiAction::OpenProject(path),
        OrchestrationCommandInput::OpenTerminal => UiAction::OpenTerminal,
        OrchestrationCommandInput::SendTerminalInput { terminal_id, input } => {
            UiAction::SendTerminalInput {
                terminal_id: parse_terminal_id(&terminal_id)?,
                input,
            }
        }
        OrchestrationCommandInput::ResizeTerminal {
            terminal_id,
            cols,
            rows,
        } => UiAction::ResizeTerminal {
            terminal_id: parse_terminal_id(&terminal_id)?,
            cols,
            rows,
        },
        OrchestrationCommandInput::CloseTerminal { terminal_id } => {
            UiAction::CloseTerminal(parse_terminal_id(&terminal_id)?)
        }
        OrchestrationCommandInput::ClearTerminal { terminal_id } => {
            UiAction::ClearTerminal(parse_terminal_id(&terminal_id)?)
        }
    };

    state.app.send(action).map_err(|error| error.to_string())
}
