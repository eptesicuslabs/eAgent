use crate::ids::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// A directed acyclic graph of tasks decomposed from a user request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    pub id: TaskGraphId,
    pub root_task_id: TaskId,
    pub user_prompt: String,
    pub nodes: HashMap<TaskId, TaskNode>,
    pub edges: Vec<(TaskId, TaskId)>, // (dependency, dependent)
}

/// A single task node within a TaskGraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    pub id: TaskId,
    pub description: String,
    pub status: TaskStatus,
    pub assigned_agent: Option<AgentId>,
    pub assigned_provider: Option<ProviderId>,
    pub tools_allowed: Vec<String>,
    pub constraints: crate::messages::TaskConstraints,
    pub result: Option<Value>,
    pub trace: Vec<TraceEntry>,
    /// Parent task that spawned this node (None for root-level tasks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<TaskId>,
    /// Current depth in the recursive tree (0 = root level).
    #[serde(default)]
    pub depth: u32,
}

/// Status of a task within the graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Ready,
    Scheduled,
    Running,
    AwaitingReview,
    Complete,
    Failed { error: String, retries: u32 },
    Cancelled { reason: String },
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// A trace entry recording agent execution activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub kind: TraceEntryKind,
}

/// The kind of trace entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceEntryKind {
    Thinking { content: String },
    ToolCall { tool_name: String, params: Value, result: Option<Value> },
    FileChange { path: String, diff: String },
    TerminalOutput { terminal_id: TerminalId, data: String },
    StatusMessage { message: String },
    Error { message: String },
    OversightRequested { request_id: String, action: String },
    OversightResolved { request_id: String, decision: String },
}
