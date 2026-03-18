//! SQLite-based event store for event sourcing.

use anyhow::{Context, Result};
use ecode_contracts::persistence::StoredEvent;
use rusqlite::{Connection, params};
use serde_json::Value;
use std::path::Path;
use std::sync::Mutex;

/// Event store backed by SQLite.
pub struct EventStore {
    conn: Mutex<Connection>,
}

impl EventStore {
    /// Open or create an event store at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open SQLite database at {}", path.display()))?;

        let store = Self {
            conn: Mutex::new(conn),
        };
        store.initialize()?;
        Ok(store)
    }

    /// Open an in-memory event store (useful for testing).
    pub fn in_memory() -> Result<Self> {
        let conn =
            Connection::open_in_memory().context("Failed to open in-memory SQLite database")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.initialize()?;
        Ok(store)
    }

    /// Initialize the database schema.
    fn initialize(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Enable WAL mode for better concurrent read performance
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS events (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                stream_id TEXT    NOT NULL,
                event_type TEXT   NOT NULL,
                payload   TEXT    NOT NULL,  -- JSON
                timestamp TEXT    NOT NULL,  -- ISO 8601
                sequence  INTEGER NOT NULL,
                UNIQUE(stream_id, sequence)
            );

            CREATE INDEX IF NOT EXISTS idx_events_stream
                ON events(stream_id, sequence);

            CREATE INDEX IF NOT EXISTS idx_events_type
                ON events(event_type);

            CREATE TABLE IF NOT EXISTS checkpoints (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                turn_id         TEXT NOT NULL UNIQUE,
                diff_data       TEXT NOT NULL,  -- JSON
                pre_commit_sha  TEXT,
                post_commit_sha TEXT,
                timestamp       TEXT NOT NULL
            );
            ",
        )?;

        // Set initial schema version if not exists
        let version: Option<i64> = conn
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
                row.get(0)
            })
            .ok();

        if version.is_none() {
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                params![1],
            )?;
        }

        Ok(())
    }

    /// Append one or more events to a stream.
    pub fn append_events(&self, stream_id: &str, events: &[(String, Value)]) -> Result<Vec<i64>> {
        let conn = self.conn.lock().unwrap();

        // Get the next sequence number for this stream
        let max_seq: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) FROM events WHERE stream_id = ?1",
                params![stream_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let mut ids = Vec::with_capacity(events.len());
        let mut seq = max_seq;

        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO events (stream_id, event_type, payload, timestamp, sequence)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;

            for (event_type, payload) in events {
                seq += 1;
                let now = chrono::Utc::now().to_rfc3339();
                let payload_str = serde_json::to_string(payload)?;

                stmt.execute(params![stream_id, event_type, payload_str, now, seq])?;
                ids.push(tx.last_insert_rowid());
            }
        }
        tx.commit()?;

        Ok(ids)
    }

    /// Read all events from a stream, optionally starting from a sequence number.
    pub fn read_stream(&self, stream_id: &str, from_sequence: i64) -> Result<Vec<StoredEvent>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT id, stream_id, event_type, payload, timestamp, sequence
             FROM events
             WHERE stream_id = ?1 AND sequence > ?2
             ORDER BY sequence ASC",
        )?;

        let events = stmt
            .query_map(params![stream_id, from_sequence], |row| {
                let payload_str: String = row.get(3)?;
                Ok(StoredEvent {
                    id: row.get(0)?,
                    stream_id: row.get(1)?,
                    event_type: row.get(2)?,
                    payload: serde_json::from_str(&payload_str).unwrap_or(Value::Null),
                    timestamp: row
                        .get::<_, String>(4)?
                        .parse()
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    sequence: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Read ALL events across all streams from a global sequence.
    pub fn read_all(&self, from_global_id: i64) -> Result<Vec<StoredEvent>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT id, stream_id, event_type, payload, timestamp, sequence
             FROM events
             WHERE id > ?1
             ORDER BY id ASC",
        )?;

        let events = stmt
            .query_map(params![from_global_id], |row| {
                let payload_str: String = row.get(3)?;
                Ok(StoredEvent {
                    id: row.get(0)?,
                    stream_id: row.get(1)?,
                    event_type: row.get(2)?,
                    payload: serde_json::from_str(&payload_str).unwrap_or(Value::Null),
                    timestamp: row
                        .get::<_, String>(4)?
                        .parse()
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    sequence: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Get the total event count.
    pub fn event_count(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Get all distinct stream IDs.
    pub fn stream_ids(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare_cached("SELECT DISTINCT stream_id FROM events ORDER BY stream_id")?;
        let ids = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;
        Ok(ids)
    }

    /// Save a turn checkpoint.
    pub fn save_checkpoint(
        &self,
        turn_id: &str,
        diff_data: &Value,
        pre_sha: Option<&str>,
        post_sha: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let diff_str = serde_json::to_string(diff_data)?;

        conn.execute(
            "INSERT OR REPLACE INTO checkpoints (turn_id, diff_data, pre_commit_sha, post_commit_sha, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![turn_id, diff_str, pre_sha, post_sha, now],
        )?;
        Ok(())
    }

    /// Load a turn checkpoint.
    pub fn load_checkpoint(&self, turn_id: &str) -> Result<Option<Value>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT diff_data FROM checkpoints WHERE turn_id = ?1",
            params![turn_id],
            |row| {
                let data: String = row.get(0)?;
                Ok(data)
            },
        );

        match result {
            Ok(data) => Ok(Some(serde_json::from_str(&data)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_create_and_read_events() {
        let store = EventStore::in_memory().unwrap();

        let events = vec![
            (
                "ThreadCreated".to_string(),
                json!({"thread_id": "abc", "name": "test"}),
            ),
            ("TurnStarted".to_string(), json!({"turn_id": "def"})),
        ];

        let ids = store.append_events("thread:abc", &events).unwrap();
        assert_eq!(ids.len(), 2);

        let read = store.read_stream("thread:abc", 0).unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].event_type, "ThreadCreated");
        assert_eq!(read[0].sequence, 1);
        assert_eq!(read[1].event_type, "TurnStarted");
        assert_eq!(read[1].sequence, 2);
    }

    #[test]
    fn test_read_from_sequence() {
        let store = EventStore::in_memory().unwrap();

        let events = vec![
            ("E1".to_string(), json!({})),
            ("E2".to_string(), json!({})),
            ("E3".to_string(), json!({})),
        ];

        store.append_events("stream:1", &events).unwrap();

        let read = store.read_stream("stream:1", 2).unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].event_type, "E3");
    }

    #[test]
    fn test_read_all_across_streams() {
        let store = EventStore::in_memory().unwrap();

        store
            .append_events("stream:a", &[("EA".to_string(), json!({}))])
            .unwrap();
        store
            .append_events("stream:b", &[("EB".to_string(), json!({}))])
            .unwrap();

        let all = store.read_all(0).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_checkpoint_roundtrip() {
        let store = EventStore::in_memory().unwrap();
        let diff = json!({"files": [{"path": "foo.rs", "status": "modified"}]});

        store
            .save_checkpoint("turn-1", &diff, Some("abc123"), Some("def456"))
            .unwrap();

        let loaded = store.load_checkpoint("turn-1").unwrap().unwrap();
        assert_eq!(loaded["files"][0]["path"], "foo.rs");
    }

    #[test]
    fn test_event_count() {
        let store = EventStore::in_memory().unwrap();
        assert_eq!(store.event_count().unwrap(), 0);

        store
            .append_events("s:1", &[("E1".to_string(), json!({}))])
            .unwrap();
        assert_eq!(store.event_count().unwrap(), 1);
    }
}
