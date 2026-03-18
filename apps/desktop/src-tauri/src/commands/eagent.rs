use crate::DesktopShellState;
use eagent_planner::simple::SimplePlanner;
use serde::Serialize;
use tauri::State;

/// Response payload returned after submitting a task to the planner.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGraphResponse {
    pub graph_id: String,
    pub root_task_id: String,
    pub status: String,
}

/// Summary information about a configured LLM provider.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub is_available: bool,
}

/// Submit a user prompt to eAgent, creating a task graph via the SimplePlanner.
#[tauri::command]
pub async fn eagent_submit_task(
    prompt: String,
    state: State<'_, DesktopShellState>,
) -> Result<TaskGraphResponse, String> {
    let workspace_root = state
        .app
        .state()
        .current_project
        .read()
        .unwrap()
        .clone()
        .unwrap_or_default();

    let planner = SimplePlanner::new();
    let graph = planner.plan(&prompt, &workspace_root);

    Ok(TaskGraphResponse {
        graph_id: graph.id.to_string(),
        root_task_id: graph.root_task_id.to_string(),
        status: "planned".into(),
    })
}

/// Cancel a running task graph by ID.
#[tauri::command]
pub async fn eagent_cancel_graph(
    _graph_id: String,
    _state: State<'_, DesktopShellState>,
) -> Result<(), String> {
    // TODO: wire up to RuntimeEngine once it tracks active graphs
    Ok(())
}

/// List the configured LLM providers.
#[tauri::command]
pub async fn eagent_get_providers(
    _state: State<'_, DesktopShellState>,
) -> Result<Vec<ProviderInfo>, String> {
    // TODO: read from provider registry once wired
    Ok(vec![])
}

/// Approve a pending oversight request.
#[tauri::command]
pub async fn eagent_approve_oversight(
    _request_id: String,
    _state: State<'_, DesktopShellState>,
) -> Result<(), String> {
    // TODO: forward to the oversight channel in RuntimeEngine
    Ok(())
}

/// Deny a pending oversight request.
#[tauri::command]
pub async fn eagent_deny_oversight(
    _request_id: String,
    _state: State<'_, DesktopShellState>,
) -> Result<(), String> {
    // TODO: forward to the oversight channel in RuntimeEngine
    Ok(())
}
