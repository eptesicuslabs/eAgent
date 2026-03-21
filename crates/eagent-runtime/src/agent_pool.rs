use eagent_contracts::provider::ProviderEvent;
use eagent_protocol::ids::{AgentId, TaskId};
use eagent_protocol::messages::AgentMessage;
use eagent_protocol::task_graph::TaskNode;
use eagent_protocol::traits::AgentContext;
use eagent_providers::registry::ProviderRegistry;
use eagent_providers::{ProviderMessage, ProviderMessageRole, SessionConfig};
use eagent_tools::registry::ToolRegistry;
use eagent_tools::ToolContext;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, oneshot};
use tracing;

use crate::error::RuntimeError;

/// Maximum number of tool-call rounds before the agent is forced to stop.
const MAX_TOOL_ROUNDS: usize = 20;

/// Handle to a running agent, used for lifecycle management.
pub struct AgentHandle {
    pub task_id: TaskId,
    pub agent_id: AgentId,
    /// Send a signal to cancel the agent's work.
    pub cancel_tx: Option<oneshot::Sender<()>>,
}

/// Pool of running agents. Spawns worker agents for tasks and manages their
/// lifecycle (including cancellation).
pub struct AgentPool {
    providers: Arc<ProviderRegistry>,
    tools: Arc<ToolRegistry>,
    active_agents: Arc<Mutex<HashMap<TaskId, AgentHandle>>>,
}

