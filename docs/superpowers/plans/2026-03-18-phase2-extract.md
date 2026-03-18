# Phase 2: Extract — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate existing ecode-core business logic into eagent-* crates behind trait interfaces, creating working Tool and Provider implementations.

**Architecture:** Each migration wraps existing code behind the new traits. Old crates are preserved — ecode-desktop-app continues to work against ecode-core. New eagent-* crates provide parallel, trait-based access to the same capabilities.

**Tech Stack:** Rust, serde, rusqlite, git2, portable-pty, reqwest, tokio

**Build env:** `export PATH="/c/Users/deyan/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"`

---

### Task 1: Extend ToolContext with ToolServices

**Files:**
- Modify: `crates/eagent-tools/src/lib.rs`

- [ ] **Step 1: Add ToolServices struct and update ToolContext**

Add to `eagent-tools/src/lib.rs`:

```rust
use std::sync::Arc;

/// Shared services available to tools during execution.
/// Populated by the runtime at startup.
pub struct ToolServices {
    // Will hold terminal_manager, event senders, etc. as tools are added.
    _private: (), // placeholder to prevent default construction
}

impl ToolServices {
    pub fn new() -> Self {
        Self { _private: () }
    }
}
```

Update `ToolContext` to add optional services:

```rust
pub struct ToolContext {
    pub workspace_root: String,
    pub agent_id: eagent_protocol::ids::AgentId,
    pub task_id: eagent_protocol::ids::TaskId,
    pub services: Option<Arc<ToolServices>>,
}
```

- [ ] **Step 2: Update existing tests to include services: None**
- [ ] **Step 3: Verify** — `cargo check -p eagent-tools && cargo test -p eagent-tools`
- [ ] **Step 4: Commit** — `git commit -m "feat(tools): add ToolServices to ToolContext for shared infrastructure access"`

---

### Task 2: Migrate EventStore to eagent-persistence

**Files:**
- Modify: `crates/eagent-persistence/Cargo.toml` (add rusqlite, chrono, anyhow)
- Create: `crates/eagent-persistence/src/event_store.rs`
- Modify: `crates/eagent-persistence/src/lib.rs`

**Source:** `crates/ecode-core/src/persistence/mod.rs` (337 lines)

- [ ] **Step 1: Add dependencies to eagent-persistence/Cargo.toml**

Add: `rusqlite = { workspace = true }`, `chrono = { workspace = true }`, `anyhow = { workspace = true }`

- [ ] **Step 2: Create event_store.rs**

Copy and adapt the EventStore from ecode-core. Key changes:
- Define a local `StoredEvent` struct (same fields: id, stream_id, event_type, payload, timestamp, sequence)
- Keep the same SQLite schema (events table + checkpoints table, WAL mode)
- Keep all methods: `new()`, `append_events()`, `load_all_events()`, `load_stream_events()`, `save_checkpoint()`, `load_checkpoint()`
- The EventStore is generic — it stores JSON payloads regardless of event type

- [ ] **Step 3: Update lib.rs with module declaration**
- [ ] **Step 4: Write tests** — create, append, load, checkpoint roundtrip
- [ ] **Step 5: Verify** — `cargo check -p eagent-persistence && cargo test -p eagent-persistence`
- [ ] **Step 6: Commit** — `git commit -m "feat(persistence): migrate EventStore from ecode-core"`

---

### Task 3: Migrate ConfigManager to eagent-persistence

**Files:**
- Modify: `crates/eagent-persistence/Cargo.toml` (add dirs, toml)
- Create: `crates/eagent-persistence/src/config.rs`
- Modify: `crates/eagent-persistence/src/lib.rs`

**Source:** `crates/ecode-core/src/config/mod.rs` (205 lines)

- [ ] **Step 1: Add deps** — `dirs = { workspace = true }`, `toml = { workspace = true }`
- [ ] **Step 2: Create config.rs**

Adapt ConfigManager to use `eagent_contracts::config::AgentConfig`:
- `load()` → reads TOML, deserializes to AgentConfig
- `save()` → serializes AgentConfig to TOML, writes to file
- `config_dir()` → returns eAgent config directory (portable-first: next to exe if writable, else system dirs)
- `data_dir()` → returns eAgent data directory for SQLite etc.
- Use "eAgent" instead of "eCode" for directory names

- [ ] **Step 3: Write tests** — load/save roundtrip with tempdir
- [ ] **Step 4: Verify and commit** — `git commit -m "feat(persistence): migrate ConfigManager with AgentConfig support"`

---

### Task 4: Create filesystem tools

