# Phase 1: eAgent Scaffolding — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create the new eAgent crate structure with all trait definitions, protocol types, and core domain types so that subsequent phases have a stable foundation to build on.

**Architecture:** Replace the existing `ecode-*` crate namespace with `eagent-*` crates. The new structure separates protocol (wire format), contracts (domain types), providers (LLM backends), tools (agent capabilities), runtime (orchestration), and persistence (storage). Existing crates are preserved during migration — new crates initially depend on old ones where needed, and migration happens in Phase 2.

**Tech Stack:** Rust 2024 edition, serde, serde_json, uuid, chrono, thiserror, tokio, trait-variant

**Spec:** `docs/superpowers/specs/2026-03-18-eagent-platform-design.md`

---

### Task 1: Create eagent-protocol crate with core types

The protocol crate is the foundation — it defines the wire format between agents and the runtime. All other eagent crates depend on it.

**Files:**
- Create: `crates/eagent-protocol/Cargo.toml`
- Create: `crates/eagent-protocol/src/lib.rs`
- Create: `crates/eagent-protocol/src/ids.rs`
- Create: `crates/eagent-protocol/src/messages.rs`
- Create: `crates/eagent-protocol/src/task_graph.rs`
- Create: `crates/eagent-protocol/src/events.rs`
- Create: `crates/eagent-protocol/src/traits.rs`
- Modify: `Cargo.toml` (workspace members and dependencies)

- [ ] **Step 1: Create the crate directory and Cargo.toml**

```toml
# crates/eagent-protocol/Cargo.toml
[package]
name = "eagent-protocol"
description = "Agent protocol types, message contracts, and TaskGraph definitions for eAgent"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
```

- [ ] **Step 2: Add eagent-protocol to workspace Cargo.toml**

Add to `[workspace.dependencies]`:
```toml
eagent-protocol = { path = "crates/eagent-protocol" }
```

- [ ] **Step 3: Create ids.rs with new eAgent ID types**

Reuse the `define_id!` macro pattern from `ecode-contracts/src/ids.rs`. Add new IDs needed by the protocol:

```rust
// crates/eagent-protocol/src/ids.rs
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self { Self(Uuid::new_v4()) }
            pub fn from_uuid(uuid: Uuid) -> Self { Self(uuid) }
            pub fn parse(s: &str) -> Result<Self, uuid::Error> { Ok(Self(Uuid::parse_str(s)?)) }
            pub fn inner(&self) -> Uuid { self.0 }
        }

        impl Default for $name {
            fn default() -> Self { Self::new() }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.0) }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
    };
}

define_id!(TaskId, "Unique identifier for a task within a TaskGraph.");
define_id!(TaskGraphId, "Unique identifier for a TaskGraph (one user request).");
define_id!(AgentId, "Unique identifier for an agent instance.");
define_id!(ProviderId, "Unique identifier for a configured provider.");
define_id!(TerminalId, "Unique identifier for a terminal instance.");
define_id!(ThreadId, "Unique identifier for a conversation thread (legacy compat).");
define_id!(SessionId, "Unique identifier for a provider session.");
```

- [ ] **Step 4: Create messages.rs with the AgentMessage protocol**

```rust
// crates/eagent-protocol/src/messages.rs
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
```

- [ ] **Step 5: Create task_graph.rs with TaskGraph and TaskNode types**

```rust
// crates/eagent-protocol/src/task_graph.rs
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
```

- [ ] **Step 6: Create events.rs with TaskGraphEvent types**

```rust
// crates/eagent-protocol/src/events.rs
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
```

- [ ] **Step 7: Create traits.rs with Agent trait and AgentChannel**

```rust
// crates/eagent-protocol/src/traits.rs
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
```

- [ ] **Step 8: Create lib.rs to wire up all modules**

