use eagent_protocol::ids::{TaskGraphId, TaskId};
use thiserror::Error;

/// Errors originating from the runtime engine.
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("task graph not found: {0}")]
    GraphNotFound(TaskGraphId),

    #[error("task not found: {0}")]
    TaskNotFound(TaskId),

    #[error("provider not found: {0}")]
    ProviderNotFound(String),

    #[error("agent spawn failed for task {task_id}: {message}")]
    AgentSpawnFailed { task_id: TaskId, message: String },

    #[error("agent cancelled for task {0}")]
    AgentCancelled(TaskId),

    #[error("persistence error: {0}")]
    Persistence(String),

    #[error("scheduler error: {0}")]
    Scheduler(#[from] SchedulerError),

    #[error("internal error: {0}")]
    Internal(String),
}

/// Errors from the TaskGraph scheduler.
#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("cycle detected in task graph")]
    CycleDetected,

    #[error("dangling dependency: task {dependent} depends on non-existent task {dependency}")]
    DanglingDependency {
        dependency: TaskId,
        dependent: TaskId,
    },

    #[error("empty task graph")]
    EmptyGraph,

    #[error("root task {0} not found in graph nodes")]
    RootTaskMissing(TaskId),
}
