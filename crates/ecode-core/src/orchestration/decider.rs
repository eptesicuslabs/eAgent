//! Decider — pure function mapping (Command, State) → Vec<Event>.
//!
//! The decider is the heart of the CQRS pattern. It validates commands
//! against the current read model and produces events if valid.

use chrono::Utc;
use ecode_contracts::orchestration::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DecisionError {
    #[error("Thread {0} not found")]
    ThreadNotFound(String),
    #[error("Thread {0} already exists")]
    ThreadAlreadyExists(String),
    #[error("Thread {0} has been deleted")]
    ThreadDeleted(String),
    #[error("Thread {0} already has an active turn")]
    TurnAlreadyActive(String),
    #[error("Turn {0} not found in thread")]
    TurnNotFound(String),
    #[error("Turn {0} is not active, cannot interrupt")]
    TurnNotActive(String),
    #[error("No turns to rollback")]
    NoTurnsToRollback,
    #[error("Approval {0} not found")]
    ApprovalNotFound(String),
    #[error("User input {0} not found")]
    UserInputNotFound(String),
}

/// Decide what events should be produced for a given command.
///
/// This is a pure function — no side effects, no I/O.
pub fn decide(command: &Command, state: &ReadModel) -> Result<Vec<Event>, DecisionError> {
    let now = Utc::now();

    match command {
        Command::CreateThread {
            thread_id,
            project_id,
            name,
            settings,
        } => {
            if state.threads.contains_key(thread_id) {
                return Err(DecisionError::ThreadAlreadyExists(thread_id.to_string()));
            }
            Ok(vec![Event::ThreadCreated {
                thread_id: *thread_id,
                project_id: *project_id,
                name: name
                    .clone()
                    .unwrap_or_else(|| format!("Thread {}", &thread_id.to_string()[..8])),
                settings: settings.clone(),
                timestamp: now,
            }])
        }

        Command::UpdateThreadSettings {
            thread_id,
            settings,
        } => {
            let thread = state
                .threads
                .get(thread_id)
                .ok_or_else(|| DecisionError::ThreadNotFound(thread_id.to_string()))?;

            if thread.deleted {
                return Err(DecisionError::ThreadDeleted(thread_id.to_string()));
            }

            Ok(vec![Event::ThreadSettingsUpdated {
                thread_id: *thread_id,
                settings: settings.clone(),
                timestamp: now,
            }])
        }

        Command::StartTurn {
            thread_id,
            turn_id,
            input,
            images,
        } => {
            let thread = state
                .threads
                .get(thread_id)
                .ok_or_else(|| DecisionError::ThreadNotFound(thread_id.to_string()))?;

            if thread.deleted {
                return Err(DecisionError::ThreadDeleted(thread_id.to_string()));
            }

            if thread.active_turn.is_some() {
                return Err(DecisionError::TurnAlreadyActive(thread_id.to_string()));
            }

            Ok(vec![Event::TurnStartRequested {
                thread_id: *thread_id,
                turn_id: *turn_id,
                input: input.clone(),
                images: images.clone(),
                settings_snapshot: thread.settings.clone(),
                timestamp: now,
            }])
        }

        Command::InterruptTurn { thread_id, turn_id } => {
            let thread = state
                .threads
                .get(thread_id)
                .ok_or_else(|| DecisionError::ThreadNotFound(thread_id.to_string()))?;

            if thread.active_turn.as_ref() != Some(turn_id) {
                return Err(DecisionError::TurnNotActive(turn_id.to_string()));
            }

            Ok(vec![Event::TurnInterrupted {
                thread_id: *thread_id,
                turn_id: *turn_id,
                timestamp: now,
            }])
        }

        Command::RollbackTurns { thread_id, n } => {
            let thread = state
                .threads
                .get(thread_id)
                .ok_or_else(|| DecisionError::ThreadNotFound(thread_id.to_string()))?;

            if thread.turns.is_empty() {
                return Err(DecisionError::NoTurnsToRollback);
            }

            Ok(vec![Event::TurnsRolledBack {
                thread_id: *thread_id,
                n: *n,
                timestamp: now,
            }])
        }

        Command::DeleteThread { thread_id } => {
            if !state.threads.contains_key(thread_id) {
                return Err(DecisionError::ThreadNotFound(thread_id.to_string()));
            }

            Ok(vec![Event::ThreadDeleted {
                thread_id: *thread_id,
                timestamp: now,
            }])
        }

        Command::RenameThread { thread_id, name } => {
            let thread = state
                .threads
                .get(thread_id)
                .ok_or_else(|| DecisionError::ThreadNotFound(thread_id.to_string()))?;

            if thread.deleted {
                return Err(DecisionError::ThreadDeleted(thread_id.to_string()));
            }

            Ok(vec![Event::ThreadRenamed {
                thread_id: *thread_id,
                name: name.clone(),
                timestamp: now,
            }])
        }

        // ── Internal commands ──
        Command::SetSession {
            thread_id,
            session_id,
            provider,
            provider_thread_id,
        } => Ok(vec![Event::SessionEstablished {
            thread_id: *thread_id,
            session_id: *session_id,
            provider: *provider,
            provider_thread_id: provider_thread_id.clone(),
            timestamp: now,
        }]),

        Command::ClearSession { thread_id } => Ok(vec![Event::SessionCleared {
            thread_id: *thread_id,
            timestamp: now,
        }]),

        Command::SetSessionStatus {
            thread_id,
            status,
            message,
        } => Ok(vec![Event::SessionStatusChanged {
            thread_id: *thread_id,
            status: *status,
            message: message.clone(),
            timestamp: now,
        }]),

        Command::RecordTurnStarted {
            thread_id,
            turn_id,
            provider_turn_id,
        } => Ok(vec![Event::TurnStarted {
            thread_id: *thread_id,
            turn_id: *turn_id,
            provider_turn_id: provider_turn_id.clone(),
            timestamp: now,
        }]),

        Command::RecordRuntimeEvent {
            thread_id,
            turn_id,
            event_type,
            item_id,
            request_id,
            summary,
            data,
        } => Ok(vec![Event::RuntimeEventRecorded {
            thread_id: *thread_id,
            turn_id: *turn_id,
            event_type: *event_type,
            item_id: item_id.clone(),
            request_id: *request_id,
            summary: summary.clone(),
            data: data.clone(),
            timestamp: now,
        }]),

        Command::AppendAssistantDelta {
            thread_id,
            turn_id,
            item_id,
            delta,
        } => Ok(vec![Event::AssistantMessageDelta {
            thread_id: *thread_id,
            turn_id: *turn_id,
            item_id: item_id.clone(),
            delta: delta.clone(),
            timestamp: now,
        }]),

        Command::CompleteTurn {
            thread_id,
            turn_id,
            status,
        } => Ok(vec![Event::TurnCompleted {
            thread_id: *thread_id,
            turn_id: *turn_id,
            status: status.clone(),
            timestamp: now,
        }]),

        Command::RecordApprovalRequest {
            thread_id,
            turn_id,
            approval_id,
            rpc_id,
            kind,
            details,
        } => Ok(vec![Event::ApprovalRequested {
            thread_id: *thread_id,
            turn_id: *turn_id,
            approval_id: *approval_id,
            rpc_id: *rpc_id,
            kind: kind.clone(),
            details: details.clone(),
            timestamp: now,
        }]),

        Command::RecordApprovalResponse {
            thread_id,
            approval_id,
            decision,
        } => {
            let thread = state
                .threads
                .get(thread_id)
                .ok_or_else(|| DecisionError::ThreadNotFound(thread_id.to_string()))?;

            if !thread.pending_approvals.contains_key(approval_id) {
                return Err(DecisionError::ApprovalNotFound(approval_id.to_string()));
            }

            Ok(vec![Event::ApprovalResponded {
                thread_id: *thread_id,
                approval_id: *approval_id,
                decision: decision.clone(),
                timestamp: now,
            }])
        }

        Command::RecordUserInputRequest {
            thread_id,
            turn_id,
            approval_id,
            rpc_id,
            questions,
        } => Ok(vec![Event::UserInputRequested {
            thread_id: *thread_id,
            turn_id: *turn_id,
            approval_id: *approval_id,
            rpc_id: *rpc_id,
            questions: questions.clone(),
            timestamp: now,
        }]),

        Command::RecordUserInputResponse {
            thread_id,
            approval_id,
            answers,
        } => {
            let thread = state
                .threads
                .get(thread_id)
                .ok_or_else(|| DecisionError::ThreadNotFound(thread_id.to_string()))?;

            if !thread.pending_inputs.contains_key(approval_id) {
                return Err(DecisionError::UserInputNotFound(approval_id.to_string()));
            }

            Ok(vec![Event::UserInputResponded {
                thread_id: *thread_id,
                approval_id: *approval_id,
                answers: answers.clone(),
                timestamp: now,
            }])
        }

        Command::RecordError {
            thread_id,
            message,
            will_retry,
        } => Ok(vec![Event::ErrorOccurred {
            thread_id: *thread_id,
            message: message.clone(),
            will_retry: *will_retry,
            timestamp: now,
        }]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecode_contracts::ids::*;

    #[test]
    fn test_create_thread() {
        let state = ReadModel::default();
        let cmd = Command::CreateThread {
            thread_id: ThreadId::new(),
            project_id: ProjectId::new(),
            name: Some("My Thread".to_string()),
            settings: ThreadSettings::default(),
        };

        let events = decide(&cmd, &state).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], Event::ThreadCreated { name, .. } if name == "My Thread"));
    }

    #[test]
    fn test_create_duplicate_thread_fails() {
        let mut state = ReadModel::default();
        let thread_id = ThreadId::new();

        state.threads.insert(
            thread_id,
            ThreadState {
                id: thread_id,
                project_id: ProjectId::new(),
                name: "Existing".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                settings: ThreadSettings::default(),
                turns: vec![],
                session: None,
                active_turn: None,
                pending_approvals: Default::default(),
                pending_inputs: Default::default(),
                runtime_events: vec![],
                errors: vec![],
                deleted: false,
            },
        );

        let cmd = Command::CreateThread {
            thread_id,
            project_id: ProjectId::new(),
            name: None,
            settings: ThreadSettings::default(),
        };

        assert!(decide(&cmd, &state).is_err());
    }

    #[test]
    fn test_start_turn_uses_thread_settings_snapshot() {
        let mut state = ReadModel::default();
        let thread_id = ThreadId::new();
        let settings = ThreadSettings {
            model: "snapshot-model".to_string(),
            ..ThreadSettings::default()
        };

        state.threads.insert(
            thread_id,
            ThreadState {
                id: thread_id,
                project_id: ProjectId::new(),
                name: "Thread".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                settings: settings.clone(),
                turns: vec![],
                session: None,
                active_turn: None,
                pending_approvals: Default::default(),
                pending_inputs: Default::default(),
                runtime_events: vec![],
                errors: vec![],
                deleted: false,
            },
        );

        let cmd = Command::StartTurn {
            thread_id,
            turn_id: TurnId::new(),
            input: "Hello".to_string(),
            images: vec![],
        };

        let events = decide(&cmd, &state).unwrap();
        assert!(matches!(
            &events[0],
            Event::TurnStartRequested {
                settings_snapshot,
                ..
            } if settings_snapshot.model == settings.model
        ));
    }
}
