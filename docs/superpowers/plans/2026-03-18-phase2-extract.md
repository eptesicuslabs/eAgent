# Phase 2: Extract — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate existing ecode-core business logic into eagent-* crates behind trait interfaces, creating working Tool and Provider implementations.

**Architecture:** Each migration wraps existing code behind the new traits. Old crates are preserved during migration — ecode-desktop-app continues to work against ecode-core. New eagent-* crates provide parallel, trait-based access to the same capabilities.

**Tech Stack:** Rust, serde, rusqlite, git2, portable-pty, reqwest, tokio

**Build env:** `export PATH="/c/Users/deyan/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"`

**Spec:** `docs/superpowers/specs/2026-03-18-eagent-platform-design.md`

**Source code (read these for full implementations):**
- `crates/ecode-core/src/persistence/mod.rs` — EventStore (337 lines)
- `crates/ecode-core/src/config/mod.rs` — ConfigManager (205 lines)
- `crates/ecode-core/src/local_agent/mod.rs` — LocalAgentExecutor (607 lines: filesystem, web, patch tools)
- `crates/ecode-core/src/git/mod.rs` — GitManager (306 lines)
- `crates/ecode-core/src/terminal/mod.rs` — TerminalManager (202 lines)
- `crates/ecode-core/src/providers/llama_cpp.rs` — LlamaCppManager (212 lines)
- `crates/ecode-core/src/codex/mod.rs` — CodexManager (710 lines)
- `crates/ecode-core/src/codex/version.rs` — version checking (111 lines)
- `crates/ecode-core/src/platform/mod.rs` — process tree kill, default shell (137 lines)
- `crates/ecode-contracts/src/codex.rs` — Codex JSON-RPC protocol types (456 lines)
- `crates/ecode-contracts/src/git.rs` — BranchInfo, FileStatus, FileDiff, DiffHunk, DiffLine types
- `crates/ecode-contracts/src/terminal.rs` — TerminalEvent, TerminalConfig
- `crates/ecode-contracts/src/persistence.rs` — StoredEvent

---

### Task 1: Extend ToolContext with ToolServices

Add a `ToolServices` struct to eagent-tools so that stateful tools (terminal, future tools) can access shared infrastructure. This is a small but foundational change that must land before other tools.

**Files:**
- Modify: `crates/eagent-tools/src/lib.rs`

**Steps:**

- [ ] **Step 1: Add ToolServices struct and update ToolContext**

Add the following to `crates/eagent-tools/src/lib.rs`, after the existing imports. Add `use std::sync::Arc;` and `use tokio::sync::mpsc;` to the import block.

```rust
/// Shared services available to tools that need stateful access.
///
/// Not all tools need services — filesystem and git tools operate statelessly.
/// Terminal tools use `terminal_manager` and `event_sender`.
pub struct ToolServices {
    /// Terminal manager for creating/writing to PTY sessions.
    /// Set to None if terminal support is not available.
    /// Downcast to `Arc<crate::terminal::TerminalManager>` in terminal tools.
    pub terminal_manager: Option<Arc<dyn std::any::Any + Send + Sync>>,
    /// Sender for terminal events (output, exit, resize).
    /// Tools that create terminals send events through this channel.
    pub event_sender: Option<mpsc::UnboundedSender<serde_json::Value>>,
}

impl std::fmt::Debug for ToolServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolServices")
            .field("terminal_manager", &self.terminal_manager.is_some())
            .field("event_sender", &self.event_sender.is_some())
            .finish()
    }
}
```

Update the existing `ToolContext` struct — add a `services` field after `task_id`:

```rust
/// Context provided to a tool during execution.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Workspace root directory (tools must not escape this).
    pub workspace_root: String,
    /// The agent ID that requested this tool call.
    pub agent_id: eagent_protocol::ids::AgentId,
    /// The task ID this tool call belongs to.
    pub task_id: eagent_protocol::ids::TaskId,
    /// Optional shared services for stateful tools (terminal, etc.).
    pub services: Option<Arc<ToolServices>>,
}
```

- [ ] **Step 2: Update existing MockTool tests to pass `services: None`**

In the `mock_tool_executes` test, update the `ToolContext` construction to include the new field:

```rust
let ctx = ToolContext {
    workspace_root: "/tmp".into(),
    agent_id: eagent_protocol::ids::AgentId::new(),
    task_id: eagent_protocol::ids::TaskId::new(),
    services: None,
};
```

- [ ] **Step 3: Build and test**

```bash
export PATH="/c/Users/deyan/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"
cargo check -p eagent-tools && cargo test -p eagent-tools
```

- [ ] **Step 4: Commit**

```bash
git add crates/eagent-tools/src/lib.rs
git commit -m "feat(eagent-tools): add ToolServices to ToolContext for stateful tool access"
```

---

### Task 2: Migrate EventStore to eagent-persistence

The EventStore is a generic SQLite-backed event log. It stores any JSON events keyed by stream ID with per-stream sequencing. This is nearly a direct copy from `ecode-core/src/persistence/mod.rs` — the only change is defining a local `StoredEvent` struct instead of importing from `ecode-contracts`.

**Files:**
- Modify: `crates/eagent-persistence/Cargo.toml`
- Create: `crates/eagent-persistence/src/event_store.rs`
- Modify: `crates/eagent-persistence/src/lib.rs`

**Steps:**

- [ ] **Step 1: Add dependencies to Cargo.toml**

Add these to `crates/eagent-persistence/Cargo.toml` under `[dependencies]`:

```toml
rusqlite = { workspace = true }
chrono = { workspace = true }
anyhow = { workspace = true }
```

- [ ] **Step 2: Create `crates/eagent-persistence/src/event_store.rs`**

Define the local `StoredEvent` struct and copy the `EventStore` implementation from `crates/ecode-core/src/persistence/mod.rs`. The only change vs. the source is using a locally-defined `StoredEvent` instead of `ecode_contracts::persistence::StoredEvent`.

```rust
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::sync::Mutex;

/// A stored event in the event log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    /// Auto-incrementing global sequence number.
    pub id: i64,
    /// Stream ID (e.g., "graph:{uuid}").
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

/// SQLite-backed event store for event sourcing.
pub struct EventStore {
    conn: Mutex<Connection>,
}

impl EventStore {
    /// Open or create an event store at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        // Copy body from ecode-core/src/persistence/mod.rs lines 17-25
    }

    /// Open an in-memory event store (useful for testing).
    pub fn in_memory() -> Result<Self> {
        // Copy body from ecode-core/src/persistence/mod.rs lines 29-36
    }

    /// Initialize the database schema.
    fn initialize(&self) -> Result<()> {
        // Copy body from ecode-core/src/persistence/mod.rs lines 39-96
        // Schema: events table (id, stream_id, event_type, payload, timestamp, sequence)
        //         checkpoints table (id, turn_id, diff_data, pre_commit_sha, post_commit_sha, timestamp)
        //         schema_version table
        // WAL mode, NORMAL synchronous, foreign keys ON
    }

    /// Append one or more events to a stream.
    pub fn append_events(&self, stream_id: &str, events: &[(String, Value)]) -> Result<Vec<i64>> {
        // Copy body from ecode-core lines 99-133
        // Gets max sequence for stream, increments, inserts in transaction
    }

    /// Read all events from a stream, optionally starting from a sequence number.
    pub fn read_stream(&self, stream_id: &str, from_sequence: i64) -> Result<Vec<StoredEvent>> {
        // Copy body from ecode-core lines 136-163
    }

    /// Read ALL events across all streams from a global sequence.
    pub fn read_all(&self, from_global_id: i64) -> Result<Vec<StoredEvent>> {
        // Copy body from ecode-core lines 166-193
    }

    /// Get the total event count.
    pub fn event_count(&self) -> Result<i64> {
        // Copy body from ecode-core lines 196-199
    }

    /// Get all distinct stream IDs.
    pub fn stream_ids(&self) -> Result<Vec<String>> {
        // Copy body from ecode-core lines 203-211
    }

    /// Save a turn checkpoint.
    pub fn save_checkpoint(
        &self,
        turn_id: &str,
        diff_data: &Value,
        pre_sha: Option<&str>,
        post_sha: Option<&str>,
    ) -> Result<()> {
        // Copy body from ecode-core lines 214-231
    }

    /// Load a turn checkpoint.
    pub fn load_checkpoint(&self, turn_id: &str) -> Result<Option<Value>> {
        // Copy body from ecode-core lines 234-250
    }
}
```

- [ ] **Step 3: Update `crates/eagent-persistence/src/lib.rs`**

Replace the current contents with:

```rust
//! eAgent Persistence — event store, config loading, and session state management.

pub mod event_store;

pub use event_store::{EventStore, StoredEvent};
```

- [ ] **Step 4: Add tests**

Add a `#[cfg(test)] mod tests { ... }` block at the bottom of `event_store.rs`. Copy all 5 tests from `crates/ecode-core/src/persistence/mod.rs`:
- `test_create_and_read_events` — appends 2 events, reads back, checks types/sequences
- `test_read_from_sequence` — appends 3, reads from seq 2, expects only E3
- `test_read_all_across_streams` — appends to 2 streams, reads all globally
- `test_checkpoint_roundtrip` — saves checkpoint with diff JSON, loads back
- `test_event_count` — checks count is 0, appends 1, checks count is 1

Update imports to use local types instead of `ecode_contracts`.

- [ ] **Step 5: Build and test**

```bash
export PATH="/c/Users/deyan/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"
cargo check -p eagent-persistence && cargo test -p eagent-persistence
```

- [ ] **Step 6: Commit**

```bash
git add crates/eagent-persistence/
git commit -m "feat(eagent-persistence): migrate EventStore from ecode-core with local StoredEvent type"
```

---

### Task 3: Migrate ConfigManager to eagent-persistence

Adapt the ConfigManager from `ecode-core/src/config/mod.rs` to load/save `AgentConfig` (from `eagent-contracts::config`) instead of the old `AppConfig`. The data directory paths use "eAgent" instead of "eCode".

**Files:**
- Modify: `crates/eagent-persistence/Cargo.toml`
- Create: `crates/eagent-persistence/src/config.rs`
- Modify: `crates/eagent-persistence/src/lib.rs`

**Steps:**

- [ ] **Step 1: Add dependencies to Cargo.toml**

Add to `crates/eagent-persistence/Cargo.toml` under `[dependencies]`:

```toml
dirs = { workspace = true }
toml = { workspace = true }
```

Add a `[dev-dependencies]` section:

