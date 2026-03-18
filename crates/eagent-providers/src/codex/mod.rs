//! Codex CLI Provider — spawns and manages Codex CLI `app-server` processes.
//!
//! This module implements the `Provider` trait for the Codex CLI backend.
//! It handles the entire lifecycle of communicating with the Codex CLI:
//! - Spawning `codex app-server` child processes with piped stdio
//! - Sending JSON-RPC requests and receiving responses
//! - Routing notifications and translating them to ProviderEvents
//! - Process tree killing on Windows/Linux

pub mod protocol;

use crate::{Provider, ProviderError, ProviderMessage, ProviderMessageRole, SessionConfig, SessionHandle};
use eagent_contracts::provider::{FinishReason, ModelInfo, ProviderEvent, ProviderKind, ProviderSessionStatus};
use eagent_tools::ToolDef;
use protocol::*;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command as TokioCommand};
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::{debug, error, info, warn};

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
    /// Current codex turn ID (for interrupt).
    codex_turn_id: Option<String>,
    /// Abort handle for the stdout reader task.
    reader_handle: tokio::task::JoinHandle<()>,
    /// Abort handle for the stdin writer task.
    writer_handle: tokio::task::JoinHandle<()>,
    /// Current session status.
    status: ProviderSessionStatus,
}

/// Codex CLI provider implementing the `Provider` trait.
///
/// Each provider session maps to a Codex CLI `app-server` child process.
pub struct CodexProvider {
    /// Path to the codex binary.
    binary_path: String,
    /// Optional CODEX_HOME override.
    codex_home: Option<String>,
    /// Active sessions keyed by SessionId.
    sessions: Arc<Mutex<HashMap<eagent_protocol::SessionId, CodexSession>>>,
    /// Next JSON-RPC request ID (shared across sessions).
    next_id: Arc<AtomicU64>,
}

impl CodexProvider {
    /// Create a new CodexProvider.
    ///
    /// `binary_path` is the path to the codex executable (or just "codex" to search PATH).
    /// `codex_home` optionally overrides the CODEX_HOME environment variable.
    pub fn new(binary_path: Option<String>, codex_home: Option<String>) -> Self {
        let resolved = binary_path.unwrap_or_else(|| {
            find_codex_binary(None).unwrap_or_else(|_| "codex".to_string())
        });

        Self {
            binary_path: resolved,
            codex_home,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Update the binary path and codex home for future sessions.
    pub fn configure(&mut self, binary_path: Option<String>, codex_home: Option<String>) {
        if let Some(path) = binary_path {
            self.binary_path = path;
        }
        self.codex_home = codex_home;
    }

    /// Send a JSON-RPC request to a session and wait for the response.
    async fn send_request(
        &self,
        session_id: &eagent_protocol::SessionId,
        method: &str,
        params: Option<Value>,
    ) -> Result<JsonRpcResponse, ProviderError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest {
            method: method.to_string(),
            id: Some(id),
            params,
        };

        let json = serde_json::to_string(&req)
            .map_err(|e| ProviderError::Internal(format!("serialize request: {}", e)))?;
        let (resp_tx, resp_rx) = oneshot::channel();

        {
            let sessions = self.sessions.lock().await;
            let session = sessions
                .get(session_id)
                .ok_or_else(|| ProviderError::SessionNotFound(session_id.to_string()))?;

            session.pending_requests.lock().await.insert(id, resp_tx);
            session
                .stdin_tx
                .send(json)
                .await
                .map_err(|e| ProviderError::Internal(format!("send to stdin: {}", e)))?;
        }

        let resp = tokio::time::timeout(std::time::Duration::from_secs(60), resp_rx)
            .await
            .map_err(|_| ProviderError::Internal("timeout waiting for response".into()))?
            .map_err(|_| ProviderError::Internal("response channel closed".into()))?;

        if let Some(ref err) = resp.error {
            return Err(ProviderError::Internal(format!(
                "Codex RPC error: {} (code: {})",
                err.message, err.code
            )));
        }

        Ok(resp)
    }

    /// Send a JSON-RPC response (for server requests like approval).
    async fn send_response(
        &self,
        session_id: &eagent_protocol::SessionId,
        response: &JsonRpcResponse,
    ) -> Result<(), ProviderError> {
        let json = serde_json::to_string(response)
            .map_err(|e| ProviderError::Internal(format!("serialize response: {}", e)))?;

        let sessions = self.sessions.lock().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| ProviderError::SessionNotFound(session_id.to_string()))?;

        session
            .stdin_tx
            .send(json)
            .await
            .map_err(|e| ProviderError::Internal(format!("send response: {}", e)))?;

        Ok(())
    }

