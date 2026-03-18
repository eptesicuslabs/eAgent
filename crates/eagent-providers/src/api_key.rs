//! ApiKeyProvider — generic OpenAI-compatible API provider.
//!
//! Works with any endpoint that speaks the OpenAI `/v1/chat/completions` SSE
//! protocol: OpenAI, Anthropic (via proxy), Together, Groq, vLLM, local
//! servers, etc.

use crate::sse::read_sse_stream;
use crate::{Provider, ProviderError, ProviderMessage, ProviderMessageRole, SessionConfig, SessionHandle};
use eagent_contracts::provider::{ModelInfo, ProviderEvent, ProviderKind, ProviderSessionStatus};
use eagent_tools::ToolDef;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Configuration for a generic OpenAI-compatible API endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyConfig {
    /// Base URL of the API, e.g. `"https://api.openai.com/v1"`.
    pub endpoint: String,
    /// Bearer token sent in the `Authorization` header.
    pub api_key: String,
    /// Default model to use when the session config does not specify one.
    pub default_model: String,
    /// Advertised context window for models from this provider.
    #[serde(default = "default_max_context")]
    pub max_context: u32,
}

fn default_max_context() -> u32 {
    128_000
}

impl Default for ApiKeyConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://api.openai.com/v1".into(),
            api_key: String::new(),
            default_model: "gpt-4o".into(),
            max_context: default_max_context(),
        }
    }
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

/// A stateless, HTTP-based provider for any OpenAI-compatible API.
pub struct ApiKeyProvider {
    client: reqwest::Client,
    config: ApiKeyConfig,
    /// Last known session status (best-effort).
    status: Arc<Mutex<ProviderSessionStatus>>,
}

impl ApiKeyProvider {
    pub fn new(config: ApiKeyConfig) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(300))
            .redirect(reqwest::redirect::Policy::limited(3))
            .user_agent("eAgent-api-key/0.1")
            .build()
            .expect("failed to build reqwest client for api-key provider");

        Self {
            client,
            config,
            status: Arc::new(Mutex::new(ProviderSessionStatus::Ready)),
        }
    }

    // -- request helpers ----------------------------------------------------

    fn build_messages_payload(messages: &[ProviderMessage]) -> Vec<Value> {
        messages
            .iter()
            .map(|m| {
                json!({
                    "role": match m.role {
                        ProviderMessageRole::System => "system",
                        ProviderMessageRole::User => "user",
                        ProviderMessageRole::Assistant => "assistant",
                        ProviderMessageRole::Tool => "tool",
                    },
                    "content": m.content,
                })
            })
            .collect()
    }

    fn build_tools_payload(tools: &[ToolDef]) -> Vec<Value> {
        tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect()
    }

    /// Determine which model to use: prefer the session config, fall back to
    /// the provider default.
    fn resolve_model(&self, session_model: &str) -> String {
        if session_model.is_empty() {
            self.config.default_model.clone()
        } else {
            session_model.to_string()
        }
    }

    /// Normalise the endpoint by stripping a trailing slash.
    fn endpoint(&self) -> &str {
        self.config.endpoint.trim_end_matches('/')
    }
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