```toml
[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 2: Create `crates/eagent-persistence/src/config.rs`**

Adapt from `crates/ecode-core/src/config/mod.rs`. Differences from the source:
- Uses `eagent_contracts::config::AgentConfig` instead of `ecode_contracts::config::AppConfig`
- Directory names: `"eAgent"` instead of `"eCode"`
- Portable root dir name: `"eAgent-data"` instead of `"eCode-data"`
- Portable root env var: `"EAGENT_PORTABLE_ROOT"` instead of `"ECODE_PORTABLE_ROOT"`
- Event store file: `"eagent-events.db"` instead of `"events.db"`
- Config file name: `"config.toml"` (same)

```rust
use anyhow::{Context, Result};
use eagent_contracts::config::AgentConfig;
use std::path::{Path, PathBuf};
use tracing::info;

/// Configuration manager for eAgent.
pub struct ConfigManager {
    config_path: PathBuf,
    config: AgentConfig,
}

impl ConfigManager {
    /// Load configuration from the default location.
    pub fn load() -> Result<Self> {
        let config_dir = Self::config_dir()?;
        std::fs::create_dir_all(&config_dir)?;
        let config_path = config_dir.join("config.toml");
        // If exists: read + toml::from_str
        // If not: create default, serialize, write
        // Copy logic from ecode-core/src/config/mod.rs lines 16-37
    }

    /// Load configuration from a specific path.
    pub fn load_from(path: PathBuf) -> Result<Self> {
        // Copy from ecode-core lines 41-53, replace AppConfig with AgentConfig
    }

    /// Save the current configuration to disk.
    pub fn save(&self) -> Result<()> {
        // Copy from ecode-core lines 56-63
    }

    /// Get a reference to the current config.
    pub fn config(&self) -> &AgentConfig { &self.config }

    /// Get a mutable reference to the current config.
    pub fn config_mut(&mut self) -> &mut AgentConfig { &mut self.config }

    /// Replace the entire configuration and save to disk.
    pub fn update(&mut self, config: AgentConfig) -> Result<()> {
        self.config = config;
        self.save()
    }

    /// Get the path to the config file.
    pub fn config_path(&self) -> &PathBuf { &self.config_path }

    /// Get the config directory.
    pub fn config_dir() -> Result<PathBuf> {
        // Check EAGENT_PORTABLE_ROOT env var first
        // Then check portable_root()
        // Fallback: dirs::config_dir().join("eAgent")
        // Copy logic from ecode-core lines 88-98, replace "eCode" with "eAgent"
    }

    /// Get the data directory for eAgent.
    pub fn data_dir() -> Result<PathBuf> {
        // portable_root().join("data") or dirs::data_local_dir().join("eAgent")
        // Copy from ecode-core lines 101-112, replace "eCode" with "eAgent"
    }

    /// Get the path to the event store database.
    pub fn event_store_path() -> Result<PathBuf> {
        Ok(Self::data_dir()?.join("eagent-events.db"))
    }

    /// Get the log directory.
    pub fn log_dir() -> Result<PathBuf> {
        // Copy from ecode-core lines 131-138
    }

    /// Resolve the portable application root.
    pub fn portable_root() -> Option<PathBuf> {
        // Check EAGENT_PORTABLE_ROOT env var
        // Then try exe_dir/eAgent-data (with write test)
        // Copy from ecode-core lines 142-152, replace env var and dir names
    }

    fn portable_root_from_exe_dir(exe_dir: &Path) -> Option<PathBuf> {
        // Copy from ecode-core lines 154-168, replace "eCode-data" with "eAgent-data"
    }
}
```

- [ ] **Step 3: Update `crates/eagent-persistence/src/lib.rs`**

```rust
//! eAgent Persistence — event store, config loading, and session state management.

pub mod config;
pub mod event_store;

pub use config::ConfigManager;
pub use event_store::{EventStore, StoredEvent};
```

- [ ] **Step 4: Add tests**

Add `#[cfg(test)] mod tests` at the bottom of `config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_default_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mgr = ConfigManager::load_from(path).unwrap();
        assert_eq!(mgr.config().general.theme, "dark");
        assert_eq!(mgr.config().general.font_size, 14.0);
    }

    #[test]
    fn test_save_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut mgr = ConfigManager::load_from(path.clone()).unwrap();
        mgr.config_mut().general.theme = "light".to_string();
        mgr.save().unwrap();
        let mgr2 = ConfigManager::load_from(path).unwrap();
        assert_eq!(mgr2.config().general.theme, "light");
    }

    #[test]
    fn test_portable_root_from_exe_dir() {
        let dir = tempfile::tempdir().unwrap();
        let root = ConfigManager::portable_root_from_exe_dir(dir.path()).unwrap();
        assert_eq!(root, dir.path().join("eAgent-data"));
        assert!(root.exists());
    }
}
```

- [ ] **Step 5: Build and test**

```bash
export PATH="/c/Users/deyan/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"
cargo check -p eagent-persistence && cargo test -p eagent-persistence
```

- [ ] **Step 6: Commit**

```bash
git add crates/eagent-persistence/
git commit -m "feat(eagent-persistence): migrate ConfigManager for AgentConfig with eAgent directory layout"
```

---

### Task 4: Create filesystem tools

Extract the file/directory operations from `LocalAgentExecutor` in `crates/ecode-core/src/local_agent/mod.rs` into individual Tool trait implementations. Each tool is a zero-sized struct implementing `eagent_tools::Tool`.

**Files:**
- Modify: `crates/eagent-tools/Cargo.toml`
- Create: `crates/eagent-tools/src/filesystem.rs`
- Modify: `crates/eagent-tools/src/lib.rs`

**Steps:**

- [ ] **Step 1: Add dependencies to Cargo.toml**

Add to `crates/eagent-tools/Cargo.toml` under `[dependencies]`:

```toml
anyhow = { workspace = true }
```

Add a `[dev-dependencies]` section:

```toml
[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 2: Create `crates/eagent-tools/src/filesystem.rs` with helpers and 7 tools**

The file structure is:

```rust
use crate::{Tool, ToolContext, ToolError, ToolResult};
use anyhow::{Context, Result, anyhow, bail};
use eagent_protocol::messages::RiskLevel;
use serde_json::{Value, json};
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

// ── Constants ──

const MAX_READ_FILE_BYTES: u64 = 1_024 * 1_024;       // 1 MB
const MAX_SEARCH_FILE_BYTES: u64 = 512 * 1_024;        // 512 KB
const MAX_SEARCH_RESULTS: usize = 200;
const MAX_OUTPUT_CHARS: usize = 16_000;

const SKIPPED_SEARCH_DIRS: &[&str] = &[
    ".git", ".hg", ".svn", ".venv", "venv",
    "node_modules", "target", "dist", "build",
];

// ── Helper functions ──
// (copy from crates/ecode-core/src/local_agent/mod.rs)

/// Resolve a path relative to workspace root, preventing directory traversal.
/// Copy logic from LocalAgentExecutor::resolve_path (lines 375-401).
fn resolve_path(workspace_root: &str, path: &str) -> std::result::Result<PathBuf, ToolError> {
    // Join workspace_root + path, canonicalize, verify starts_with workspace_root
    // Map errors to ToolError::ExecutionFailed
}

/// Truncate output string to max_len characters.
/// Copy from local_agent/mod.rs line 498-503.
fn limit_output(mut value: String, max_len: usize) -> String {
    if value.len() > max_len {
        value.truncate(max_len);
        value.push_str("\n...<truncated>");
    }
    value
}

/// Check if a directory entry should be skipped during search.
/// Copy from local_agent/mod.rs lines 453-457.
fn should_skip_search_entry(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| SKIPPED_SEARCH_DIRS.contains(&name))
}

/// Helper to extract a required string param from JSON.
fn required_str<'a>(value: &'a Value, key: &str) -> std::result::Result<&'a str, ToolError> {
    value.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidParams(format!("missing string argument `{}`", key)))
}

// ── Tool implementations ──

/// List files and directories in a workspace path.
pub struct ListDirectoryTool;

impl Tool for ListDirectoryTool {
    fn name(&self) -> &str { "list_directory" }
    fn description(&self) -> &str { "List files and directories in a workspace path" }
    fn risk_level(&self) -> RiskLevel { RiskLevel::Low }
    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative path within the workspace to list. Defaults to \".\" (workspace root)."
                }
            }
        })
    }
    fn execute(&self, params: Value, ctx: &ToolContext)
        -> Pin<Box<dyn Future<Output = std::result::Result<ToolResult, ToolError>> + Send + '_>>
    {
        Box::pin(async move {
            let path = params.get("path").and_then(Value::as_str).unwrap_or(".");
            let target = resolve_path(&ctx.workspace_root, path)?;
            // Copy logic from LocalAgentExecutor::list_directory (lines 131-146)
            // fs::read_dir, format "[DIR] name" / "[FILE] name", sort, join("\n")
        })
    }
}

/// Read a single file from the workspace.
pub struct ReadFileTool;
// name: "read_file", risk: Low
// params: { "path": string (required) }
// execute: resolve_path, check metadata().len() <= MAX_READ_FILE_BYTES,
//          fs::read_to_string, limit_output(MAX_OUTPUT_CHARS)
// Copy logic from LocalAgentExecutor::read_text_file (lines 148-156)

/// Read multiple files from the workspace in a single call.
pub struct ReadMultipleFilesTool;
// name: "read_multiple_files", risk: Low
// params: { "paths": string[] (required) }
// execute: iterate paths, read each like ReadFileTool, join with "FILE: {path}\n{content}\n\n"
// Copy logic from LocalAgentExecutor::read_multiple_files (lines 158-165)

/// Search for a text pattern across files in the workspace.
pub struct SearchFilesTool;
// name: "search_files", risk: Low
// params: { "pattern": string (required), "path": string (optional, default ".") }
// execute: resolve_path, recursive search skipping SKIPPED_SEARCH_DIRS,
//          skip files > MAX_SEARCH_FILE_BYTES, string .contains() matching,
//          format "rel_path:line_num: line_text", max MAX_SEARCH_RESULTS
// Copy logic from LocalAgentExecutor::search_files + search_path_recursive (lines 167-220)

/// Create or overwrite a file in the workspace.
pub struct WriteFileTool;
// name: "write_file", risk: Medium
// params: { "path": string (required), "content": string (required) }
// execute: resolve_path, create parent dirs, fs::write

/// Apply a targeted edit to a file (old_string -> new_string replacement).
pub struct EditFileTool;
// name: "edit_file", risk: Medium
// params: { "path": string (required), "old_string": string (required), "new_string": string (required) }
// execute: resolve_path, read file, verify old_string exists,
//          content.replacen(old_string, new_string, 1), atomic write (temp file + rename)

/// Apply a multi-edit patch to a file (multiple old_text -> new_text replacements).
pub struct ApplyPatchTool;
// name: "apply_patch", risk: Medium
// params: { "path": string (required), "edits": [{"old_text": string, "new_text": string}] (required) }
// execute: copy logic from LocalAgentExecutor::apply_patch (lines 288-311)
//          resolve_path, iterate edits, replacen each, atomic write
```

**Important:** For each tool struct above, implement all 5 `Tool` trait methods (`name`, `description`, `risk_level`, `parameter_schema`, `execute`). The `execute` method returns `Pin<Box<dyn Future<...> + Send + '_>>` via `Box::pin(async move { ... })`. Map `anyhow::Error` to `ToolError::ExecutionFailed(e.to_string())`.

- [ ] **Step 3: Add module declaration to `crates/eagent-tools/src/lib.rs`**

Add `pub mod filesystem;` after the existing `pub mod registry;` line.

- [ ] **Step 4: Write tests**

At the bottom of `filesystem.rs`, add `#[cfg(test)] mod tests { ... }` with these tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_ctx(workspace_root: &str) -> ToolContext {
        ToolContext {
            workspace_root: workspace_root.to_string(),
            agent_id: eagent_protocol::ids::AgentId::new(),
            task_id: eagent_protocol::ids::TaskId::new(),
            services: None,
        }
    }

    #[tokio::test]
    async fn test_list_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("hello.txt"), "world").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        let ctx = make_ctx(dir.path().to_str().unwrap());
        let result = ListDirectoryTool.execute(json!({}), &ctx).await.unwrap();
        let output = result.output.as_str().unwrap();
        assert!(output.contains("[FILE] hello.txt"));
        assert!(output.contains("[DIR] subdir"));
    }

    #[tokio::test]
    async fn test_read_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();
        let ctx = make_ctx(dir.path().to_str().unwrap());
        let result = ReadFileTool.execute(json!({"path": "test.rs"}), &ctx).await.unwrap();
        assert!(result.output.as_str().unwrap().contains("fn main()"));
    }

    #[tokio::test]
    async fn test_read_file_escapes_root() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = make_ctx(dir.path().to_str().unwrap());
        let result = ReadFileTool.execute(json!({"path": "../../etc/passwd"}), &ctx).await;
        assert!(result.is_err()); // Should fail with path escape error
    }

    #[tokio::test]
    async fn test_search_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello world").unwrap();
        fs::write(dir.path().join("b.txt"), "goodbye world").unwrap();
        let ctx = make_ctx(dir.path().to_str().unwrap());
        let result = SearchFilesTool.execute(json!({"pattern": "hello"}), &ctx).await.unwrap();
        let output = result.output.as_str().unwrap();
        assert!(output.contains("a.txt"));
        assert!(!output.contains("b.txt"));
    }

    #[tokio::test]
    async fn test_write_file() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = make_ctx(dir.path().to_str().unwrap());
        WriteFileTool.execute(json!({"path": "new.txt", "content": "hello"}), &ctx).await.unwrap();
        assert_eq!(fs::read_to_string(dir.path().join("new.txt")).unwrap(), "hello");
    }

    #[tokio::test]
    async fn test_edit_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("test.txt"), "hello world").unwrap();
        let ctx = make_ctx(dir.path().to_str().unwrap());
        EditFileTool.execute(json!({
            "path": "test.txt",
            "old_string": "world",
            "new_string": "rust"
        }), &ctx).await.unwrap();
        assert_eq!(fs::read_to_string(dir.path().join("test.txt")).unwrap(), "hello rust");
    }

    #[tokio::test]
    async fn test_apply_patch() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("test.txt"), "hello world").unwrap();
        let ctx = make_ctx(dir.path().to_str().unwrap());
        ApplyPatchTool.execute(json!({
            "path": "test.txt",
            "edits": [{"old_text": "world", "new_text": "rust"}]
        }), &ctx).await.unwrap();
        assert_eq!(fs::read_to_string(dir.path().join("test.txt")).unwrap(), "hello rust");
    }

    #[tokio::test]
    async fn test_read_multiple_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "aaa").unwrap();
        fs::write(dir.path().join("b.txt"), "bbb").unwrap();
        let ctx = make_ctx(dir.path().to_str().unwrap());
        let result = ReadMultipleFilesTool.execute(
            json!({"paths": ["a.txt", "b.txt"]}), &ctx
        ).await.unwrap();
        let output = result.output.as_str().unwrap();
        assert!(output.contains("FILE: a.txt"));
        assert!(output.contains("aaa"));
        assert!(output.contains("FILE: b.txt"));
        assert!(output.contains("bbb"));
    }
}
```

- [ ] **Step 5: Build and test**

```bash
export PATH="/c/Users/deyan/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"
cargo check -p eagent-tools && cargo test -p eagent-tools
```

- [ ] **Step 6: Commit**

```bash
git add crates/eagent-tools/
git commit -m "feat(eagent-tools): implement 7 filesystem tools (list, read, read_multiple, search, write, edit, apply_patch)"
```

---

### Task 5: Create web tools

Extract web search and URL fetching from `LocalAgentExecutor` into Tool trait implementations.

**Files:**
- Modify: `crates/eagent-tools/Cargo.toml`
- Create: `crates/eagent-tools/src/web.rs`
- Modify: `crates/eagent-tools/src/lib.rs`

**Steps:**

- [ ] **Step 1: Add dependencies to Cargo.toml**

Add to `crates/eagent-tools/Cargo.toml` under `[dependencies]`:

```toml
reqwest = { workspace = true }
```

- [ ] **Step 2: Create `crates/eagent-tools/src/web.rs`**

```rust
use crate::{Tool, ToolContext, ToolError, ToolResult};
use eagent_protocol::messages::RiskLevel;
use reqwest::Url;
use serde_json::{Value, json};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

