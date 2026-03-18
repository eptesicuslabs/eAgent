use eagent_contracts::provider::ProviderEvent;
use eagent_protocol::ids::{AgentId, TaskId};
use eagent_protocol::messages::AgentMessage;
use eagent_protocol::task_graph::TaskNode;
use eagent_protocol::traits::AgentContext;
use eagent_providers::registry::ProviderRegistry;
use eagent_providers::{ProviderMessage, ProviderMessageRole, SessionConfig};
use eagent_tools::registry::ToolRegistry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, oneshot};
use tracing;

use crate::error::RuntimeError;

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
    /// The method:
    /// 1. Gets the provider from the registry
    /// 2. Creates a session with the provider
    /// 3. Builds the system prompt + messages from the task description
    /// 4. Calls provider.send() to start streaming
    /// 5. Spawns a tokio task that reads ProviderEvents and translates them to AgentMessages
    /// 6. Returns the AgentMessage receiver
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

        // Build tool defs for the allowed tools
        let tool_defs = if task.tools_allowed.is_empty() {
            self.tools.list_defs()
        } else {
            self.tools.list_defs_filtered(&task.tools_allowed)
        };

        // Build system prompt
        let system_prompt = format!(
            "You are an AI agent working on a sub-task.\n\
             Workspace: {}\n\
             Project: {}\n\n\
             Your task: {}",
            ctx.workspace_root,
            ctx.project_name.as_deref().unwrap_or("unknown"),
            description,
        );

        // Create a provider session
        let session = provider
            .create_session(SessionConfig {
                model: String::new(), // use provider default
                system_prompt: Some(system_prompt),
                workspace_root: Some(ctx.workspace_root.clone()),
            })
            .await
            .map_err(|e| RuntimeError::AgentSpawnFailed {
                task_id,
                message: format!("session creation failed: {e}"),
            })?;

        // Build messages
        let messages = vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: description.clone(),
        }];

        // Send to provider and get the event receiver
        let mut event_rx = provider
            .send(&session, messages, tool_defs)
            .await
            .map_err(|e| RuntimeError::AgentSpawnFailed {
                task_id,
                message: format!("provider send failed: {e}"),
            })?;

        // Create the agent message channel
        let (agent_tx, agent_rx) = mpsc::unbounded_channel::<AgentMessage>();

        // Create cancellation channel
        let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();

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

        // Spawn a background task that translates ProviderEvents into AgentMessages
        tokio::spawn(async move {
            let mut accumulated_text = String::new();

            loop {
                tokio::select! {
                    // Check for cancellation
                    _ = &mut cancel_rx => {
                        let _ = agent_tx.send(AgentMessage::TaskFailed {
                            task_id,
                            error: "cancelled".into(),
                            partial_results: None,
                        });
                        break;
                    }
                    // Read provider events
                    event = event_rx.recv() => {
                        match event {
                            Some(provider_event) => {
                                match provider_event {
                                    ProviderEvent::TokenDelta { text } => {
                                        accumulated_text.push_str(&text);
                                        let _ = agent_tx.send(AgentMessage::StatusUpdate {
                                            task_id,
                                            phase: "generating".into(),
                                            message: text,
                                            progress: None,
                                        });
                                    }
                                    ProviderEvent::ToolCallComplete { id, name, params } => {
                                        let _ = agent_tx.send(AgentMessage::ToolRequest {
                                            task_id,
                                            request_id: id,
                                            tool_name: name,
                                            params,
                                        });
                                    }
                                    ProviderEvent::ToolCallStart { .. }
                                    | ProviderEvent::ToolCallDelta { .. } => {
                                        // Intermediate streaming events; we wait for ToolCallComplete
                                    }
                                    ProviderEvent::Completion { .. } => {
                                        let _ = agent_tx.send(AgentMessage::TaskComplete {
                                            task_id,
                                            result: serde_json::Value::String(accumulated_text.clone()),
                                            artifacts: vec![],
                                        });
                                        // Normal completion — exit the loop
                                        break;
                                    }
                                    ProviderEvent::Error { message } => {
                                        let _ = agent_tx.send(AgentMessage::TaskFailed {
                                            task_id,
                                            error: message,
                                            partial_results: if accumulated_text.is_empty() {
                                                None
                                            } else {
                                                Some(serde_json::Value::String(accumulated_text.clone()))
                                            },
                                        });
                                        break;
                                    }
                                }
                            }
                            None => {
                                // Provider stream ended without a Completion event
                                let _ = agent_tx.send(AgentMessage::TaskFailed {
                                    task_id,
                                    error: "provider stream ended unexpectedly".into(),
                                    partial_results: if accumulated_text.is_empty() {
                                        None
                                    } else {
                                        Some(serde_json::Value::String(accumulated_text.clone()))
                                    },
                                });
                                break;
                            }
                        }
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