```rust
// crates/eagent-protocol/src/lib.rs
//! eAgent Protocol — message contracts, TaskGraph types, and agent traits.
//!
//! This crate defines the wire format between agents and the eAgent runtime.
//! All harness↔agent communication uses the types defined here.

pub mod ids;
pub mod messages;
pub mod task_graph;
pub mod events;
pub mod traits;

// Re-export key types at crate root for convenience.
pub use ids::*;
pub use messages::{AgentMessage, HarnessMessage, RiskLevel, OversightDecision, TaskConstraints};
pub use task_graph::{TaskGraph, TaskNode, TaskStatus, TraceEntry, TraceEntryKind};
pub use events::TaskGraphEvent;
pub use traits::{Agent, AgentChannel, AgentContext, TaskAssignment, TaskResult, AgentError};
```

- [ ] **Step 9: Write tests for protocol types**

Add to `crates/eagent-protocol/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn task_id_roundtrip() {
        let id = TaskId::new();
        let json = serde_json::to_string(&id).unwrap();
        let back: TaskId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn task_graph_id_display_parse() {
        let id = TaskGraphId::new();
        let s = id.to_string();
        let parsed = TaskGraphId::parse(&s).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn agent_message_serde_roundtrip() {
        let msg = AgentMessage::StatusUpdate {
            task_id: TaskId::new(),
            phase: "reading".into(),
            message: "Reading auth module".into(),
            progress: Some(0.5),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "StatusUpdate");
        let back: AgentMessage = serde_json::from_value(json).unwrap();
        match back {
            AgentMessage::StatusUpdate { phase, progress, .. } => {
                assert_eq!(phase, "reading");
                assert_eq!(progress, Some(0.5));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn task_status_default_is_pending() {
        let status = TaskStatus::default();
        assert_eq!(status, TaskStatus::Pending);
    }

    #[test]
    fn task_graph_event_stream_id() {
        let gid = TaskGraphId::new();
        let event = TaskGraphEvent::PlanApproved {
            graph_id: gid,
            timestamp: chrono::Utc::now(),
        };
        assert!(event.stream_id().starts_with("graph:"));
        assert_eq!(event.graph_id(), gid);
    }

    #[test]
    fn harness_message_serde() {
        let msg = HarnessMessage::TaskCancellation {
            task_id: TaskId::new(),
            reason: "user cancelled".into(),
        };
        let json = serde_json::to_value(&msg).unwrap();
        let back: HarnessMessage = serde_json::from_value(json).unwrap();
        match back {
            HarnessMessage::TaskCancellation { reason, .. } => {
                assert_eq!(reason, "user cancelled");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn task_constraints_default_is_unbounded() {
        let c = TaskConstraints::default();
        assert!(c.max_tokens.is_none());
        assert!(c.max_tool_calls.is_none());
        assert!(c.max_time_secs.is_none());
        assert!(c.allowed_paths.is_none());
    }
}
```

- [ ] **Step 10: Verify eagent-protocol compiles and tests pass**

Run: `cargo check -p eagent-protocol && cargo test -p eagent-protocol`
Expected: compiles with no errors, all tests pass

- [ ] **Step 11: Commit**

```bash
git add crates/eagent-protocol/ Cargo.toml
git commit -m "feat: add eagent-protocol crate with protocol types, TaskGraph, and Agent traits"
```

---

### Task 2: Create eagent-contracts crate with domain types

This crate holds shared non-protocol domain types: config structs, provider metadata, UI DTOs. Initially references existing `ecode-contracts` types via re-exports to avoid a big-bang migration.

**Files:**
- Create: `crates/eagent-contracts/Cargo.toml`
- Create: `crates/eagent-contracts/src/lib.rs`
- Create: `crates/eagent-contracts/src/config.rs`
- Create: `crates/eagent-contracts/src/provider.rs`
- Create: `crates/eagent-contracts/src/oversight.rs`
- Modify: `Cargo.toml` (workspace deps)

- [ ] **Step 1: Create the crate Cargo.toml**

```toml
# crates/eagent-contracts/Cargo.toml
[package]
name = "eagent-contracts"
description = "Shared domain types, configuration, and provider metadata for eAgent"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
eagent-protocol = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
thiserror = { workspace = true }
toml = { workspace = true }
```

- [ ] **Step 2: Add to workspace Cargo.toml**

```toml
eagent-contracts = { path = "crates/eagent-contracts" }
```

- [ ] **Step 3: Create provider.rs with ProviderKind and ProviderEvent**