const MAX_WEB_RESULTS: usize = 5;
const MAX_WEB_BODY_BYTES: usize = 256 * 1_024;

// ── Helper functions ──
// Copy these from crates/ecode-core/src/local_agent/mod.rs:

/// Build a shared reqwest client for web tools.
fn web_client() -> reqwest::Client {
    // Copy client builder from LocalAgentExecutor::new (lines 46-57)
    // Change user_agent to "eAgent/0.1"
}

/// Check if a URL is a public web URL (not localhost, not private IP).
/// Copy from local_agent/mod.rs lines 459-496.
pub fn is_public_web_url(url: &Url) -> bool { /* ... */ }

/// Strip HTML tags from a string.
/// Copy from local_agent/mod.rs lines 555-567.
pub fn strip_html_tags(input: &str) -> String { /* ... */ }

/// Decode common HTML entities.
/// Copy from local_agent/mod.rs lines 569-578.
pub fn html_entity_decode(input: &str) -> String { /* ... */ }

/// Extract a text snippet from HTML content.
/// Copy from local_agent/mod.rs lines 549-553.
pub fn extract_text_snippet(html: &str, max_len: usize) -> String { /* ... */ }

/// Fetch HTML from a URL with size and content-type checks.
/// Adapted from LocalAgentExecutor::fetch_html (lines 346-373).
async fn fetch_html(client: &reqwest::Client, url: Url) -> anyhow::Result<String> { /* ... */ }

#[derive(Debug, Clone)]
struct SearchResult {
    title: String,
    url: String,
}

/// Parse DuckDuckGo HTML results.
/// Copy from local_agent/mod.rs lines 512-547.
fn extract_duckduckgo_results(html: &str, max_results: usize) -> Vec<SearchResult> { /* ... */ }

/// Truncate output.
fn limit_output(mut value: String, max_len: usize) -> String {
    if value.len() > max_len { value.truncate(max_len); value.push_str("\n...<truncated>"); }
    value
}

// ── Tool implementations ──

/// Search the web using DuckDuckGo.
pub struct WebSearchTool;

impl Tool for WebSearchTool {
    fn name(&self) -> &str { "web_search" }
    fn description(&self) -> &str { "Search the web using DuckDuckGo and return result titles, URLs, and content snippets" }
    fn risk_level(&self) -> RiskLevel { RiskLevel::Low }
    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "The search query" },
                "max_results": { "type": "integer", "description": "Maximum number of results to return (1-5, default 5)" }
            },
            "required": ["query"]
        })
    }
    fn execute(&self, params: Value, _ctx: &ToolContext)
        -> Pin<Box<dyn Future<Output = std::result::Result<ToolResult, ToolError>> + Send + '_>>
    {
        Box::pin(async move {
            // Extract query, max_results params
            // Build DuckDuckGo URL, fetch HTML, extract results
            // For each result: fetch URL content, extract snippet
            // Format output as "TITLE: ...\nURL: ...\nSNIPPET: ...\n"
            // Copy logic from LocalAgentExecutor::web_search (lines 313-343)
        })
    }
}

/// Fetch and extract text content from a URL.
pub struct WebFetchTool;

impl Tool for WebFetchTool {
    fn name(&self) -> &str { "web_fetch" }
    fn description(&self) -> &str { "Fetch and extract text content from a URL" }
    fn risk_level(&self) -> RiskLevel { RiskLevel::Medium }
    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "The URL to fetch" },
                "max_length": { "type": "integer", "description": "Maximum text length to return (default 2000)" }
            },
            "required": ["url"]
        })
    }
    fn execute(&self, params: Value, _ctx: &ToolContext)
        -> Pin<Box<dyn Future<Output = std::result::Result<ToolResult, ToolError>> + Send + '_>>
    {
        Box::pin(async move {
            // Parse URL param, validate is_public_web_url
            // Fetch HTML, extract_text_snippet with max_length
            // Return as ToolResult
        })
    }
}
```

- [ ] **Step 3: Add module declaration to `crates/eagent-tools/src/lib.rs`**

Add `pub mod web;` after `pub mod filesystem;`.

- [ ] **Step 4: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(strip_html_tags("<p>hello</p>"), "hello");
        assert_eq!(strip_html_tags("<b>bold</b> text"), "bold text");
        assert_eq!(strip_html_tags("no tags"), "no tags");
    }

    #[test]
    fn test_html_entity_decode() {
        assert_eq!(html_entity_decode("&amp;"), "&");
        assert_eq!(html_entity_decode("&lt;b&gt;"), "<b>");
        assert_eq!(html_entity_decode("it&#x27;s"), "it's");
    }

    #[test]
    fn test_is_public_web_url_public() {
        let url = Url::parse("https://example.com").unwrap();
        assert!(is_public_web_url(&url));
    }

    #[test]
    fn test_is_public_web_url_localhost() {
        let url = Url::parse("http://localhost:8080").unwrap();
        assert!(!is_public_web_url(&url));
    }

    #[test]
    fn test_is_public_web_url_private_ip() {
        let url = Url::parse("http://192.168.1.1").unwrap();
        assert!(!is_public_web_url(&url));
        let url2 = Url::parse("http://10.0.0.1").unwrap();
        assert!(!is_public_web_url(&url2));
    }

    #[test]
    fn test_extract_text_snippet() {
        let html = "<html><body><p>Hello world</p></body></html>";
        let snippet = extract_text_snippet(html, 100);
        assert!(snippet.contains("Hello world"));
    }

    #[test]
    fn test_web_search_tool_def() {
        let tool = WebSearchTool;
        assert_eq!(tool.name(), "web_search");
        assert_eq!(tool.risk_level(), RiskLevel::Low);
        let schema = tool.parameter_schema();
        assert!(schema["properties"]["query"].is_object());
    }

    #[test]
    fn test_web_fetch_tool_def() {
        let tool = WebFetchTool;
        assert_eq!(tool.name(), "web_fetch");
        assert_eq!(tool.risk_level(), RiskLevel::Medium);
    }
}
```

