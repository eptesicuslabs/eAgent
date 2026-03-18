use crate::ids::*;
use crate::messages::{AgentMessage, HarnessMessage, TaskConstraints};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::mpsc;

/// Bidirectional channel between an agent and the harness.
pub struct AgentChannel {
    /// Agent sends protocol messages to the harness.
    pub tx: mpsc::UnboundedSender<AgentMessage>,
    /// Agent receives oversight responses, context updates, tool results from the harness.
    pub rx: mpsc::UnboundedReceiver<HarnessMessage>,
}

/// Context provided to an agent when executing a task.
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// Workspace root directory.
    pub workspace_root: String,
    /// Project name.
    pub project_name: Option<String>,
    /// Project summary from the index (file tree, key symbols, etc.).
    pub project_summary: Option<String>,
}

/// The assignment given to an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignment {
    pub task_id: TaskId,
    pub graph_id: TaskGraphId,
    pub description: String,
    pub context: Value,
    pub tools_available: Vec<String>,
    pub constraints: TaskConstraints,
    pub system_prompt: Option<String>,
}

/// Result of a completed task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub summary: String,
    pub artifacts: Vec<String>,
    pub data: Option<Value>,
}

/// Error during agent execution.
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("provider error: {0}")]
    Provider(String),
    #[error("tool execution failed: {tool} — {message}")]
    ToolFailed { tool: String, message: String },
    #[error("cancelled: {0}")]
    Cancelled(String),
    #[error("budget exceeded: {0}")]
    BudgetExceeded(String),
    #[error("internal: {0}")]
    Internal(String),
}

/// The Agent trait — the runtime's unit of execution.
/// An agent receives a task assignment, communicates with the harness
/// via an AgentChannel, and returns a TaskResult on completion.
pub trait Agent: Send + Sync {
    /// Execute a task. The agent communicates with the harness via the channel
    /// (emitting StatusUpdates, ToolRequests, OversightRequests) and returns
    /// the final result when done.
    fn execute(
        &self,
        task: TaskAssignment,
        channel: AgentChannel,
        ctx: AgentContext,
    ) -> impl std::future::Future<Output = Result<TaskResult, AgentError>> + Send;

    /// Request cancellation of the currently running task.
    fn cancel(&self);
}
