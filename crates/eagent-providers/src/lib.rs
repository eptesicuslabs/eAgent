//! eAgent Providers — trait and implementations for LLM backends.

pub mod registry;

use eagent_contracts::provider::{ModelInfo, ProviderEvent, ProviderSessionStatus};
use eagent_tools::ToolDef;
use std::future::Future;
use std::pin::Pin;
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
///
/// Uses boxed future return types so the trait is dyn-compatible
/// and providers can be stored in the registry as `Arc<dyn Provider>`.
pub trait Provider: Send + Sync {
    /// Create a new session with this provider.
    fn create_session(
        &self,
        config: SessionConfig,
    ) -> Pin<Box<dyn Future<Output = Result<SessionHandle, ProviderError>> + Send + '_>>;

    /// Send messages and tool definitions, receive a stream of ProviderEvents.
    /// The receiver end of the channel will emit ProviderEvents as they arrive.
    fn send(
        &self,
        session: &SessionHandle,
        messages: Vec<ProviderMessage>,
        tools: Vec<ToolDef>,
    ) -> Pin<Box<dyn Future<Output = Result<mpsc::UnboundedReceiver<ProviderEvent>, ProviderError>> + Send + '_>>;

    /// Cancel an active session.
    fn cancel(
        &self,
        session: &SessionHandle,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderError>> + Send + '_>>;

    /// List available models.
    fn list_models(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ModelInfo>, ProviderError>> + Send + '_>>;

    /// Get current session status.
    fn session_status(&self, session: &SessionHandle) -> ProviderSessionStatus;
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::registry::ProviderRegistry;

    #[test]
    fn registry_operations() {
        let reg = ProviderRegistry::new();
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
