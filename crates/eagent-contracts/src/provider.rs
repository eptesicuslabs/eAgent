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
