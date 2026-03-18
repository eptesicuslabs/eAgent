//! Terminal tools — PTY terminal session management for agents.

use crate::{Tool, ToolContext, ToolError, ToolResult};
use eagent_protocol::ids::TerminalId;
use eagent_protocol::messages::RiskLevel;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::future::Future;
use std::io::{Read, Write};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{error, info};

// ---------------------------------------------------------------------------
// Terminal events
// ---------------------------------------------------------------------------

/// Terminal event emitted by the manager.
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    /// Terminal produced output data.
    Output {
        terminal_id: TerminalId,
        data: String,
    },
    /// Terminal process exited.
    Exited {
        terminal_id: TerminalId,
        exit_code: Option<u32>,
    },
    /// Terminal was resized.
    Resized {
        terminal_id: TerminalId,
        cols: u16,
        rows: u16,
    },
}

// ---------------------------------------------------------------------------
// TerminalSession
// ---------------------------------------------------------------------------

/// A running terminal session.
struct TerminalSession {
    /// Writer to the PTY master.
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    /// Whether the session is still alive.
    alive: Arc<AtomicBool>,
}

// ---------------------------------------------------------------------------
// TerminalManager
// ---------------------------------------------------------------------------

/// Manager for PTY terminal sessions.
///
/// Manages the lifecycle of terminal sessions: create, write, resize, close.
/// Terminal output and exit events are emitted through an unbounded channel.
pub struct TerminalManager {
    sessions: Arc<Mutex<HashMap<TerminalId, TerminalSession>>>,
    event_tx: mpsc::UnboundedSender<TerminalEvent>,
}

