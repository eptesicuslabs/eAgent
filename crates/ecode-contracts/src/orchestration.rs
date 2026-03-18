//! Orchestration domain types — commands, events, and read model.
//!
//! Follows the CQRS/Event Sourcing pattern:
//! - Commands express intent
//! - Events record what happened
//! - The ReadModel is built from events

use crate::codex::{ApprovalDecision, SandboxMode};
use crate::ids::*;
use crate::provider::{DEFAULT_CODEX_MODEL, ProviderKind};
use crate::provider_runtime::{
    ProviderRuntimeEvent, ProviderRuntimeEventKind, ProviderSessionStatus,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// ─── Thread Settings ────────────────────────────────────────────────

/// The interaction mode for a conversation turn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionMode {
    /// Agent executes autonomously.
    #[default]
    Chat,
    /// Agent plans first, then executes on approval.
    Plan,
}

/// Runtime safety mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeMode {
    /// Agent needs approval for file reads, writes, commands.
    #[default]
    ApprovalRequired,
    /// Agent has full access without approval gates.
    FullAccess,
}

impl RuntimeMode {
    /// Convert to the Codex CLI sandbox mode.
    pub fn to_sandbox_mode(self) -> SandboxMode {
        match self {
            RuntimeMode::ApprovalRequired => SandboxMode::WorkspaceWrite,
            RuntimeMode::FullAccess => SandboxMode::DangerFullAccess,
        }
    }
}

/// Codex reasoning effort stored with a thread.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodexReasoningEffort {
    Low,
    #[default]
    Medium,
    High,
}

/// Persisted thread-scoped settings used as the source of truth for turns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadSettings {
    pub provider: ProviderKind,
    pub model: String,
    pub runtime_mode: RuntimeMode,
    pub interaction_mode: InteractionMode,
    pub codex_reasoning_effort: CodexReasoningEffort,
    pub codex_fast_mode: bool,
    pub local_agent_web_search_enabled: bool,
}

impl Default for ThreadSettings {
    fn default() -> Self {
        Self {
            provider: ProviderKind::Codex,
            model: DEFAULT_CODEX_MODEL.to_string(),
            runtime_mode: RuntimeMode::default(),
            interaction_mode: InteractionMode::default(),
            codex_reasoning_effort: CodexReasoningEffort::default(),
            codex_fast_mode: false,
            local_agent_web_search_enabled: false,
        }
    }
}

// ─── Commands ───────────────────────────────────────────────────────

/// Commands that can be dispatched to the orchestration engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Command {
    /// Create a new thread.
    CreateThread {
        thread_id: ThreadId,
        project_id: ProjectId,
        name: Option<String>,
        settings: ThreadSettings,
    },
    /// Update the persisted settings for a thread.
    UpdateThreadSettings {
        thread_id: ThreadId,
        settings: ThreadSettings,
    },
    /// Start a new turn in an existing thread using that thread's settings snapshot.
    StartTurn {
        thread_id: ThreadId,
        turn_id: TurnId,
        input: String,
        images: Vec<String>,
    },
    /// Interrupt the currently active turn.
    InterruptTurn {
        thread_id: ThreadId,
        turn_id: TurnId,
    },
    /// Rollback N turns.
    RollbackTurns { thread_id: ThreadId, n: u32 },
    /// Delete a thread and all its data.
    DeleteThread { thread_id: ThreadId },
    /// Rename a thread.
    RenameThread { thread_id: ThreadId, name: String },

    // ── Internal commands (dispatched by reactor) ──
    /// Record that a provider session has been established.
    SetSession {
        thread_id: ThreadId,
        session_id: SessionId,
        provider: ProviderKind,
        provider_thread_id: String,
    },
    /// Clear any persisted provider session for the thread.
    ClearSession { thread_id: ThreadId },
    /// Update the normalized session status.
    SetSessionStatus {
        thread_id: ThreadId,
        status: ProviderSessionStatus,
        message: Option<String>,
    },
    /// Record that the provider confirmed a turn has started.
    RecordTurnStarted {
        thread_id: ThreadId,
        turn_id: TurnId,
        provider_turn_id: String,
    },
    /// Record a normalized runtime event.
    RecordRuntimeEvent {
        thread_id: ThreadId,
        turn_id: Option<TurnId>,
        event_type: ProviderRuntimeEventKind,
        item_id: Option<String>,
        request_id: Option<ApprovalRequestId>,
        summary: Option<String>,
        data: Value,
    },
    /// Record an assistant message delta.
    AppendAssistantDelta {
        thread_id: ThreadId,
        turn_id: TurnId,
        item_id: String,
        delta: String,
    },
    /// Record that a turn has completed.
    CompleteTurn {
        thread_id: ThreadId,
        turn_id: TurnId,
        status: String,
    },
    /// Record an approval request.
    RecordApprovalRequest {
        thread_id: ThreadId,
        turn_id: TurnId,
        approval_id: ApprovalRequestId,
        rpc_id: u64,
        kind: ApprovalKind,
        details: Value,
    },
    /// Record an approval response.
    RecordApprovalResponse {
        thread_id: ThreadId,
        approval_id: ApprovalRequestId,
        decision: ApprovalDecision,
    },
    /// Record user input request.
    RecordUserInputRequest {
        thread_id: ThreadId,
        turn_id: TurnId,
        approval_id: ApprovalRequestId,
        rpc_id: u64,
        questions: Option<Value>,
    },
    /// Record user input response.
    RecordUserInputResponse {
        thread_id: ThreadId,
        approval_id: ApprovalRequestId,
        answers: Value,
    },
    /// Record a provider/runtime error.
    RecordError {
        thread_id: ThreadId,
        message: String,
        will_retry: bool,
    },
}