```rust
// crates/eagent-contracts/src/provider.rs
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The kind of provider backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    #[default]
    Codex,
    LlamaCpp,
    ApiKey,
}

/// Raw events from an LLM provider, before translation into AgentMessage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderEvent {
    TokenDelta { text: String },
    ToolCallStart { id: String, name: String, params_partial: String },
    ToolCallDelta { id: String, params_partial: String },
    ToolCallComplete { id: String, name: String, params: Value },
    Completion { finish_reason: FinishReason },
    Error { message: String },
}

/// Why the provider stopped generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
    Error,
}

/// Information about a model available from a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub max_context: Option<u32>,
    pub provider_kind: ProviderKind,
}

/// Coarse provider session state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSessionStatus {
    Starting,
    Ready,
    Running,
    Waiting,
    #[default]
    Stopped,
    Error,
}
```

- [ ] **Step 4: Create oversight.rs with the three-tier oversight model**

```rust
// crates/eagent-contracts/src/oversight.rs
use eagent_protocol::messages::RiskLevel;
use serde::{Deserialize, Serialize};

/// Oversight mode controlling when agents must ask for approval.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OversightMode {
    /// Agents execute all tool calls without asking.
    FullAutonomy,
    /// Auto-proceed on Low risk, ask approval for Medium and High.
    #[default]
    ApproveRisky,
    /// Every tool call requires explicit approval.
    ApproveAll,
}

impl OversightMode {
    /// Whether a tool call at the given risk level requires human approval.
    pub fn requires_approval(&self, risk: RiskLevel) -> bool {
        match self {
            OversightMode::FullAutonomy => false,
            OversightMode::ApproveRisky => matches!(risk, RiskLevel::Medium | RiskLevel::High),
            OversightMode::ApproveAll => true,
        }
    }
}
```

- [ ] **Step 5: Create config.rs with extended AppConfig for multi-provider**

```rust
// crates/eagent-contracts/src/config.rs
use crate::oversight::OversightMode;
use crate::provider::ProviderKind;
use eagent_protocol::ids::ProviderId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level eAgent application configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub agent_defaults: AgentDefaults,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub projects: ProjectsConfig,
}

/// General UI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self { theme: default_theme(), font_size: default_font_size() }
    }
}

fn default_theme() -> String { "dark".into() }
fn default_font_size() -> f32 { 14.0 }

/// Default settings for agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefaults {
    #[serde(default = "default_planner_provider")]
    pub planner_provider: String,
    #[serde(default = "default_worker_provider")]
    pub worker_provider: String,
    #[serde(default)]
    pub fallback_provider: Option<String>,
    #[serde(default)]
    pub oversight_mode: OversightMode,
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: u32,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

impl Default for AgentDefaults {
    fn default() -> Self {
        Self {
            planner_provider: default_planner_provider(),
            worker_provider: default_worker_provider(),
            fallback_provider: None,
            oversight_mode: OversightMode::default(),
            max_concurrency: default_max_concurrency(),
            max_retries: default_max_retries(),
        }
    }
}

fn default_planner_provider() -> String { "codex".into() }
fn default_worker_provider() -> String { "codex".into() }
fn default_max_concurrency() -> u32 { 4 }
fn default_max_retries() -> u32 { 2 }

/// Configuration for a single provider instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_concurrent_sessions")]
    pub max_concurrent_sessions: u32,
    #[serde(default)]
    pub default_model: String,
    #[serde(flatten)]
    pub specific: ProviderSpecificConfig,
}

fn default_true() -> bool { true }
fn default_concurrent_sessions() -> u32 { 4 }

/// Provider-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider_type", rename_all = "snake_case")]
pub enum ProviderSpecificConfig {
    Codex {
        #[serde(default)]
        binary_path: String,
        #[serde(default)]
        home_dir: String,
    },
    LlamaCpp {
        #[serde(default)]
        server_binary_path: String,
        #[serde(default)]
        model_path: String,
        #[serde(default = "default_llama_host")]
        host: String,
        #[serde(default = "default_llama_port")]
        port: u16,
        #[serde(default = "default_ctx_size")]
        ctx_size: u32,
        #[serde(default = "default_llama_threads")]
        threads: u16,
        #[serde(default)]
        gpu_layers: i32,
    },
    ApiKey {
        endpoint: String,
        #[serde(default)]
        api_key: String,
        #[serde(default)]
        models: Vec<String>,
        #[serde(default = "default_api_max_context")]
        max_context: u32,
    },
}

fn default_llama_host() -> String { "127.0.0.1".into() }
fn default_llama_port() -> u16 { 8012 }
fn default_ctx_size() -> u32 { 4096 }
fn default_llama_threads() -> u16 {
    std::thread::available_parallelism()
        .map(|p| p.get().saturating_sub(2).clamp(1, 8) as u16)
        .unwrap_or(4)
}
fn default_api_max_context() -> u32 { 128_000 }

/// Projects configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectsConfig {
    #[serde(default)]
    pub entries: Vec<ProjectEntry>,
}

/// A single project entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub path: String,
    pub name: String,
    #[serde(default)]
    pub default_provider: Option<String>,
}
```

