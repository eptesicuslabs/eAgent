use crate::{DesktopShellState, EAgentState};
use eagent_planner::llm::LlmPlanner;
use eagent_planner::simple::SimplePlanner;
use eagent_protocol::ids::TaskGraphId;
use eagent_providers::api_key::{ApiKeyConfig, ApiKeyProvider};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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

/// Submit a user prompt to eAgent, creating a task graph via the SimplePlanner
/// and submitting it to the RuntimeEngine for execution.
#[tauri::command]
pub async fn eagent_submit_task(
    prompt: String,
    state: State<'_, DesktopShellState>,
    eagent: State<'_, EAgentState>,
) -> Result<TaskGraphResponse, String> {
    let workspace_root = state
        .app
        .state()
        .current_project
        .read()
        .unwrap()
        .clone()
        .ok_or_else(|| "no project open — open a folder before submitting tasks".to_string())?;

    // Use LLM planner when a provider is available, fall back to SimplePlanner
    let graph = if !eagent.provider_registry.is_empty() {
        let planner_provider = eagent
            .provider_registry
            .names()
            .into_iter()
            .next()
            .unwrap_or_else(|| "api-key".into());
        let llm_planner = LlmPlanner::new(
            eagent.provider_registry.clone(),
            planner_provider,
        );
        match llm_planner.plan(&prompt, &workspace_root, None).await {
            Ok(graph) => graph,
            Err(e) => {
                tracing::warn!(err = %e, "LLM planner failed, falling back to SimplePlanner");
                SimplePlanner::new().plan(&prompt, &workspace_root)
            }
        }
    } else {
        SimplePlanner::new().plan(&prompt, &workspace_root)
    };

    let graph_id = graph.id.to_string();
    let root_task_id = graph.root_task_id.to_string();

    // Submit to RuntimeEngine for execution
    eagent
        .engine
        .submit(graph)
        .await
        .map_err(|e| format!("failed to submit task graph: {e}"))?;

    Ok(TaskGraphResponse {
        graph_id,
        root_task_id,
        status: "submitted".into(),
    })
}

/// Cancel a running task graph by ID.
#[tauri::command]
pub async fn eagent_cancel_graph(
    graph_id: String,
    eagent: State<'_, EAgentState>,
) -> Result<(), String> {
    let id = TaskGraphId::parse(&graph_id)
        .map_err(|e| format!("invalid graph_id: {e}"))?;
    eagent
        .engine
        .cancel_graph(id)
        .await
        .map_err(|e| format!("failed to cancel graph: {e}"))
}

/// List the configured LLM providers.
#[tauri::command]
pub async fn eagent_get_providers(
    eagent: State<'_, EAgentState>,
) -> Result<Vec<ProviderInfo>, String> {
    let names = eagent.provider_registry.names();
    Ok(names
        .into_iter()
        .map(|name| ProviderInfo {
            id: name.clone(),
            name: name.clone(),
            kind: "api-key".into(),
            is_available: true,
        })
        .collect())
}

/// Approve a pending oversight request.
#[tauri::command]
pub async fn eagent_approve_oversight(
    _request_id: String,
    _eagent: State<'_, EAgentState>,
) -> Result<(), String> {
    // TODO: forward to the oversight channel in RuntimeEngine
    Ok(())
}

/// Deny a pending oversight request.
#[tauri::command]
pub async fn eagent_deny_oversight(
    _request_id: String,
    _eagent: State<'_, EAgentState>,
) -> Result<(), String> {
    // TODO: forward to the oversight channel in RuntimeEngine
    Ok(())
}

/// Input for configuring an API provider at runtime.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureProviderInput {
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
    pub name: Option<String>,
}

/// Configure an OpenAI-compatible API provider at runtime.
/// Supports NVIDIA NIM, OpenAI, Anthropic-compatible, Together, etc.
#[tauri::command]
pub async fn eagent_configure_provider(
    input: ConfigureProviderInput,
    eagent: State<'_, EAgentState>,
) -> Result<ProviderInfo, String> {
    let provider_name = input.name.unwrap_or_else(|| "api-key".into());

    let config = ApiKeyConfig {
        endpoint: input.endpoint,
        api_key: input.api_key,
        default_model: input.model,
        ..Default::default()
    };

    eagent.provider_registry.register(
        provider_name.clone(),
        Arc::new(ApiKeyProvider::new(config)),
    );

    tracing::info!(provider = %provider_name, "provider configured via settings");

    Ok(ProviderInfo {
        id: provider_name.clone(),
        name: provider_name,
        kind: "api-key".into(),
        is_available: true,
    })
}
