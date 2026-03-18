use anyhow::{Result, anyhow, bail};
use ecode_contracts::config::LlamaCppConfig;
use ecode_contracts::orchestration::{MessageRole, ThreadState};
use serde_json::{Value, json};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::warn;

#[derive(Clone)]
pub struct LlamaCppManager {
    client: reqwest::Client,
    process: Arc<Mutex<Option<Child>>>,
}

impl LlamaCppManager {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::limited(3))
                .user_agent("eCode-llama-cpp/0.1")
                .build()
                .expect("llama.cpp client"),
            process: Arc::new(Mutex::new(None)),
        }
    }

    pub fn base_url(config: &LlamaCppConfig) -> String {
        format!("http://{}:{}", config.host, config.port)
    }

    pub async fn ensure_ready(&self, config: &LlamaCppConfig) -> Result<()> {
        if self.probe_models(config).await.is_ok() {
            return Ok(());
        }

        if !config.enabled {
            bail!("llama.cpp is disabled in settings");
        }
        if config.llama_server_binary_path.trim().is_empty() {
            bail!("llama-server binary path is not configured");
        }
        if config.model_path.trim().is_empty() {
            bail!("llama.cpp model path is not configured");
        }

        {
            let mut process = self.process.lock().await;
            if let Some(child) = process.as_mut()
                && child.try_wait()?.is_some()
            {
                *process = None;
            }

            if process.is_none() {
                let mut command = Command::new(&config.llama_server_binary_path);
                command
                    .arg("-m")
                    .arg(&config.model_path)
                    .arg("--host")
                    .arg(&config.host)
                    .arg("--port")
                    .arg(config.port.to_string())
                    .arg("--ctx-size")
                    .arg(config.ctx_size.to_string())
                    .arg("--threads")
                    .arg(config.threads.to_string())
                    .arg("--n-gpu-layers")
                    .arg(config.gpu_layers.to_string())
                    .kill_on_drop(true)
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped());

                if config.flash_attention {
                    command.arg("--flash-attn");
                }

                let mut child = command.spawn()?;
                if let Some(stderr) = child.stderr.take() {
                    tokio::spawn(async move {
                        let reader = BufReader::new(stderr);
                        let mut lines = reader.lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            warn!(target: "ecode::llama_cpp", "llama-server stderr: {}", line);
                        }
                    });
                }
                *process = Some(child);
            }
        }

        for _ in 0..20 {
            if self.probe_models(config).await.is_ok() {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        Err(anyhow!("llama.cpp server did not become ready"))
    }

    pub async fn send_turn(
        &self,
        config: &LlamaCppConfig,
        thread: &ThreadState,
        prompt: &str,
    ) -> Result<String> {
        self.ensure_ready(config).await?;

        let messages = build_messages(thread, prompt);
        self.complete_messages(config, &thread.settings.model, messages)
            .await
    }

    pub async fn complete_messages(
        &self,
        config: &LlamaCppConfig,
        model: &str,
        messages: Vec<Value>,
    ) -> Result<String> {
        self.ensure_ready(config).await?;

        let url = format!("{}/v1/chat/completions", Self::base_url(config));
        let body = json!({
            "model": model,
            "messages": messages,
            "stream": false,
            "temperature": config.temperature,
            "top_p": config.top_p,
        });

        let response = self.client.post(url).json(&body).send().await?;
        if !response.status().is_success() {
            bail!("llama.cpp request failed with status {}", response.status());
        }

        let json: Value = response.json().await?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
            .or_else(|| {
                json["choices"][0]["message"]["content"]
                    .as_array()
                    .map(|parts| {
                        parts
                            .iter()
                            .filter_map(|part| part.get("text").and_then(|text| text.as_str()))
                            .collect::<String>()
                    })
            })
            .unwrap_or_default();

        if content.is_empty() {
            bail!("llama.cpp returned an empty response");
        }

        Ok(content)
    }

    async fn probe_models(&self, config: &LlamaCppConfig) -> Result<()> {
        let url = format!("{}/v1/models", Self::base_url(config));
        let response = self.client.get(url).send().await?;
        if response.status().is_success() {
            Ok(())
        } else {
            bail!("model probe failed with status {}", response.status())
        }
    }
}

impl Default for LlamaCppManager {
    fn default() -> Self {
        Self::new()
    }
}

fn build_messages(thread: &ThreadState, prompt: &str) -> Vec<Value> {
    let mut messages = Vec::new();

    for turn in &thread.turns {
        messages.push(json!({
            "role": "user",
            "content": turn.input,
        }));

        let assistant = turn
            .messages
            .iter()
            .filter(|message| message.role == MessageRole::Assistant)
            .map(|message| message.content.as_str())
            .collect::<String>();
        if !assistant.is_empty() {
            messages.push(json!({
                "role": "assistant",
                "content": assistant,
            }));
        }
    }

    messages.push(json!({
        "role": "user",
        "content": prompt,
    }));

    messages
}