- [ ] **Step 5: Build and test**

```bash
export PATH="/c/Users/deyan/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"
cargo check -p eagent-tools && cargo test -p eagent-tools
```

- [ ] **Step 6: Commit**

```bash
git add crates/eagent-tools/
git commit -m "feat(eagent-tools): implement web_search and web_fetch tools with DuckDuckGo backend"
```

---

### Task 6: Create git tools

Extract git operations from `crates/ecode-core/src/git/mod.rs` into Tool trait implementations. The domain types (`BranchInfo`, `FileStatus`, `FileDiff`, etc.) are defined locally since they are tool-internal and get serialized to JSON in ToolResult output.

**Files:**
- Modify: `crates/eagent-tools/Cargo.toml`
- Create: `crates/eagent-tools/src/git.rs`
- Modify: `crates/eagent-tools/src/lib.rs`

**Steps:**

- [ ] **Step 1: Add dependencies to Cargo.toml**

Add to `crates/eagent-tools/Cargo.toml` under `[dependencies]`:

```toml
git2 = { workspace = true }
```

- [ ] **Step 2: Create `crates/eagent-tools/src/git.rs`**

The file has three sections: (1) local domain types, (2) internal GitManager, (3) tool structs.

**Section 1: Local domain types** (copy from `crates/ecode-contracts/src/git.rs`):

```rust
use crate::{Tool, ToolContext, ToolError, ToolResult};
use anyhow::{Context, Result};
use eagent_protocol::messages::RiskLevel;
use git2::{DiffOptions, Repository, StatusOptions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::future::Future;
use std::pin::Pin;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_head: bool,
    pub is_remote: bool,
    pub upstream: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatus {
    pub path: String,
    pub status: FileStatusKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatusKind {
    New, Modified, Deleted, Renamed, TypeChange, Conflicted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub hunks: Vec<DiffHunk>,
    pub is_binary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffLineKind {
    Context, Addition, Deletion,
}
```

**Section 2: Internal GitManager** (adapted from `crates/ecode-core/src/git/mod.rs`):

```rust
/// Internal git operations wrapper.
struct GitManager {
    repo_path: String,
}

impl GitManager {
    /// Open a git repository at the given path.
    fn open(path: &str) -> Result<Self> {
        // Copy from ecode-core/src/git/mod.rs lines 16-20
    }

    fn repo(&self) -> Result<Repository> {
        // Copy from ecode-core line 38-40
    }

    /// Get file status.
    fn status(&self) -> Result<Vec<FileStatus>> {
        // Copy from ecode-core lines 105-149
    }

    /// Get diff of working directory against HEAD.
    fn diff_workdir(&self) -> Result<Vec<FileDiff>> {
        // Copy from ecode-core lines 152-160
    }

    /// Get current branch name.
    fn current_branch(&self) -> Result<Option<String>> {
        // Copy from ecode-core lines 70-82
    }

    /// List all branches.
    fn list_branches(&self) -> Result<Vec<BranchInfo>> {
        // Copy from ecode-core lines 43-67
    }

    /// Create a new branch at HEAD.
    fn create_branch(&self, name: &str) -> Result<()> {
        // Copy from ecode-core lines 85-91
    }

    /// Checkout a branch.
    fn checkout(&self, branch_name: &str) -> Result<()> {
        // Copy from ecode-core lines 94-102
    }

    /// Get HEAD SHA.
    fn head_sha(&self) -> Result<Option<String>> {
        // Copy from ecode-core lines 174-181
    }
}

/// Parse a git2::Diff into domain FileDiff types.
fn parse_diff(diff: &git2::Diff<'_>) -> Result<Vec<FileDiff>> {
    // Copy from ecode-core lines 250-305
}
```

**Section 3: Tool structs:**

```rust
/// Show working tree status (modified, new, deleted files).
pub struct GitStatusTool;

impl Tool for GitStatusTool {
    fn name(&self) -> &str { "git_status" }
    fn description(&self) -> &str { "Show working tree file status (new, modified, deleted files)" }
    fn risk_level(&self) -> RiskLevel { RiskLevel::Low }
    fn parameter_schema(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }
    fn execute(&self, _params: Value, ctx: &ToolContext)
        -> Pin<Box<dyn Future<Output = std::result::Result<ToolResult, ToolError>> + Send + '_>>
    {
        Box::pin(async move {
            let mgr = GitManager::open(&ctx.workspace_root)
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            let statuses = mgr.status()
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            let branch = mgr.current_branch()
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            let output = json!({
                "branch": branch,
                "files": statuses,
            });
            Ok(ToolResult { output, is_error: false })
        })
    }
}

/// Show diffs of the working directory against HEAD.
pub struct GitDiffTool;
// name: "git_diff", risk: Low
// params: {} (no params)
// execute: GitManager::open, diff_workdir, serialize as JSON array of FileDiff

/// Create a git commit with staged changes.
pub struct GitCommitTool;
// name: "git_commit", risk: Medium
// params: { "message": string (required) }
// execute: use git2 to:
//   1. repo.index() -> add_all(["*"], ADD_DEFAULT, None) -> write()
//   2. index.write_tree()
//   3. repo.find_tree(tree_oid)
//   4. repo.signature() (or create default)
//   5. repo.head().peel_to_commit() for parent
//   6. repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])
//   Return: { "sha": commit_id, "message": message }

/// Create, checkout, or list git branches.
pub struct GitBranchTool;
// name: "git_branch", risk: Medium
// params: { "action": "create"|"checkout"|"list" (required), "name": string (required for create/checkout) }
// execute: dispatch to GitManager methods based on action
```

- [ ] **Step 3: Add module declaration to `crates/eagent-tools/src/lib.rs`**

Add `pub mod git;` after `pub mod web;`.

- [ ] **Step 4: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Create a tempdir with an initialized git repo and initial commit.
    fn setup_git_repo() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        let repo = git2::Repository::init(&path).unwrap();
        // Create initial commit so HEAD exists
        let sig = git2::Signature::now("Test", "test@test.com").unwrap();
        let mut index = repo.index().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[]).unwrap();
        (dir, path)
    }

    fn make_ctx(workspace_root: &str) -> ToolContext {
        ToolContext {
            workspace_root: workspace_root.to_string(),
            agent_id: eagent_protocol::ids::AgentId::new(),
            task_id: eagent_protocol::ids::TaskId::new(),
            services: None,
        }
    }

    #[tokio::test]
    async fn test_git_status_clean() {
        let (_dir, path) = setup_git_repo();
        let ctx = make_ctx(&path);
        let result = GitStatusTool.execute(json!({}), &ctx).await.unwrap();
        let files = result.output["files"].as_array().unwrap();
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn test_git_status_with_new_file() {
        let (dir, path) = setup_git_repo();
        std::fs::write(dir.path().join("new.txt"), "content").unwrap();
        let ctx = make_ctx(&path);
        let result = GitStatusTool.execute(json!({}), &ctx).await.unwrap();
        let files = result.output["files"].as_array().unwrap();
        assert!(!files.is_empty());
    }

    #[tokio::test]
    async fn test_git_branch_list() {
        let (_dir, path) = setup_git_repo();
        let ctx = make_ctx(&path);
        let result = GitBranchTool.execute(json!({"action": "list"}), &ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_git_branch_create_and_checkout() {
        let (_dir, path) = setup_git_repo();
        let ctx = make_ctx(&path);
        GitBranchTool.execute(json!({"action": "create", "name": "feature-x"}), &ctx).await.unwrap();
        GitBranchTool.execute(json!({"action": "checkout", "name": "feature-x"}), &ctx).await.unwrap();
        let status = GitStatusTool.execute(json!({}), &ctx).await.unwrap();
        assert_eq!(status.output["branch"].as_str().unwrap(), "feature-x");
    }
}
```

- [ ] **Step 5: Build and test**

```bash
export PATH="/c/Users/deyan/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"
cargo check -p eagent-tools && cargo test -p eagent-tools
```

- [ ] **Step 6: Commit**

```bash
git add crates/eagent-tools/
git commit -m "feat(eagent-tools): implement git tools (status, diff, commit, branch)"
```

---

### Task 7: Create terminal tools

Move the `TerminalManager` from `crates/ecode-core/src/terminal/mod.rs` into `eagent-tools` and wrap it with Tool trait implementations. Terminal tools access the manager via `ToolContext.services.terminal_manager` (downcasting from `Arc<dyn Any>` to `Arc<TerminalManager>`).

**Files:**
- Modify: `crates/eagent-tools/Cargo.toml`
- Create: `crates/eagent-tools/src/terminal.rs`
- Modify: `crates/eagent-tools/src/lib.rs`

**Steps:**

- [ ] **Step 1: Add dependencies to Cargo.toml**

Add to `crates/eagent-tools/Cargo.toml` under `[dependencies]`:

```toml
portable-pty = { workspace = true }
```

- [ ] **Step 2: Create `crates/eagent-tools/src/terminal.rs`**

This file has four sections: (1) local types, (2) default_shell helper, (3) TerminalManager, (4) tool structs.

**Section 1: Local types** (adapted from `crates/ecode-contracts/src/terminal.rs`):

```rust
use crate::{Tool, ToolContext, ToolError, ToolResult};
use anyhow::{Context, Result};
use eagent_protocol::ids::TerminalId;
use eagent_protocol::messages::RiskLevel;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::future::Future;
use std::io::{Read, Write};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Terminal event emitted by the terminal manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminalEvent {
    Output { terminal_id: TerminalId, data: String },
    Exited { terminal_id: TerminalId, exit_code: Option<u32> },
    Resized { terminal_id: TerminalId, cols: u16, rows: u16 },
}

/// Configuration for spawning a terminal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    pub cwd: String,
    pub shell: Option<String>,
    pub cols: u16,
    pub rows: u16,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self { cwd: String::new(), shell: None, cols: 120, rows: 30 }
    }
}
```

**Section 2: Platform helper:**

```rust
/// Get the default shell for the current platform.
fn default_shell() -> String {
    #[cfg(windows)]
    { "powershell.exe".to_string() }
    #[cfg(not(windows))]
    { std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()) }
}
```

**Section 3: TerminalManager** (copy from `crates/ecode-core/src/terminal/mod.rs`, replacing `ecode_contracts` types with local types):

```rust
/// A running terminal session.
struct TerminalSession {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    alive: Arc<std::sync::atomic::AtomicBool>,
}