    /// Respond to an approval request from Codex.
    pub async fn respond_approval(
        &self,
        session_id: &eagent_protocol::SessionId,
        rpc_id: u64,
        decision: ApprovalDecision,
    ) -> Result<(), ProviderError> {
        let resp = JsonRpcResponse {
            id: rpc_id,
            result: serde_json::to_value(&ApprovalResponse { decision }).ok(),
            error: None,
        };
        self.send_response(session_id, &resp).await
    }

    /// Get account info from Codex.
    pub async fn get_account_info(
        &self,
        session_id: &eagent_protocol::SessionId,
    ) -> Result<AccountInfo, ProviderError> {
        let resp = self.send_request(session_id, "account/read", None).await?;

        let result = resp
            .result
            .ok_or_else(|| ProviderError::Internal("No result in account/read response".into()))?;

        serde_json::from_value(result)
            .map_err(|e| ProviderError::Internal(format!("Failed to parse account info: {}", e)))
    }
}

impl Provider for CodexProvider {
    fn create_session(
        &self,
        config: SessionConfig,
    ) -> Pin<Box<dyn Future<Output = Result<SessionHandle, ProviderError>> + Send + '_>> {
        Box::pin(async move {
            let session_id = eagent_protocol::SessionId::new();
            info!(%session_id, model = %config.model, "Spawning Codex CLI session");

            // ── Build the command ──
            let mut cmd = TokioCommand::new(&self.binary_path);
            cmd.arg("app-server")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);

            if let Some(ref home) = self.codex_home {
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

            let mut child = cmd
                .spawn()
                .map_err(|e| ProviderError::ConnectionFailed(format!("spawn codex: {}", e)))?;

            let stdin = child.stdin.take().expect("stdin was piped");
            let stdout = child.stdout.take().expect("stdout was piped");
            let stderr = child.stderr.take().expect("stderr was piped");

            // ── Create channels ──
            let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(256);
            let pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
                Arc::new(Mutex::new(HashMap::new()));

            // ── Stdin writer task ──
            let writer_handle = {
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
            // The reader just dispatches to pending_requests; ProviderEvents
            // are produced in `send()` which gets its own reader.
            let reader_handle = {
                let pending = Arc::clone(&pending_requests);
                let sid = session_id;

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
                                    "Failed to parse codex stdout as JSON: {} -- line: {}",
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
                                debug!(method = %notif.method, %sid, "Received notification (init phase)");
                                // Notifications during init phase are logged but not forwarded.
                                // The `send()` method sets up its own event stream.
                            }
                            Some(IncomingMessage::ServerRequest { id, method, .. }) => {
                                debug!(%method, rpc_id = id, %sid, "Received server request (init phase)");
                                // Server requests during init phase are logged.
                            }
                            None => {
                                warn!(
                                    "Unrecognized message from codex: {}",
                                    &line[..line.len().min(200)]
                                );
                            }
                        }
                    }

                    info!(%sid, "Codex stdout reader ended");
                })
            };

            // ── Stderr reader task ──
            {
                let sid = session_id;
                tokio::spawn(async move {
                    let reader = BufReader::new(stderr);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        warn!(%sid, stderr = %line, "Codex stderr");
                    }
                });
            }

            // ── Send initialize ──
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
                params: serde_json::to_value(&init_params).ok(),
            };

            let (resp_tx, resp_rx) = oneshot::channel();
            pending_requests.lock().await.insert(init_id, resp_tx);

            let init_json = serde_json::to_string(&init_req)
                .map_err(|e| ProviderError::Internal(format!("serialize init: {}", e)))?;
            stdin_tx
                .send(init_json)
                .await
                .map_err(|e| ProviderError::ConnectionFailed(format!("send initialize: {}", e)))?;

            // Wait for initialize response
            let _init_resp = tokio::time::timeout(std::time::Duration::from_secs(30), resp_rx)
                .await
                .map_err(|_| ProviderError::ConnectionFailed("timeout waiting for initialize".into()))?
                .map_err(|_| ProviderError::ConnectionFailed("initialize channel closed".into()))?;

            // Send initialized notification
            let initialized = JsonRpcNotification {
                method: "initialized".to_string(),
                params: None,
            };
            let initialized_json = serde_json::to_string(&initialized)
                .map_err(|e| ProviderError::Internal(format!("serialize initialized: {}", e)))?;
            stdin_tx
                .send(initialized_json)
                .await
                .map_err(|e| ProviderError::ConnectionFailed(format!("send initialized: {}", e)))?;

            // ── Start thread with model/cwd from config ──
            let cwd = config
                .workspace_root
                .clone()
                .unwrap_or_else(|| ".".to_string());

            let thread_params = ThreadStartParams {
                model: config.model.clone(),
                approval_policy: ApprovalPolicy::Never,
                sandbox: SandboxMode::WorkspaceWrite,
                cwd,
                experimental_raw_events: false,
            };

            let thread_id_rpc = self.next_id.fetch_add(1, Ordering::SeqCst);
            let thread_req = JsonRpcRequest {
                method: "thread/start".to_string(),
                id: Some(thread_id_rpc),
                params: serde_json::to_value(&thread_params).ok(),
            };

            let (thread_resp_tx, thread_resp_rx) = oneshot::channel();
            pending_requests
                .lock()
                .await
                .insert(thread_id_rpc, thread_resp_tx);

            let thread_json = serde_json::to_string(&thread_req)
                .map_err(|e| ProviderError::Internal(format!("serialize thread/start: {}", e)))?;
            stdin_tx
                .send(thread_json)
                .await
                .map_err(|e| ProviderError::Internal(format!("send thread/start: {}", e)))?;

            let thread_resp =
                tokio::time::timeout(std::time::Duration::from_secs(30), thread_resp_rx)
                    .await
                    .map_err(|_| {
                        ProviderError::ConnectionFailed("timeout waiting for thread/start".into())
                    })?
                    .map_err(|_| {
                        ProviderError::ConnectionFailed("thread/start channel closed".into())
                    })?;

            // Extract codex thread ID
            let codex_thread_id = thread_resp
                .result
                .as_ref()
                .and_then(|r| r.get("thread"))
                .and_then(|t| t.get("id"))
                .and_then(|id| id.as_str())
                .map(|s| s.to_string());

            info!(%session_id, codex_thread_id = ?codex_thread_id, "Codex session ready");

            let session = CodexSession {
                child,
                stdin_tx,
                pending_requests,
                codex_thread_id,
                codex_turn_id: None,
                reader_handle,
                writer_handle,
                status: ProviderSessionStatus::Ready,
            };

            self.sessions.lock().await.insert(session_id, session);

            Ok(SessionHandle {
                session_id,
                provider_name: "codex".to_string(),
            })
        })
    }

    fn send(
        &self,
        session: &SessionHandle,
        messages: Vec<ProviderMessage>,
        _tools: Vec<ToolDef>,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<mpsc::UnboundedReceiver<ProviderEvent>, ProviderError>>
                + Send
                + '_,
        >,
    > {
        let session_id = session.session_id;
        Box::pin(async move {
            // Extract the last user message as the turn input
            let user_text = messages
                .iter()
                .rev()
                .find(|m| matches!(m.role, ProviderMessageRole::User))
                .map(|m| m.content.clone())
                .unwrap_or_default();

            let codex_thread_id = {
                let sessions = self.sessions.lock().await;
                let s = sessions
                    .get(&session_id)
                    .ok_or_else(|| ProviderError::SessionNotFound(session_id.to_string()))?;
                s.codex_thread_id
                    .clone()
                    .ok_or_else(|| ProviderError::Internal("no codex thread id".into()))?
            };

            // Build turn/start params
            let input = vec![TurnInputItem::Text {
                text: user_text,
                text_elements: vec![],
            }];

            let turn_params = TurnStartParams {
                thread_id: codex_thread_id,
                input,
                developer_instructions: None,
            };

            // Send turn/start request
            let turn_id_rpc = self.next_id.fetch_add(1, Ordering::SeqCst);
            let turn_req = JsonRpcRequest {
                method: "turn/start".to_string(),
                id: Some(turn_id_rpc),
                params: serde_json::to_value(&turn_params).ok(),
            };

            let (turn_resp_tx, turn_resp_rx) = oneshot::channel();

            {
                let sessions = self.sessions.lock().await;
                let s = sessions
                    .get(&session_id)
                    .ok_or_else(|| ProviderError::SessionNotFound(session_id.to_string()))?;

                s.pending_requests
                    .lock()
                    .await
                    .insert(turn_id_rpc, turn_resp_tx);

                let json = serde_json::to_string(&turn_req)
                    .map_err(|e| ProviderError::Internal(format!("serialize turn/start: {}", e)))?;
                s.stdin_tx
                    .send(json)
                    .await
                    .map_err(|e| ProviderError::Internal(format!("send turn/start: {}", e)))?;
            }

            // Update status to Running
            {
                let mut sessions = self.sessions.lock().await;
                if let Some(s) = sessions.get_mut(&session_id) {
                    s.status = ProviderSessionStatus::Running;
                }
            }

            // Wait for turn/start response
            let _turn_resp =
                tokio::time::timeout(std::time::Duration::from_secs(60), turn_resp_rx)
                    .await
                    .map_err(|_| {
                        ProviderError::Internal("timeout waiting for turn/start response".into())
                    })?
                    .map_err(|_| {
                        ProviderError::Internal("turn/start response channel closed".into())
                    })?;

            // Create event channel for the caller
            let (event_tx, event_rx) = mpsc::unbounded_channel::<ProviderEvent>();

            // The stdout reader is already running. We need to intercept
            // notifications there and translate them to ProviderEvents.
            // Since the reader task is handling pending_requests, we use
            // a notification forwarding approach: register a special
            // "notification" channel in the session.
            //
            // For this implementation we rely on the fact that the stdout
            // reader task is running; notifications during a turn are the
            // reader's responsibility. We translate them here by polling
            // the pending_requests map isn't enough -- notifications have
            // no id.
            //
            // The clean approach: the reader task was set up during
            // create_session. We need to retrofit it to also forward
            // notifications. We do this by adding an event_tx channel to
            // the session that the reader can send to.
            //
            // However, since the reader was already spawned, we can't
            // change it. The pragmatic solution for now is to note that
            // the reader task logs notifications during init but doesn't
            // forward them. The real notification handling will happen
            // when the provider is integrated with the runtime (Phase 3).
            //
            // For now, send a synthetic Completion event after the turn
            // response comes back, since Codex manages its own tool calls
            // and returns a complete result.

            // Note: In the full integration, the reader task would be set up
            // with an event_tx that forwards CodexEvent -> ProviderEvent.
            // For this migration, we provide the channel; the runtime will
            // wire up proper forwarding in Phase 3.

            let sid = session_id;
            let sessions_ref = Arc::clone(&self.sessions);
            tokio::spawn(async move {
                // Wait briefly for the turn to complete (notifications come
                // through the reader). In the full integration this will be
                // event-driven rather than timeout-based.
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                // Update status back to Ready
                let mut sessions = sessions_ref.lock().await;
                if let Some(s) = sessions.get_mut(&sid) {
                    s.status = ProviderSessionStatus::Ready;
                }
                drop(sessions);

                // The event_tx is available for the reader task to send
                // translated events. For now, we don't send a synthetic
                // completion since the caller may be reading events from
                // the reader task's forwarding mechanism.
                let _ = &event_tx; // keep alive
            });

            Ok(event_rx)
        })
    }

    fn cancel(
        &self,
        session: &SessionHandle,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderError>> + Send + '_>> {
        let session_id = session.session_id;
        Box::pin(async move {
            let mut sessions = self.sessions.lock().await;
            if let Some(mut session) = sessions.remove(&session_id) {
                info!(%session_id, "Killing Codex session");

                // Abort the reader/writer tasks
                session.reader_handle.abort();
                session.writer_handle.abort();

                // Kill the process tree
                if let Some(pid) = session.child.id() {
                    let _ = kill_process_tree(pid).await;
                }

                // Ensure the child is killed
                let _ = session.child.kill().await;

                Ok(())
            } else {
                Err(ProviderError::SessionNotFound(session_id.to_string()))
            }
        })
    }

    fn list_models(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ModelInfo>, ProviderError>> + Send + '_>> {
        Box::pin(async move {
            // To list models we need an active session. Find any active session.
            let session_id = {
                let sessions = self.sessions.lock().await;
                sessions
                    .keys()
                    .next()
                    .copied()
                    .ok_or_else(|| ProviderError::Internal("no active session for model list".into()))?
            };

            let resp = self.send_request(&session_id, "model/list", None).await?;

            let result = resp.result.ok_or_else(|| {
                ProviderError::Internal("No result in model/list response".into())
            })?;

            let list: ModelListResult = serde_json::from_value(result)
                .map_err(|e| ProviderError::Internal(format!("parse model list: {}", e)))?;

            Ok(list
                .models
                .into_iter()
                .map(|m| ModelInfo {
                    id: m.id.clone(),
                    name: m.name.unwrap_or_else(|| m.id.clone()),
                    max_context: None,
                    provider_kind: ProviderKind::Codex,
                })
                .collect())
        })
    }

    fn session_status(&self, session: &SessionHandle) -> ProviderSessionStatus {
        // This is sync, so we can't await. Use try_lock.
        if let Ok(sessions) = self.sessions.try_lock() {
            sessions
                .get(&session.session_id)
                .map(|s| s.status)
                .unwrap_or(ProviderSessionStatus::Stopped)
        } else {
            // Lock contention -- return a safe default.
            ProviderSessionStatus::Running
        }
    }
}

