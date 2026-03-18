//! Codex CLI JSON-RPC protocol types.
//!
//! These types mirror the Codex CLI `app-server` protocol exactly:
//! newline-delimited JSON-RPC 2.0 over stdin/stdout.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ─── Generic JSON-RPC Frames ────────────────────────────────────────

/// A JSON-RPC 2.0 request (client → Codex).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// A JSON-RPC 2.0 response (Codex → client, or client → Codex for server requests).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// A JSON-RPC 2.0 notification (no id — fire-and-forget).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// Parsed incoming message from Codex stdout.
#[derive(Debug, Clone)]
pub enum IncomingMessage {
    /// Response to a request we sent (has id, no method).
    Response(JsonRpcResponse),
    /// Notification from Codex (has method, no id).
    Notification(JsonRpcNotification),
    /// Server request from Codex that needs our response (has both method and id).
    ServerRequest {
        id: u64,
        method: String,
        params: Option<Value>,
    },
}

impl IncomingMessage {
    /// Parse a raw JSON value into an IncomingMessage.
    pub fn parse(value: &Value) -> Option<Self> {
        let has_method = value.get("method").and_then(|m| m.as_str()).is_some();
        let has_id = value.get("id").is_some();
        let has_result_or_error = value.get("result").is_some() || value.get("error").is_some();

        match (has_method, has_id, has_result_or_error) {
            // Response: has id + result/error, no method
            (false, true, true) => serde_json::from_value(value.clone())
                .ok()
                .map(IncomingMessage::Response),
            // Server request: has both method and id
            (true, true, false) => {
                let id = value.get("id")?.as_u64()?;
                let method = value.get("method")?.as_str()?.to_string();
                let params = value.get("params").cloned();
                Some(IncomingMessage::ServerRequest { id, method, params })
            }
            // Notification: has method, no id
            (true, false, _) => serde_json::from_value(value.clone())
                .ok()
                .map(IncomingMessage::Notification),
            _ => None,
        }
    }
}

// ─── Initialize ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub title: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    #[serde(rename = "experimentalApi")]
    pub experimental_api: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    #[serde(rename = "clientInfo")]
    pub client_info: ClientInfo,
    pub capabilities: ClientCapabilities,
}

// ─── Thread Management ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalPolicy {
    Never,
    OnRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadStartParams {
    pub model: String,
    #[serde(rename = "approvalPolicy")]
    pub approval_policy: ApprovalPolicy,
    pub sandbox: SandboxMode,
    pub cwd: String,
    #[serde(rename = "experimentalRawEvents")]
    #[serde(default)]
    pub experimental_raw_events: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadResumeParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadReadParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadRollbackParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    pub n: u32,
}

// ─── Turn Management ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TurnInputItem {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(default)]
        text_elements: Vec<Value>,
    },
    #[serde(rename = "image")]
    Image {
        #[serde(rename = "base64Data")]
        base64_data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnStartParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    pub input: Vec<TurnInputItem>,
    /// Optional developer instructions (used for plan mode).
    #[serde(rename = "developerInstructions")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnInterruptParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "turnId")]
    pub turn_id: String,
}

// ─── Notifications from Codex ───────────────────────────────────────

/// Thread info returned in thread/* notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexThreadInfo {
    pub id: String,
}