/// Kind of approval request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalKind {
    CommandExecution,
    FileChange,
    FileRead,
}

// ─── Events ─────────────────────────────────────────────────────────

/// Domain events emitted by the orchestration engine.
/// These are persisted to the event store and broadcast to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Event {
    ThreadCreated {
        thread_id: ThreadId,
        project_id: ProjectId,
        name: String,
        settings: ThreadSettings,
        timestamp: DateTime<Utc>,
    },
    ThreadSettingsUpdated {
        thread_id: ThreadId,
        settings: ThreadSettings,
        timestamp: DateTime<Utc>,
    },
    ThreadRenamed {
        thread_id: ThreadId,
        name: String,
        timestamp: DateTime<Utc>,
    },
    ThreadDeleted {
        thread_id: ThreadId,
        timestamp: DateTime<Utc>,
    },
    TurnStartRequested {
        thread_id: ThreadId,
        turn_id: TurnId,
        input: String,
        images: Vec<String>,
        settings_snapshot: ThreadSettings,
        timestamp: DateTime<Utc>,
    },
    SessionEstablished {
        thread_id: ThreadId,
        session_id: SessionId,
        provider: ProviderKind,
        provider_thread_id: String,
        timestamp: DateTime<Utc>,
    },
    SessionCleared {
        thread_id: ThreadId,
        timestamp: DateTime<Utc>,
    },
    SessionStatusChanged {
        thread_id: ThreadId,
        status: ProviderSessionStatus,
        message: Option<String>,
        timestamp: DateTime<Utc>,
    },
    TurnStarted {
        thread_id: ThreadId,
        turn_id: TurnId,
        provider_turn_id: String,
        timestamp: DateTime<Utc>,
    },
    RuntimeEventRecorded {
        thread_id: ThreadId,
        turn_id: Option<TurnId>,
        event_type: ProviderRuntimeEventKind,
        item_id: Option<String>,
        request_id: Option<ApprovalRequestId>,
        summary: Option<String>,
        data: Value,
        timestamp: DateTime<Utc>,
    },
    AssistantMessageDelta {
        thread_id: ThreadId,
        turn_id: TurnId,
        item_id: String,
        delta: String,
        timestamp: DateTime<Utc>,
    },
    TurnCompleted {
        thread_id: ThreadId,
        turn_id: TurnId,
        status: String,
        timestamp: DateTime<Utc>,
    },
    TurnInterrupted {
        thread_id: ThreadId,
        turn_id: TurnId,
        timestamp: DateTime<Utc>,
    },
    TurnsRolledBack {
        thread_id: ThreadId,
        n: u32,
        timestamp: DateTime<Utc>,
    },
    ApprovalRequested {
        thread_id: ThreadId,
        turn_id: TurnId,
        approval_id: ApprovalRequestId,
        rpc_id: u64,
        kind: ApprovalKind,
        details: Value,
        timestamp: DateTime<Utc>,
    },
    ApprovalResponded {
        thread_id: ThreadId,
        approval_id: ApprovalRequestId,
        decision: ApprovalDecision,
        timestamp: DateTime<Utc>,
    },
    UserInputRequested {
        thread_id: ThreadId,
        turn_id: TurnId,
        approval_id: ApprovalRequestId,
        rpc_id: u64,
        questions: Option<Value>,
        timestamp: DateTime<Utc>,
    },
    UserInputResponded {
        thread_id: ThreadId,
        approval_id: ApprovalRequestId,
        answers: Value,
        timestamp: DateTime<Utc>,
    },
    ErrorOccurred {
        thread_id: ThreadId,
        message: String,
        will_retry: bool,
        timestamp: DateTime<Utc>,
    },
}

