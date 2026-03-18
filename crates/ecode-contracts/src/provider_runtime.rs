//! Normalized provider runtime types shared across providers.

use crate::ids::{ApprovalRequestId, TurnId};
use crate::provider::ProviderKind;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Coarse provider session state used by the UI and orchestration layer.
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

/// Normalized runtime event kinds inspired by T3 Code's provider-runtime model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRuntimeEventKind {
    SessionStateChanged,
    TurnStarted,
    ContentDelta,
    TurnCompleted,
    RequestOpened,
    RequestResolved,
    UserInputRequested,
    ToolStarted,
    ToolUpdated,
    ToolCompleted,
    RuntimeWarning,
    RuntimeError,
}

/// A normalized runtime event stored on the thread for UI rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRuntimeEvent {
    pub provider: ProviderKind,
    pub event_type: ProviderRuntimeEventKind,
    pub turn_id: Option<TurnId>,
    pub item_id: Option<String>,
    pub request_id: Option<ApprovalRequestId>,
    pub summary: Option<String>,
    #[serde(default)]
    pub data: Value,
    pub timestamp: DateTime<Utc>,
}