- [ ] **Step 6: Create lib.rs**

```rust
// crates/eagent-contracts/src/lib.rs
//! eAgent Contracts — shared non-protocol domain types.
//!
//! Configuration, provider metadata, oversight model, and UI DTOs.
//! Does NOT contain AgentMessage or TaskGraph types (those live in eagent-protocol).

pub mod config;
pub mod oversight;
pub mod provider;
```

- [ ] **Step 7: Write tests**

Add to `crates/eagent-contracts/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::oversight::OversightMode;
    use eagent_protocol::messages::RiskLevel;

    #[test]
    fn oversight_full_autonomy_never_requires_approval() {
        let mode = OversightMode::FullAutonomy;
        assert!(!mode.requires_approval(RiskLevel::Low));
        assert!(!mode.requires_approval(RiskLevel::Medium));
        assert!(!mode.requires_approval(RiskLevel::High));
    }

    #[test]
    fn oversight_approve_risky_skips_low() {
        let mode = OversightMode::ApproveRisky;
        assert!(!mode.requires_approval(RiskLevel::Low));
        assert!(mode.requires_approval(RiskLevel::Medium));
        assert!(mode.requires_approval(RiskLevel::High));
    }

    #[test]
    fn oversight_approve_all_always_requires() {
        let mode = OversightMode::ApproveAll;
        assert!(mode.requires_approval(RiskLevel::Low));
        assert!(mode.requires_approval(RiskLevel::Medium));
        assert!(mode.requires_approval(RiskLevel::High));
    }

    #[test]
    fn agent_config_default_serde_roundtrip() {
        let config = config::AgentConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let back: config::AgentConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.general.theme, "dark");
        assert_eq!(back.agent_defaults.max_concurrency, 4);
    }

    #[test]
    fn provider_event_serde() {
        let evt = provider::ProviderEvent::TokenDelta { text: "hello".into() };
        let json = serde_json::to_value(&evt).unwrap();
        assert_eq!(json["type"], "token_delta");
    }
}
```

- [ ] **Step 8: Verify compilation and tests**

Run: `cargo check -p eagent-contracts && cargo test -p eagent-contracts`
Expected: compiles, all tests pass

- [ ] **Step 9: Commit**

```bash
git add crates/eagent-contracts/ Cargo.toml
git commit -m "feat: add eagent-contracts crate with config, provider, and oversight types"
```

---

### Task 3: Create eagent-tools crate with Tool trait and registry

**Files:**
- Create: `crates/eagent-tools/Cargo.toml`
- Create: `crates/eagent-tools/src/lib.rs`
- Create: `crates/eagent-tools/src/registry.rs`
- Modify: `Cargo.toml` (workspace deps)

- [ ] **Step 1: Create the crate Cargo.toml**

```toml
# crates/eagent-tools/Cargo.toml
[package]
name = "eagent-tools"
description = "Tool trait, registry, and built-in tool implementations for eAgent"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
eagent-protocol = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
trait-variant = { workspace = true }
```

- [ ] **Step 2: Add to workspace Cargo.toml**

```toml
eagent-tools = { path = "crates/eagent-tools" }
trait-variant = "0.1"
```