impl Event {
    /// Get the thread ID this event belongs to.
    pub fn thread_id(&self) -> ThreadId {
        match self {
            Event::ThreadCreated { thread_id, .. }
            | Event::ThreadSettingsUpdated { thread_id, .. }
            | Event::ThreadRenamed { thread_id, .. }
            | Event::ThreadDeleted { thread_id, .. }
            | Event::TurnStartRequested { thread_id, .. }
            | Event::SessionEstablished { thread_id, .. }
            | Event::SessionCleared { thread_id, .. }
            | Event::SessionStatusChanged { thread_id, .. }
            | Event::TurnStarted { thread_id, .. }
            | Event::RuntimeEventRecorded { thread_id, .. }
            | Event::AssistantMessageDelta { thread_id, .. }
            | Event::TurnCompleted { thread_id, .. }
            | Event::TurnInterrupted { thread_id, .. }
            | Event::TurnsRolledBack { thread_id, .. }
            | Event::ApprovalRequested { thread_id, .. }
            | Event::ApprovalResponded { thread_id, .. }
            | Event::UserInputRequested { thread_id, .. }
            | Event::UserInputResponded { thread_id, .. }
            | Event::ErrorOccurred { thread_id, .. } => *thread_id,
        }
    }

    /// Get the timestamp of this event.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Event::ThreadCreated { timestamp, .. }
            | Event::ThreadSettingsUpdated { timestamp, .. }
            | Event::ThreadRenamed { timestamp, .. }
            | Event::ThreadDeleted { timestamp, .. }
            | Event::TurnStartRequested { timestamp, .. }
            | Event::SessionEstablished { timestamp, .. }
            | Event::SessionCleared { timestamp, .. }
            | Event::SessionStatusChanged { timestamp, .. }
            | Event::TurnStarted { timestamp, .. }
            | Event::RuntimeEventRecorded { timestamp, .. }
            | Event::AssistantMessageDelta { timestamp, .. }
            | Event::TurnCompleted { timestamp, .. }
            | Event::TurnInterrupted { timestamp, .. }
            | Event::TurnsRolledBack { timestamp, .. }
            | Event::ApprovalRequested { timestamp, .. }
            | Event::ApprovalResponded { timestamp, .. }
            | Event::UserInputRequested { timestamp, .. }
            | Event::UserInputResponded { timestamp, .. }
            | Event::ErrorOccurred { timestamp, .. } => *timestamp,
        }
    }

    /// Get the stream ID for event store partitioning.
    pub fn stream_id(&self) -> String {
        format!("thread:{}", self.thread_id())
    }
}

// ─── Read Model ─────────────────────────────────────────────────────

/// The full in-memory read model built from events.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReadModel {
    pub threads: HashMap<ThreadId, ThreadState>,
    pub projects: HashMap<ProjectId, ProjectState>,
}

/// State of a single thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadState {
    pub id: ThreadId,
    pub project_id: ProjectId,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub settings: ThreadSettings,
    pub turns: Vec<TurnState>,
    pub session: Option<SessionState>,
    pub active_turn: Option<TurnId>,
    pub pending_approvals: HashMap<ApprovalRequestId, PendingApproval>,
    pub pending_inputs: HashMap<ApprovalRequestId, PendingUserInput>,
    pub runtime_events: Vec<ProviderRuntimeEvent>,
    pub errors: Vec<ThreadError>,
    pub deleted: bool,
}

/// State of a single turn within a thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnState {
    pub id: TurnId,
    pub input: String,
    pub images: Vec<String>,
    pub settings_snapshot: ThreadSettings,
    pub status: TurnStatus,
    pub messages: Vec<MessageItem>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub provider_turn_id: Option<String>,
}

/// Status of a turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Requested,
    Running,
    Waiting,
    Completed,
    Interrupted,
    Failed,
}

/// A message item (assistant response or system message).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageItem {
    pub item_id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// Role of a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Active provider session info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub session_id: SessionId,
    pub provider: ProviderKind,
    pub provider_thread_id: String,
    pub status: ProviderSessionStatus,
    pub established_at: DateTime<Utc>,
    pub last_error: Option<String>,
    pub last_message: Option<String>,
}

/// A pending approval request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingApproval {
    pub id: ApprovalRequestId,
    pub turn_id: TurnId,
    pub rpc_id: u64,
    pub kind: ApprovalKind,
    pub details: Value,
    pub requested_at: DateTime<Utc>,
}

/// A pending user input request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingUserInput {
    pub id: ApprovalRequestId,
    pub turn_id: TurnId,
    pub rpc_id: u64,
    pub questions: Option<Value>,
    pub requested_at: DateTime<Utc>,
}

/// A recorded error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadError {
    pub message: String,
    pub will_retry: bool,
    pub timestamp: DateTime<Utc>,
}

/// State of a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectState {
    pub id: ProjectId,
    pub name: String,
    pub path: String,
    pub default_model: Option<String>,
    pub thread_ids: Vec<ThreadId>,
}