/// Manager for PTY terminal sessions.
pub struct TerminalManager {
    sessions: Arc<Mutex<HashMap<TerminalId, TerminalSession>>>,
    event_tx: mpsc::UnboundedSender<TerminalEvent>,
}

impl TerminalManager {
    /// Create a new terminal manager. Returns the manager and a receiver for terminal events.
    pub fn new() -> (Self, mpsc::UnboundedReceiver<TerminalEvent>) {
        // Copy from ecode-core/src/terminal/mod.rs lines 29-37
    }

    /// Open a new terminal session.
    pub fn open(&self, config: TerminalConfig) -> Result<TerminalId> {
        // Copy from ecode-core lines 41-125
        // Replace crate::platform::default_shell() with local default_shell()
    }

    /// Write data to a terminal.
    pub fn write(&self, terminal_id: &TerminalId, data: &[u8]) -> Result<()> {
        // Copy from ecode-core lines 128-139
    }

    /// Resize a terminal.
    pub fn resize(&self, terminal_id: &TerminalId, cols: u16, rows: u16) -> Result<()> {
        // Copy from ecode-core lines 143-152
    }

    /// Close a terminal session.
    pub fn close(&self, terminal_id: &TerminalId) -> Result<()> {
        // Copy from ecode-core lines 155-163
    }

    /// Check if a terminal session is alive.
    pub fn is_alive(&self, terminal_id: &TerminalId) -> bool {
        // Copy from ecode-core lines 166-172
    }

    /// Get the list of active terminal IDs.
    pub fn active_terminals(&self) -> Vec<TerminalId> {
        // Copy from ecode-core lines 175-182
    }

    /// Close all terminal sessions.
    pub fn close_all(&self) {
        // Copy from ecode-core lines 185-193
    }
}

impl Drop for TerminalManager {
    fn drop(&mut self) { self.close_all(); }
}
```

**Section 4: Tool structs:**

```rust
/// Create a new terminal session.
pub struct CreateTerminalTool;

impl Tool for CreateTerminalTool {
    fn name(&self) -> &str { "create_terminal" }
    fn description(&self) -> &str { "Spawn a new PTY terminal session in the workspace directory" }
    fn risk_level(&self) -> RiskLevel { RiskLevel::Medium }
    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "shell": { "type": "string", "description": "Shell to use (default: system default shell)" },
                "cols": { "type": "integer", "description": "Terminal columns (default: 120)" },
                "rows": { "type": "integer", "description": "Terminal rows (default: 30)" }
            }
        })
    }
    fn execute(&self, params: Value, ctx: &ToolContext)
        -> Pin<Box<dyn Future<Output = std::result::Result<ToolResult, ToolError>> + Send + '_>>
    {
        Box::pin(async move {
            // Get terminal_manager from ctx.services
            let services = ctx.services.as_ref()
                .ok_or_else(|| ToolError::ExecutionFailed("ToolServices not available".into()))?;
            let manager = services.terminal_manager.as_ref()
                .ok_or_else(|| ToolError::ExecutionFailed("TerminalManager not available".into()))?;
            let manager = manager.downcast_ref::<TerminalManager>()
                .ok_or_else(|| ToolError::ExecutionFailed("Invalid TerminalManager type".into()))?;

            let config = TerminalConfig {
                cwd: ctx.workspace_root.clone(),
                shell: params.get("shell").and_then(Value::as_str).map(String::from),
                cols: params.get("cols").and_then(Value::as_u64).unwrap_or(120) as u16,
                rows: params.get("rows").and_then(Value::as_u64).unwrap_or(30) as u16,
            };

            let terminal_id = manager.open(config)
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            Ok(ToolResult {
                output: json!({"terminal_id": terminal_id.to_string()}),
                is_error: false,
            })
        })
    }
}

/// Write data to an active terminal.
pub struct TerminalWriteTool;

impl Tool for TerminalWriteTool {
    fn name(&self) -> &str { "terminal_write" }
    fn description(&self) -> &str { "Write input data to an active terminal session" }
    fn risk_level(&self) -> RiskLevel { RiskLevel::Medium }
    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "terminal_id": { "type": "string", "description": "The terminal ID to write to" },
                "data": { "type": "string", "description": "Data to send to the terminal (include \\n for enter)" }
            },
            "required": ["terminal_id", "data"]
        })
    }
    fn execute(&self, params: Value, ctx: &ToolContext)
        -> Pin<Box<dyn Future<Output = std::result::Result<ToolResult, ToolError>> + Send + '_>>
    {
        Box::pin(async move {
            let services = ctx.services.as_ref()
                .ok_or_else(|| ToolError::ExecutionFailed("ToolServices not available".into()))?;
            let manager = services.terminal_manager.as_ref()
                .ok_or_else(|| ToolError::ExecutionFailed("TerminalManager not available".into()))?;
            let manager = manager.downcast_ref::<TerminalManager>()
                .ok_or_else(|| ToolError::ExecutionFailed("Invalid TerminalManager type".into()))?;

            let terminal_id_str = params.get("terminal_id").and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidParams("missing terminal_id".into()))?;
            let terminal_id = TerminalId::parse(terminal_id_str)
                .map_err(|e| ToolError::InvalidParams(format!("invalid terminal_id: {}", e)))?;
            let data = params.get("data").and_then(Value::as_str)
                .ok_or_else(|| ToolError::InvalidParams("missing data".into()))?;

            manager.write(&terminal_id, data.as_bytes())
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            Ok(ToolResult {
                output: json!({"written": true}),
                is_error: false,
            })
        })
    }
}
```

- [ ] **Step 3: Add module declaration to `crates/eagent-tools/src/lib.rs`**

Add `pub mod terminal;` after `pub mod git;`.

- [ ] **Step 4: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_manager_new() {
        let (manager, _rx) = TerminalManager::new();
        assert!(manager.active_terminals().is_empty());
    }

    #[test]
    fn test_create_terminal_tool_def() {
        let tool = CreateTerminalTool;
        assert_eq!(tool.name(), "create_terminal");
        assert_eq!(tool.risk_level(), RiskLevel::Medium);
        let schema = tool.parameter_schema();
        assert!(schema["properties"]["shell"].is_object());
    }

    #[test]
    fn test_terminal_write_tool_def() {
        let tool = TerminalWriteTool;
        assert_eq!(tool.name(), "terminal_write");
        assert_eq!(tool.risk_level(), RiskLevel::Medium);
        let schema = tool.parameter_schema();
        assert!(schema["required"].as_array().unwrap().contains(&json!("terminal_id")));
    }

    #[test]
    fn test_default_shell() {
        let shell = default_shell();
        assert!(!shell.is_empty());
        #[cfg(windows)]
        assert_eq!(shell, "powershell.exe");
    }

    // Live PTY tests — mark as #[ignore] for CI environments
    #[tokio::test]
    #[ignore = "requires PTY support, may be flaky in CI"]
    async fn test_terminal_open_write_close() {
        let (manager, mut rx) = TerminalManager::new();
        let config = TerminalConfig {
            cwd: std::env::temp_dir().to_string_lossy().to_string(),
            shell: None,
            cols: 80,
            rows: 24,
        };
        let tid = manager.open(config).unwrap();
        assert!(manager.is_alive(&tid));

        manager.write(&tid, b"echo hello\n").unwrap();

        // Wait briefly for output
        let event = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            rx.recv(),
        ).await;
        assert!(event.is_ok());

        manager.close(&tid).unwrap();
        assert!(!manager.is_alive(&tid));
    }
}
```

- [ ] **Step 5: Build and test**

```bash
export PATH="/c/Users/deyan/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"
cargo check -p eagent-tools && cargo test -p eagent-tools
```

- [ ] **Step 6: Commit**

```bash
git add crates/eagent-tools/
git commit -m "feat(eagent-tools): implement terminal tools (create_terminal, terminal_write) with TerminalManager"
```

---

### Task 8: Migrate LlamaCpp provider

Wrap the `LlamaCppManager` from `crates/ecode-core/src/providers/llama_cpp.rs` behind the `Provider` trait. The key improvement is streaming support — instead of `stream: false`, the new provider uses `stream: true` and parses SSE events to emit `ProviderEvent` variants through an mpsc channel.

**Files:**
- Modify: `crates/eagent-providers/Cargo.toml`
- Create: `crates/eagent-providers/src/llama_cpp.rs`
- Modify: `crates/eagent-providers/src/lib.rs`

**Steps:**

- [ ] **Step 1: Add dependencies to Cargo.toml**

Add to `crates/eagent-providers/Cargo.toml` under `[dependencies]`:

```toml
reqwest = { workspace = true }
shared_child = { workspace = true }
anyhow = { workspace = true }
```

- [ ] **Step 2: Create `crates/eagent-providers/src/llama_cpp.rs`**