- [ ] **Step 3: Create lib.rs with Tool trait**

```rust
// crates/eagent-tools/src/lib.rs
//! eAgent Tools — trait definition and built-in tool implementations.

pub mod registry;

use eagent_protocol::messages::RiskLevel;
use serde_json::Value;
use thiserror::Error;

/// Error from tool execution.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("invalid parameters: {0}")]
    InvalidParams(String),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("timeout after {0} seconds")]
    Timeout(u64),
}

/// Context provided to a tool during execution.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Workspace root directory (tools must not escape this).
    pub workspace_root: String,
    /// The agent ID that requested this tool call.
    pub agent_id: eagent_protocol::ids::AgentId,
    /// The task ID this tool call belongs to.
    pub task_id: eagent_protocol::ids::TaskId,
}

/// Result of a tool execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolResult {
    /// The output data.
    pub output: Value,
    /// Whether this result represents an error.
    pub is_error: bool,
}

/// Definition of a tool for LLM function-calling schemas.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub risk_level: RiskLevel,
}

/// The Tool trait that all built-in and eMCP tools implement.
#[trait_variant::make(Send)]
pub trait Tool: Send + Sync {
    /// The tool's unique name.
    fn name(&self) -> &str;

    /// Human-readable description.
    fn description(&self) -> &str;

    /// Risk level for the oversight system.
    fn risk_level(&self) -> RiskLevel;

    /// JSON Schema for the tool's parameters.
    fn parameter_schema(&self) -> Value;

    /// Execute the tool with the given parameters.
    async fn execute(&self, params: Value, ctx: &ToolContext) -> Result<ToolResult, ToolError>;

    /// Build a ToolDef for LLM function-calling.
    fn to_def(&self) -> ToolDef {
        ToolDef {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameter_schema(),
            risk_level: self.risk_level(),
        }
    }
}
```

- [ ] **Step 4: Create registry.rs**

```rust
// crates/eagent-tools/src/registry.rs
use crate::{Tool, ToolDef};
use std::collections::HashMap;
use std::sync::Arc;

/// Registry of available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    /// Register a tool. Overwrites any existing tool with the same name.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// List all registered tool definitions.
    pub fn list_defs(&self) -> Vec<ToolDef> {
        self.tools.values().map(|t| t.to_def()).collect()
    }

    /// List tool definitions filtered by allowed names.
    pub fn list_defs_filtered(&self, allowed: &[String]) -> Vec<ToolDef> {
        allowed.iter()
            .filter_map(|name| self.tools.get(name))
            .map(|t| t.to_def())
            .collect()
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 5: Write tests**

Add to `crates/eagent-tools/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use super::registry::ToolRegistry;
    use eagent_protocol::messages::RiskLevel;
    use serde_json::json;
    use std::sync::Arc;

    struct MockTool;

    impl Tool for MockTool {
        fn name(&self) -> &str { "mock_tool" }
        fn description(&self) -> &str { "A mock tool for testing" }
        fn risk_level(&self) -> RiskLevel { RiskLevel::Low }
        fn parameter_schema(&self) -> serde_json::Value {
            json!({"type": "object", "properties": {"input": {"type": "string"}}})
        }
        async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult, ToolError> {
            let input = params.get("input").and_then(|v| v.as_str()).unwrap_or("");
            Ok(ToolResult { output: json!({"echo": input}), is_error: false })
        }
    }

    #[test]
    fn registry_register_and_get() {
        let mut reg = ToolRegistry::new();
        reg.register(Arc::new(MockTool));
        assert_eq!(reg.len(), 1);
        assert!(reg.get("mock_tool").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn registry_list_defs() {
        let mut reg = ToolRegistry::new();
        reg.register(Arc::new(MockTool));
        let defs = reg.list_defs();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "mock_tool");
        assert_eq!(defs[0].risk_level, RiskLevel::Low);
    }

    #[test]
    fn registry_list_filtered() {
        let mut reg = ToolRegistry::new();
        reg.register(Arc::new(MockTool));
        let allowed = vec!["mock_tool".to_string()];
        assert_eq!(reg.list_defs_filtered(&allowed).len(), 1);
        let empty = vec!["nonexistent".to_string()];
        assert_eq!(reg.list_defs_filtered(&empty).len(), 0);
    }

    #[test]
    fn tool_def_from_trait() {
        let tool = MockTool;
        let def = tool.to_def();
        assert_eq!(def.name, "mock_tool");
        assert_eq!(def.description, "A mock tool for testing");
    }

    #[tokio::test]
    async fn mock_tool_executes() {
        let tool = MockTool;
        let ctx = ToolContext {
            workspace_root: "/tmp".into(),
            agent_id: eagent_protocol::ids::AgentId::new(),
            task_id: eagent_protocol::ids::TaskId::new(),
        };
        let result = tool.execute(json!({"input": "hello"}), &ctx).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(result.output["echo"], "hello");
    }
}
```

- [ ] **Step 6: Verify compilation and tests**

Run: `cargo check -p eagent-tools && cargo test -p eagent-tools`
Expected: compiles, all tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/eagent-tools/ Cargo.toml
git commit -m "feat: add eagent-tools crate with Tool trait and ToolRegistry"
```

