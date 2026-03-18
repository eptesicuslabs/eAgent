//! Codex CLI Manager — spawns and manages Codex CLI `app-server` processes.
//!
//! This module handles the entire lifecycle of communicating with the Codex CLI:
//! - Spawning `codex app-server` child processes with piped stdio
//! - Sending JSON-RPC requests and receiving responses
//! - Routing notifications and server requests
//! - Process tree killing on Windows/Linux

use crate::platform;
use anyhow::{Context, Result, bail};
use ecode_contracts::codex::*;
use ecode_contracts::ids::*;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command as TokioCommand};
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::{debug, error, info, warn};

mod version;
pub use version::{check_codex_version, find_codex_binary};

/// A session with a Codex CLI process.
struct CodexSession {
    /// The child process.
    child: Child,
    /// Sender to write JSON-RPC messages to stdin.
    stdin_tx: mpsc::Sender<String>,
    /// Pending JSON-RPC request responses (keyed by request ID).
    pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    /// Codex-side thread ID.
    codex_thread_id: Option<String>,
    /// Abort handle for the stdout reader task.
    reader_abort: tokio::task::JoinHandle<()>,
    /// Abort handle for the stdin writer task.
    writer_abort: tokio::task::JoinHandle<()>,
}

/// Manager for all Codex CLI sessions.
pub struct CodexManager {
    /// Active sessions keyed by thread ID.
    sessions: Arc<Mutex<HashMap<ThreadId, CodexSession>>>,
    /// Channel for sending Codex events to the orchestration layer.
    event_tx: mpsc::UnboundedSender<(ThreadId, CodexEvent)>,
    /// Next JSON-RPC request ID (shared across sessions).
    next_id: Arc<AtomicU64>,
    /// Path to the codex binary.
    codex_binary: Arc<RwLock<String>>,
    /// Optional CODEX_HOME override.
    codex_home: Arc<RwLock<Option<String>>>,
}

