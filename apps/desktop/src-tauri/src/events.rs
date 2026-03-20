use crate::dto::{
    AppStatusPayload, EAgentAgentTracePayload, EAgentTraceEntryPayload,
    task_graph_to_update_payload,
};
use eagent_runtime::engine::{RuntimeEngine, RuntimeEvent};
use ecode_desktop_app::AppState;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

pub const DOMAIN_EVENT: &str = "ecode://domain-event";
pub const TERMINAL_EVENT: &str = "ecode://terminal-event";
pub const SETTINGS_UPDATED_EVENT: &str = "ecode://settings-updated";
pub const APP_STATUS_EVENT: &str = "ecode://app-status";

pub fn emit_domain_event(app_handle: &AppHandle) {
    let _ = app_handle.emit(DOMAIN_EVENT, ());
}

pub fn emit_terminal_event(app_handle: &AppHandle) {
    let _ = app_handle.emit(TERMINAL_EVENT, ());
}

pub fn emit_settings_updated(app_handle: &AppHandle) {
    let _ = app_handle.emit(SETTINGS_UPDATED_EVENT, ());
}

pub fn emit_status_event(app_handle: &AppHandle, state: &Arc<AppState>) {
    let payload = AppStatusPayload {
        status_message: state.status_message.read().unwrap().clone(),
    };
    let _ = app_handle.emit(APP_STATUS_EVENT, payload);
}

// =============================================================================
// eAgent event bridge — RuntimeEvent → Tauri events for React frontend
// =============================================================================

pub const EAGENT_TASK_GRAPH_UPDATE: &str = "eagent://task-graph-update";
pub const EAGENT_AGENT_TRACE: &str = "eagent://agent-trace";

/// Async loop that reads RuntimeEvents and emits Tauri events for the React
/// frontend. Should be spawned via `tokio::spawn` during Tauri setup.
pub async fn eagent_event_bridge(
    mut event_rx: mpsc::UnboundedReceiver<RuntimeEvent>,
    engine: Arc<RuntimeEngine>,
    app_handle: AppHandle,
) {
    tracing::info!("eAgent event bridge started");

    while let Some(event) = event_rx.recv().await {
        match &event {
            // Graph lifecycle events — send full graph snapshot
            RuntimeEvent::GraphCreated { graph_id }
            | RuntimeEvent::TaskScheduled { graph_id, .. }
            | RuntimeEvent::TaskStarted { graph_id, .. }
            | RuntimeEvent::TaskCompleted { graph_id, .. }
            | RuntimeEvent::TaskFailed { graph_id, .. }
            | RuntimeEvent::GraphCompleted { graph_id } => {
                let graph_id = *graph_id;
                if let Some(graph) = engine.get_graph(graph_id).await {
                    let payload = task_graph_to_update_payload(&graph);
                    let _ = app_handle.emit(EAGENT_TASK_GRAPH_UPDATE, &payload);
                }
            }

            // Agent messages — convert to trace entries
            RuntimeEvent::AgentMessage {
                graph_id,
                task_id,
                message,
            } => {
                let now = chrono::Utc::now().to_rfc3339();
                let (kind, summary, detail, tool_name) = match message {
                    eagent_protocol::messages::AgentMessage::StatusUpdate {
                        message: msg, ..
                    } => ("status", msg.clone(), None, None),
                    eagent_protocol::messages::AgentMessage::ToolRequest {
                        tool_name,
                        params,
                        ..
                    } => (
                        "tool-call",
                        format!("Calling {tool_name}"),
                        Some(params.to_string()),
                        Some(tool_name.clone()),
                    ),
                    eagent_protocol::messages::AgentMessage::TaskComplete { .. } => {
                        ("status", "Task completed".into(), None, None)
                    }
                    eagent_protocol::messages::AgentMessage::TaskFailed { error, .. } => {
                        ("error", error.clone(), None, None)
                    }
                    _ => ("status", format!("{message:?}"), None, None),
                };

                let trace = EAgentAgentTracePayload {
                    graph_id: graph_id.to_string(),
                    task_id: task_id.to_string(),
                    agent_id: "worker".into(),
                    entry: EAgentTraceEntryPayload {
                        id: uuid::Uuid::new_v4().to_string(),
                        kind: kind.into(),
                        agent_id: Some("worker".into()),
                        timestamp: now,
                        summary,
                        detail,
                        tool_name,
                        file_path: None,
                    },
                };
                let _ = app_handle.emit(EAGENT_AGENT_TRACE, &trace);
            }

            // Tool results — also as trace entries
            RuntimeEvent::ToolResult {
                graph_id,
                task_id,
                tool_name,
                result,
            } => {
                let now = chrono::Utc::now().to_rfc3339();
                let summary = if result.is_error {
                    format!("{tool_name} failed")
                } else {
                    format!("{tool_name} completed")
                };
                let trace = EAgentAgentTracePayload {
                    graph_id: graph_id.to_string(),
                    task_id: task_id.to_string(),
                    agent_id: "worker".into(),
                    entry: EAgentTraceEntryPayload {
                        id: uuid::Uuid::new_v4().to_string(),
                        kind: "tool-result".into(),
                        agent_id: Some("worker".into()),
                        timestamp: now,
                        summary,
                        detail: Some(result.output.to_string()),
                        tool_name: Some(tool_name.clone()),
                        file_path: None,
                    },
                };
                let _ = app_handle.emit(EAGENT_AGENT_TRACE, &trace);
            }
        }
    }

    tracing::info!("eAgent event bridge stopped");
}
