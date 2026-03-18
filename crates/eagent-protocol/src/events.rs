use crate::ids::*;
use crate::messages::OversightDecision;
use crate::task_graph::{TaskNode, TraceEntry};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Events that record TaskGraph state changes for persistence and replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TaskGraphEvent {
    GraphCreated {
        graph_id: TaskGraphId,
        root_task: TaskNode,
        user_prompt: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    SubTasksProposed {
        graph_id: TaskGraphId,
        nodes: Vec<TaskNode>,
        edges: Vec<(TaskId, TaskId)>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    PlanApproved {
        graph_id: TaskGraphId,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    TaskScheduled {
        graph_id: TaskGraphId,
        task_id: TaskId,
        agent_id: AgentId,
        provider_id: ProviderId,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    TaskStarted {
        graph_id: TaskGraphId,
        task_id: TaskId,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    TraceAppended {
        graph_id: TaskGraphId,
        task_id: TaskId,
        entry: TraceEntry,
    },
    FileMutationRecorded {
        graph_id: TaskGraphId,
        task_id: TaskId,
        path: String,
        diff: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    OversightRequested {
        graph_id: TaskGraphId,
        task_id: TaskId,
        request_id: String,
        action: String,
        context: Value,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    OversightResolved {
        graph_id: TaskGraphId,
        task_id: TaskId,
        request_id: String,
        decision: OversightDecision,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    TaskCompleted {
        graph_id: TaskGraphId,
        task_id: TaskId,
        result: Value,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    TaskFailed {
        graph_id: TaskGraphId,
        task_id: TaskId,
        error: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    TaskCancelled {
        graph_id: TaskGraphId,
        task_id: TaskId,
        reason: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    GraphCompleted {
        graph_id: TaskGraphId,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

impl TaskGraphEvent {
    pub fn graph_id(&self) -> TaskGraphId {
        match self {
            Self::GraphCreated { graph_id, .. }
            | Self::SubTasksProposed { graph_id, .. }
            | Self::PlanApproved { graph_id, .. }
            | Self::TaskScheduled { graph_id, .. }
            | Self::TaskStarted { graph_id, .. }
            | Self::TraceAppended { graph_id, .. }
            | Self::FileMutationRecorded { graph_id, .. }
            | Self::OversightRequested { graph_id, .. }
            | Self::OversightResolved { graph_id, .. }
            | Self::TaskCompleted { graph_id, .. }
            | Self::TaskFailed { graph_id, .. }
            | Self::TaskCancelled { graph_id, .. }
            | Self::GraphCompleted { graph_id, .. } => *graph_id,
        }
    }

    pub fn stream_id(&self) -> String {
        format!("graph:{}", self.graph_id())
    }
}