/// Turn info returned in turn/* notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexTurnInfo {
    pub id: String,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadStartedNotification {
    pub thread: CodexThreadInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnStartedNotification {
    pub turn: CodexTurnInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnCompletedNotification {
    pub turn: CodexTurnInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessageDelta {
    pub delta: String,
    #[serde(rename = "turnId")]
    pub turn_id: String,
    #[serde(rename = "itemId")]
    pub item_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexError {
    pub error: CodexErrorDetail,
    #[serde(rename = "willRetry")]
    #[serde(default)]
    pub will_retry: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexErrorDetail {
    pub message: String,
    #[serde(default)]
    pub code: Option<String>,
}

// ─── Server Requests (Codex → client, need response) ────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandApprovalRequest {
    #[serde(rename = "turnId")]
    pub turn_id: String,
    #[serde(rename = "itemId")]
    pub item_id: String,
    pub command: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeApprovalRequest {
    #[serde(rename = "turnId")]
    pub turn_id: String,
    #[serde(rename = "itemId")]
    pub item_id: String,
    #[serde(rename = "filePath")]
    pub file_path: Option<String>,
    pub diff: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadApprovalRequest {
    #[serde(rename = "turnId")]
    pub turn_id: String,
    #[serde(rename = "itemId")]
    pub item_id: String,
    #[serde(rename = "filePath")]
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInputRequest {
    #[serde(rename = "turnId")]
    pub turn_id: String,
    #[serde(rename = "itemId")]
    pub item_id: String,
    pub questions: Option<Value>,
}

// ─── Client Responses to Server Requests ────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Accept,
    Deny,
    AlwaysApprove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalResponse {
    pub decision: ApprovalDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInputAnswer {
    pub answers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInputResponse {
    pub answers: std::collections::HashMap<String, UserInputAnswer>,
}

// ─── Account & Model ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    #[serde(rename = "accountType")]
    pub account_type: String,
    #[serde(rename = "planType")]
    pub plan_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelListResult {
    pub models: Vec<ModelInfo>,
}

// ─── Codex Events (parsed from notifications) ──────────────────────

/// High-level events emitted by the Codex manager for the orchestration layer.
#[derive(Debug, Clone)]
pub enum CodexEvent {
    ThreadStarted {
        codex_thread_id: String,
    },
    TurnStarted {
        codex_turn_id: String,
    },
    TurnCompleted {
        codex_turn_id: String,
        status: String,
    },
    AgentMessageDelta {
        codex_turn_id: String,
        item_id: String,
        delta: String,
    },
    CommandApprovalRequested {
        rpc_id: u64,
        turn_id: String,
        item_id: String,
        command: Option<Value>,
    },
    FileChangeApprovalRequested {
        rpc_id: u64,
        turn_id: String,
        item_id: String,
        file_path: Option<String>,
        diff: Option<String>,
    },
    FileReadApprovalRequested {
        rpc_id: u64,
        turn_id: String,
        item_id: String,
        file_path: Option<String>,
    },
    UserInputRequested {
        rpc_id: u64,
        turn_id: String,
        item_id: String,
        questions: Option<Value>,
    },
    Error {
        message: String,
        will_retry: bool,
    },
    SessionClosed {
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_response() {
        let json = serde_json::json!({
            "id": 1,
            "result": { "thread": { "id": "thread_abc123" } }
        });
        let msg = IncomingMessage::parse(&json).unwrap();
        assert!(matches!(msg, IncomingMessage::Response(_)));
    }

    #[test]
    fn parse_notification() {
        let json = serde_json::json!({
            "method": "item/agentMessage/delta",
            "params": { "delta": "Hello", "turnId": "t1", "itemId": "i1" }
        });
        let msg = IncomingMessage::parse(&json).unwrap();
        assert!(matches!(msg, IncomingMessage::Notification(_)));
    }

    #[test]
    fn parse_server_request() {
        let json = serde_json::json!({
            "method": "item/commandExecution/requestApproval",
            "id": 42,
            "params": { "turnId": "t1", "itemId": "i1" }
        });
        let msg = IncomingMessage::parse(&json).unwrap();
        assert!(matches!(msg, IncomingMessage::ServerRequest { id: 42, .. }));
    }

    #[test]
    fn approval_decision_serde() {
        let decision = ApprovalDecision::Accept;
        let json = serde_json::to_string(&decision).unwrap();
        assert_eq!(json, r#""accept""#);
    }

    #[test]
    fn turn_input_text_serde() {
        let input = TurnInputItem::Text {
            text: "Hello, world!".to_string(),
            text_elements: vec![],
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello, world!");
    }

    #[test]
    fn sandbox_mode_serde() {
        let mode = SandboxMode::DangerFullAccess;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, r#""danger-full-access""#);
    }
}