impl AgentPool {
    pub fn new(providers: Arc<ProviderRegistry>, tools: Arc<ToolRegistry>) -> Self {
        Self {
            providers,
            tools,
            active_agents: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn a worker agent for a task. Returns a channel that will receive
    /// `AgentMessage`s as the agent progresses, ending with `TaskComplete` or
    /// `TaskFailed`.
    ///
    /// The agent runs a full **agentic tool loop**: it calls the provider,
    /// processes tool call requests by executing them locally, feeds the results
    /// back to the provider, and repeats until the LLM signals completion.
    pub async fn spawn_agent(
        &self,
        task: &TaskNode,
        provider_name: &str,
        ctx: AgentContext,
    ) -> Result<mpsc::UnboundedReceiver<AgentMessage>, RuntimeError> {
        let provider = self
            .providers
            .get(provider_name)
            .ok_or_else(|| RuntimeError::ProviderNotFound(provider_name.to_string()))?
            .clone();

        let task_id = task.id;
        let agent_id = AgentId::new();
        let description = task.description.clone();
        let tools = self.tools.clone();

        // Build tool defs for the allowed tools
        let tool_defs = if task.tools_allowed.is_empty() {
            tools.list_defs()
        } else {
            tools.list_defs_filtered(&task.tools_allowed)
        };

        // Build system prompt
        let system_prompt = format!(
            "You are an AI agent working on a sub-task.\n\
             Workspace: {}\n\
             Project: {}\n\n\
             Your task: {}\n\n\
             Use the available tools to complete this task. Read files to understand \
             the codebase, make edits, run commands, and verify your work.",
            ctx.workspace_root,
            ctx.project_name.as_deref().unwrap_or("unknown"),
            description,
        );

        // Create a provider session
        let session = provider
            .create_session(SessionConfig {
                model: String::new(), // use provider default
                system_prompt: Some(system_prompt.clone()),
                workspace_root: Some(ctx.workspace_root.clone()),
            })
            .await
            .map_err(|e| RuntimeError::AgentSpawnFailed {
                task_id,
                message: format!("session creation failed: {e}"),
            })?;

        // Create the agent message channel
        let (agent_tx, agent_rx) = mpsc::unbounded_channel::<AgentMessage>();

        // Create cancellation channel
        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();

        // Register the agent handle
        {
            let mut agents = self.active_agents.lock().await;
            agents.insert(
                task_id,
                AgentHandle {
                    task_id,
                    agent_id,
                    cancel_tx: Some(cancel_tx),
                },
            );
        }

        let active_agents = self.active_agents.clone();

        // Spawn the agentic tool loop
        tokio::spawn(async move {
            let result = run_agent_loop(
                task_id,
                agent_id,
                &provider,
                &session,
                &tools,
                &tool_defs,
                &ctx,
                &system_prompt,
                &description,
                &agent_tx,
                cancel_rx,
            )
            .await;

            match result {
                Ok(text) => {
                    let _ = agent_tx.send(AgentMessage::TaskComplete {
                        task_id,
                        result: serde_json::Value::String(text),
                        artifacts: vec![],
                    });
                }
                Err(AgentLoopError::Cancelled) => {
                    let _ = agent_tx.send(AgentMessage::TaskFailed {
                        task_id,
                        error: "cancelled".into(),
                        partial_results: None,
                    });
                }
                Err(AgentLoopError::Failed(error, partial)) => {
                    let _ = agent_tx.send(AgentMessage::TaskFailed {
                        task_id,
                        error,
                        partial_results: partial.map(serde_json::Value::String),
                    });
                }
            }

            // Remove from active agents
            let mut agents = active_agents.lock().await;
            agents.remove(&task_id);
            tracing::debug!(?task_id, "agent worker task finished");
        });

        Ok(agent_rx)
    }

    /// Cancel a running agent by sending it a cancellation signal.
    pub async fn cancel_agent(&self, task_id: TaskId) -> Result<(), RuntimeError> {
        let mut agents = self.active_agents.lock().await;
        if let Some(mut handle) = agents.remove(&task_id) {
            if let Some(cancel_tx) = handle.cancel_tx.take() {
                let _ = cancel_tx.send(());
            }
            Ok(())
        } else {
            Err(RuntimeError::TaskNotFound(task_id))
        }
    }

    /// Number of currently active agents.
    pub async fn active_count(&self) -> usize {
        self.active_agents.lock().await.len()
    }
}

// =============================================================================
// Agentic tool loop
// =============================================================================

enum AgentLoopError {
    Cancelled,
    Failed(String, Option<String>),
}

/// Collected tool call from a single provider turn.
struct PendingToolCall {
    id: String,
    name: String,
    params: serde_json::Value,
}

/// Run the full agentic loop: provider call → tool execution → feed back → repeat.
///
/// Returns the accumulated assistant text on success.
async fn run_agent_loop(
    task_id: TaskId,
    agent_id: AgentId,
    provider: &Arc<dyn eagent_providers::Provider>,
    session: &eagent_providers::SessionHandle,
    tools: &Arc<ToolRegistry>,
    tool_defs: &[eagent_tools::ToolDef],
    ctx: &AgentContext,
    system_prompt: &str,
    description: &str,
    agent_tx: &mpsc::UnboundedSender<AgentMessage>,
    mut cancel_rx: oneshot::Receiver<()>,
) -> Result<String, AgentLoopError> {
    // Conversation history — accumulates across turns
    let mut messages: Vec<ProviderMessage> = vec![
        ProviderMessage {
            role: ProviderMessageRole::System,
            content: system_prompt.to_string(),
            tool_call_id: None,
            tool_calls: None,
        },
        ProviderMessage {
            role: ProviderMessageRole::User,
            content: description.to_string(),
            tool_call_id: None,
            tool_calls: None,
        },
    ];

    let mut final_text = String::new();

    for round in 0..MAX_TOOL_ROUNDS {
        tracing::debug!(?task_id, round, "starting tool round");

        // Call the provider
        let mut event_rx = provider
            .send(session, messages.clone(), tool_defs.to_vec())
            .await
            .map_err(|e| AgentLoopError::Failed(
                format!("provider send failed: {e}"),
                if final_text.is_empty() { None } else { Some(final_text.clone()) },
            ))?;

        // Collect this turn's output
        let mut turn_text = String::new();
        let mut pending_tool_calls: Vec<PendingToolCall> = Vec::new();
        let mut completed = false;

        loop {
            tokio::select! {
                _ = &mut cancel_rx => {
                    return Err(AgentLoopError::Cancelled);
                }
                event = event_rx.recv() => {
                    match event {
                        Some(ProviderEvent::TokenDelta { text }) => {
                            turn_text.push_str(&text);
                            let _ = agent_tx.send(AgentMessage::StatusUpdate {
                                task_id,
                                phase: "generating".into(),
                                message: text,
                                progress: None,
                            });
                        }
                        Some(ProviderEvent::ToolCallComplete { id, name, params }) => {
                            let _ = agent_tx.send(AgentMessage::ToolRequest {
                                task_id,
                                request_id: id.clone(),
                                tool_name: name.clone(),
                                params: params.clone(),
                            });
                            pending_tool_calls.push(PendingToolCall { id, name, params });
                        }
                        Some(ProviderEvent::ToolCallStart { .. })
                        | Some(ProviderEvent::ToolCallDelta { .. }) => {
                            // Intermediate streaming; wait for ToolCallComplete
                        }
                        Some(ProviderEvent::Completion { .. }) => {
                            completed = true;
                            break;
                        }
                        Some(ProviderEvent::Error { message }) => {
                            return Err(AgentLoopError::Failed(
                                message,
                                if final_text.is_empty() { None } else { Some(final_text.clone()) },
                            ));
                        }
                        None => {
                            return Err(AgentLoopError::Failed(
                                "provider stream ended unexpectedly".into(),
                                if final_text.is_empty() { None } else { Some(final_text.clone()) },
                            ));
                        }
                    }
                }
            }
        }

        final_text.push_str(&turn_text);

        if !completed {
            return Err(AgentLoopError::Failed(
                "provider did not complete".into(),
                Some(final_text),
            ));
        }

        // If there are no tool calls, the agent is done
        if pending_tool_calls.is_empty() {
            tracing::info!(?task_id, rounds = round + 1, "agent completed");
            return Ok(final_text);
        }

        // Execute tool calls and build the next turn's messages
        // First, add the assistant's response (text + tool calls) to history
        let assistant_tool_calls: Vec<eagent_providers::ProviderToolCall> = pending_tool_calls
            .iter()
            .map(|tc| eagent_providers::ProviderToolCall {
                id: tc.id.clone(),
                name: tc.name.clone(),
                arguments: tc.params.to_string(),
            })
            .collect();
        messages.push(ProviderMessage {
            role: ProviderMessageRole::Assistant,
            content: turn_text,
            tool_call_id: None,
            tool_calls: Some(assistant_tool_calls),
        });

        // Execute each tool call and append results
        for tool_call in &pending_tool_calls {
            let tool_result = if let Some(tool) = tools.get(&tool_call.name) {
                let tool_ctx = ToolContext {
                    workspace_root: ctx.workspace_root.clone(),
                    agent_id,
                    task_id,
                    services: None,
                };
                match tool.execute(tool_call.params.clone(), &tool_ctx).await {
                    Ok(result) => {
                        tracing::debug!(
                            ?task_id, tool = %tool_call.name,
                            "tool executed successfully"
                        );
                        result
                    }
                    Err(e) => {
                        tracing::warn!(
                            ?task_id, tool = %tool_call.name, err = %e,
                            "tool execution failed"
                        );
                        eagent_tools::ToolResult {
                            output: serde_json::json!({"error": e.to_string()}),
                            is_error: true,
                        }
                    }
                }
            } else {
                tracing::warn!(?task_id, tool = %tool_call.name, "tool not found");
                eagent_tools::ToolResult {
                    output: serde_json::json!({"error": format!("tool '{}' not found", tool_call.name)}),
                    is_error: true,
                }
            };

            // Emit tool result to UI
            let _ = agent_tx.send(AgentMessage::StatusUpdate {
                task_id,
                phase: "tool_result".into(),
                message: format!(
                    "{}: {}",
                    tool_call.name,
                    if tool_result.is_error { "error" } else { "ok" }
                ),
                progress: None,
            });

            // Append tool result to conversation for the next provider call
            messages.push(ProviderMessage {
                role: ProviderMessageRole::Tool,
                content: tool_result.output.to_string(),
                tool_call_id: Some(tool_call.id.clone()),
                tool_calls: None,
            });
        }

        tracing::debug!(
            ?task_id,
            round,
            tool_calls = pending_tool_calls.len(),
            "tool round complete, continuing"
        );
    }

    // Hit the max rounds limit — treat as failure, not success
    tracing::warn!(?task_id, MAX_TOOL_ROUNDS, "agent hit max tool rounds");
    Err(AgentLoopError::Failed(
        format!("exceeded max tool rounds ({MAX_TOOL_ROUNDS})"),
        if final_text.is_empty() { None } else { Some(final_text) },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eagent_providers::registry::ProviderRegistry;

    #[test]
    fn agent_pool_creation() {
        let providers = Arc::new(ProviderRegistry::new());
        let tools = Arc::new(ToolRegistry::new());
        let _pool = AgentPool::new(providers, tools);
    }

    #[tokio::test]
    async fn spawn_agent_fails_with_missing_provider() {
        let providers = Arc::new(ProviderRegistry::new());
        let tools = Arc::new(ToolRegistry::new());
        let pool = AgentPool::new(providers, tools);

        let task = TaskNode {
            id: TaskId::new(),
            description: "test task".into(),
            status: eagent_protocol::task_graph::TaskStatus::Ready,
            assigned_agent: None,
            assigned_provider: None,
            tools_allowed: vec![],
            constraints: eagent_protocol::messages::TaskConstraints::default(),
            result: None,
            trace: vec![],
            parent_task_id: None,
            depth: 0,
        };

        let ctx = AgentContext {
            workspace_root: "/tmp".into(),
            project_name: Some("test".into()),
            project_summary: None,
        };

        let result = pool.spawn_agent(&task, "nonexistent", ctx).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            RuntimeError::ProviderNotFound(name) => assert_eq!(name, "nonexistent"),
            other => panic!("expected ProviderNotFound, got: {other}"),
        }
    }

    #[tokio::test]
    async fn active_count_starts_at_zero() {
        let providers = Arc::new(ProviderRegistry::new());
        let tools = Arc::new(ToolRegistry::new());
        let pool = AgentPool::new(providers, tools);
        assert_eq!(pool.active_count().await, 0);
    }

    #[tokio::test]
    async fn cancel_nonexistent_agent_returns_error() {
        let providers = Arc::new(ProviderRegistry::new());
        let tools = Arc::new(ToolRegistry::new());
        let pool = AgentPool::new(providers, tools);
        let result = pool.cancel_agent(TaskId::new()).await;
        assert!(result.is_err());
    }
}