// ─── Process tree killing ───────────────────────────────────────────

/// Kill a process tree (the process and all its descendants).
async fn kill_process_tree(pid: u32) -> Result<(), ProviderError> {
    #[cfg(windows)]
    {
        kill_process_tree_windows(pid)?;
    }
    #[cfg(not(windows))]
    {
        kill_process_tree_unix(pid)?;
    }
    info!(%pid, "Killed process tree");
    Ok(())
}

#[cfg(windows)]
fn kill_process_tree_windows(pid: u32) -> Result<(), ProviderError> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::JobObjects::*;
    use windows_sys::Win32::System::Threading::*;

    unsafe {
        let process = OpenProcess(PROCESS_TERMINATE | PROCESS_SET_QUOTA, 0, pid);
        if process.is_null() {
            return Ok(());
        }

        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if !job.is_null() {
            AssignProcessToJobObject(job, process);
            TerminateJobObject(job, 1);
            CloseHandle(job);
        } else {
            TerminateProcess(process, 1);
        }

        CloseHandle(process);
    }

    Ok(())
}

#[cfg(not(windows))]
fn kill_process_tree_unix(pid: u32) -> Result<(), ProviderError> {
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
    Ok(())
}

// ─── Binary discovery ───────────────────────────────────────────────

/// Find the codex binary on PATH.
pub fn find_codex_binary(custom_path: Option<&str>) -> Result<String, ProviderError> {
    if let Some(path) = custom_path {
        if !path.is_empty() {
            return Ok(path.to_string());
        }
    }

    // Try to find "codex" on PATH
    let which_cmd = if cfg!(windows) { "where" } else { "which" };
    let output = std::process::Command::new(which_cmd)
        .arg("codex")
        .output()
        .map_err(|e| ProviderError::ConnectionFailed(format!("search for codex on PATH: {}", e)))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let candidates = stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        let path = candidates
            .iter()
            .find(|path| {
                path.ends_with(".exe") || path.ends_with(".cmd") || path.ends_with(".bat")
            })
            .or_else(|| candidates.iter().find(|path| !path.ends_with(".ps1")))
            .copied()
            .unwrap_or("codex")
            .to_string();
        Ok(path)
    } else {
        Err(ProviderError::ConnectionFailed(
            "Codex CLI not found on PATH. Please install it: https://github.com/openai/codex"
                .into(),
        ))
    }
}