---

### Task 4: Create eagent-providers crate with Provider trait

**Files:**
- Create: `crates/eagent-providers/Cargo.toml`
- Create: `crates/eagent-providers/src/lib.rs`
- Create: `crates/eagent-providers/src/registry.rs`
- Modify: `Cargo.toml` (workspace deps)

- [ ] **Step 1: Create the crate Cargo.toml**

```toml
# crates/eagent-providers/Cargo.toml
[package]
name = "eagent-providers"
description = "Provider trait, registry, and backend implementations for eAgent"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
eagent-protocol = { workspace = true }
eagent-contracts = { workspace = true }
eagent-tools = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
trait-variant = { workspace = true }
```

- [ ] **Step 2: Add to workspace Cargo.toml**

```toml
eagent-providers = { path = "crates/eagent-providers" }
```

- [ ] **Step 3: Create lib.rs with Provider trait**

```rust
// crates/eagent-providers/src/lib.rs
//! eAgent Providers — trait and implementations for LLM backends.

pub mod registry;

use eagent_contracts::provider::{ModelInfo, ProviderEvent, ProviderSessionStatus};
use eagent_tools::ToolDef;
use serde_json::Value;
use thiserror::Error;
use tokio::sync::mpsc;

/// Handle to an active provider session.
#[derive(Debug, Clone)]
pub struct SessionHandle {
    pub session_id: eagent_protocol::ids::SessionId,
    pub provider_name: String,
}

/// Configuration for creating a session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub model: String,
    pub system_prompt: Option<String>,
    pub workspace_root: Option<String>,
}

/// Error from a provider operation.
#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("model not available: {0}")]
    ModelNotAvailable(String),
    #[error("rate limited")]
    RateLimited,
    #[error("cancelled")]
    Cancelled,
    #[error("internal: {0}")]
    Internal(String),
}

/// Message sent to the provider for a turn.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderMessage {
    pub role: ProviderMessageRole,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// The Provider trait that all LLM backends implement.
/// Providers translate between the LLM's raw output and ProviderEvent.
#[trait_variant::make(Send)]
pub trait Provider: Send + Sync {
    /// Create a new session with this provider.
    async fn create_session(&self, config: SessionConfig) -> Result<SessionHandle, ProviderError>;

    /// Send messages and tool definitions, receive a stream of ProviderEvents.
    /// The receiver end of the channel will emit ProviderEvents as they arrive.
    async fn send(
        &self,
        session: &SessionHandle,
        messages: Vec<ProviderMessage>,
        tools: Vec<ToolDef>,
    ) -> Result<mpsc::UnboundedReceiver<ProviderEvent>, ProviderError>;

    /// Cancel an active session.
    async fn cancel(&self, session: &SessionHandle) -> Result<(), ProviderError>;

    /// List available models.
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError>;

    /// Get current session status.
    fn session_status(&self, session: &SessionHandle) -> ProviderSessionStatus;
}
```

- [ ] **Step 4: Create registry.rs**