**Files:**
- Modify: `crates/eagent-tools/Cargo.toml` (add anyhow)
- Create: `crates/eagent-tools/src/filesystem.rs`
- Modify: `crates/eagent-tools/src/lib.rs`

**Source:** `crates/ecode-core/src/local_agent/mod.rs` — functions list_directory, read_text_file, read_multiple_files, search_files, apply_patch + helpers

- [ ] **Step 1: Add anyhow dep**
- [ ] **Step 2: Create filesystem.rs with security helpers**

```rust
// Shared security helpers
fn resolve_path(workspace_root: &str, path: &str) -> Result<PathBuf, ToolError>
fn should_skip_entry(name: &str) -> bool  // .git, node_modules, target, etc.
fn limit_output(text: &str, max_bytes: usize) -> String
```

- [ ] **Step 3: Implement ListDirectoryTool**
- name: "list_directory", risk: Low
- Lists files and directories in the workspace, respecting skip patterns

- [ ] **Step 4: Implement ReadFileTool**
- name: "read_file", risk: Low (renamed from read_text_file per spec)
- Reads a single file, max 1MB, workspace-rooted

- [ ] **Step 5: Implement ReadMultipleFilesTool**
- name: "read_multiple_files", risk: Low

- [ ] **Step 6: Implement SearchFilesTool**
- name: "search_files", risk: Low
- Regex search across workspace files, max 512KB per file

- [ ] **Step 7: Implement WriteFileTool**
- name: "write_file", risk: Medium
- Creates or overwrites a file

- [ ] **Step 8: Implement EditFileTool**
- name: "edit_file", risk: Medium
- old_string → new_string replacement

- [ ] **Step 9: Implement ApplyPatchTool**
- name: "apply_patch", risk: Medium
- Apply unified diff

- [ ] **Step 10: Write tests** — use tempdir for filesystem operations
- [ ] **Step 11: Verify and commit** — `git commit -m "feat(tools): add filesystem tools (list, read, write, edit, search, patch)"`

---

### Task 5: Create web tools

**Files:**
- Modify: `crates/eagent-tools/Cargo.toml` (add reqwest)
- Create: `crates/eagent-tools/src/web.rs`
- Modify: `crates/eagent-tools/src/lib.rs`

**Source:** `crates/ecode-core/src/local_agent/mod.rs` — web_search, fetch_html + HTML helpers

- [ ] **Step 1: Add reqwest dep**
- [ ] **Step 2: Create web.rs with HTML helpers**

Move: `strip_html_tags`, `html_entity_decode`, `extract_text_snippet`, `is_public_web_url`, `extract_duckduckgo_results`

- [ ] **Step 3: Implement WebSearchTool**
- name: "web_search", risk: Low
- DuckDuckGo HTML scraping, max 5 results, private host guards

- [ ] **Step 4: Implement WebFetchTool**
- name: "web_fetch", risk: Medium
- Fetch URL content, extract text, max 256KB body

- [ ] **Step 5: Write tests and verify**
- [ ] **Step 6: Commit** — `git commit -m "feat(tools): add web search and fetch tools"`

---

### Task 6: Create git tools

**Files:**
- Modify: `crates/eagent-tools/Cargo.toml` (add git2)
- Create: `crates/eagent-tools/src/git.rs`
- Modify: `crates/eagent-tools/src/lib.rs`

**Source:** `crates/ecode-core/src/git/mod.rs` (306 lines)

- [ ] **Step 1: Add git2 dep**
- [ ] **Step 2: Create git.rs with internal helpers**
- [ ] **Step 3: Implement GitStatusTool** — name: "git_status", risk: Low
- [ ] **Step 4: Implement GitDiffTool** — name: "git_diff", risk: Low
- [ ] **Step 5: Implement GitCommitTool** — name: "git_commit", risk: Medium
- [ ] **Step 6: Implement GitBranchTool** — name: "git_branch", risk: Medium
- [ ] **Step 7: Write tests** — use tempdir with git init
- [ ] **Step 8: Commit** — `git commit -m "feat(tools): add git tools (status, diff, commit, branch)"`

---

### Task 7: Create terminal tools

**Files:**
- Modify: `crates/eagent-tools/Cargo.toml` (add portable-pty)
- Create: `crates/eagent-tools/src/terminal.rs`
- Modify: `crates/eagent-tools/src/lib.rs`
- Modify: `crates/eagent-tools/src/lib.rs` (extend ToolServices)

**Source:** `crates/ecode-core/src/terminal/mod.rs` (202 lines)

