//! eAgent Runtime — the harness that orchestrates agents, schedules tasks,
//! and manages the agent lifecycle.
//!
//! # Architecture
//!
//! The runtime consists of four core components:
//!
//! - **Scheduler** — TaskGraph DAG validation, dependency tracking, and
//!   ready-state management. Determines which tasks can run next while
//!   respecting concurrency limits.
//!
//! - **AgentPool** — Spawns and manages worker agents. Each agent gets a
//!   provider session, translates ProviderEvents into AgentMessages, and
//!   supports cancellation.
//!
//! - **ConflictResolver** — Detects when parallel agents modify the same
//!   file and either merges non-conflicting mutations or reports conflicts
//!   for human resolution.
//!
//! - **RuntimeEngine** — Ties everything together. Accepts TaskGraphs,
//!   runs the scheduling loop, persists state transitions to the EventStore,
//!   and emits RuntimeEvents for the UI layer.

pub mod agent_pool;
pub mod conflict;
pub mod engine;
pub mod error;
pub mod scheduler;

// Re-export key types at crate root.
pub use agent_pool::{AgentHandle, AgentPool};
pub use conflict::{Conflict, ConflictResolver, FileMutation, FileMutationKind};
pub use engine::{RuntimeConfig, RuntimeEngine, RuntimeEvent};
pub use error::{RuntimeError, SchedulerError};
pub use scheduler::Scheduler;