```rust
use crate::{Provider, ProviderError, ProviderMessage, ProviderMessageRole, SessionConfig, SessionHandle};
use eagent_contracts::provider::{FinishReason, ModelInfo, ProviderEvent, ProviderKind, ProviderSessionStatus};
use eagent_protocol::ids::SessionId;
use eagent_tools::ToolDef;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, mpsc};
use tracing::warn;

/// Configuration for the LlamaCpp provider.
#[derive(Debug, Clone)]
pub struct LlamaCppConfig {
    pub host: String,
    pub port: u16,
    pub server_binary_path: String,
    pub model_path: String,
    pub ctx_size: u32,
    pub threads: u16,
    pub gpu_layers: i32,
    pub flash_attention: bool,
    pub temperature: f32,
    pub top_p: f32,
}

struct LlamaCppSession {
    id: SessionId,
    model: String,
    status: ProviderSessionStatus,
}

/// LlamaCpp provider wrapping llama-server's OpenAI-compatible API.
pub struct LlamaCppProvider {
    config: LlamaCppConfig,
    client: reqwest::Client,
    process: Arc<Mutex<Option<Child>>>,
    sessions: Arc<Mutex<HashMap<SessionId, LlamaCppSession>>>,
}

impl LlamaCppProvider {
    pub fn new(config: LlamaCppConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(120))  // longer for streaming
                .user_agent("eAgent-LlamaCpp/0.1")
                .build()
                .expect("llama.cpp client"),
            process: Arc::new(Mutex::new(None)),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}:{}", self.config.host, self.config.port)
    }

    /// Ensure llama-server is running and responsive.
    /// Adapted from LlamaCppManager::ensure_ready (ecode-core lines 37-105).
    async fn ensure_ready(&self) -> Result<(), ProviderError> {
        // 1. Probe /v1/models — if OK, return
        // 2. If server_binary_path empty, return error
        // 3. Check/clean stale child process
        // 4. Spawn llama-server with args: -m model_path --host --port --ctx-size --threads --n-gpu-layers
        // 5. Poll /v1/models up to 20 times with 500ms delay
        // Map all errors to ProviderError::ConnectionFailed
    }

    async fn probe_models(&self) -> Result<(), ProviderError> {
        // GET {base_url}/v1/models
        // Copy from LlamaCppManager::probe_models (ecode-core lines 165-173)
    }

    /// Convert ProviderMessage vec to OpenAI-format JSON messages.
    fn convert_messages(messages: &[ProviderMessage]) -> Vec<Value> {
        messages.iter().map(|m| {
            json!({
                "role": match m.role {
                    ProviderMessageRole::System => "system",
                    ProviderMessageRole::User => "user",
                    ProviderMessageRole::Assistant => "assistant",
                    ProviderMessageRole::Tool => "tool",
                },
                "content": m.content,
            })
        }).collect()
    }

    /// Stream a completion from /v1/chat/completions with SSE.
    async fn stream_completion(
        &self,
        model: &str,
        messages: Vec<Value>,
        _tools: &[ToolDef],
        tx: mpsc::UnboundedSender<ProviderEvent>,
    ) -> Result<(), ProviderError> {
        let url = format!("{}/v1/chat/completions", self.base_url());
        let body = json!({
            "model": model,
            "messages": messages,
            "stream": true,
            "temperature": self.config.temperature,
            "top_p": self.config.top_p,
        });

        let response = self.client.post(&url).json(&body).send().await
            .map_err(|e| ProviderError::ConnectionFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ProviderError::ConnectionFailed(
                format!("llama.cpp returned status {}", response.status())
            ));
        }

        // Parse SSE stream
        // Each line is either empty, "data: [DONE]", or "data: {json}"
        // JSON format: { "choices": [{ "delta": { "content": "..." }, "finish_reason": null|"stop" }] }
        // For each chunk:
        //   - delta.content -> emit ProviderEvent::TokenDelta { text }
        //   - delta.tool_calls -> emit ToolCallStart/Delta/Complete events
        //   - finish_reason != null -> emit ProviderEvent::Completion { finish_reason }
        // Read bytes, split on newlines, parse each "data: " prefixed line
    }
}

impl Provider for LlamaCppProvider {
    fn create_session(&self, config: SessionConfig)
        -> Pin<Box<dyn Future<Output = Result<SessionHandle, ProviderError>> + Send + '_>>
    {
        Box::pin(async move {
            self.ensure_ready().await?;
            let id = SessionId::new();
            self.sessions.lock().await.insert(id, LlamaCppSession {
                id, model: config.model.clone(), status: ProviderSessionStatus::Ready,
            });
            Ok(SessionHandle { session_id: id, provider_name: "llama-cpp".into() })
        })
    }

    fn send(&self, session: &SessionHandle, messages: Vec<ProviderMessage>, tools: Vec<ToolDef>)
        -> Pin<Box<dyn Future<Output = Result<mpsc::UnboundedReceiver<ProviderEvent>, ProviderError>> + Send + '_>>
    {
        let session_id = session.session_id;
        Box::pin(async move {
            let model = {
                let sessions = self.sessions.lock().await;
                let s = sessions.get(&session_id)
                    .ok_or_else(|| ProviderError::SessionNotFound(session_id.to_string()))?;
                s.model.clone()
            };
            let json_messages = Self::convert_messages(&messages);
            let (tx, rx) = mpsc::unbounded_channel();

            // Spawn streaming task
            let this_client = self.client.clone();
            let base_url = self.base_url();
            let temperature = self.config.temperature;
            let top_p = self.config.top_p;
            tokio::spawn(async move {
                // POST with stream:true, parse SSE, send ProviderEvents through tx
                // On error, send ProviderEvent::Error
            });

            Ok(rx)
        })
    }

    fn cancel(&self, session: &SessionHandle)
        -> Pin<Box<dyn Future<Output = Result<(), ProviderError>> + Send + '_>>
    {
        let session_id = session.session_id;
        Box::pin(async move {
            self.sessions.lock().await.remove(&session_id);
            Ok(())
        })
    }

    fn list_models(&self)
        -> Pin<Box<dyn Future<Output = Result<Vec<ModelInfo>, ProviderError>> + Send + '_>>
    {
        Box::pin(async move {
            self.ensure_ready().await?;
            let url = format!("{}/v1/models", self.base_url());
            let resp = self.client.get(&url).send().await
                .map_err(|e| ProviderError::ConnectionFailed(e.to_string()))?;
            let json: Value = resp.json().await
                .map_err(|e| ProviderError::Internal(e.to_string()))?;
            let models = json["data"].as_array()
                .map(|arr| arr.iter().filter_map(|m| {
                    Some(ModelInfo {
                        id: m["id"].as_str()?.to_string(),
                        name: m["id"].as_str()?.to_string(),
                        max_context: Some(self.config.ctx_size),
                        provider_kind: ProviderKind::LlamaCpp,
                    })
                }).collect())
                .unwrap_or_default();
            Ok(models)
        })
    }

    fn session_status(&self, session: &SessionHandle) -> ProviderSessionStatus {
        // Use try_lock since this is sync
        self.sessions.try_lock()
            .ok()
            .and_then(|s| s.get(&session.session_id).map(|s| s.status))
            .unwrap_or(ProviderSessionStatus::Stopped)
    }
}
```

- [ ] **Step 3: Add module declaration to `crates/eagent-providers/src/lib.rs`**

Add `pub mod llama_cpp;` after `pub mod registry;`.

- [ ] **Step 4: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> LlamaCppConfig {
        LlamaCppConfig {
            host: "127.0.0.1".into(), port: 8012,
            server_binary_path: String::new(), model_path: String::new(),
            ctx_size: 4096, threads: 4, gpu_layers: 0,
            flash_attention: false, temperature: 0.2, top_p: 0.95,
        }
    }

    #[test]
    fn test_base_url() {
        let provider = LlamaCppProvider::new(test_config());
        assert_eq!(provider.base_url(), "http://127.0.0.1:8012");
    }

    #[test]
    fn test_convert_messages() {
        let messages = vec![
            ProviderMessage { role: ProviderMessageRole::User, content: "hello".into() },
        ];
        let json = LlamaCppProvider::convert_messages(&messages);
        assert_eq!(json[0]["role"], "user");
        assert_eq!(json[0]["content"], "hello");
    }

    #[tokio::test]
    async fn test_create_session_fails_without_server() {
        let provider = LlamaCppProvider::new(test_config());
        let result = provider.create_session(SessionConfig {
            model: "test".into(), system_prompt: None, workspace_root: None,
        }).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_session_status_unknown() {
        let provider = LlamaCppProvider::new(test_config());
        let handle = SessionHandle { session_id: SessionId::new(), provider_name: "llama-cpp".into() };
        assert_eq!(provider.session_status(&handle), ProviderSessionStatus::Stopped);
    }
}
```

- [ ] **Step 5: Build and test**

```bash
export PATH="/c/Users/deyan/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"
cargo check -p eagent-providers && cargo test -p eagent-providers
```

- [ ] **Step 6: Commit**

```bash
git add crates/eagent-providers/
git commit -m "feat(eagent-providers): implement LlamaCppProvider with SSE streaming and Provider trait"
```

---

### Task 9: Migrate Codex provider

Wrap the `CodexManager` from `crates/ecode-core/src/codex/mod.rs` behind the `Provider` trait. Codex JSON-RPC protocol types are copied from `crates/ecode-contracts/src/codex.rs` into a local submodule. Approval handling is deferred to Phase 3 — approval requests are logged as warnings but not routed through the Provider trait.

**Files:**
- Modify: `crates/eagent-providers/Cargo.toml`
- Create: `crates/eagent-providers/src/codex/mod.rs`
- Create: `crates/eagent-providers/src/codex/protocol.rs`
- Create: `crates/eagent-providers/src/codex/version.rs`
- Modify: `crates/eagent-providers/src/lib.rs`

**Steps:**

- [ ] **Step 1: Add dependencies to Cargo.toml**

Add to `crates/eagent-providers/Cargo.toml` under `[dependencies]`:

```toml
anyhow = { workspace = true }
```

Add a platform-conditional dependency for Windows process tree killing:

```toml
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.59", features = ["Win32_Foundation", "Win32_System_Threading", "Win32_System_JobObjects"] }
```

- [ ] **Step 2: Create `crates/eagent-providers/src/codex/protocol.rs`**

Copy ALL types from `crates/ecode-contracts/src/codex.rs` (456 lines). This includes:

**JSON-RPC frames:**
- `JsonRpcRequest` — `{ method: String, id: Option<u64>, params: Option<Value> }`
- `JsonRpcResponse` — `{ id: u64, result: Option<Value>, error: Option<JsonRpcError> }`
- `JsonRpcError` — `{ code: i32, message: String, data: Option<Value> }`
- `JsonRpcNotification` — `{ method: String, params: Option<Value> }`
- `IncomingMessage` — enum with `Response`, `Notification`, `ServerRequest` variants + `parse()` method

**Initialize types:**
- `ClientInfo`, `ClientCapabilities`, `InitializeParams`

**Thread management:**
- `ApprovalPolicy` (Never, OnRequest), `SandboxMode` (WorkspaceWrite, DangerFullAccess)
- `ThreadStartParams`, `ThreadResumeParams`

**Turn management:**
- `TurnInputItem` (Text, Image), `TurnStartParams`, `TurnInterruptParams`

**Notification types:**
- `CodexThreadInfo`, `CodexTurnInfo`
- `ThreadStartedNotification`, `TurnStartedNotification`, `TurnCompletedNotification`
- `AgentMessageDelta`, `CodexError`, `CodexErrorDetail`

**Server request types:**
- `CommandApprovalRequest`, `FileChangeApprovalRequest`, `FileReadApprovalRequest`, `UserInputRequest`

**Response types:**
- `ApprovalDecision`, `ApprovalResponse`

**Account/Model:**
- `AccountInfo`
- `CodexModelInfo` (rename from `ModelInfo` to avoid clash with `eagent_contracts::provider::ModelInfo`) — `{ id: String, name: Option<String> }`
- `ModelListResult` — `{ models: Vec<CodexModelInfo> }`

**Events:**
- `CodexEvent` enum (all 10 variants: ThreadStarted, TurnStarted, TurnCompleted, AgentMessageDelta, CommandApprovalRequested, FileChangeApprovalRequested, FileReadApprovalRequested, UserInputRequested, Error, SessionClosed)

Copy the `#[cfg(test)]` module with the 5 existing tests (parse_response, parse_notification, parse_server_request, approval_decision_serde, etc.).

