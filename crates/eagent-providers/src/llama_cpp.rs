//! LlamaCpp provider — manages a local llama-server process and speaks
//! the OpenAI-compatible `/v1/chat/completions` SSE protocol.

use crate::sse::read_sse_stream;
use crate::{Provider, ProviderError, ProviderMessage, ProviderMessageRole, SessionConfig, SessionHandle};
use eagent_contracts::provider::{ModelInfo, ProviderEvent, ProviderKind, ProviderSessionStatus};
use eagent_tools::ToolDef;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Configuration for the llama-server backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaCppConfig {
    pub server_binary_path: String,
    pub model_path: String,
    pub host: String,
    pub port: u16,
    pub ctx_size: u32,
    pub threads: u16,
    pub gpu_layers: i32,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_top_p")]
    pub top_p: f32,
    #[serde(default)]
    pub flash_attention: bool,
}

fn default_temperature() -> f32 {
    0.7
}
fn default_top_p() -> f32 {
    0.9
}

impl Default for LlamaCppConfig {
    fn default() -> Self {
        Self {
            server_binary_path: String::new(),
            model_path: String::new(),
            host: "127.0.0.1".into(),
            port: 8080,
            ctx_size: 4096,
            threads: 4,
            gpu_layers: -1,
            temperature: default_temperature(),
            top_p: default_top_p(),
            flash_attention: false,
        }
    }
}

impl LlamaCppConfig {
    fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

/// Provider that manages a local `llama-server` process and streams responses
/// via the OpenAI-compatible `/v1/chat/completions` endpoint.
pub struct LlamaCppProvider {
    config: LlamaCppConfig,
    client: reqwest::Client,
    process: Arc<Mutex<Option<Child>>>,
    status: Arc<Mutex<ProviderSessionStatus>>,
}

impl LlamaCppProvider {
    pub fn new(config: LlamaCppConfig) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(120))
            .redirect(reqwest::redirect::Policy::limited(3))
            .user_agent("eAgent-llama-cpp/0.1")
            .build()
            .expect("failed to build reqwest client for llama-cpp");

