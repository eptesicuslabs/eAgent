//! Orchestration Engine — the stateful coordinator.
//!
//! Receives commands, runs them through the decider, persists events,
//! applies them to the read model, and broadcasts them.

use crate::orchestration::{apply, decide};
use crate::persistence::EventStore;
use anyhow::{Context, Result};
use ecode_contracts::orchestration::*;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

/// The orchestration engine.
pub struct OrchestrationEngine {
    /// The in-memory read model.
    state: Arc<RwLock<ReadModel>>,
    /// The persistent event store.
    event_store: Arc<EventStore>,
    /// Broadcast channel for events (to UI and reactor).
    event_tx: broadcast::Sender<Event>,
}

impl OrchestrationEngine {
    /// Create a new engine with the given event store.
    pub fn new(event_store: Arc<EventStore>) -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        Self {
            state: Arc::new(RwLock::new(ReadModel::default())),
            event_store,
            event_tx,
        }
    }

    /// Rebuild the read model by replaying all events from the store.
    pub fn rebuild(&self) -> Result<()> {
        info!("Rebuilding read model from event store...");
        let stored_events = self
            .event_store
            .read_all(0)
            .context("Failed to read events for rebuild")?;

        let mut state = self.state.write().unwrap();
        *state = ReadModel::default();

        let mut count = 0;
        for stored in &stored_events {
            if let Ok(event) = serde_json::from_value::<Event>(stored.payload.clone()) {
                apply(&mut state, &event);
                count += 1;
            } else {
                warn!(
                    event_type = %stored.event_type,
                    "Failed to deserialize event during rebuild, skipping"
                );
            }
        }

        info!(event_count = count, "Read model rebuilt successfully");
        Ok(())
    }

    /// Dispatch a command: validate → persist → apply → broadcast.
    pub fn dispatch(&self, command: Command) -> Result<Vec<Event>> {
        // 1. Read current state
        let state = self.state.read().unwrap();
        let events = decide(&command, &state).map_err(|e| anyhow::anyhow!("{}", e))?;
        drop(state);

        if events.is_empty() {
            return Ok(events);
        }

        // 2. Persist events
        let first_event = &events[0];
        let stream_id = first_event.stream_id();

        let stored: Vec<(String, serde_json::Value)> = events
            .iter()
            .map(|e| {
                let event_type = event_type_name(e);
                let payload = serde_json::to_value(e).unwrap_or_default();
                (event_type, payload)
            })
            .collect();

        self.event_store
            .append_events(&stream_id, &stored)
            .context("Failed to persist events")?;

        // 3. Apply to read model
        {
            let mut state = self.state.write().unwrap();
            for event in &events {
                apply(&mut state, event);
            }
        }

        // 4. Broadcast
        for event in &events {
            let _ = self.event_tx.send(event.clone());
            debug!(event_type = %event_type_name(event), "Event broadcast");
        }

        Ok(events)
    }

    /// Subscribe to the event broadcast channel.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }

    /// Get a read-only reference to the current state.
    pub fn state(&self) -> Arc<RwLock<ReadModel>> {
        Arc::clone(&self.state)
    }

    /// Get a snapshot of the current read model.
    pub fn snapshot(&self) -> ReadModel {
        self.state.read().unwrap().clone()
    }

    /// Get threads for a specific project.
    pub fn threads_for_project(
        &self,
        project_id: &ecode_contracts::ids::ProjectId,
    ) -> Vec<ThreadState> {
        let state = self.state.read().unwrap();
        state
            .threads
            .values()
            .filter(|t| !t.deleted && t.project_id == *project_id)
            .cloned()
            .collect()
    }
}

/// Extract the event type name for storage.
fn event_type_name(event: &Event) -> String {
    match event {
        Event::ThreadCreated { .. } => "ThreadCreated",
        Event::ThreadSettingsUpdated { .. } => "ThreadSettingsUpdated",
        Event::ThreadRenamed { .. } => "ThreadRenamed",
        Event::ThreadDeleted { .. } => "ThreadDeleted",
        Event::TurnStartRequested { .. } => "TurnStartRequested",
        Event::SessionEstablished { .. } => "SessionEstablished",
        Event::SessionCleared { .. } => "SessionCleared",
        Event::SessionStatusChanged { .. } => "SessionStatusChanged",
        Event::TurnStarted { .. } => "TurnStarted",
        Event::RuntimeEventRecorded { .. } => "RuntimeEventRecorded",
        Event::AssistantMessageDelta { .. } => "AssistantMessageDelta",
        Event::TurnCompleted { .. } => "TurnCompleted",
        Event::TurnInterrupted { .. } => "TurnInterrupted",
        Event::TurnsRolledBack { .. } => "TurnsRolledBack",
        Event::ApprovalRequested { .. } => "ApprovalRequested",
        Event::ApprovalResponded { .. } => "ApprovalResponded",
        Event::UserInputRequested { .. } => "UserInputRequested",
        Event::UserInputResponded { .. } => "UserInputResponded",
        Event::ErrorOccurred { .. } => "ErrorOccurred",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecode_contracts::ids::*;

    fn test_engine() -> OrchestrationEngine {
        let store = Arc::new(EventStore::in_memory().unwrap());
        OrchestrationEngine::new(store)
    }

    #[test]
    fn test_dispatch_create_thread() {
        let engine = test_engine();

        let events = engine
            .dispatch(Command::CreateThread {
                thread_id: ThreadId::new(),
                project_id: ProjectId::new(),
                name: Some("Test".to_string()),
                settings: ThreadSettings::default(),
            })
            .unwrap();

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], Event::ThreadCreated { .. }));

        let snapshot = engine.snapshot();
        assert_eq!(snapshot.threads.len(), 1);
    }

    #[test]
    fn test_dispatch_and_rebuild() {
        let store = Arc::new(EventStore::in_memory().unwrap());
        let thread_id = ThreadId::new();
        let project_id = ProjectId::new();

        // Dispatch some events
        {
            let engine = OrchestrationEngine::new(Arc::clone(&store));
            engine
                .dispatch(Command::CreateThread {
                    thread_id,
                    project_id,
                    name: Some("Persistent Thread".to_string()),
                    settings: ThreadSettings::default(),
                })
                .unwrap();
        }

        // Create new engine and rebuild from stored events
        {
            let engine = OrchestrationEngine::new(Arc::clone(&store));
            engine.rebuild().unwrap();

            let snapshot = engine.snapshot();
            assert_eq!(snapshot.threads.len(), 1);
            assert_eq!(snapshot.threads[&thread_id].name, "Persistent Thread");
        }
    }
}