impl CodexManager {
    /// Create a new CodexManager.
    ///
    /// `event_tx` is used to forward parsed Codex events to the orchestration reactor.
    pub fn new(
        event_tx: mpsc::UnboundedSender<(ThreadId, CodexEvent)>,
        codex_binary: Option<String>,
        codex_home: Option<String>,
    ) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            next_id: Arc::new(AtomicU64::new(1)),
            codex_binary: Arc::new(RwLock::new(
                codex_binary.unwrap_or_else(|| "codex".to_string()),
            )),
            codex_home: Arc::new(RwLock::new(codex_home)),
        }
    }

    /// Update the Codex executable settings used for future sessions.
    pub fn configure(&self, codex_binary: Option<String>, codex_home: Option<String>) {
        *self.codex_binary.write().unwrap() = codex_binary.unwrap_or_else(|| "codex".to_string());
        *self.codex_home.write().unwrap() = codex_home;
    }

    /// Spawn a new Codex session for a thread.
    pub async fn spawn_session(&self, thread_id: ThreadId) -> Result<SessionId> {
        let session_id = SessionId::new();
        info!(%thread_id, %session_id, "Spawning Codex CLI session");

        // Spawn the child process
        let codex_binary = self.codex_binary.read().unwrap().clone();
        let codex_home = self.codex_home.read().unwrap().clone();

        let mut cmd = TokioCommand::new(&codex_binary);
        cmd.arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if let Some(ref home) = codex_home {
            cmd.env("CODEX_HOME", home);
        }

        // On Windows, use CREATE_NEW_PROCESS_GROUP for clean termination
        #[cfg(windows)]
        {
            #[allow(unused_imports)]
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x00000200); // CREATE_NEW_PROCESS_GROUP
        }

        // On Unix, create a new process group
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                cmd.pre_exec(|| {
                    libc::setpgid(0, 0);
                    Ok(())
                });
            }
        }

        let mut child = cmd.spawn().context("Failed to spawn codex app-server")?;

        let stdin = child.stdin.take().expect("stdin was piped");
        let stdout = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");

        // Create channels
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(256);
        let pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // ── Stdin writer task ──
        let writer_abort = {
            let mut stdin = stdin;
            tokio::spawn(async move {
                while let Some(msg) = stdin_rx.recv().await {
                    if let Err(e) = stdin.write_all(msg.as_bytes()).await {
                        error!("Failed to write to codex stdin: {}", e);
                        break;
                    }
                    if let Err(e) = stdin.write_all(b"\n").await {
                        error!("Failed to write newline to codex stdin: {}", e);
                        break;
                    }
                    if let Err(e) = stdin.flush().await {
                        error!("Failed to flush codex stdin: {}", e);
                        break;
                    }
                }
            })
        };

        // ── Stdout reader task ──
        let reader_abort = {
            let event_tx = self.event_tx.clone();
            let pending = Arc::clone(&pending_requests);
            let tid = thread_id;

            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    let line = line.trim().to_string();
                    if line.is_empty() {
                        continue;
                    }

                    let value: Value = match serde_json::from_str(&line) {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(
                                "Failed to parse codex stdout as JSON: {} — line: {}",
                                e,
                                &line[..line.len().min(200)]
                            );
                            continue;
                        }
                    };

                    match IncomingMessage::parse(&value) {
                        Some(IncomingMessage::Response(resp)) => {
                            debug!(id = resp.id, "Received JSON-RPC response");
                            let mut pending = pending.lock().await;
                            if let Some(sender) = pending.remove(&resp.id) {
                                let _ = sender.send(resp);
                            }
                        }
                        Some(IncomingMessage::Notification(notif)) => {
                            debug!(method = %notif.method, "Received notification");
                            if let Some(event) = parse_notification(&notif) {
                                let _ = event_tx.send((tid, event));
                            }
                        }
                        Some(IncomingMessage::ServerRequest { id, method, params }) => {
                            debug!(%method, rpc_id = id, "Received server request");
                            if let Some(event) = parse_server_request(id, &method, params) {
                                let _ = event_tx.send((tid, event));
                            }
                        }
                        None => {
                            warn!(
                                "Unrecognized message from codex: {}",
                                &line[..line.len().min(200)]
                            );
                        }
                    }
                }

                info!(%tid, "Codex stdout reader ended");
                let _ = event_tx.send((
                    tid,
                    CodexEvent::SessionClosed {
                        reason: "stdout EOF".to_string(),
                    },
                ));
            })
        };

        // ── Stderr reader task ──
        {
            let tid = thread_id;
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    warn!(%tid, stderr = %line, "Codex stderr");
                }
            });
        }

        // Send initialize
        let init_params = InitializeParams {
            client_info: ClientInfo {
                name: "ecode".to_string(),
                title: "eCode".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: ClientCapabilities {
                experimental_api: true,
            },
        };

        let init_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let init_req = JsonRpcRequest {
            method: "initialize".to_string(),
            id: Some(init_id),
            params: Some(serde_json::to_value(&init_params)?),
        };

        let (resp_tx, resp_rx) = oneshot::channel();
        pending_requests.lock().await.insert(init_id, resp_tx);

        let init_json = serde_json::to_string(&init_req)?;
        stdin_tx
            .send(init_json)
            .await
            .context("Failed to send initialize")?;

        // Wait for initialize response
        let _init_resp = tokio::time::timeout(std::time::Duration::from_secs(30), resp_rx)
            .await
            .context("Timeout waiting for initialize response")?
            .context("Initialize response channel closed")?;

        // Send initialized notification
        let initialized = JsonRpcNotification {
            method: "initialized".to_string(),
            params: None,
        };
        let initialized_json = serde_json::to_string(&initialized)?;
        stdin_tx
            .send(initialized_json)
            .await
            .context("Failed to send initialized")?;

        info!(%thread_id, %session_id, "Codex session initialized");

        let session = CodexSession {
            child,
            stdin_tx,
            pending_requests,
            codex_thread_id: None,
            reader_abort,
            writer_abort,
        };

        self.sessions.lock().await.insert(thread_id, session);
        Ok(session_id)
    }

    /// Start a new thread in the Codex session.
    pub async fn start_thread(
        &self,
        thread_id: ThreadId,
        model: &str,
        cwd: &str,
        approval_policy: ApprovalPolicy,
        sandbox: SandboxMode,
    ) -> Result<String> {
        let params = ThreadStartParams {
            model: model.to_string(),
            approval_policy,
            sandbox,
            cwd: cwd.to_string(),
            experimental_raw_events: false,
        };

        let resp = self
            .send_request(
                thread_id,
                "thread/start",
                Some(serde_json::to_value(&params)?),
            )
            .await?;

        // Extract codex thread ID from response
        let Some(result) = resp.result else {
            bail!("thread/start returned no result");
        };
        let Some(thread_info) = result.get("thread") else {
            bail!("thread/start response missing thread payload");
        };
        let Some(codex_tid) = thread_info.get("id").and_then(|v| v.as_str()) else {
            bail!("thread/start response missing thread id");
        };

        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get_mut(&thread_id) {
            session.codex_thread_id = Some(codex_tid.to_string());
        }

        Ok(codex_tid.to_string())
    }

    /// Resume an existing Codex thread.
    pub async fn resume_thread(
        &self,
        thread_id: ThreadId,
        codex_thread_id: &str,
    ) -> Result<String> {
        let params = ThreadResumeParams {
            thread_id: codex_thread_id.to_string(),
        };

        self.send_request(
            thread_id,
            "thread/resume",
            Some(serde_json::to_value(&params)?),
        )
        .await?;
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get_mut(&thread_id) {
            session.codex_thread_id = Some(codex_thread_id.to_string());
        }
        Ok(codex_thread_id.to_string())
    }

    /// Send a turn (user message) to the Codex session.
    pub async fn send_turn(
        &self,
        thread_id: ThreadId,
        text: &str,
        images: &[String],
        developer_instructions: Option<String>,
    ) -> Result<()> {
        let codex_thread_id = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(&thread_id)
                .and_then(|s| s.codex_thread_id.clone())
                .ok_or_else(|| anyhow::anyhow!("No codex thread ID for thread {}", thread_id))?
        };

        let mut input = vec![TurnInputItem::Text {
            text: text.to_string(),
            text_elements: vec![],
        }];

        // Add images
        for img_b64 in images {
            input.push(TurnInputItem::Image {
                base64_data: img_b64.clone(),
                mime_type: "image/png".to_string(),
            });
        }

        let params = TurnStartParams {
            thread_id: codex_thread_id,
            input,
            developer_instructions,
        };

        self.send_request(
            thread_id,
            "turn/start",
            Some(serde_json::to_value(&params)?),
        )
        .await?;
        Ok(())
    }

    /// Interrupt the active turn.
    pub async fn interrupt_turn(&self, thread_id: ThreadId, codex_turn_id: &str) -> Result<()> {
        let codex_thread_id = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(&thread_id)
                .and_then(|s| s.codex_thread_id.clone())
                .ok_or_else(|| anyhow::anyhow!("No codex thread ID"))?
        };

        let params = TurnInterruptParams {
            thread_id: codex_thread_id,
            turn_id: codex_turn_id.to_string(),
        };

        self.send_request(
            thread_id,
            "turn/interrupt",
            Some(serde_json::to_value(&params)?),
        )
        .await?;
        Ok(())
    }

    /// Respond to an approval request from Codex.
    pub async fn respond_approval(
        &self,
        thread_id: ThreadId,
        rpc_id: u64,
        decision: ApprovalDecision,
    ) -> Result<()> {
        let resp = JsonRpcResponse {
            id: rpc_id,
            result: Some(serde_json::to_value(&ApprovalResponse { decision })?),
            error: None,
        };

        let json = serde_json::to_string(&resp)?;
        let sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(&thread_id) {
            session
                .stdin_tx
                .send(json)
                .await
                .context("Failed to send approval response")?;
        } else {
            bail!("No session for thread {}", thread_id);
        }
        Ok(())
    }

    /// Respond to a user input request from Codex.
    pub async fn respond_user_input(
        &self,
        thread_id: ThreadId,
        rpc_id: u64,
        answers: Value,
    ) -> Result<()> {
        let resp = JsonRpcResponse {
            id: rpc_id,
            result: Some(answers),
            error: None,
        };

        let json = serde_json::to_string(&resp)?;
        let sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(&thread_id) {
            session
                .stdin_tx
                .send(json)
                .await
                .context("Failed to send user input response")?;
        } else {
            bail!("No session for thread {}", thread_id);
        }
        Ok(())
    }

    /// Get account info from Codex.
    pub async fn get_account_info(&self, thread_id: ThreadId) -> Result<AccountInfo> {
        let resp = self.send_request(thread_id, "account/read", None).await?;

        let result = resp
            .result
            .ok_or_else(|| anyhow::anyhow!("No result in account/read response"))?;

        serde_json::from_value(result).context("Failed to parse account info")
    }

    /// List available models from Codex.
    pub async fn list_models(&self, thread_id: ThreadId) -> Result<Vec<ModelInfo>> {
        let resp = self.send_request(thread_id, "model/list", None).await?;

        let result = resp
            .result
            .ok_or_else(|| anyhow::anyhow!("No result in model/list response"))?;

        let list: ModelListResult =
            serde_json::from_value(result).context("Failed to parse model list")?;
        Ok(list.models)
    }

    /// Kill a session, terminating the child process tree.
    pub async fn kill_session(&self, thread_id: ThreadId) -> Result<()> {
        let mut sessions = self.sessions.lock().await;
        if let Some(mut session) = sessions.remove(&thread_id) {
            info!(%thread_id, "Killing Codex session");

            // Abort the reader/writer tasks
            session.reader_abort.abort();
            session.writer_abort.abort();

            // Kill the process tree
            if let Some(pid) = session.child.id() {
                let _ = platform::kill_process_tree(pid).await;
            }

            // Ensure the child is killed
            let _ = session.child.kill().await;
        }
        Ok(())
    }

    /// Check if a session exists for a thread.
    pub async fn has_session(&self, thread_id: &ThreadId) -> bool {
        self.sessions.lock().await.contains_key(thread_id)
    }

    // ── Private helpers ──

    /// Send a JSON-RPC request and wait for the response.
    async fn send_request(
        &self,
        thread_id: ThreadId,
        method: &str,
        params: Option<Value>,
    ) -> Result<JsonRpcResponse> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest {
            method: method.to_string(),
            id: Some(id),
            params,
        };

        let json = serde_json::to_string(&req)?;
        let (resp_tx, resp_rx) = oneshot::channel();

        {
            let sessions = self.sessions.lock().await;
            let session = sessions
                .get(&thread_id)
                .ok_or_else(|| anyhow::anyhow!("No session for thread {}", thread_id))?;

            session.pending_requests.lock().await.insert(id, resp_tx);
            session
                .stdin_tx
                .send(json)
                .await
                .context("Failed to send request to codex")?;
        }

        let resp = tokio::time::timeout(std::time::Duration::from_secs(60), resp_rx)
            .await
            .context("Timeout waiting for response")?
            .context("Response channel closed")?;

        if let Some(ref err) = resp.error {
            bail!("Codex RPC error: {} (code: {})", err.message, err.code);
        }

        Ok(resp)
    }
}