- [ ] **Step 3: Create `crates/eagent-providers/src/codex/version.rs`**

Copy the entire file from `crates/ecode-core/src/codex/version.rs` (111 lines):

```rust
use anyhow::{Context, Result, bail};
use std::process::Command;
use tracing::info;

/// Check that the codex binary exists and meets the minimum version requirement.
pub fn check_codex_version(binary_path: &str, min_version: &str) -> Result<String> {
    // Copy body from ecode-core/src/codex/version.rs lines 8-39
}

/// Check if a version string meets the minimum.
fn version_meets_minimum(version: &str, minimum: &str) -> bool {
    // Copy from lines 42-60
}

/// Find the codex binary on PATH.
pub fn find_codex_binary(custom_path: Option<&str>) -> Result<String> {
    // Copy from lines 63-95
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        // Copy from lines 102-109
        assert!(version_meets_minimum("0.37.0", "0.37.0"));
        assert!(version_meets_minimum("0.38.0", "0.37.0"));
        assert!(version_meets_minimum("1.0.0", "0.37.0"));
        assert!(!version_meets_minimum("0.36.9", "0.37.0"));
        assert!(!version_meets_minimum("0.36.0", "0.37.0"));
        assert!(version_meets_minimum("0.37.1", "0.37.0"));
    }
}
```

- [ ] **Step 4: Create `crates/eagent-providers/src/codex/mod.rs`**

This is the main provider implementation.

```rust
pub mod protocol;
pub mod version;

use crate::{Provider, ProviderError, ProviderMessage, ProviderMessageRole, SessionConfig, SessionHandle};
use eagent_contracts::provider::{FinishReason, ModelInfo, ProviderEvent, ProviderKind, ProviderSessionStatus};
use eagent_protocol::ids::SessionId;
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

pub use version::{check_codex_version, find_codex_binary};

/// Configuration for the Codex provider.
#[derive(Debug, Clone)]
pub struct CodexConfig {
    /// Path to the codex binary (empty = find on PATH).
    pub binary_path: String,
    /// Optional CODEX_HOME override.
    pub home_dir: Option<String>,
}

/// Internal session state for a Codex app-server process.
struct CodexSession {
    child: Child,
    stdin_tx: mpsc::Sender<String>,
    pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    codex_thread_id: Option<String>,
    reader_handle: tokio::task::JoinHandle<()>,
    writer_handle: tokio::task::JoinHandle<()>,
    status: ProviderSessionStatus,
}

/// Codex provider wrapping the Codex CLI app-server via JSON-RPC over stdio.
pub struct CodexProvider {
    config: CodexConfig,
    sessions: Arc<Mutex<HashMap<SessionId, CodexSession>>>,
    next_rpc_id: Arc<AtomicU64>,
}

impl CodexProvider {
    pub fn new(config: CodexConfig) -> Self {
        Self {
            config,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            next_rpc_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Spawn codex app-server and initialize the JSON-RPC session.
    /// Adapted from CodexManager::spawn_session (ecode-core/src/codex/mod.rs lines 84-292).
    async fn spawn_session(&self, session_id: SessionId, config: &SessionConfig)
        -> Result<mpsc::UnboundedReceiver<ProviderEvent>, ProviderError>
    {
        // 1. Resolve binary path
        let binary = find_codex_binary(if self.config.binary_path.is_empty() {
            None
        } else {
            Some(&self.config.binary_path)
        }).map_err(|e| ProviderError::ConnectionFailed(e.to_string()))?;

        // 2. Build command: codex app-server, with piped stdio
        //    Copy platform-specific process group setup from ecode-core
        //    On Windows: creation_flags(CREATE_NEW_PROCESS_GROUP)
        //    On Unix: pre_exec setpgid

        // 3. Spawn child, take stdin/stdout/stderr

        // 4. Create channels:
        //    - (stdin_tx, stdin_rx) for writing to codex stdin
        //    - (event_tx, event_rx) for ProviderEvent output
        //    - pending_requests map for RPC responses

        // 5. Spawn writer task: reads from stdin_rx, writes to child stdin

        // 6. Spawn reader task: reads lines from stdout, parses JSON
        //    - IncomingMessage::Response -> resolve pending request
        //    - IncomingMessage::Notification -> map to ProviderEvent:
        //        "item/agentMessage/delta" -> ProviderEvent::TokenDelta { text: delta }
        //        "turn/completed" -> ProviderEvent::Completion { finish_reason: Stop }
        //        "error" -> ProviderEvent::Error { message }
        //    - IncomingMessage::ServerRequest (approval) -> log warning, defer to Phase 3
        //    - On EOF: send ProviderEvent::Error { message: "session closed" }

        // 7. Spawn stderr reader task: logs warnings

        // 8. Send "initialize" RPC request, wait for response (30s timeout)
        // 9. Send "initialized" notification

        // 10. If config.workspace_root is set, send "thread/start" RPC:
        //     ThreadStartParams { model, approval_policy: Never, sandbox: WorkspaceWrite, cwd }
        //     Store codex_thread_id from response

        // 11. Store session in self.sessions, return event_rx
    }

    /// Send a JSON-RPC request and wait for the response.
    /// Adapted from CodexManager::send_request (ecode-core lines 536-576).
    async fn send_rpc(
        &self,
        session_id: SessionId,
        method: &str,
        params: Option<Value>,
    ) -> Result<JsonRpcResponse, ProviderError> {
        // Get session, create request with next_rpc_id, send via stdin_tx
        // Wait on oneshot receiver with 60s timeout
        // Check for RPC error in response
    }
}

impl Provider for CodexProvider {
    fn create_session(&self, config: SessionConfig)
        -> Pin<Box<dyn Future<Output = Result<SessionHandle, ProviderError>> + Send + '_>>
    {
        Box::pin(async move {
            let session_id = SessionId::new();
            let _rx = self.spawn_session(session_id, &config).await?;
            Ok(SessionHandle { session_id, provider_name: "codex".into() })
        })
    }

    fn send(&self, session: &SessionHandle, messages: Vec<ProviderMessage>, _tools: Vec<ToolDef>)
        -> Pin<Box<dyn Future<Output = Result<mpsc::UnboundedReceiver<ProviderEvent>, ProviderError>> + Send + '_>>
    {
        let session_id = session.session_id;
        Box::pin(async move {
            // Get session's codex_thread_id
            // Extract text from last user message
            // Send "turn/start" RPC with TurnStartParams
            // Create new (event_tx, event_rx) channel
            // The reader task (started in spawn_session) will continue to route events
            // Return the rx end
            // NOTE: The reader task needs to use a swappable tx or we need to
            // create the event channel in spawn_session and return it here.
            // Simplest approach: store event_tx in session, swap it on each send() call
        })
    }

    fn cancel(&self, session: &SessionHandle)
        -> Pin<Box<dyn Future<Output = Result<(), ProviderError>> + Send + '_>>
    {
        let session_id = session.session_id;
        Box::pin(async move {
            let mut sessions = self.sessions.lock().await;
            if let Some(mut session) = sessions.remove(&session_id) {
                session.reader_handle.abort();
                session.writer_handle.abort();
                // Kill process tree
                if let Some(pid) = session.child.id() {
                    let _ = kill_process_tree(pid).await;
                }
                let _ = session.child.kill().await;
            }
            Ok(())
        })
    }

    fn list_models(&self)
        -> Pin<Box<dyn Future<Output = Result<Vec<ModelInfo>, ProviderError>> + Send + '_>>
    {
        Box::pin(async move {
            // Try to get models from an active session via "model/list" RPC
            // If no active session, return hardcoded fallback list:
            let fallback = vec![
                ModelInfo { id: "o4-mini".into(), name: "o4-mini".into(), max_context: Some(200_000), provider_kind: ProviderKind::Codex },
                ModelInfo { id: "o3".into(), name: "o3".into(), max_context: Some(200_000), provider_kind: ProviderKind::Codex },
                ModelInfo { id: "gpt-5.4".into(), name: "gpt-5.4".into(), max_context: Some(128_000), provider_kind: ProviderKind::Codex },
            ];
            // Try first active session for model/list
            let sessions = self.sessions.lock().await;
            if let Some((&sid, _)) = sessions.iter().next() {
                drop(sessions);
                match self.send_rpc(sid, "model/list", None).await {
                    Ok(resp) => {
                        if let Some(result) = resp.result {
                            if let Ok(list) = serde_json::from_value::<ModelListResult>(result) {
                                return Ok(list.models.into_iter().map(|m| ModelInfo {
                                    id: m.id.clone(),
                                    name: m.name.unwrap_or(m.id),
                                    max_context: Some(200_000),
                                    provider_kind: ProviderKind::Codex,
                                }).collect());
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
            Ok(fallback)
        })
    }

    fn session_status(&self, session: &SessionHandle) -> ProviderSessionStatus {
        self.sessions.try_lock()
            .ok()
            .and_then(|s| s.get(&session.session_id).map(|s| s.status))
            .unwrap_or(ProviderSessionStatus::Stopped)
    }
}

// ── Platform helpers for process tree killing ──
// (copied from crates/ecode-core/src/platform/mod.rs)

async fn kill_process_tree(pid: u32) -> anyhow::Result<()> {
    #[cfg(windows)]
    { kill_process_tree_windows(pid)?; }
    #[cfg(not(windows))]
    { kill_process_tree_unix(pid)?; }
    info!(%pid, "Killed process tree");
    Ok(())
}

#[cfg(windows)]
fn kill_process_tree_windows(pid: u32) -> anyhow::Result<()> {
    // Copy from ecode-core/src/platform/mod.rs lines 52-80
    // Uses windows_sys: OpenProcess, CreateJobObjectW, AssignProcessToJobObject,
    // TerminateJobObject, CloseHandle
}

#[cfg(not(windows))]
fn kill_process_tree_unix(pid: u32) -> anyhow::Result<()> {
    // Copy from ecode-core/src/platform/mod.rs lines 83-89
    // Uses libc::kill with negative pid (process group)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codex_provider_creation() {
        let config = CodexConfig { binary_path: "codex".into(), home_dir: None };
        let _provider = CodexProvider::new(config);
    }

    #[test]
    fn test_session_status_unknown() {
        let config = CodexConfig { binary_path: "codex".into(), home_dir: None };
        let provider = CodexProvider::new(config);
        let handle = SessionHandle { session_id: SessionId::new(), provider_name: "codex".into() };
        assert_eq!(provider.session_status(&handle), ProviderSessionStatus::Stopped);
    }
}
```

