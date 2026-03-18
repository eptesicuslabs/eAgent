//! Projector — pure function applying events to the read model.
//!
//! Given the current state and an event, produce the new state.
//! This is deterministic and replay-safe.

use ecode_contracts::orchestration::*;
use ecode_contracts::provider_runtime::{ProviderRuntimeEvent, ProviderSessionStatus};

const MAX_RUNTIME_EVENTS: usize = 256;

/// Apply a single event to the read model, mutating it in place.
pub fn apply(state: &mut ReadModel, event: &Event) {
    match event {
        Event::ThreadCreated {
            thread_id,
            project_id,
            name,
            settings,
            timestamp,
        } => {
            state.threads.insert(
                *thread_id,
                ThreadState {
                    id: *thread_id,
                    project_id: *project_id,
                    name: name.clone(),
                    created_at: *timestamp,
                    updated_at: *timestamp,
                    settings: settings.clone(),
                    turns: Vec::new(),
                    session: None,
                    active_turn: None,
                    pending_approvals: Default::default(),
                    pending_inputs: Default::default(),
                    runtime_events: Vec::new(),
                    errors: Vec::new(),
                    deleted: false,
                },
            );

            if let Some(project) = state.projects.get_mut(project_id)
                && !project.thread_ids.contains(thread_id)
            {
                project.thread_ids.push(*thread_id);
            }
        }

        Event::ThreadSettingsUpdated {
            thread_id,
            settings,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                thread.settings = settings.clone();
                thread.updated_at = *timestamp;
            }
        }

        Event::ThreadRenamed {
            thread_id,
            name,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                thread.name = name.clone();
                thread.updated_at = *timestamp;
            }
        }

        Event::ThreadDeleted {
            thread_id,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                thread.deleted = true;
                thread.updated_at = *timestamp;
            }
        }

        Event::TurnStartRequested {
            thread_id,
            turn_id,
            input,
            images,
            settings_snapshot,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                let turn = TurnState {
                    id: *turn_id,
                    input: input.clone(),
                    images: images.clone(),
                    settings_snapshot: settings_snapshot.clone(),
                    status: TurnStatus::Requested,
                    messages: vec![MessageItem {
                        item_id: format!("user-{}", turn_id),
                        role: MessageRole::User,
                        content: input.clone(),
                        timestamp: *timestamp,
                    }],
                    started_at: *timestamp,
                    completed_at: None,
                    provider_turn_id: None,
                };
                thread.turns.push(turn);
                thread.active_turn = Some(*turn_id);
                if let Some(session) = thread.session.as_mut() {
                    session.status = ProviderSessionStatus::Starting;
                    session.last_message = Some("Starting turn".to_string());
                }
                thread.updated_at = *timestamp;
            }
        }

        Event::SessionEstablished {
            thread_id,
            session_id,
            provider,
            provider_thread_id,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                thread.session = Some(SessionState {
                    session_id: *session_id,
                    provider: *provider,
                    provider_thread_id: provider_thread_id.clone(),
                    status: ProviderSessionStatus::Ready,
                    established_at: *timestamp,
                    last_error: None,
                    last_message: Some("Session established".to_string()),
                });
                thread.updated_at = *timestamp;
            }
        }

        Event::SessionCleared {
            thread_id,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                thread.session = None;
                thread.updated_at = *timestamp;
            }
        }

        Event::SessionStatusChanged {
            thread_id,
            status,
            message,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                if let Some(session) = thread.session.as_mut() {
                    session.status = *status;
                    session.last_message = message.clone();
                    if *status == ProviderSessionStatus::Error {
                        session.last_error = message.clone();
                    }
                }
                thread.updated_at = *timestamp;
            }
        }

        Event::TurnStarted {
            thread_id,
            turn_id,
            provider_turn_id,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                if let Some(turn) = thread.turns.iter_mut().find(|t| t.id == *turn_id) {
                    turn.status = TurnStatus::Running;
                    turn.provider_turn_id = Some(provider_turn_id.clone());
                }
                if let Some(session) = thread.session.as_mut() {
                    session.status = ProviderSessionStatus::Running;
                    session.last_message = Some("Turn running".to_string());
                }
                thread.updated_at = *timestamp;
            }
        }

        Event::RuntimeEventRecorded {
            thread_id,
            turn_id,
            event_type,
            item_id,
            request_id,
            summary,
            data,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                let provider = thread
                    .session
                    .as_ref()
                    .map(|session| session.provider)
                    .unwrap_or(thread.settings.provider);
                thread.runtime_events.push(ProviderRuntimeEvent {
                    provider,
                    event_type: *event_type,
                    turn_id: *turn_id,
                    item_id: item_id.clone(),
                    request_id: *request_id,
                    summary: summary.clone(),
                    data: data.clone(),
                    timestamp: *timestamp,
                });
                if thread.runtime_events.len() > MAX_RUNTIME_EVENTS {
                    let overflow = thread.runtime_events.len() - MAX_RUNTIME_EVENTS;
                    thread.runtime_events.drain(..overflow);
                }
                thread.updated_at = *timestamp;
            }
        }

        Event::AssistantMessageDelta {
            thread_id,
            turn_id,
            item_id,
            delta,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                if let Some(turn) = thread.turns.iter_mut().find(|t| t.id == *turn_id) {
                    if let Some(msg) = turn.messages.iter_mut().find(|m| m.item_id == *item_id) {
                        msg.content.push_str(delta);
                    } else {
                        turn.messages.push(MessageItem {
                            item_id: item_id.clone(),
                            role: MessageRole::Assistant,
                            content: delta.clone(),
                            timestamp: *timestamp,
                        });
                    }
                }
                thread.updated_at = *timestamp;
            }
        }

        Event::TurnCompleted {
            thread_id,
            turn_id,
            status,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                if let Some(turn) = thread.turns.iter_mut().find(|t| t.id == *turn_id) {
                    turn.status = if status == "completed" {
                        TurnStatus::Completed
                    } else {
                        TurnStatus::Failed
                    };
                    turn.completed_at = Some(*timestamp);
                }
                if thread.active_turn == Some(*turn_id) {
                    thread.active_turn = None;
                }
                if let Some(session) = thread.session.as_mut()
                    && !matches!(
                        session.status,
                        ProviderSessionStatus::Error | ProviderSessionStatus::Stopped
                    )
                {
                    session.status = ProviderSessionStatus::Ready;
                    session.last_message = Some(format!("Turn {}", status));
                }
                thread.updated_at = *timestamp;
            }
        }

        Event::TurnInterrupted {
            thread_id,
            turn_id,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                if let Some(turn) = thread.turns.iter_mut().find(|t| t.id == *turn_id) {
                    turn.status = TurnStatus::Interrupted;
                    turn.completed_at = Some(*timestamp);
                }
                if thread.active_turn == Some(*turn_id) {
                    thread.active_turn = None;
                }
                if let Some(session) = thread.session.as_mut()
                    && !matches!(
                        session.status,
                        ProviderSessionStatus::Error | ProviderSessionStatus::Stopped
                    )
                {
                    session.status = ProviderSessionStatus::Ready;
                    session.last_message = Some("Interrupted".to_string());
                }
                thread.updated_at = *timestamp;
            }
        }

        Event::TurnsRolledBack {
            thread_id,
            n,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                let n = *n as usize;
                let len = thread.turns.len();
                if n <= len {
                    thread.turns.truncate(len - n);
                }
                thread.active_turn = None;
                if let Some(session) = thread.session.as_mut() {
                    session.status = ProviderSessionStatus::Ready;
                    session.last_message = Some("Rolled back turns".to_string());
                }
                thread.updated_at = *timestamp;
            }
        }

        Event::ApprovalRequested {
            thread_id,
            turn_id,
            approval_id,
            rpc_id,
            kind,
            details,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                thread.pending_approvals.insert(
                    *approval_id,
                    PendingApproval {
                        id: *approval_id,
                        turn_id: *turn_id,
                        rpc_id: *rpc_id,
                        kind: kind.clone(),
                        details: details.clone(),
                        requested_at: *timestamp,
                    },
                );
                if let Some(turn) = thread.turns.iter_mut().find(|t| t.id == *turn_id) {
                    turn.status = TurnStatus::Waiting;
                }
                if let Some(session) = thread.session.as_mut() {
                    session.status = ProviderSessionStatus::Waiting;
                    session.last_message = Some("Waiting for approval".to_string());
                }
                thread.updated_at = *timestamp;
            }
        }

        Event::ApprovalResponded {
            thread_id,
            approval_id,
            timestamp,
            ..
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                let turn_id = thread
                    .pending_approvals
                    .remove(approval_id)
                    .map(|pending| pending.turn_id);
                if let Some(turn_id) = turn_id
                    && let Some(turn) = thread.turns.iter_mut().find(|t| t.id == turn_id)
                    && thread.active_turn == Some(turn_id)
                {
                    turn.status = TurnStatus::Running;
                }
                if let Some(session) = thread.session.as_mut() {
                    session.status = if thread.active_turn.is_some() {
                        ProviderSessionStatus::Running
                    } else {
                        ProviderSessionStatus::Ready
                    };
                    session.last_message = Some("Approval resolved".to_string());
                }
                thread.updated_at = *timestamp;
            }
        }

        Event::UserInputRequested {
            thread_id,
            turn_id,
            approval_id,
            rpc_id,
            questions,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                thread.pending_inputs.insert(
                    *approval_id,
                    PendingUserInput {
                        id: *approval_id,
                        turn_id: *turn_id,
                        rpc_id: *rpc_id,
                        questions: questions.clone(),
                        requested_at: *timestamp,
                    },
                );
                if let Some(turn) = thread.turns.iter_mut().find(|t| t.id == *turn_id) {
                    turn.status = TurnStatus::Waiting;
                }
                if let Some(session) = thread.session.as_mut() {
                    session.status = ProviderSessionStatus::Waiting;
                    session.last_message = Some("Waiting for user input".to_string());
                }
                thread.updated_at = *timestamp;
            }
        }

        Event::UserInputResponded {
            thread_id,
            approval_id,
            timestamp,
            ..
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                let turn_id = thread
                    .pending_inputs
                    .remove(approval_id)
                    .map(|pending| pending.turn_id);
                if let Some(turn_id) = turn_id
                    && let Some(turn) = thread.turns.iter_mut().find(|t| t.id == turn_id)
                    && thread.active_turn == Some(turn_id)
                {
                    turn.status = TurnStatus::Running;
                }
                if let Some(session) = thread.session.as_mut() {
                    session.status = if thread.active_turn.is_some() {
                        ProviderSessionStatus::Running
                    } else {
                        ProviderSessionStatus::Ready
                    };
                    session.last_message = Some("User input resolved".to_string());
                }
                thread.updated_at = *timestamp;
            }
        }

        Event::ErrorOccurred {
            thread_id,
            message,
            will_retry,
            timestamp,
        } => {
            if let Some(thread) = state.threads.get_mut(thread_id) {
                thread.errors.push(ThreadError {
                    message: message.clone(),
                    will_retry: *will_retry,
                    timestamp: *timestamp,
                });
                if let Some(session) = thread.session.as_mut() {
                    session.last_error = Some(message.clone());
                    session.last_message = Some(message.clone());
                    if !will_retry {
                        session.status = ProviderSessionStatus::Error;
                    }
                }
                thread.updated_at = *timestamp;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use ecode_contracts::ids::*;
    use ecode_contracts::provider::ProviderKind;
    use ecode_contracts::provider_runtime::ProviderRuntimeEventKind;

    #[test]
    fn test_thread_created() {
        let mut state = ReadModel::default();
        let thread_id = ThreadId::new();
        let project_id = ProjectId::new();

        apply(
            &mut state,
            &Event::ThreadCreated {
                thread_id,
                project_id,
                name: "Test".to_string(),
                settings: ThreadSettings::default(),
                timestamp: Utc::now(),
            },
        );

        assert!(state.threads.contains_key(&thread_id));
        assert_eq!(state.threads[&thread_id].name, "Test");
        assert!(!state.threads[&thread_id].deleted);
    }

    #[test]
    fn test_turn_lifecycle() {
        let mut state = ReadModel::default();
        let thread_id = ThreadId::new();
        let turn_id = TurnId::new();
        let project_id = ProjectId::new();
        let now = Utc::now();

        apply(
            &mut state,
            &Event::ThreadCreated {
                thread_id,
                project_id,
                name: "Test".to_string(),
                settings: ThreadSettings::default(),
                timestamp: now,
            },
        );

        apply(
            &mut state,
            &Event::SessionEstablished {
                thread_id,
                session_id: SessionId::new(),
                provider: ProviderKind::Codex,
                provider_thread_id: "provider-thread".to_string(),
                timestamp: now,
            },
        );

        apply(
            &mut state,
            &Event::TurnStartRequested {
                thread_id,
                turn_id,
                input: "Hello".to_string(),
                images: vec![],
                settings_snapshot: ThreadSettings::default(),
                timestamp: now,
            },
        );

        assert_eq!(state.threads[&thread_id].active_turn, Some(turn_id));
        assert_eq!(state.threads[&thread_id].turns.len(), 1);
        assert_eq!(
            state.threads[&thread_id].turns[0].status,
            TurnStatus::Requested
        );

        apply(
            &mut state,
            &Event::TurnStarted {
                thread_id,
                turn_id,
                provider_turn_id: "provider-turn".to_string(),
                timestamp: now,
            },
        );

        assert_eq!(
            state.threads[&thread_id].turns[0].status,
            TurnStatus::Running
        );

        apply(
            &mut state,
            &Event::AssistantMessageDelta {
                thread_id,
                turn_id,
                item_id: "msg-1".to_string(),
                delta: "Hi there!".to_string(),
                timestamp: now,
            },
        );

        assert_eq!(state.threads[&thread_id].turns[0].messages.len(), 2);
        assert_eq!(
            state.threads[&thread_id].turns[0].messages[1].content,
            "Hi there!"
        );

        apply(
            &mut state,
            &Event::TurnCompleted {
                thread_id,
                turn_id,
                status: "completed".to_string(),
                timestamp: now,
            },
        );

        assert_eq!(state.threads[&thread_id].active_turn, None);
        assert_eq!(
            state.threads[&thread_id].turns[0].status,
            TurnStatus::Completed
        );
        assert_eq!(
            state.threads[&thread_id]
                .session
                .as_ref()
                .map(|session| session.status),
            Some(ProviderSessionStatus::Ready)
        );
    }

    #[test]
    fn test_message_delta_appending() {
        let mut state = ReadModel::default();
        let thread_id = ThreadId::new();
        let turn_id = TurnId::new();
        let now = Utc::now();

        apply(
            &mut state,
            &Event::ThreadCreated {
                thread_id,
                project_id: ProjectId::new(),
                name: "Test".to_string(),
                settings: ThreadSettings::default(),
                timestamp: now,
            },
        );
        apply(
            &mut state,
            &Event::TurnStartRequested {
                thread_id,
                turn_id,
                input: "Hello".to_string(),
                images: vec![],
                settings_snapshot: ThreadSettings::default(),
                timestamp: now,
            },
        );

        apply(
            &mut state,
            &Event::AssistantMessageDelta {
                thread_id,
                turn_id,
                item_id: "msg-1".to_string(),
                delta: "Hello ".to_string(),
                timestamp: now,
            },
        );
        apply(
            &mut state,
            &Event::AssistantMessageDelta {
                thread_id,
                turn_id,
                item_id: "msg-1".to_string(),
                delta: "world!".to_string(),
                timestamp: now,
            },
        );

        let turn = &state.threads[&thread_id].turns[0];
        let assistant_msg = turn.messages.iter().find(|m| m.item_id == "msg-1").unwrap();
        assert_eq!(assistant_msg.content, "Hello world!");
    }

    #[test]
    fn test_runtime_event_recorded() {
        let mut state = ReadModel::default();
        let thread_id = ThreadId::new();
        let now = Utc::now();

        apply(
            &mut state,
            &Event::ThreadCreated {
                thread_id,
                project_id: ProjectId::new(),
                name: "Test".to_string(),
                settings: ThreadSettings::default(),
                timestamp: now,
            },
        );
        apply(
            &mut state,
            &Event::RuntimeEventRecorded {
                thread_id,
                turn_id: None,
                event_type: ProviderRuntimeEventKind::RuntimeWarning,
                item_id: None,
                request_id: None,
                summary: Some("warning".to_string()),
                data: serde_json::Value::Null,
                timestamp: now,
            },
        );

        assert_eq!(state.threads[&thread_id].runtime_events.len(), 1);
    }
}
