//! Persistence types for the event store.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A stored event in the event log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    /// Auto-incrementing global sequence number.
    pub id: i64,
    /// Stream ID (e.g., "thread:{uuid}").
    pub stream_id: String,
    /// Event type discriminator.
    pub event_type: String,
    /// Event payload as JSON.
    pub payload: Value,
    /// When the event was persisted.
    pub timestamp: DateTime<Utc>,
    /// Per-stream sequence number.
    pub sequence: i64,
}