impl Provider for ApiKeyProvider {
    fn create_session(
        &self,
        _config: SessionConfig,
    ) -> Pin<Box<dyn Future<Output = Result<SessionHandle, ProviderError>> + Send + '_>> {
        Box::pin(async move {
            // Stateless — no connection needed.
            Ok(SessionHandle {
                session_id: eagent_protocol::ids::SessionId::new(),
                provider_name: "api-key".into(),
            })
        })
    }

    fn send(
        &self,
        _session: &SessionHandle,
        messages: Vec<ProviderMessage>,
        tools: Vec<ToolDef>,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<mpsc::UnboundedReceiver<ProviderEvent>, ProviderError>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            *self.status.lock().await = ProviderSessionStatus::Running;

            let url = format!("{}/chat/completions", self.endpoint());

            let model = self.resolve_model("");

            let mut body = json!({
                "model": model,
                "messages": Self::build_messages_payload(&messages),
                "stream": true,
            });

            if !tools.is_empty() {
                body["tools"] = Value::Array(Self::build_tools_payload(&tools));
            }

            let response = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| ProviderError::ConnectionFailed(e.to_string()))?;

            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                *self.status.lock().await = ProviderSessionStatus::Ready;
                return Err(ProviderError::RateLimited);
            }

            if !response.status().is_success() {
                let status = response.status();
                let body_text = response.text().await.unwrap_or_default();
                *self.status.lock().await = ProviderSessionStatus::Error;
                return Err(ProviderError::ConnectionFailed(format!(
                    "API returned {status}: {body_text}"
                )));
            }

            let (tx, rx) = mpsc::unbounded_channel();
            let status = Arc::clone(&self.status);

            tokio::spawn(async move {
                if let Err(e) = read_sse_stream(response, &tx).await {
                    let _ = tx.send(ProviderEvent::Error {
                        message: e.to_string(),
                    });
                }
                let mut s = status.lock().await;
                if *s == ProviderSessionStatus::Running {
                    *s = ProviderSessionStatus::Ready;
                }
            });

            Ok(rx)
        })
    }

    fn cancel(
        &self,
        _session: &SessionHandle,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderError>> + Send + '_>> {
        Box::pin(async move {
            // HTTP requests cannot be cancelled mid-stream easily — no-op.
            Ok(())
        })
    }

    fn list_models(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ModelInfo>, ProviderError>> + Send + '_>> {
        Box::pin(async move {
            let url = format!("{}/models", self.endpoint());

            let resp = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .send()
                .await
                .map_err(|e| ProviderError::ConnectionFailed(e.to_string()))?;

            if !resp.status().is_success() {
                return Err(ProviderError::ConnectionFailed(format!(
                    "/models returned {}",
                    resp.status()
                )));
            }

            let json: Value = resp
                .json()
                .await
                .map_err(|e| ProviderError::Internal(e.to_string()))?;

            let models = json["data"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .map(|m| {
                            let id = m["id"].as_str().unwrap_or("unknown").to_string();
                            ModelInfo {
                                name: id.clone(),
                                id,
                                max_context: Some(self.config.max_context),
                                provider_kind: ProviderKind::ApiKey,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();

            Ok(models)
        })
    }

    fn session_status(&self, _session: &SessionHandle) -> ProviderSessionStatus {
        // Stateless provider — always ready unless mid-stream.
        // If the mutex is locked, the provider is actively running.
        self.status
            .try_lock()
            .map(|s| *s)
            .unwrap_or(ProviderSessionStatus::Running)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sse::parse_sse_data;
    use eagent_contracts::provider::FinishReason;
    use std::collections::HashMap;

    // -- config tests -------------------------------------------------------

    #[test]
    fn config_defaults() {
        let cfg = ApiKeyConfig::default();
        assert_eq!(cfg.endpoint, "https://api.openai.com/v1");
        assert_eq!(cfg.default_model, "gpt-4o");
        assert_eq!(cfg.max_context, 128_000);
        assert!(cfg.api_key.is_empty());
    }

    #[test]
    fn config_serde_round_trip() {
        let cfg = ApiKeyConfig {
            endpoint: "https://api.together.xyz/v1".into(),
            api_key: "tok-abc123".into(),
            default_model: "mistralai/Mixtral-8x7B-Instruct-v0.1".into(),
            max_context: 32768,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: ApiKeyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.endpoint, "https://api.together.xyz/v1");
        assert_eq!(back.api_key, "tok-abc123");
        assert_eq!(back.default_model, "mistralai/Mixtral-8x7B-Instruct-v0.1");
        assert_eq!(back.max_context, 32768);
    }

    // -- message serialization tests ----------------------------------------

    #[test]
    fn build_messages_payload_all_roles() {
        let msgs = vec![
            ProviderMessage {
                role: ProviderMessageRole::System,
                content: "You are helpful.".into(),
            },
            ProviderMessage {
                role: ProviderMessageRole::User,
                content: "Hello".into(),
            },
            ProviderMessage {
                role: ProviderMessageRole::Assistant,
                content: "Hi there!".into(),
            },
            ProviderMessage {
                role: ProviderMessageRole::Tool,
                content: r#"{"result":"ok"}"#.into(),
            },
        ];
        let payload = ApiKeyProvider::build_messages_payload(&msgs);
        assert_eq!(payload.len(), 4);
        assert_eq!(payload[0]["role"], "system");
        assert_eq!(payload[1]["role"], "user");
        assert_eq!(payload[2]["role"], "assistant");
        assert_eq!(payload[3]["role"], "tool");
        assert_eq!(payload[1]["content"], "Hello");
    }

    #[test]
    fn build_tools_payload_basic() {
        use eagent_protocol::messages::RiskLevel;
        let tools = vec![ToolDef {
            name: "read_file".into(),
            description: "Read a file from disk".into(),
            parameters: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            risk_level: RiskLevel::Low,
        }];
        let payload = ApiKeyProvider::build_tools_payload(&tools);
        assert_eq!(payload.len(), 1);
        assert_eq!(payload[0]["type"], "function");
        assert_eq!(payload[0]["function"]["name"], "read_file");
        assert_eq!(payload[0]["function"]["description"], "Read a file from disk");
    }

    // -- model resolution ---------------------------------------------------

    #[test]
    fn resolve_model_uses_session_model_when_provided() {
        let provider = ApiKeyProvider::new(ApiKeyConfig {
            default_model: "gpt-4o".into(),
            ..Default::default()
        });
        assert_eq!(provider.resolve_model("claude-3-opus"), "claude-3-opus");
    }

    #[test]
    fn resolve_model_falls_back_to_default() {
        let provider = ApiKeyProvider::new(ApiKeyConfig {
            default_model: "gpt-4o".into(),
            ..Default::default()
        });
        assert_eq!(provider.resolve_model(""), "gpt-4o");
    }

    // -- endpoint normalization ---------------------------------------------

    #[test]
    fn endpoint_strips_trailing_slash() {
        let provider = ApiKeyProvider::new(ApiKeyConfig {
            endpoint: "https://api.openai.com/v1/".into(),
            ..Default::default()
        });
        assert_eq!(provider.endpoint(), "https://api.openai.com/v1");
    }

    #[test]
    fn endpoint_no_trailing_slash() {
        let provider = ApiKeyProvider::new(ApiKeyConfig {
            endpoint: "https://api.openai.com/v1".into(),
            ..Default::default()
        });
        assert_eq!(provider.endpoint(), "https://api.openai.com/v1");
    }

    // -- SSE parsing (exercises the shared module via this provider) ---------

    #[test]
    fn sse_token_delta() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = HashMap::new();
        let data = r#"{"choices":[{"delta":{"content":"world"}}]}"#;
        parse_sse_data(data, &tx, &mut tc).unwrap();
        drop(tx);

        match rx.blocking_recv().unwrap() {
            ProviderEvent::TokenDelta { text } => assert_eq!(text, "world"),
            other => panic!("expected TokenDelta, got {:?}", other),
        }
    }

    #[test]
    fn sse_finish_reason_stop() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = HashMap::new();
        let data = r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#;
        parse_sse_data(data, &tx, &mut tc).unwrap();
        drop(tx);

        match rx.blocking_recv().unwrap() {
            ProviderEvent::Completion { finish_reason } => {
                assert_eq!(finish_reason, FinishReason::Stop);
            }
            other => panic!("expected Completion, got {:?}", other),
        }
    }

    #[test]
    fn sse_tool_call_round_trip() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = HashMap::new();

        // Start
        let d1 = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_abc","function":{"name":"search","arguments":"{\"q\":"}}]}}]}"#;
        parse_sse_data(d1, &tx, &mut tc).unwrap();

        // Delta
        let d2 = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_abc","function":{"arguments":"\"rust\"}"}}]}}]}"#;
        parse_sse_data(d2, &tx, &mut tc).unwrap();

        // Finish
        let d3 = r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#;
        parse_sse_data(d3, &tx, &mut tc).unwrap();

        drop(tx);

        // ToolCallStart
        match rx.blocking_recv().unwrap() {
            ProviderEvent::ToolCallStart { id, name, .. } => {
                assert_eq!(id, "call_abc");
                assert_eq!(name, "search");
            }
            other => panic!("expected ToolCallStart, got {:?}", other),
        }
        // ToolCallDelta
        match rx.blocking_recv().unwrap() {
            ProviderEvent::ToolCallDelta { id, .. } => assert_eq!(id, "call_abc"),
            other => panic!("expected ToolCallDelta, got {:?}", other),
        }
        // ToolCallComplete
        match rx.blocking_recv().unwrap() {
            ProviderEvent::ToolCallComplete { id, name, params } => {
                assert_eq!(id, "call_abc");
                assert_eq!(name, "search");
                assert_eq!(params["q"], "rust");
            }
            other => panic!("expected ToolCallComplete, got {:?}", other),
        }
        // Completion
        match rx.blocking_recv().unwrap() {
            ProviderEvent::Completion { finish_reason } => {
                assert_eq!(finish_reason, FinishReason::ToolCalls);
            }
            other => panic!("expected Completion, got {:?}", other),
        }
    }

    // -- model list response parsing ----------------------------------------

    #[test]
    fn parse_models_response() {
        let json: Value = serde_json::from_str(
            r#"{
                "object": "list",
                "data": [
                    {"id": "gpt-4o", "object": "model", "owned_by": "openai"},
                    {"id": "gpt-4o-mini", "object": "model", "owned_by": "openai"}
                ]
            }"#,
        )
        .unwrap();

        let models: Vec<ModelInfo> = json["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| {
                let id = m["id"].as_str().unwrap_or("unknown").to_string();
                ModelInfo {
                    name: id.clone(),
                    id,
                    max_context: Some(128_000),
                    provider_kind: ProviderKind::ApiKey,
                }
            })
            .collect();

        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gpt-4o");
        assert_eq!(models[1].id, "gpt-4o-mini");
        assert_eq!(models[0].provider_kind, ProviderKind::ApiKey);
        assert_eq!(models[0].max_context, Some(128_000));
    }

    // -- session lifecycle --------------------------------------------------

    #[tokio::test]
    async fn create_session_returns_handle() {
        let provider = ApiKeyProvider::new(ApiKeyConfig::default());
        let handle = provider
            .create_session(SessionConfig {
                model: "gpt-4o".into(),
                system_prompt: None,
                workspace_root: None,
            })
            .await
            .unwrap();

        assert_eq!(handle.provider_name, "api-key");
    }

    #[test]
    fn session_status_defaults_to_ready() {
        let provider = ApiKeyProvider::new(ApiKeyConfig::default());
        let handle = SessionHandle {
            session_id: eagent_protocol::ids::SessionId::new(),
            provider_name: "api-key".into(),
        };
        assert_eq!(provider.session_status(&handle), ProviderSessionStatus::Ready);
    }

    #[tokio::test]
    async fn cancel_is_noop() {
        let provider = ApiKeyProvider::new(ApiKeyConfig::default());
        let handle = SessionHandle {
            session_id: eagent_protocol::ids::SessionId::new(),
            provider_name: "api-key".into(),
        };
        // Should not error.
        provider.cancel(&handle).await.unwrap();
    }
}
