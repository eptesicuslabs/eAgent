use ecode_contracts::config::AppConfig;
use ecode_contracts::git::{BranchInfo, FileDiff, FileStatus, WorktreeInfo};
use ecode_contracts::ids::{ApprovalRequestId, TerminalId, ThreadId};
use ecode_contracts::orchestration::{
    CodexReasoningEffort, InteractionMode, ReadModel, RuntimeMode, ThreadSettings,
};
use ecode_contracts::provider::ProviderKind;
use ecode_desktop_app::{AppState, TerminalState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppBootstrapPayload {
    pub app_name: &'static str,
    pub shell: &'static str,
    pub migration_stage: &'static str,
    pub current_project: Option<String>,
    pub current_thread_id: Option<String>,
    pub status_message: String,
    pub codex_available: bool,
    pub codex_version: Option<String>,
    pub codex_binary_source: String,
    pub codex_resolved_path: Option<String>,
    pub codex_models: Vec<String>,
    pub config_path: Option<String>,
    pub config: AppConfig,
    pub recent_projects: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestrationSnapshotPayload {
    pub read_model: ReadModel,
    pub current_project: Option<String>,
    pub current_thread_id: Option<String>,
    pub terminals: Vec<TerminalState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatusPayload {
    pub status_message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusPayload {
    pub is_git_repo: bool,
    pub current_branch: Option<String>,
    pub statuses: Vec<FileStatus>,
    pub diffs: Vec<FileDiff>,
    pub worktrees: Vec<WorktreeInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCreateWorktreeInput {
    pub cwd: String,
    pub name: String,
    pub path: String,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRemoveWorktreeInput {
    pub cwd: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSearchEntry {
    pub path: String,
    pub is_directory: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSearchEntriesInput {
    pub cwd: String,
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWriteFileInput {
    pub cwd: String,
    pub relative_path: String,
    pub contents: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWriteFileResult {
    pub relative_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum OrchestrationCommandInput {
    CreateThread {
        name: String,
    },
    SelectThread {
        thread_id: String,
    },
    DeleteThread {
        thread_id: String,
    },
    RenameThread {
        thread_id: String,
        name: String,
    },
    SendMessage {
        message: String,
    },
    InterruptTurn,
    UpdateCurrentThreadSettings {
        settings: ThreadSettingsInput,
    },
    Approve {
        approval_id: String,
    },
    Deny {
        approval_id: String,
    },
    UserInputResponse {
        approval_id: String,
        response: String,
    },
    OpenProject {
        path: String,
    },
    OpenTerminal,
    SendTerminalInput {
        terminal_id: String,
        input: String,
    },
    ResizeTerminal {
        terminal_id: String,
        cols: u16,
        rows: u16,
    },
    CloseTerminal {
        terminal_id: String,
    },
    ClearTerminal {
        terminal_id: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSettingsInput {
    pub provider: ProviderKindInput,
    pub model: String,
    pub runtime_mode: RuntimeModeInput,
    pub interaction_mode: InteractionModeInput,
    pub codex_reasoning_effort: CodexReasoningEffortInput,
    pub codex_fast_mode: bool,
    pub local_agent_web_search_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKindInput {
    Codex,
    LlamaCpp,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeModeInput {
    ApprovalRequired,
    FullAccess,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionModeInput {
    Chat,
    Plan,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodexReasoningEffortInput {
    Low,
    Medium,
    High,
}

impl From<ProviderKindInput> for ProviderKind {
    fn from(value: ProviderKindInput) -> Self {
        match value {
            ProviderKindInput::Codex => ProviderKind::Codex,
            ProviderKindInput::LlamaCpp => ProviderKind::LlamaCpp,
        }
    }
}

impl From<RuntimeModeInput> for RuntimeMode {
    fn from(value: RuntimeModeInput) -> Self {
        match value {
            RuntimeModeInput::ApprovalRequired => RuntimeMode::ApprovalRequired,
            RuntimeModeInput::FullAccess => RuntimeMode::FullAccess,
        }
    }
}

impl From<InteractionModeInput> for InteractionMode {
    fn from(value: InteractionModeInput) -> Self {
        match value {
            InteractionModeInput::Chat => InteractionMode::Chat,
            InteractionModeInput::Plan => InteractionMode::Plan,
        }
    }
}

impl From<CodexReasoningEffortInput> for CodexReasoningEffort {
    fn from(value: CodexReasoningEffortInput) -> Self {
        match value {
            CodexReasoningEffortInput::Low => CodexReasoningEffort::Low,
            CodexReasoningEffortInput::Medium => CodexReasoningEffort::Medium,
            CodexReasoningEffortInput::High => CodexReasoningEffort::High,
        }
    }
}

impl From<ThreadSettingsInput> for ThreadSettings {
    fn from(value: ThreadSettingsInput) -> Self {
        ThreadSettings {
            provider: value.provider.into(),
            model: value.model,
            runtime_mode: value.runtime_mode.into(),
            interaction_mode: value.interaction_mode.into(),
            codex_reasoning_effort: value.codex_reasoning_effort.into(),
            codex_fast_mode: value.codex_fast_mode,
            local_agent_web_search_enabled: value.local_agent_web_search_enabled,
        }
    }
}

impl AppBootstrapPayload {
    pub fn from_state(state: &AppState) -> Self {
        Self {
            app_name: "eCode",
            shell: "Tauri + React",
            migration_stage: "t3-shell-rewrite",
            current_project: state.current_project.read().unwrap().clone(),
            current_thread_id: state.current_thread.read().unwrap().map(|id| id.to_string()),
            status_message: state.status_message.read().unwrap().clone(),
            codex_available: *state.codex_available.read().unwrap(),
            codex_version: state.codex_version.read().unwrap().clone(),
            codex_binary_source: state.codex_binary_source.read().unwrap().clone(),
            codex_resolved_path: state.codex_resolved_path.read().unwrap().clone(),
            codex_models: state.codex_models.read().unwrap().clone(),
            config_path: state
                .config_path
                .read()
                .unwrap()
                .as_ref()
                .map(|path| path.display().to_string()),
            config: state.config.read().unwrap().clone(),
            recent_projects: state.recent_projects.read().unwrap().clone(),
        }
    }
}

impl OrchestrationSnapshotPayload {
    pub fn from_state(state: &AppState) -> Self {
        Self {
            read_model: state.read_model.read().unwrap().clone(),
            current_project: state.current_project.read().unwrap().clone(),
            current_thread_id: state.current_thread.read().unwrap().map(|id| id.to_string()),
            terminals: state.terminals.read().unwrap().clone(),
        }
    }
}

pub fn parse_thread_id(value: &str) -> Result<ThreadId, String> {
    ThreadId::parse(value).map_err(|error| error.to_string())
}

pub fn parse_terminal_id(value: &str) -> Result<TerminalId, String> {
    TerminalId::parse(value).map_err(|error| error.to_string())
}

pub fn parse_approval_id(value: &str) -> Result<ApprovalRequestId, String> {
    ApprovalRequestId::parse(value).map_err(|error| error.to_string())
}

pub fn branch_list_payload(branches: Vec<BranchInfo>) -> Vec<BranchInfo> {
    branches
}