/// Parse a Codex notification into a CodexEvent.
fn parse_notification(notif: &JsonRpcNotification) -> Option<CodexEvent> {
    let params = notif.params.as_ref()?;

    match notif.method.as_str() {
        "thread/started" => {
            let info: ThreadStartedNotification = serde_json::from_value(params.clone()).ok()?;
            Some(CodexEvent::ThreadStarted {
                codex_thread_id: info.thread.id,
            })
        }
        "turn/started" => {
            let info: TurnStartedNotification = serde_json::from_value(params.clone()).ok()?;
            Some(CodexEvent::TurnStarted {
                codex_turn_id: info.turn.id,
            })
        }
        "turn/completed" => {
            let info: TurnCompletedNotification = serde_json::from_value(params.clone()).ok()?;
            Some(CodexEvent::TurnCompleted {
                codex_turn_id: info.turn.id.clone(),
                status: info.turn.status.unwrap_or_else(|| "completed".to_string()),
            })
        }
        "item/agentMessage/delta" => {
            let delta: AgentMessageDelta = serde_json::from_value(params.clone()).ok()?;
            Some(CodexEvent::AgentMessageDelta {
                codex_turn_id: delta.turn_id,
                item_id: delta.item_id,
                delta: delta.delta,
            })
        }
        "error" => {
            let err: CodexError = serde_json::from_value(params.clone()).ok()?;
            Some(CodexEvent::Error {
                message: err.error.message,
                will_retry: err.will_retry,
            })
        }
        _ => {
            debug!(method = %notif.method, "Unhandled Codex notification");
            None
        }
    }
}

