//! Shared application state for desktop shells.

use ecode_contracts::config::AppConfig;
use ecode_contracts::ids::*;
use ecode_contracts::orchestration::*;
use ecode_contracts::provider::{DEFAULT_CODEX_MODEL, FALLBACK_CODEX_MODELS};
use ecode_contracts::provider_runtime::ProviderSessionStatus;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalState {
    pub id: TerminalId,
    pub thread_id: Option<ThreadId>,
    pub title: String,
    pub buffer: String,
    pub is_alive: bool,
}

/// Shared application state that flows between desktop shells and background tasks.
pub struct AppState {
    pub read_model: Arc<RwLock<ReadModel>>,
    pub config: Arc<RwLock<AppConfig>>,
    pub config_path: Arc<RwLock<Option<PathBuf>>>,
    pub current_project: Arc<RwLock<Option<String>>>,
    pub current_thread: Arc<RwLock<Option<ThreadId>>>,
    pub terminal_buffers: Arc<RwLock<HashMap<TerminalId, String>>>,
    pub status_message: Arc<RwLock<String>>,
    pub codex_available: Arc<RwLock<bool>>,
    pub codex_version: Arc<RwLock<Option<String>>>,
    pub codex_binary_source: Arc<RwLock<String>>,
    pub codex_resolved_path: Arc<RwLock<Option<String>>>,
    pub codex_models: Arc<RwLock<Vec<String>>>,
    pub recent_projects: Arc<RwLock<Vec<String>>>,
    pub terminals: Arc<RwLock<Vec<TerminalState>>>,
    pub errors: Arc<RwLock<Vec<String>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            read_model: Arc::new(RwLock::new(ReadModel::default())),
            config: Arc::new(RwLock::new(AppConfig::default())),
            config_path: Arc::new(RwLock::new(None)),
            current_project: Arc::new(RwLock::new(None)),
            current_thread: Arc::new(RwLock::new(None)),
            terminal_buffers: Arc::new(RwLock::new(HashMap::new())),
            status_message: Arc::new(RwLock::new("Ready".to_string())),
            codex_available: Arc::new(RwLock::new(false)),
            codex_version: Arc::new(RwLock::new(None)),
            codex_binary_source: Arc::new(RwLock::new("Not checked".to_string())),
            codex_resolved_path: Arc::new(RwLock::new(None)),
            codex_models: Arc::new(RwLock::new(
                FALLBACK_CODEX_MODELS
                    .iter()
                    .map(|model| (*model).to_string())
                    .collect(),
            )),
            recent_projects: Arc::new(RwLock::new(Vec::new())),
            terminals: Arc::new(RwLock::new(Vec::new())),
            errors: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn set_status(&self, msg: impl Into<String>) {
        *self.status_message.write().unwrap() = msg.into();
    }

    pub fn push_error(&self, msg: impl Into<String>) {
        self.errors.write().unwrap().push(msg.into());
    }

    pub fn drain_errors(&self) -> Vec<String> {
        std::mem::take(&mut *self.errors.write().unwrap())
    }

    pub fn current_thread_state(&self) -> Option<ThreadState> {
        let thread_id = *self.current_thread.read().unwrap();
        let model = self.read_model.read().unwrap();
        model.threads.get(&thread_id?).cloned()
    }

    pub fn current_thread_session_status(&self) -> Option<ProviderSessionStatus> {
        self.current_thread_state()
            .and_then(|thread| thread.session.map(|session| session.status))
    }

    pub fn current_thread_busy(&self) -> bool {
        self.current_thread_state().is_some_and(|thread| {
            thread.active_turn.is_some()
                || matches!(
                    thread.session.as_ref().map(|session| session.status),
                    Some(
                        ProviderSessionStatus::Starting
                            | ProviderSessionStatus::Running
                            | ProviderSessionStatus::Waiting
                    )
                )
        })
    }

    pub fn preferred_codex_model(&self) -> String {
        self.codex_models
            .read()
            .unwrap()
            .first()
            .cloned()
            .unwrap_or_else(|| DEFAULT_CODEX_MODEL.to_string())
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