        Self {
            config,
            client,
            process: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(ProviderSessionStatus::Stopped)),
        }
    }

    // -- lifecycle helpers --------------------------------------------------

    /// Ensure the llama-server process is running and responding.
    /// Spawns the process if needed and retries the health probe up to 20 times.
    async fn ensure_ready(&self) -> Result<(), ProviderError> {
        // Fast path: already up.
        if self.probe_models().await.is_ok() {
            return Ok(());
        }

        if self.config.server_binary_path.trim().is_empty() {
            return Err(ProviderError::ConnectionFailed(
                "llama-server binary path is not configured".into(),
            ));
        }
        if self.config.model_path.trim().is_empty() {
            return Err(ProviderError::ConnectionFailed(
                "llama.cpp model path is not configured".into(),
            ));
        }

        // Spawn if no child process exists (or the previous one exited).
        {
            let mut process = self.process.lock().await;
            if let Some(child) = process.as_mut() {
                match child.try_wait() {
                    Ok(Some(_exited)) => {
                        *process = None;
                    }
                    Ok(None) => { /* still running */ }
                    Err(_) => {
                        *process = None;
                    }
                }
            }

            if process.is_none() {
                *self.status.lock().await = ProviderSessionStatus::Starting;

                let mut cmd = Command::new(&self.config.server_binary_path);
                cmd.arg("-m")
                    .arg(&self.config.model_path)
                    .arg("--host")
                    .arg(&self.config.host)
                    .arg("--port")
                    .arg(self.config.port.to_string())
                    .arg("--ctx-size")
                    .arg(self.config.ctx_size.to_string())
                    .arg("--threads")
                    .arg(self.config.threads.to_string())
                    .arg("--n-gpu-layers")
                    .arg(self.config.gpu_layers.to_string())
                    .kill_on_drop(true)
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped());

                if self.config.flash_attention {
                    cmd.arg("--flash-attn");
                }

                let mut child = cmd.spawn().map_err(|e| {
                    ProviderError::ConnectionFailed(format!("failed to spawn llama-server: {e}"))
                })?;

                // Drain stderr in background so the pipe doesn't block.
                if let Some(stderr) = child.stderr.take() {
                    tokio::spawn(async move {
                        let reader = BufReader::new(stderr);
                        let mut lines = reader.lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            warn!(target: "eagent::llama_cpp", "llama-server stderr: {}", line);
                        }
                    });
                }

                *process = Some(child);
            }
        }

        // Poll until the server is ready.
        for i in 0..20 {
            if self.probe_models().await.is_ok() {
                debug!("llama-server ready after {} probes", i + 1);
                *self.status.lock().await = ProviderSessionStatus::Ready;
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        *self.status.lock().await = ProviderSessionStatus::Error;
        Err(ProviderError::ConnectionFailed(
            "llama-server did not become ready after 10 s".into(),
        ))
    }

    /// Quick check: GET /v1/models must return 2xx.
    async fn probe_models(&self) -> Result<(), ProviderError> {
        let url = format!("{}/v1/models", self.config.base_url());
        let resp = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .map_err(|e| ProviderError::ConnectionFailed(e.to_string()))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(ProviderError::ConnectionFailed(format!(
                "probe /v1/models returned {}",
                resp.status()
            )))
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
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

impl Provider for LlamaCppProvider {
    fn create_session(
        &self,
        _config: SessionConfig,
    ) -> Pin<Box<dyn Future<Output = Result<SessionHandle, ProviderError>> + Send + '_>> {
        Box::pin(async move {
            self.ensure_ready().await?;
            Ok(SessionHandle {
                session_id: eagent_protocol::ids::SessionId::new(),
                provider_name: "llama-cpp".into(),
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
            self.ensure_ready().await?;

            *self.status.lock().await = ProviderSessionStatus::Running;

            let url = format!("{}/v1/chat/completions", self.config.base_url());

            let mut body = json!({
                "model": "default",
                "messages": Self::build_messages_payload(&messages),
                "stream": true,
                "temperature": self.config.temperature,
                "top_p": self.config.top_p,
            });

            if !tools.is_empty() {
                body["tools"] = Value::Array(Self::build_tools_payload(&tools));
            }

            let response = self
                .client
                .post(&url)
                .json(&body)
                .timeout(Duration::from_secs(300))
                .send()
                .await
                .map_err(|e| ProviderError::ConnectionFailed(e.to_string()))?;

            if !response.status().is_success() {
                let status = response.status();
                let body_text = response.text().await.unwrap_or_default();
                *self.status.lock().await = ProviderSessionStatus::Error;
                return Err(ProviderError::ConnectionFailed(format!(
                    "llama-server returned {status}: {body_text}"
                )));
            }

            let (tx, rx) = mpsc::unbounded_channel();
            let status = Arc::clone(&self.status);

            // Spawn a task that reads the SSE stream and forwards ProviderEvents.
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
            let mut process = self.process.lock().await;
            if let Some(mut child) = process.take() {
                let _ = child.kill().await;
            }
            *self.status.lock().await = ProviderSessionStatus::Stopped;
            Ok(())
        })
    }

    fn list_models(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ModelInfo>, ProviderError>> + Send + '_>> {
        Box::pin(async move {
            self.ensure_ready().await?;

            let url = format!("{}/v1/models", self.config.base_url());
            let resp = self
                .client
                .get(&url)
                .send()
                .await
                .map_err(|e| ProviderError::ConnectionFailed(e.to_string()))?;

            if !resp.status().is_success() {
                return Err(ProviderError::ConnectionFailed(format!(
                    "/v1/models returned {}",
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
                                max_context: Some(self.config.ctx_size),
                                provider_kind: ProviderKind::LlamaCpp,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();

            Ok(models)
        })
    }

    fn session_status(&self, _session: &SessionHandle) -> ProviderSessionStatus {
        // status is behind an async Mutex; return a best-effort snapshot.
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

    #[test]
    fn config_defaults() {
        let cfg = LlamaCppConfig::default();
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.ctx_size, 4096);
        assert_eq!(cfg.threads, 4);
        assert_eq!(cfg.gpu_layers, -1);
        assert!(!cfg.flash_attention);
        assert!((cfg.temperature - 0.7).abs() < f32::EPSILON);
        assert!((cfg.top_p - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn config_base_url() {
        let cfg = LlamaCppConfig {
            host: "localhost".into(),
            port: 9999,
            ..Default::default()
        };
        assert_eq!(cfg.base_url(), "http://localhost:9999");
    }

    #[test]
    fn config_serde_round_trip() {
        let cfg = LlamaCppConfig {
            server_binary_path: "/usr/bin/llama-server".into(),
            model_path: "/models/mistral.gguf".into(),
            host: "0.0.0.0".into(),
            port: 5555,
            ctx_size: 8192,
            threads: 8,
            gpu_layers: 32,
            temperature: 0.5,
            top_p: 0.95,
            flash_attention: true,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: LlamaCppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.port, 5555);
        assert!(back.flash_attention);
    }

    #[test]
    fn build_messages_payload_basic() {
        let msgs = vec![
            ProviderMessage {
                role: ProviderMessageRole::System,
                content: "You are helpful.".into(),
            },
            ProviderMessage {
                role: ProviderMessageRole::User,
                content: "Hello".into(),
            },
        ];
        let payload = LlamaCppProvider::build_messages_payload(&msgs);
        assert_eq!(payload.len(), 2);
        assert_eq!(payload[0]["role"], "system");
        assert_eq!(payload[1]["content"], "Hello");
    }

    #[test]
    fn build_tools_payload_basic() {
        use eagent_protocol::messages::RiskLevel;
        let tools = vec![ToolDef {
            name: "read_file".into(),
            description: "Read a file".into(),
            parameters: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            risk_level: RiskLevel::Low,
        }];
        let payload = LlamaCppProvider::build_tools_payload(&tools);
        assert_eq!(payload.len(), 1);
        assert_eq!(payload[0]["type"], "function");
        assert_eq!(payload[0]["function"]["name"], "read_file");
    }

    #[tokio::test]
    async fn create_session_returns_handle_shape() {
        // Without a running server, create_session should fail with ConnectionFailed.
        let cfg = LlamaCppConfig {
            server_binary_path: "nonexistent-binary".into(),
            model_path: "/tmp/fake.gguf".into(),
            host: "127.0.0.1".into(),
            port: 19999,
            ..Default::default()
        };
        let provider = LlamaCppProvider::new(cfg);
        let result = provider
            .create_session(SessionConfig {
                model: "test".into(),
                system_prompt: None,
                workspace_root: None,
            })
            .await;

        // Should be an error because no server is running (and binary doesn't exist).
        assert!(result.is_err());
        match result.unwrap_err() {
            ProviderError::ConnectionFailed(msg) => {
                assert!(!msg.is_empty());
            }
            other => panic!("expected ConnectionFailed, got {:?}", other),
        }
    }

    #[test]
    fn session_status_defaults_to_stopped() {
        let provider = LlamaCppProvider::new(LlamaCppConfig::default());
        let handle = SessionHandle {
            session_id: eagent_protocol::ids::SessionId::new(),
            provider_name: "llama-cpp".into(),
        };
        assert_eq!(provider.session_status(&handle), ProviderSessionStatus::Stopped);
    }

    // -- SSE parsing tests --------------------------------------------------

    #[test]
    fn parse_sse_token_delta() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = std::collections::HashMap::new();
        let data = r#"{"choices":[{"delta":{"content":"Hello"}}]}"#;
        parse_sse_data(data, &tx, &mut tc).unwrap();
        drop(tx);

        let event = rx.blocking_recv().unwrap();
        match event {
            ProviderEvent::TokenDelta { text } => assert_eq!(text, "Hello"),
            other => panic!("expected TokenDelta, got {:?}", other),
        }
    }

    #[test]
    fn parse_sse_finish_reason_stop() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = std::collections::HashMap::new();
        let data = r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#;
        parse_sse_data(data, &tx, &mut tc).unwrap();
        drop(tx);

        let event = rx.blocking_recv().unwrap();
        match event {
            ProviderEvent::Completion { finish_reason } => {
                assert_eq!(finish_reason, FinishReason::Stop);
            }
            other => panic!("expected Completion, got {:?}", other),
        }
    }

    #[test]
    fn parse_sse_finish_reason_length() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = std::collections::HashMap::new();
        let data = r#"{"choices":[{"delta":{},"finish_reason":"length"}]}"#;
        parse_sse_data(data, &tx, &mut tc).unwrap();
        drop(tx);

        let event = rx.blocking_recv().unwrap();
        match event {
            ProviderEvent::Completion { finish_reason } => {
                assert_eq!(finish_reason, FinishReason::Length);
            }
            other => panic!("expected Completion, got {:?}", other),
        }
    }

    #[test]
    fn parse_sse_tool_call_start_and_delta() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = std::collections::HashMap::new();

        // First chunk: tool call start
        let data1 = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"read_file","arguments":"{\"pa"}}]}}]}"#;
        parse_sse_data(data1, &tx, &mut tc).unwrap();

        // Second chunk: tool call delta (continuation)
        let data2 = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"arguments":"th\":\"/tmp\"}"}}]}}]}"#;
        parse_sse_data(data2, &tx, &mut tc).unwrap();

        drop(tx);

        let e1 = rx.blocking_recv().unwrap();
        match e1 {
            ProviderEvent::ToolCallStart { id, name, .. } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "read_file");
            }
            other => panic!("expected ToolCallStart, got {:?}", other),
        }

        let e2 = rx.blocking_recv().unwrap();
        match e2 {
            ProviderEvent::ToolCallDelta { id, .. } => {
                assert_eq!(id, "call_1");
            }
            other => panic!("expected ToolCallDelta, got {:?}", other),
        }
    }

    #[test]
    fn parse_sse_tool_call_complete_on_finish() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = std::collections::HashMap::new();

        // Start
        let data1 = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_2","function":{"name":"ls","arguments":"{\"dir\":\".\"}"}}]}}]}"#;
        parse_sse_data(data1, &tx, &mut tc).unwrap();

        // Finish
        let data2 = r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#;
        parse_sse_data(data2, &tx, &mut tc).unwrap();

        drop(tx);

        // ToolCallStart
        let _ = rx.blocking_recv().unwrap();
        // ToolCallComplete
        let e2 = rx.blocking_recv().unwrap();
        match e2 {
            ProviderEvent::ToolCallComplete { id, name, params } => {
                assert_eq!(id, "call_2");
                assert_eq!(name, "ls");
                assert_eq!(params["dir"], ".");
            }
            other => panic!("expected ToolCallComplete, got {:?}", other),
        }
        // Completion
        let e3 = rx.blocking_recv().unwrap();
        match e3 {
            ProviderEvent::Completion { finish_reason } => {
                assert_eq!(finish_reason, FinishReason::ToolCalls);
            }
            other => panic!("expected Completion, got {:?}", other),
        }
    }

    #[test]
    fn parse_sse_ignores_empty_content() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = std::collections::HashMap::new();
        let data = r#"{"choices":[{"delta":{"content":""}}]}"#;
        parse_sse_data(data, &tx, &mut tc).unwrap();
        drop(tx);

        // No event should have been sent for empty content.
        assert!(rx.blocking_recv().is_none());
    }

    #[test]
    fn parse_sse_invalid_json_returns_error() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut tc = std::collections::HashMap::new();
        let result = parse_sse_data("not json", &tx, &mut tc);
        assert!(result.is_err());
    }
}
