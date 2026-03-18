//! eAgent Protocol — message contracts, TaskGraph types, and agent traits.
//!
//! This crate defines the wire format between agents and the eAgent runtime.
//! All harness<->agent communication uses the types defined here.

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

#[cfg(test)]
mod tests {
    use super::*;

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