- [ ] **Step 1: Add portable-pty dep**
- [ ] **Step 2: Migrate TerminalManager into terminal.rs**
- [ ] **Step 3: Add terminal_manager field to ToolServices**
- [ ] **Step 4: Implement CreateTerminalTool** — name: "create_terminal", risk: Medium
- [ ] **Step 5: Implement TerminalWriteTool** — name: "terminal_write", risk: Medium
- [ ] **Step 6: Write tests**
- [ ] **Step 7: Commit** — `git commit -m "feat(tools): add terminal tools with TerminalManager"`

---

### Task 8: Migrate LlamaCpp provider

**Files:**
- Modify: `crates/eagent-providers/Cargo.toml` (add reqwest, shared_child, anyhow)
- Create: `crates/eagent-providers/src/llama_cpp.rs`
- Modify: `crates/eagent-providers/src/lib.rs`

**Source:** `crates/ecode-core/src/providers/llama_cpp.rs` (212 lines)

- [ ] **Step 1: Add deps**
- [ ] **Step 2: Create llama_cpp.rs**

```rust
pub struct LlamaCppProvider {
    client: reqwest::Client,
    config: LlamaCppConfig,
    process: Arc<Mutex<Option<shared_child::SharedChild>>>,
}

impl Provider for LlamaCppProvider { ... }
```

Key methods:
- `ensure_ready()` — spawn llama-server if not running, probe /v1/models
- `create_session()` — ensure ready, return SessionHandle
- `send()` — POST /v1/chat/completions with stream:true, parse SSE into ProviderEvent
- `list_models()` — GET /v1/models
- `cancel()` — kill child process

- [ ] **Step 3: Implement SSE parsing for streaming**
- [ ] **Step 4: Write tests**
- [ ] **Step 5: Commit** — `git commit -m "feat(providers): migrate LlamaCpp provider with streaming support"`

---

### Task 9: Migrate Codex provider

**Files:**
- Modify: `crates/eagent-providers/Cargo.toml` (add platform deps)
- Create: `crates/eagent-providers/src/codex/mod.rs`
- Create: `crates/eagent-providers/src/codex/protocol.rs`
- Modify: `crates/eagent-providers/src/lib.rs`

**Source:** `crates/ecode-core/src/codex/mod.rs` (710 lines) + `crates/ecode-contracts/src/codex.rs`

- [ ] **Step 1: Add deps** — windows-sys (conditional), shared_child, anyhow
- [ ] **Step 2: Create codex/protocol.rs** — copy JSON-RPC types from ecode-contracts/src/codex.rs
- [ ] **Step 3: Create codex/mod.rs**

```rust
pub struct CodexProvider {
    binary_path: String,
    codex_home: Option<String>,
    sessions: Arc<Mutex<HashMap<SessionId, CodexSession>>>,
}

struct CodexSession {
    child: SharedChild,
    stdin_tx: mpsc::Sender<String>,
    event_rx: mpsc::Receiver<CodexEvent>,
    thread_id: Option<String>,
}

impl Provider for CodexProvider { ... }
```

Key mapping: CodexEvent → ProviderEvent:
- AgentMessageDelta → TokenDelta
- TurnCompleted → Completion
- Error → Error
- Approval requests → deferred to Phase 3 (logged but not surfaced through ProviderEvent)

- [ ] **Step 4: Implement process spawning and JSON-RPC communication**
- [ ] **Step 5: Implement event translation**
- [ ] **Step 6: Write tests**
- [ ] **Step 7: Commit** — `git commit -m "feat(providers): migrate Codex provider with JSON-RPC protocol"`

---

### Task 10: Wire up modules and verify

**Files:**
- Modify: `crates/eagent-tools/src/lib.rs` (module declarations, register function)
- Modify: `crates/eagent-providers/src/lib.rs` (module declarations)
- Modify: `crates/eagent-persistence/src/lib.rs` (module declarations)
- Modify: `LOG.md`

- [ ] **Step 1: Add module declarations to all lib.rs files**
- [ ] **Step 2: Create register_builtin_tools() in eagent-tools**

```rust
pub fn register_builtin_tools(registry: &mut ToolRegistry) {
    registry.register(Arc::new(filesystem::ListDirectoryTool));
    registry.register(Arc::new(filesystem::ReadFileTool));
    // ... all tools
}
```

- [ ] **Step 3: Full workspace verification**

```bash
export PATH="..."
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 4: Update LOG.md with Phase 2 completion**
- [ ] **Step 5: Commit** — `git commit -m "feat: complete Phase 2 — all tools, providers, and persistence migrated"`