/// Parse a Codex server request into a CodexEvent.
fn parse_server_request(rpc_id: u64, method: &str, params: Option<Value>) -> Option<CodexEvent> {
    let params = params?;

    match method {
        "item/commandExecution/requestApproval" => {
            let req: CommandApprovalRequest = serde_json::from_value(params).ok()?;
            Some(CodexEvent::CommandApprovalRequested {
                rpc_id,
                turn_id: req.turn_id,
                item_id: req.item_id,
                command: req.command,
            })
        }
        "item/fileChange/requestApproval" => {
            let req: FileChangeApprovalRequest = serde_json::from_value(params).ok()?;
            Some(CodexEvent::FileChangeApprovalRequested {
                rpc_id,
                turn_id: req.turn_id,
                item_id: req.item_id,
                file_path: req.file_path,
                diff: req.diff,
            })
        }
        "item/fileRead/requestApproval" => {
            let req: FileReadApprovalRequest = serde_json::from_value(params).ok()?;
            Some(CodexEvent::FileReadApprovalRequested {
                rpc_id,
                turn_id: req.turn_id,
                item_id: req.item_id,
                file_path: req.file_path,
            })
        }
        "item/tool/requestUserInput" => {
            let req: UserInputRequest = serde_json::from_value(params).ok()?;
            Some(CodexEvent::UserInputRequested {
                rpc_id,
                turn_id: req.turn_id,
                item_id: req.item_id,
                questions: req.questions,
            })
        }
        _ => {
            debug!(%method, "Unhandled Codex server request");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    #[ignore = "requires a local, authenticated Codex CLI install"]
    async fn live_smoke_can_start_thread() {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let codex_binary = find_codex_binary(None).unwrap();
        let manager = CodexManager::new(event_tx, Some(codex_binary), None);
        let thread_id = ThreadId::new();
        let cwd = std::env::current_dir().unwrap();

        manager.spawn_session(thread_id).await.unwrap();
        let codex_thread_id = manager
            .start_thread(
                thread_id,
                "o4-mini",
                &cwd.display().to_string(),
                ApprovalPolicy::Never,
                SandboxMode::WorkspaceWrite,
            )
            .await
            .unwrap();

        assert!(!codex_thread_id.is_empty());
        let maybe_event = tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
            .await
            .ok()
            .flatten();
        assert!(maybe_event.is_some());

        manager.kill_session(thread_id).await.unwrap();
    }
}