```rust
// crates/eagent-providers/src/registry.rs
use crate::Provider;
use std::collections::HashMap;
use std::sync::Arc;

/// Registry of configured provider instances.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self { providers: HashMap::new() }
    }

    pub fn register(&mut self, name: String, provider: Arc<dyn Provider>) {
        self.providers.insert(name, provider);
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn Provider>> {
        self.providers.get(name)
    }

    pub fn names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 5: Write tests**

Add to `crates/eagent-providers/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use super::registry::ProviderRegistry;

    #[test]
    fn registry_operations() {
        let mut reg = ProviderRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(reg.get("codex").is_none());
    }

    #[test]
    fn session_config_defaults() {
        let config = SessionConfig {
            model: "gpt-5.4".into(),
            system_prompt: None,
            workspace_root: None,
        };
        assert_eq!(config.model, "gpt-5.4");
    }

    #[test]
    fn provider_message_serde() {
        let msg = ProviderMessage {
            role: ProviderMessageRole::User,
            content: "hello".into(),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["role"], "user");
        let back: ProviderMessage = serde_json::from_value(json).unwrap();
        assert_eq!(back.content, "hello");
    }
}
```

- [ ] **Step 6: Verify and commit**

Run: `cargo check -p eagent-providers && cargo test -p eagent-providers`

```bash
git add crates/eagent-providers/ Cargo.toml
git commit -m "feat: add eagent-providers crate with Provider trait and registry"
```

---

### Task 5: Create remaining shell crates

Create the remaining crates as minimal shells so the full crate graph compiles. These will be filled in during later phases.

**Files:**
- Create: `crates/eagent-persistence/Cargo.toml`
- Create: `crates/eagent-persistence/src/lib.rs`
- Create: `crates/eagent-runtime/Cargo.toml`
- Create: `crates/eagent-runtime/src/lib.rs`
- Create: `crates/eagent-planner/Cargo.toml`
- Create: `crates/eagent-planner/src/lib.rs`
- Create: `crates/eagent-skills/Cargo.toml`
- Create: `crates/eagent-skills/src/lib.rs`
- Create: `crates/eagent-mcp/Cargo.toml`
- Create: `crates/eagent-mcp/src/lib.rs`
- Create: `crates/eagent-index/Cargo.toml`
- Create: `crates/eagent-index/src/lib.rs`
- Modify: `Cargo.toml` (workspace deps)

- [ ] **Step 1: Create eagent-persistence**

```toml
# crates/eagent-persistence/Cargo.toml
[package]
name = "eagent-persistence"
description = "Event store, configuration persistence, and session state for eAgent"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
eagent-protocol = { workspace = true }
eagent-contracts = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
```

```rust
// crates/eagent-persistence/src/lib.rs
//! eAgent Persistence — event store, config loading, and session state management.
//! Populated in Phase 2 with migration from ecode-core's EventStore and ConfigManager.
```

- [ ] **Step 2: Create eagent-runtime**

```toml
# crates/eagent-runtime/Cargo.toml
[package]
name = "eagent-runtime"
description = "eAgent runtime: TaskGraph scheduler, AgentPool, and conflict resolver"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
eagent-protocol = { workspace = true }
eagent-contracts = { workspace = true }
eagent-providers = { workspace = true }
eagent-tools = { workspace = true }
eagent-persistence = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
```

```rust
// crates/eagent-runtime/src/lib.rs
//! eAgent Runtime — the harness that orchestrates agents, schedules tasks,
//! and manages the agent lifecycle. Populated in Phase 3.
```

- [ ] **Step 3: Create eagent-planner**

```toml
# crates/eagent-planner/Cargo.toml
[package]
name = "eagent-planner"
description = "Planner agent logic and TaskGraph generation for eAgent"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
eagent-protocol = { workspace = true }
eagent-contracts = { workspace = true }
eagent-providers = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
```

```rust
// crates/eagent-planner/src/lib.rs
//! eAgent Planner — the planner agent that decomposes user requests into TaskGraphs.
//! Populated in Phase 3.
```

- [ ] **Step 4: Create eagent-skills**

```toml
# crates/eagent-skills/Cargo.toml
[package]
name = "eagent-skills"
description = "eSkill loader, manifest parser, and invocation for eAgent"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
eagent-protocol = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
```

```rust
// crates/eagent-skills/src/lib.rs
//! eAgent Skills — eSkill loading, manifest parsing, and agent capability packaging.
//! Populated in Phase 7.
```

- [ ] **Step 5: Create eagent-mcp**

```toml
# crates/eagent-mcp/Cargo.toml
[package]
name = "eagent-mcp"
description = "eMCP client and MCP protocol bridge for eAgent"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
eagent-protocol = { workspace = true }
eagent-tools = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
```

```rust
// crates/eagent-mcp/src/lib.rs
//! eAgent MCP — eMCP client that bridges MCP-compatible servers into the ToolRegistry.
//! Populated in Phase 7.
```

- [ ] **Step 6: Create eagent-index**

```toml
# crates/eagent-index/Cargo.toml
[package]
name = "eagent-index"
description = "Project graph, symbol index, and codebase understanding for eAgent"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
eagent-protocol = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
```

```rust
// crates/eagent-index/src/lib.rs
//! eAgent Index — tree-sitter based project graph, symbol index, and codebase understanding.
//! Populated in Phase 8.
```

- [ ] **Step 7: Add all new crates to workspace Cargo.toml**

Add to `[workspace.dependencies]`:
```toml
eagent-persistence = { path = "crates/eagent-persistence" }
eagent-runtime = { path = "crates/eagent-runtime" }
eagent-planner = { path = "crates/eagent-planner" }
eagent-skills = { path = "crates/eagent-skills" }
eagent-mcp = { path = "crates/eagent-mcp" }
eagent-index = { path = "crates/eagent-index" }
```

- [ ] **Step 8: Verify the full workspace compiles**

Run: `cargo check`
Expected: all crates compile, including existing ecode-* crates (they should be unaffected)

- [ ] **Step 9: Run full test suite**

Run: `cargo test`
Expected: all existing tests pass, new eagent-* tests pass

- [ ] **Step 10: Commit**

```bash
git add crates/eagent-persistence/ crates/eagent-runtime/ crates/eagent-planner/ crates/eagent-skills/ crates/eagent-mcp/ crates/eagent-index/ Cargo.toml
git commit -m "feat: add shell crates for persistence, runtime, planner, skills, mcp, and index"
```

---

### Task 6: Update project records

**Files:**
- Modify: `PROJECT_BRIEF.md`
- Modify: `STATE.yaml`
- Modify: `LOG.md`

- [ ] **Step 1: Update PROJECT_BRIEF.md**

Change the project name and goal to reflect eAgent. Update the focus areas to include the new crate architecture, multi-agent orchestration, and eWork.

- [ ] **Step 2: Update STATE.yaml**

Add the new crate structure, update `current_focus` to include Phase 1 completion and Phase 2 preparation.

- [ ] **Step 3: Append to LOG.md**

```markdown
## 2026-03-18
- Renamed project direction from eCode to eAgent — an agentic engineering platform with eCode (coding) and eWork (general-purpose) workstation modes.
- Created design spec at `docs/superpowers/specs/2026-03-18-eagent-platform-design.md` covering protocol-first architecture, multi-agent orchestration, eMCPs, eSkills, and 10 implementation phases.
- Phase 1 Scaffolding: created 9 new crates — eagent-protocol, eagent-contracts, eagent-tools, eagent-providers, eagent-persistence, eagent-runtime, eagent-planner, eagent-skills, eagent-mcp, eagent-index.
- Defined core traits: Agent (with AgentChannel for streaming), Provider (with ProviderEvent), Tool (with ToolRegistry).
- Defined protocol types: AgentMessage, HarnessMessage, TaskGraph, TaskNode, TaskStatus, TaskGraphEvent, TraceEntry.
- Defined domain types: OversightMode (three-tier), AgentConfig (multi-provider), ProviderKind (Codex/LlamaCpp/ApiKey).
- All new crates compile and tests pass alongside existing ecode-* crates.
```

- [ ] **Step 4: Commit**

```bash
git add PROJECT_BRIEF.md STATE.yaml LOG.md
git commit -m "docs: update project records for eAgent Phase 1 scaffolding"
```