- [ ] **Step 5: Update module declaration in `crates/eagent-providers/src/lib.rs`**

Add `pub mod codex;` after `pub mod llama_cpp;`.

- [ ] **Step 6: Build and test**

```bash
export PATH="/c/Users/deyan/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"
cargo check -p eagent-providers && cargo test -p eagent-providers
```

- [ ] **Step 7: Commit**

```bash
git add crates/eagent-providers/
git commit -m "feat(eagent-providers): implement CodexProvider with JSON-RPC protocol and Provider trait"
```

---

### Task 10: Wire up modules and verify

Register all built-in tools and providers with convenience functions, ensure the full workspace compiles and tests pass, update LOG.md, and commit.

**Files:**
- Modify: `crates/eagent-tools/src/lib.rs` (verify all module declarations)
- Modify: `crates/eagent-tools/src/registry.rs` (add `register_builtin_tools` method)
- Modify: `crates/eagent-providers/src/lib.rs` (verify all module declarations)
- Modify: `crates/eagent-providers/src/registry.rs` (add `register_from_config` method)
- Modify: `LOG.md`

**Steps:**

- [ ] **Step 1: Verify all module declarations in `crates/eagent-tools/src/lib.rs`**

Ensure these are present after `pub mod registry;`:

```rust
pub mod registry;
pub mod filesystem;
pub mod web;
pub mod git;
pub mod terminal;
```

- [ ] **Step 2: Add `register_builtin_tools()` to `crates/eagent-tools/src/registry.rs`**

Add a method to `ToolRegistry`:

```rust
use std::sync::Arc;

impl ToolRegistry {
    /// Register all built-in eAgent tools.
    ///
    /// Call this during harness startup. Terminal tools will only work
    /// if ToolContext.services.terminal_manager is populated.
    pub fn register_builtin_tools(&mut self) {
        use crate::filesystem::*;
        use crate::web::*;
        use crate::git::*;
        use crate::terminal::*;

        // Filesystem tools (7)
        self.register(Arc::new(ListDirectoryTool));
        self.register(Arc::new(ReadFileTool));
        self.register(Arc::new(ReadMultipleFilesTool));
        self.register(Arc::new(SearchFilesTool));
        self.register(Arc::new(WriteFileTool));
        self.register(Arc::new(EditFileTool));
        self.register(Arc::new(ApplyPatchTool));

        // Web tools (2)
        self.register(Arc::new(WebSearchTool));
        self.register(Arc::new(WebFetchTool));

        // Git tools (4)
        self.register(Arc::new(GitStatusTool));
        self.register(Arc::new(GitDiffTool));
        self.register(Arc::new(GitCommitTool));
        self.register(Arc::new(GitBranchTool));

        // Terminal tools (2)
        self.register(Arc::new(CreateTerminalTool));
        self.register(Arc::new(TerminalWriteTool));
    }
}
```

- [ ] **Step 3: Add test for register_builtin_tools**

In `crates/eagent-tools/src/lib.rs` tests section, add:

```rust
#[test]
fn register_builtin_tools_populates_registry() {
    let mut reg = registry::ToolRegistry::new();
    reg.register_builtin_tools();
    assert_eq!(reg.len(), 15); // 7 filesystem + 2 web + 4 git + 2 terminal
    assert!(reg.get("list_directory").is_some());
    assert!(reg.get("read_file").is_some());
    assert!(reg.get("web_search").is_some());
    assert!(reg.get("git_status").is_some());
    assert!(reg.get("create_terminal").is_some());
}
```

- [ ] **Step 4: Verify all module declarations in `crates/eagent-providers/src/lib.rs`**

Ensure these are present:

```rust
pub mod registry;
pub mod llama_cpp;
pub mod codex;
```

- [ ] **Step 5: Add `register_from_config()` to `crates/eagent-providers/src/registry.rs`**

Add a method to `ProviderRegistry`:

```rust
use std::sync::Arc;

impl ProviderRegistry {
    /// Register built-in providers based on the application configuration.
    ///
    /// Iterates over `config.providers` and creates the appropriate provider
    /// for each enabled entry. ApiKey providers are skipped (Phase 4).
    pub fn register_from_config(&mut self, config: &eagent_contracts::config::AgentConfig) {
        use crate::codex::{CodexConfig, CodexProvider};
        use crate::llama_cpp::{LlamaCppConfig, LlamaCppProvider};

        for (name, provider_config) in &config.providers {
            if !provider_config.enabled {
                continue;
            }
            match &provider_config.specific {
                eagent_contracts::config::ProviderSpecificConfig::Codex {
                    binary_path, home_dir,
                } => {
                    let codex = CodexProvider::new(CodexConfig {
                        binary_path: binary_path.clone(),
                        home_dir: if home_dir.is_empty() { None } else { Some(home_dir.clone()) },
                    });
                    self.register(name.clone(), Arc::new(codex));
                    tracing::info!(name = %name, "Registered Codex provider");
                }
                eagent_contracts::config::ProviderSpecificConfig::LlamaCpp {
                    server_binary_path, model_path, host, port,
                    ctx_size, threads, gpu_layers,
                } => {
                    let llama = LlamaCppProvider::new(LlamaCppConfig {
                        host: host.clone(), port: *port,
                        server_binary_path: server_binary_path.clone(),
                        model_path: model_path.clone(),
                        ctx_size: *ctx_size, threads: *threads, gpu_layers: *gpu_layers,
                        flash_attention: false, temperature: 0.2, top_p: 0.95,
                    });
                    self.register(name.clone(), Arc::new(llama));
                    tracing::info!(name = %name, "Registered LlamaCpp provider");
                }
                eagent_contracts::config::ProviderSpecificConfig::ApiKey { .. } => {
                    tracing::info!(name = %name, "Skipping ApiKey provider (Phase 4)");
                }
            }
        }
    }
}
```

- [ ] **Step 6: Run full workspace check and test**

```bash
export PATH="/c/Users/deyan/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"
cargo check && cargo test
```

Fix any compilation errors. Common issues to watch for:
- Missing `use std::sync::Arc;` imports in registry files
- `ProviderSpecificConfig` field names may need adjustment to match the eagent-contracts definitions
- Ensure `tracing` is in the dependency list for eagent-providers (it already is)

- [ ] **Step 7: Update LOG.md**

Append to `LOG.md` under the `## 2026-03-18` section:

```markdown
- Phase 2 Extract complete — migrated ecode-core business logic into eagent-* crates:
  - `eagent-persistence`: EventStore (SQLite event sourcing), ConfigManager (AgentConfig load/save with eAgent directory layout)
  - `eagent-tools`: 15 built-in tools behind the Tool trait:
    - Filesystem (7): list_directory, read_file, read_multiple_files, search_files, write_file, edit_file, apply_patch
    - Web (2): web_search (DuckDuckGo), web_fetch (URL content extraction)
    - Git (4): git_status, git_diff, git_commit, git_branch
    - Terminal (2): create_terminal, terminal_write (with TerminalManager)
  - `eagent-providers`: 2 providers behind the Provider trait:
    - LlamaCppProvider: SSE streaming from llama-server /v1/chat/completions
    - CodexProvider: JSON-RPC over stdio with codex app-server
  - Added ToolServices to ToolContext for stateful tool access (terminal manager, event sender)
  - Added register_builtin_tools() and register_from_config() convenience functions
  - Codex approval handling deferred to Phase 3 (logged but not routed through Provider trait)
```

- [ ] **Step 8: Commit**

```bash
git add crates/eagent-tools/ crates/eagent-providers/ crates/eagent-persistence/ LOG.md
git commit -m "feat(eagent): Phase 2 complete — wire up all tool/provider modules with registration functions"
```

---

## Dependency Graph

```
Task 1 (ToolServices)
  |
  +---> Task 4 (filesystem tools) ── uses ToolContext
  +---> Task 5 (web tools) ── uses ToolContext
  +---> Task 6 (git tools) ── uses ToolContext
  +---> Task 7 (terminal tools) ── uses ToolContext.services.terminal_manager
  |
  +---> Task 8 (LlamaCpp provider) ── uses ToolDef from eagent-tools
  +---> Task 9 (Codex provider) ── uses ToolDef from eagent-tools

Task 2 (EventStore) ── independent, no deps
  |
  +---> Task 3 (ConfigManager) ── same crate, builds on Task 2's lib.rs changes

Task 10 (wire up) ── depends on ALL Tasks 1-9
```

**Parallelism:**
- Tasks 2-3 can run in parallel with Tasks 4-7
- Tasks 4, 5, 6, 7 can run in parallel with each other (all depend only on Task 1)
- Tasks 8 and 9 can run in parallel with each other
- Task 10 must run last after all others complete

## Verification

After all tasks complete, the following must pass:

```bash
export PATH="/c/Users/deyan/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"
cargo check && cargo test
```

Expected test counts (approximate):
- `eagent-protocol`: 7 tests (unchanged from Phase 1)
- `eagent-contracts`: 4 tests (unchanged from Phase 1)
- `eagent-tools`: ~20+ tests (existing mock tests + filesystem + web + git + terminal + builtin registration)
- `eagent-providers`: ~10+ tests (existing registry + llama_cpp + codex + protocol + version)
- `eagent-persistence`: ~8 tests (event_store + config)
- Existing `ecode-*` crate tests: continue to pass unchanged
