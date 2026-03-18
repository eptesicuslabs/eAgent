use crate::ids::*;
use crate::task_graph::TaskNode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Risk level for a tool call, used by the oversight system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// Oversight decision made by the human.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OversightDecision {
    Approve,
    Deny,
    Modify { new_instructions: String },
}

/// File mutation kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileMutationKind {
    Create,
    Edit,
    Delete,
}

/// Messages from the harness to an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HarnessMessage {
    TaskAssignment {
        task_id: TaskId,
        description: String,
        context: Value,
        tools_available: Vec<String>,
        constraints: TaskConstraints,
    },
    TaskCancellation {
        task_id: TaskId,
        reason: String,
    },
    OversightResponse {
        request_id: String,
        decision: OversightDecision,
    },
    ContextUpdate {
        task_id: TaskId,
        new_files: Vec<String>,
        new_constraints: Option<TaskConstraints>,
    },
    ToolResult {
        task_id: TaskId,
        request_id: String,
        result: Value,
        is_error: bool,
    },
}

/// Messages from an agent to the harness.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentMessage {
    ToolRequest {
        task_id: TaskId,
        request_id: String,
        tool_name: String,
        params: Value,
    },
    StatusUpdate {
        task_id: TaskId,
        phase: String,
        message: String,
        progress: Option<f32>,
    },
    OversightRequest {
        task_id: TaskId,
        request_id: String,
        action: String,
        context: Value,
        risk_level: RiskLevel,
    },
    SubTaskProposal {
        task_id: TaskId,
        sub_tasks: Vec<TaskNode>,
        edges: Vec<(TaskId, TaskId)>,
    },
    FileMutation {
        task_id: TaskId,
        path: String,
        kind: FileMutationKind,
        content: Option<String>,
        diff: Option<String>,
    },
    TaskComplete {
        task_id: TaskId,
        result: Value,
        artifacts: Vec<String>,
    },
    TaskFailed {
        task_id: TaskId,
        error: String,
        partial_results: Option<Value>,
    },
}

/// Constraints for task execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskConstraints {
    /// Maximum tokens the agent can consume.
    pub max_tokens: Option<u64>,
    /// Maximum tool calls the agent can make.
    pub max_tool_calls: Option<u32>,
    /// Maximum execution time in seconds.
    pub max_time_secs: Option<u64>,
    /// Files the agent is allowed to touch.
    pub allowed_paths: Option<Vec<String>>,
}