impl TerminalManager {
    /// Create a new terminal manager.
    /// Returns the manager and a receiver for terminal events.
    pub fn new() -> (Self, mpsc::UnboundedReceiver<TerminalEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Self {
                sessions: Arc::new(Mutex::new(HashMap::new())),
                event_tx: tx,
            },
            rx,
        )
    }

    /// Open a new terminal session.
    ///
    /// - `cwd`: optional working directory (empty string or None = inherit).
    /// - `shell`: optional shell override (None = platform default).
    /// - `cols` / `rows`: initial terminal size (defaults: 120x30).
    pub fn open(
        &self,
        cwd: Option<&str>,
        shell: Option<&str>,
        cols: u16,
        rows: u16,
    ) -> Result<TerminalId, String> {
        let terminal_id = TerminalId::new();
        let pty_system = native_pty_system();

        let pty_pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("failed to open PTY: {}", e))?;

        let shell_cmd = shell
            .map(|s| s.to_string())
            .unwrap_or_else(default_shell);

        let mut cmd = CommandBuilder::new(&shell_cmd);
        if let Some(dir) = cwd {
            if !dir.is_empty() {
                cmd.cwd(dir);
            }
        }

        pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("failed to spawn terminal process: {}", e))?;

        let writer = pty_pair
            .master
            .take_writer()
            .map_err(|e| format!("failed to take PTY writer: {}", e))?;

        let alive = Arc::new(AtomicBool::new(true));

        // Spawn a reader thread to capture output and forward as events.
        let mut reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("failed to clone PTY reader: {}", e))?;
        let event_tx = self.event_tx.clone();
        let tid = terminal_id;
        let alive_clone = alive.clone();

        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        alive_clone.store(false, Ordering::SeqCst);
                        let _ = event_tx.send(TerminalEvent::Exited {
                            terminal_id: tid,
                            exit_code: None,
                        });
                        break;
                    }
                    Ok(n) => {
                        let data = String::from_utf8_lossy(&buf[..n]).to_string();
                        let _ = event_tx.send(TerminalEvent::Output {
                            terminal_id: tid,
                            data,
                        });
                    }
                    Err(e) => {
                        error!(%e, "Terminal reader error");
                        alive_clone.store(false, Ordering::SeqCst);
                        let _ = event_tx.send(TerminalEvent::Exited {
                            terminal_id: tid,
                            exit_code: None,
                        });
                        break;
                    }
                }
            }
        });

        let session = TerminalSession {
            writer: Arc::new(Mutex::new(writer)),
            alive,
        };

        self.sessions.lock().unwrap().insert(terminal_id, session);
        info!(%terminal_id, shell = %shell_cmd, "Opened terminal session");
        Ok(terminal_id)
    }

    /// Write data to a terminal session.
    pub fn write(&self, terminal_id: &TerminalId, data: &[u8]) -> Result<(), String> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get(terminal_id)
            .ok_or_else(|| format!("terminal session not found: {}", terminal_id))?;

        let mut writer = session.writer.lock().unwrap();
        writer
            .write_all(data)
            .map_err(|e| format!("failed to write to terminal: {}", e))?;
        writer
            .flush()
            .map_err(|e| format!("failed to flush terminal writer: {}", e))?;
        Ok(())
    }

    /// Resize a terminal session.
    pub fn resize(&self, terminal_id: &TerminalId, cols: u16, rows: u16) -> Result<(), String> {
        let _ = self.event_tx.send(TerminalEvent::Resized {
            terminal_id: *terminal_id,
            cols,
            rows,
        });
        Ok(())
    }

    /// Close a terminal session.
    pub fn close(&self, terminal_id: &TerminalId) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.remove(terminal_id) {
            session.alive.store(false, Ordering::SeqCst);
            info!(%terminal_id, "Closed terminal session");
        }
        Ok(())
    }

    /// Check if a terminal session is alive.
    pub fn is_alive(&self, terminal_id: &TerminalId) -> bool {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .get(terminal_id)
            .map(|s| s.alive.load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    /// Get the list of active terminal IDs.
    pub fn active_terminals(&self) -> Vec<TerminalId> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .iter()
            .filter(|(_, s)| s.alive.load(Ordering::SeqCst))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Close all terminal sessions.
    pub fn close_all(&self) {
        let mut sessions = self.sessions.lock().unwrap();
        for (id, session) in sessions.drain() {
            session.alive.store(false, Ordering::SeqCst);
            info!(%id, "Closed terminal session (shutdown)");
        }
    }
}

impl Drop for TerminalManager {
    fn drop(&mut self) {
        self.close_all();
    }
}

// ---------------------------------------------------------------------------
// Platform default shell
// ---------------------------------------------------------------------------

fn default_shell() -> String {
    #[cfg(windows)]
    {
        "powershell.exe".to_string()
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

// ---------------------------------------------------------------------------
// CreateTerminalTool
// ---------------------------------------------------------------------------

/// Tool that creates a new PTY terminal session.
pub struct CreateTerminalTool;

impl Tool for CreateTerminalTool {
    fn name(&self) -> &str {
        "create_terminal"
    }

    fn description(&self) -> &str {
        "Create a new PTY terminal session. Returns the terminal ID for subsequent interactions."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "cwd": {
                    "type": "string",
                    "description": "Working directory for the terminal. Defaults to workspace root."
                },
                "shell": {
                    "type": "string",
                    "description": "Shell to use (e.g. 'bash', 'powershell.exe'). Defaults to platform default."
                }
            }
        })
    }

    fn execute(
        &self,
        params: Value,
        ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        let workspace_root = ctx.workspace_root.clone();
        let services = ctx.services.clone();
        Box::pin(async move {
            let services = services
                .as_ref()
                .ok_or_else(|| ToolError::ExecutionFailed("no services available".into()))?;
            let tm = services
                .terminal_manager
                .as_ref()
                .ok_or_else(|| ToolError::ExecutionFailed("no terminal manager available".into()))?;

            let cwd = params
                .get("cwd")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
                .unwrap_or_else(|| workspace_root.clone());

            let shell = params.get("shell").and_then(Value::as_str).map(|s| s.to_string());

            let terminal_id = tm
                .open(Some(&cwd), shell.as_deref(), 120, 30)
                .map_err(|e| ToolError::ExecutionFailed(e))?;

            Ok(ToolResult {
                output: json!({
                    "terminal_id": terminal_id.to_string(),
                    "message": format!("Terminal session created: {}", terminal_id)
                }),
                is_error: false,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// TerminalWriteTool
// ---------------------------------------------------------------------------

/// Tool that writes input to an existing terminal session.
pub struct TerminalWriteTool;

impl Tool for TerminalWriteTool {
    fn name(&self) -> &str {
        "terminal_write"
    }

    fn description(&self) -> &str {
        "Write input to an existing PTY terminal session."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "terminal_id": {
                    "type": "string",
                    "description": "The UUID of the terminal session to write to."
                },
                "input": {
                    "type": "string",
                    "description": "Text to write to the terminal."
                }
            },
            "required": ["terminal_id", "input"]
        })
    }

    fn execute(
        &self,
        params: Value,
        ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        let services = ctx.services.clone();
        Box::pin(async move {
            let services = services
                .as_ref()
                .ok_or_else(|| ToolError::ExecutionFailed("no services available".into()))?;
            let tm = services
                .terminal_manager
                .as_ref()
                .ok_or_else(|| ToolError::ExecutionFailed("no terminal manager available".into()))?;

            let terminal_id_str = params
                .get("terminal_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    ToolError::InvalidParams("missing required string parameter 'terminal_id'".into())
                })?;

            let terminal_id = TerminalId::parse(terminal_id_str).map_err(|e| {
                ToolError::InvalidParams(format!("invalid terminal_id '{}': {}", terminal_id_str, e))
            })?;

            let input = params
                .get("input")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    ToolError::InvalidParams("missing required string parameter 'input'".into())
                })?;

            tm.write(&terminal_id, input.as_bytes())
                .map_err(|e| ToolError::ExecutionFailed(e))?;

            Ok(ToolResult {
                output: json!({
                    "message": format!("Wrote {} bytes to terminal {}", input.len(), terminal_id)
                }),
                is_error: false,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Tool, ToolContext};
    use serde_json::json;

    fn test_ctx_with_services() -> (ToolContext, mpsc::UnboundedReceiver<TerminalEvent>) {
        let (tm, rx) = TerminalManager::new();
        let services = crate::ToolServices {
            terminal_manager: Some(Arc::new(tm)),
        };
        let ctx = ToolContext {
            workspace_root: std::env::temp_dir().to_string_lossy().to_string(),
            agent_id: eagent_protocol::ids::AgentId::new(),
            task_id: eagent_protocol::ids::TaskId::new(),
            services: Some(Arc::new(services)),
        };
        (ctx, rx)
    }

    fn test_ctx_no_services() -> ToolContext {
        ToolContext {
            workspace_root: std::env::temp_dir().to_string_lossy().to_string(),
            agent_id: eagent_protocol::ids::AgentId::new(),
            task_id: eagent_protocol::ids::TaskId::new(),
            services: None,
        }
    }

    // -- TerminalManager structural tests --

    #[test]
    fn terminal_manager_creation() {
        let (tm, _rx) = TerminalManager::new();
        assert!(tm.active_terminals().is_empty());
    }

    #[test]
    fn terminal_manager_write_nonexistent_fails() {
        let (tm, _rx) = TerminalManager::new();
        let fake_id = TerminalId::new();
        let result = tm.write(&fake_id, b"hello");
        assert!(result.is_err());
    }

    #[test]
    fn terminal_manager_close_nonexistent_ok() {
        let (tm, _rx) = TerminalManager::new();
        let fake_id = TerminalId::new();
        assert!(tm.close(&fake_id).is_ok());
    }

    #[test]
    fn terminal_manager_is_alive_nonexistent() {
        let (tm, _rx) = TerminalManager::new();
        let fake_id = TerminalId::new();
        assert!(!tm.is_alive(&fake_id));
    }

    #[test]
    fn terminal_manager_close_all_empty() {
        let (tm, _rx) = TerminalManager::new();
        tm.close_all(); // should not panic
    }

    // -- Tool definition tests --

    #[test]
    fn create_terminal_tool_def() {
        let tool = CreateTerminalTool;
        assert_eq!(tool.name(), "create_terminal");
        assert_eq!(tool.risk_level(), RiskLevel::Medium);
        let schema = tool.parameter_schema();
        assert!(schema.get("properties").is_some());
        assert!(schema["properties"].get("cwd").is_some());
        assert!(schema["properties"].get("shell").is_some());
    }

    #[test]
    fn terminal_write_tool_def() {
        let tool = TerminalWriteTool;
        assert_eq!(tool.name(), "terminal_write");
        assert_eq!(tool.risk_level(), RiskLevel::Medium);
        let schema = tool.parameter_schema();
        assert!(schema.get("properties").is_some());
        assert!(schema["properties"].get("terminal_id").is_some());
        assert!(schema["properties"].get("input").is_some());
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("terminal_id")));
        assert!(required.contains(&json!("input")));
    }

    #[test]
    fn create_terminal_tool_to_def() {
        let tool = CreateTerminalTool;
        let def = tool.to_def();
        assert_eq!(def.name, "create_terminal");
        assert_eq!(def.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn terminal_write_tool_to_def() {
        let tool = TerminalWriteTool;
        let def = tool.to_def();
        assert_eq!(def.name, "terminal_write");
        assert_eq!(def.risk_level, RiskLevel::Medium);
    }

    // -- Tool execution error paths --

    #[tokio::test]
    async fn create_terminal_no_services_fails() {
        let tool = CreateTerminalTool;
        let ctx = test_ctx_no_services();
        let result = tool.execute(json!({}), &ctx).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ExecutionFailed(msg) => assert!(msg.contains("no services")),
            other => panic!("expected ExecutionFailed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn terminal_write_no_services_fails() {
        let tool = TerminalWriteTool;
        let ctx = test_ctx_no_services();
        let result = tool
            .execute(
                json!({"terminal_id": "00000000-0000-0000-0000-000000000000", "input": "hello"}),
                &ctx,
            )
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ExecutionFailed(msg) => assert!(msg.contains("no services")),
            other => panic!("expected ExecutionFailed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn terminal_write_invalid_id_fails() {
        let tool = TerminalWriteTool;
        let (ctx, _rx) = test_ctx_with_services();
        let result = tool
            .execute(json!({"terminal_id": "not-a-uuid", "input": "hello"}), &ctx)
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidParams(msg) => assert!(msg.contains("invalid terminal_id")),
            other => panic!("expected InvalidParams, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn terminal_write_missing_params_fails() {
        let tool = TerminalWriteTool;
        let (ctx, _rx) = test_ctx_with_services();
        let result = tool.execute(json!({}), &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn terminal_write_nonexistent_terminal_fails() {
        let tool = TerminalWriteTool;
        let (ctx, _rx) = test_ctx_with_services();
        let result = tool
            .execute(
                json!({"terminal_id": "00000000-0000-0000-0000-000000000000", "input": "hello"}),
                &ctx,
            )
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ExecutionFailed(msg) => assert!(msg.contains("not found")),
            other => panic!("expected ExecutionFailed, got {:?}", other),
        }
    }

    // -- Integration: create a real terminal --

    #[tokio::test]
    async fn create_terminal_succeeds() {
        let tool = CreateTerminalTool;
        let (ctx, _rx) = test_ctx_with_services();
        let result = tool.execute(json!({}), &ctx).await;
        assert!(
            result.is_ok(),
            "create_terminal failed: {:?}",
            result.unwrap_err()
        );
        let output = result.unwrap();
        assert!(!output.is_error);
        assert!(output.output.get("terminal_id").is_some());
    }
}