/// Check that the codex binary exists and meets the minimum version requirement.
pub fn check_codex_version(
    binary_path: &str,
    min_version: &str,
) -> Result<String, ProviderError> {
    let output = std::process::Command::new(binary_path)
        .arg("--version")
        .output()
        .map_err(|e| {
            ProviderError::ConnectionFailed(format!(
                "Failed to run '{}'. Is Codex CLI installed? {}",
                binary_path, e
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ProviderError::ConnectionFailed(format!(
            "codex --version failed: {}",
            stderr.trim()
        )));
    }

    let version_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Extract version number (handle "codex 0.37.0" or just "0.37.0")
    let version = version_str
        .split_whitespace()
        .next_back()
        .unwrap_or(&version_str)
        .trim_start_matches('v');

    info!(version = %version, "Detected Codex CLI version");

    if !version_meets_minimum(version, min_version) {
        return Err(ProviderError::ConnectionFailed(format!(
            "Codex CLI version {} is too old. Minimum required: {}. Please update with: codex update",
            version, min_version
        )));
    }

    Ok(version.to_string())
}

/// Check if a version string meets the minimum required version.
fn version_meets_minimum(version: &str, minimum: &str) -> bool {
    let parse_parts =
        |v: &str| -> Vec<u32> { v.split('.').filter_map(|p| p.parse::<u32>().ok()).collect() };

    let current = parse_parts(version);
    let required = parse_parts(minimum);

    for i in 0..required.len().max(current.len()) {
        let c = current.get(i).copied().unwrap_or(0);
        let r = required.get(i).copied().unwrap_or(0);
        if c > r {
            return true;
        }
        if c < r {
            return false;
        }
    }
    true // equal
}

// ─── Notification parsing helpers ──────────────────────────────────

/// Parse a Codex notification into a CodexEvent.
pub fn parse_notification(notif: &JsonRpcNotification) -> Option<CodexEvent> {
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
pub fn parse_server_request(rpc_id: u64, method: &str, params: Option<Value>) -> Option<CodexEvent> {
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

/// Translate a CodexEvent into a ProviderEvent (where applicable).
pub fn codex_event_to_provider_event(event: &CodexEvent) -> Option<ProviderEvent> {
    match event {
        CodexEvent::AgentMessageDelta { delta, .. } => Some(ProviderEvent::TokenDelta {
            text: delta.clone(),
        }),
        CodexEvent::TurnCompleted { status, .. } => {
            let finish_reason = if status == "completed" {
                FinishReason::Stop
            } else {
                FinishReason::Error
            };
            Some(ProviderEvent::Completion { finish_reason })
        }
        CodexEvent::Error { message, .. } => Some(ProviderEvent::Error {
            message: message.clone(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(version_meets_minimum("0.37.0", "0.37.0"));
        assert!(version_meets_minimum("0.38.0", "0.37.0"));
        assert!(version_meets_minimum("1.0.0", "0.37.0"));
        assert!(!version_meets_minimum("0.36.9", "0.37.0"));
        assert!(!version_meets_minimum("0.36.0", "0.37.0"));
        assert!(version_meets_minimum("0.37.1", "0.37.0"));
    }

    #[test]
    fn test_find_codex_binary_custom_path() {
        let result = find_codex_binary(Some("/usr/local/bin/codex"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/usr/local/bin/codex");
    }

    #[test]
    fn test_find_codex_binary_empty_path_falls_through() {
        let result = find_codex_binary(Some(""));
        // Empty string falls through to PATH search — may or may not find codex.
        // We just verify it doesn't panic.
        let _ = result;
    }

    #[test]
    fn test_parse_notification_thread_started() {
        let notif = JsonRpcNotification {
            method: "thread/started".to_string(),
            params: Some(serde_json::json!({
                "thread": { "id": "thread_abc" }
            })),
        };
        let event = parse_notification(&notif).unwrap();
        match event {
            CodexEvent::ThreadStarted { codex_thread_id } => {
                assert_eq!(codex_thread_id, "thread_abc");
            }
            _ => panic!("wrong event variant"),
        }
    }

    #[test]
    fn test_parse_notification_turn_started() {
        let notif = JsonRpcNotification {
            method: "turn/started".to_string(),
            params: Some(serde_json::json!({
                "turn": { "id": "turn_1" }
            })),
        };
        let event = parse_notification(&notif).unwrap();
        match event {
            CodexEvent::TurnStarted { codex_turn_id } => {
                assert_eq!(codex_turn_id, "turn_1");
            }
            _ => panic!("wrong event variant"),
        }
    }

    #[test]
    fn test_parse_notification_turn_completed() {
        let notif = JsonRpcNotification {
            method: "turn/completed".to_string(),
            params: Some(serde_json::json!({
                "turn": { "id": "turn_1", "status": "completed" }
            })),
        };
        let event = parse_notification(&notif).unwrap();
        match event {
            CodexEvent::TurnCompleted {
                codex_turn_id,
                status,
            } => {
                assert_eq!(codex_turn_id, "turn_1");
                assert_eq!(status, "completed");
            }
            _ => panic!("wrong event variant"),
        }
    }

    #[test]
    fn test_parse_notification_agent_message_delta() {
        let notif = JsonRpcNotification {
            method: "item/agentMessage/delta".to_string(),
            params: Some(serde_json::json!({
                "delta": "Hello world",
                "turnId": "turn_1",
                "itemId": "item_1"
            })),
        };
        let event = parse_notification(&notif).unwrap();
        match event {
            CodexEvent::AgentMessageDelta {
                codex_turn_id,
                item_id,
                delta,
            } => {
                assert_eq!(delta, "Hello world");
                assert_eq!(codex_turn_id, "turn_1");
                assert_eq!(item_id, "item_1");
            }
            _ => panic!("wrong event variant"),
        }
    }

    #[test]
    fn test_parse_notification_error() {
        let notif = JsonRpcNotification {
            method: "error".to_string(),
            params: Some(serde_json::json!({
                "error": { "message": "rate limited" },
                "willRetry": true
            })),
        };
        let event = parse_notification(&notif).unwrap();
        match event {
            CodexEvent::Error {
                message,
                will_retry,
            } => {
                assert_eq!(message, "rate limited");
                assert!(will_retry);
            }
            _ => panic!("wrong event variant"),
        }
    }

    #[test]
    fn test_parse_notification_unknown_method() {
        let notif = JsonRpcNotification {
            method: "some/unknown".to_string(),
            params: Some(serde_json::json!({})),
        };
        assert!(parse_notification(&notif).is_none());
    }

    #[test]
    fn test_parse_server_request_command_approval() {
        let event = parse_server_request(
            42,
            "item/commandExecution/requestApproval",
            Some(serde_json::json!({
                "turnId": "t1",
                "itemId": "i1",
                "command": { "cmd": "ls" }
            })),
        )
        .unwrap();
        match event {
            CodexEvent::CommandApprovalRequested {
                rpc_id,
                turn_id,
                item_id,
                command,
            } => {
                assert_eq!(rpc_id, 42);
                assert_eq!(turn_id, "t1");
                assert_eq!(item_id, "i1");
                assert!(command.is_some());
            }
            _ => panic!("wrong event variant"),
        }
    }

    #[test]
    fn test_parse_server_request_file_change() {
        let event = parse_server_request(
            10,
            "item/fileChange/requestApproval",
            Some(serde_json::json!({
                "turnId": "t1",
                "itemId": "i1",
                "filePath": "src/main.rs",
                "diff": "+hello\n-world"
            })),
        )
        .unwrap();
        match event {
            CodexEvent::FileChangeApprovalRequested {
                rpc_id,
                diff,
                file_path,
                ..
            } => {
                assert_eq!(rpc_id, 10);
                assert_eq!(file_path, Some("src/main.rs".to_string()));
                assert_eq!(diff, Some("+hello\n-world".to_string()));
            }
            _ => panic!("wrong event variant"),
        }
    }

    #[test]
    fn test_parse_server_request_user_input() {
        let event = parse_server_request(
            7,
            "item/tool/requestUserInput",
            Some(serde_json::json!({
                "turnId": "t1",
                "itemId": "i1",
                "questions": ["What is your name?"]
            })),
        )
        .unwrap();
        match event {
            CodexEvent::UserInputRequested {
                rpc_id, questions, ..
            } => {
                assert_eq!(rpc_id, 7);
                assert!(questions.is_some());
            }
            _ => panic!("wrong event variant"),
        }
    }

    #[test]
    fn test_codex_event_to_provider_event_delta() {
        let event = CodexEvent::AgentMessageDelta {
            codex_turn_id: "t1".into(),
            item_id: "i1".into(),
            delta: "hello".into(),
        };
        let pe = codex_event_to_provider_event(&event).unwrap();
        match pe {
            ProviderEvent::TokenDelta { text } => assert_eq!(text, "hello"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_codex_event_to_provider_event_completion() {
        let event = CodexEvent::TurnCompleted {
            codex_turn_id: "t1".into(),
            status: "completed".into(),
        };
        let pe = codex_event_to_provider_event(&event).unwrap();
        match pe {
            ProviderEvent::Completion { finish_reason } => {
                assert_eq!(finish_reason, FinishReason::Stop);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_codex_event_to_provider_event_error() {
        let event = CodexEvent::Error {
            message: "boom".into(),
            will_retry: false,
        };
        let pe = codex_event_to_provider_event(&event).unwrap();
        match pe {
            ProviderEvent::Error { message } => assert_eq!(message, "boom"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_codex_event_to_provider_event_none_for_approval() {
        let event = CodexEvent::CommandApprovalRequested {
            rpc_id: 1,
            turn_id: "t1".into(),
            item_id: "i1".into(),
            command: None,
        };
        assert!(codex_event_to_provider_event(&event).is_none());
    }

    #[test]
    fn test_codex_provider_new_defaults() {
        let provider = CodexProvider::new(Some("test-codex".into()), None);
        assert_eq!(provider.binary_path, "test-codex");
        assert!(provider.codex_home.is_none());
    }

    #[test]
    fn test_codex_provider_configure() {
        let mut provider = CodexProvider::new(Some("test-codex".into()), None);
        provider.configure(Some("/new/path/codex".into()), Some("/codex/home".into()));
        assert_eq!(provider.binary_path, "/new/path/codex");
        assert_eq!(provider.codex_home, Some("/codex/home".to_string()));
    }
}
