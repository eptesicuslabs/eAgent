# eAgent Platform Design Specification

> **Product**: eAgent by Eptesicus Laboratories
> **Date**: 2026-03-18
> **Status**: Draft
> **Scope**: Full platform design — eCode, eWork, multi-agent orchestration, eMCPs, eSkills

---

## 1. Vision & Positioning

eAgent is an **agentic engineering platform** — a desktop application where developers and knowledge workers orchestrate AI agents that perform complex, multi-step tasks autonomously. It is not a traditional IDE with a chat sidebar; it is a mission control for AI-driven work.

### Brand Hierarchy

| Component | Purpose |
|-----------|---------|
| **eAgent** | The platform / harness — hosts everything |
| **eCode** | Coding workstation mode — agentic software engineering |
| **eWork** | General-purpose workstation mode — research, documents, data |
| **eMCPs** | MCP-compatible connectors to external services |
| **eSkills** | Reusable agent capabilities (packaged prompts + tools) |

### Differentiators

1. **Local-first / privacy**: runs local models (llama.cpp, future Eptesicus models), code never leaves the machine unless the user chooses a cloud provider
2. **Performance**: Rust-native backend — instant startup, low memory, faster than Electron competitors
3. **Agentic engineering with transparency**: full parallel multi-agent orchestration with live execution traces, human-in-the-loop oversight, and mid-flight steering
4. **Multi-agent parallel orchestration**: planner decomposes tasks into a DAG, multiple worker agents execute sub-tasks concurrently with conflict resolution

### Target Users (priority order)

1. **Vibe coders / less technical builders** — describe software in English, agents build it
2. **Professional developers** using CLI tools (Claude Code, Codex) who want a visual orchestration interface
3. **Developers on Cursor / Windsurf** who want something purpose-built for agentic workflows

---

## 2. Core Protocol & Runtime Architecture

### 2.1 AgentProtocol

The protocol is the message contract between the harness and any agent. Every component (planner, worker, eSkill, eMCP) speaks this protocol.

**Harness → Agent messages:**

| Message | Fields | Purpose |
|---------|--------|---------|
| `TaskAssignment` | task_id, description, context, tools_available, constraints | Assign work |
| `TaskCancellation` | task_id, reason | Cancel a running task |
| `OversightResponse` | request_id, decision (Approve/Deny/Modify) | Human decision on a risky action |
| `ContextUpdate` | task_id, new_files, new_constraints | Update agent's context mid-flight |

**Agent → Harness messages:**

| Message | Fields | Purpose |
|---------|--------|---------|
| `ToolRequest` | task_id, tool_name, params | Agent wants to use a tool |
| `StatusUpdate` | task_id, phase, message, progress | Real-time trace entry |
| `OversightRequest` | task_id, action, context, risk_level | Agent asks for human approval |
| `SubTaskProposal` | task_id, sub_tasks: Vec\<TaskNode\> | Planner emits a TaskGraph |
| `FileMutation` | task_id, path, kind (Create/Edit/Delete), content/diff | Agent modifies a file |
| `TaskComplete` | task_id, result, artifacts | Agent finished successfully |
| `TaskFailed` | task_id, error, partial_results | Agent encountered an error |

### 2.2 Planner Agent

The Planner Agent is an LLM-powered agent that receives the user's natural language request and produces a structured TaskGraph. It is not a deterministic code-analysis pass — it uses the same provider/model infrastructure as worker agents.

**How it works:**

1. The harness assembles a planner system prompt containing: the user's request, the project graph summary (file tree, key symbols, recent changes), the available tool set, and instructions to produce a structured JSON TaskGraph.
2. The planner is given read-only tools (`read_file`, `list_directory`, `search_files`, `git_status`) to explore the codebase before proposing a plan.
3. After exploration, the planner emits a `SubTaskProposal` message containing a JSON-encoded TaskGraph DAG. The harness validates the DAG (rejects cycles, checks tool references exist) before presenting to the user.
4. The planner's output format is enforced via structured output (JSON schema) when the provider supports it, or via prompt-based formatting with validation/retry when it does not.

**Planner system prompt** is an eSkill (`eskill-planner`) shipped with eAgent. This allows users to customize planning behavior and allows different planner strategies for eCode vs eWork.

### 2.3 Runtime Architecture

```
eAgent Harness (Rust)
├── TaskGraph Scheduler
│   ├── DAG of sub-tasks with dependency tracking
│   ├── Topological sort for execution ordering
│   └── Priority-based scheduling with max concurrency
├── AgentPool Manager
│   ├── Spawns N worker agents
│   ├── Routes protocol messages
│   └── Agent lifecycle management
├── Conflict Resolver
│   ├── File-level optimistic locking (via git worktrees)
│   ├── Automatic merge on completion
│   └── Human escalation on conflicts
└── Runtime Services
    ├── Providers (Codex, llama.cpp, API keys, Eptesicus)
    ├── ToolRegistry (built-in + eMCP tools)
    ├── EventStore (SQLite, persistent)
    ├── Terminal Manager (PTY)
    ├── Git Operations
    ├── File System (workspace-scoped)
    ├── Project Index (tree-sitter, symbol index)
    ├── eMCP Client (MCP protocol bridge)
    └── eSkill Loader (manifest parser, invocation)
```

### 2.4 Task Flow

1. User submits a natural language prompt
2. Harness creates a root Task, routes to Planner Agent
3. Planner analyzes codebase (via ToolRequests) and emits SubTaskProposal (a TaskGraph DAG)
4. User reviews plan in the Plan View — can approve, modify, add, or remove tasks
5. Scheduler identifies independent sub-tasks, spawns parallel Worker Agents
6. Workers emit StatusUpdates (rendered in real-time traces), ToolRequests (harness executes), and OversightRequests (user approves/denies)
7. As tasks complete, scheduler starts dependent tasks
8. ConflictResolver handles when parallel agents modify the same file
9. User reviews final diffs, approves or requests changes

### 2.5 Error Handling & Recovery

**Provider session failure:** If a provider session drops mid-task, the harness marks the task as `Failed` with the error, retries up to N times (configurable, default 2) by creating a fresh session and replaying the task with accumulated context. If all retries fail, the task is marked `Failed`, dependents are blocked, and the user is notified with the option to reassign to a different provider or manually resolve.

**Task failure:** Failed tasks do not cascade — only direct dependents are blocked. The user can retry a failed task, skip it (unblocking dependents), or modify the TaskGraph to work around it.

**DAG validation:** The planner's SubTaskProposal is validated before acceptance: cycles are rejected, referenced tools must exist in the ToolRegistry, and task descriptions must be non-empty. Invalid proposals are sent back to the planner with the validation error for a retry (max 2 retries).

**Crash recovery:** The TaskGraph and all task state transitions are persisted to the EventStore as they happen (see section 6.5). On restart, the harness replays the event log to reconstruct the TaskGraph. Tasks that were `Running` at crash time are reset to `Ready` and rescheduled. Partial results (file mutations already applied) are preserved — the harness performs a git diff on restart to detect uncommitted agent work and presents it to the user for review.

**Conflict resolution failure:** If auto-merge fails and the user ignores the conflict notification, the conflicting task remains in `AwaitingReview` state. The scheduler continues scheduling non-conflicting tasks. A persistent notification badge shows unresolved conflicts.

---

## 3. Provider System

### 3.1 Provider Trait

```rust
pub trait Provider: Send + Sync {
    async fn create_session(&self, config: SessionConfig) -> Result<SessionHandle>;
    async fn send(
        &self,
        session: &SessionHandle,
        messages: Vec<Message>,
        tools: Vec<ToolDef>,
    ) -> Result<Pin<Box<dyn Stream<Item = ProviderEvent>>>>;
    async fn cancel(&self, session: &SessionHandle);
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;
}

// ProviderEvent represents raw LLM output. The Agent implementation in
// eagent-runtime translates these into AgentMessage protocol messages.
pub enum ProviderEvent {
    TokenDelta(String),              // streaming text token
    ToolCallStart { id: String, name: String, params_partial: String },
    ToolCallDelta { id: String, params_partial: String },
    ToolCallComplete { id: String, name: String, params: serde_json::Value },
    Completion { finish_reason: FinishReason },
    Error(String),
}
```

### 3.2 Built-in Providers

| Provider | Backend | Transport |
|----------|---------|-----------|
| `CodexProvider` | Codex CLI app-server | JSON-RPC over stdio |
| `LlamaCppProvider` | llama-server process | OpenAI-compatible HTTP |
| `ApiKeyProvider` | Any OpenAI-compatible endpoint | HTTP REST |
| `EptesicusProvider` | Future company models (local or cloud) | TBD — scoped out of v1 implementation, placeholder in provider registry |

### 3.3 Role Assignment

Users configure which provider powers which role:

```toml
[agent_defaults]
planner_provider = "claude-api"
worker_provider = "local-llama"
fallback_provider = "codex"
```

Providers are configured in settings with endpoint, API key, default model, and model discovery. Each provider specifies a `max_concurrent_sessions` limit (default 4 for API providers, 1 for local models) to support rate-limit-aware scheduling.

### 3.4 Secrets Management

API keys and OAuth tokens are stored encrypted at rest using the OS keychain (Windows Credential Manager via `windows-sys`, macOS Keychain, Linux Secret Service). If the OS keychain is unavailable (e.g., headless or restricted environments), keys are stored in a local encrypted file using AES-256-GCM with a key derived from a user-provided passphrase. OAuth redirect handling uses Tauri's deep-link capability to capture OAuth callbacks without a separate web server.

---

## 4. eCode — Coding Workstation

### 4.1 Tool Registry

| Tool | Description | Risk Level |
|------|-------------|------------|
| `read_file` | Read any file in workspace (renamed from `read_text_file`) | Low |
| `read_multiple_files` | Batch file reads (renamed from `read_multiple_files`) | Low |
| `list_directory` | List files/dirs in workspace | Low |
| `search_files` | Regex search across codebase | Low |
| `write_file` | Create or overwrite a file | Medium |
| `edit_file` | Apply targeted edit (old string → new string, new tool) | Medium |
| `apply_patch` | Apply unified diff (existing, kept for complex diffs) | Medium |
| `run_command` | Execute shell command in terminal | High |
| `git_status` | Show working tree status | Low |
| `git_diff` | Show file diffs | Low |
| `git_commit` | Create a commit | Medium |
| `git_branch` | Create/switch branches | Medium |
| `git_stash` | Stash/pop changes | Medium |
| `web_search` | Web search | Low |
| `web_fetch` | Fetch URL content | Medium |
| `request_user_input` | Ask the user a question | Low |
| `create_terminal` | Spawn a new PTY terminal | Medium |
| `terminal_write` | Write to an active terminal | Medium |

### 4.2 Features

**Live Agent Trace Panel**: Real-time structured execution log with parallel swim lanes per agent. Entries are typed (Thinking, ToolCall, FileChange, TerminalOutput, Question, Error). File changes render as inline collapsible diffs. Terminal output renders inline and links to terminal drawer.

**Integrated Terminal with Agent Awareness**: Terminals are first-class. Each agent can spawn terminals with live output. Agent-owned terminals show which agent and command. User can open independent terminals. Terminal output is captured, searchable, and persisted. Full xterm.js with ANSI support.

**Diff Review Surface**: File mutations grouped by agent/sub-task. Inline approve/reject per-hunk. Side-by-side or unified view. Syntax highlighting via syntect. Conflict markers with merge UI when parallel agents touch the same file.

**Project Graph / Codebase Understanding**: Persistent project index with file dependency relationships (imports/exports), symbol index (functions, classes, types) via tree-sitter, recent change history from git, detected project conventions. Persists between sessions.

**Plan View**: Interactive DAG visualization of the TaskGraph. Nodes = sub-tasks with status coloring. Click node to jump to its trace. Drag to reorder, right-click to add/remove/edit. Estimated progress.

**Git Integration**: Branch-per-task option. Worktree support for parallel agent isolation. Auto-commit checkpoints at task boundaries. PR draft generation.

**Oversight & Approval System**: Three modes (replacing the existing two-mode `RuntimeMode::ApprovalRequired` / `RuntimeMode::FullAccess`):
- **Full Autonomy**: agents execute all tool calls without asking. Equivalent to the existing `FullAccess` mode. Best for vibe coding users (target user C).
- **Approve Risky**: agents auto-proceed on `Low` risk tools, request approval for `Medium` and `High` risk tools. This is the new default mode — balances speed with safety.
- **Approve All**: every tool call requires explicit approval. Maximum control for sensitive codebases.

Configurable per session and per task. Users can change the mode mid-execution.

---

## 5. eWork — General-Purpose Workstation

### 5.1 Tool Registry

| Tool | Description | Risk Level |
|------|-------------|------------|
| `read_file` | Read any file in workspace | Low |
| `write_file` | Create/overwrite file | Medium |
| `list_directory` | List files/dirs | Low |
| `search_files` | Search file contents | Low |
| `web_search` | Search the internet | Low |
| `web_fetch` | Fetch and parse a URL | Medium |
| `create_document` | Create formatted doc (md, txt, csv) — writes directly to workspace | Medium |
| `create_spreadsheet` | Create CSV files. For Excel (.xlsx), uses the `rust_xlsxwriter` crate via a Rust tool backend. Formulas are expressed as standard Excel syntax in the tool params. | Medium |
| `create_presentation` | Create slide decks as HTML files using a bundled template engine (Handlebars). PDF export via headless Chromium if available, otherwise HTML-only. | Medium |
| `read_pdf` | Extract text/tables from PDF | Low |
| `read_image` | Describe/analyze an image | Low |
| `summarize` | Summarize long text or multiple files | Low |
| `research` | Multi-step web research with source tracking | Medium |
| `run_command` | Execute shell command | High |
| `send_email_draft` | Compose email draft (via eMCP) | Medium |
| `calendar_query` | Check calendar (via eMCP) | Low |
| `request_user_input` | Ask the user a question | Low |

### 5.2 Features

**Research Mode**: User describes a research task, planner decomposes into parallel research sub-tasks, workers use web_search/web_fetch/summarize, output is a structured document with sources.

**Document Generation**: Markdown, plain text, CSV, and structured HTML output. Templates for common formats. Agents can create folder structures.

**Data Processing**: Read CSVs/spreadsheets, analyze, transform, output new files. Parse PDFs and extract structured data. Batch file operations.

**Integration via eMCPs**: Email, calendar, project management, cloud storage, communication — all via MCP connectors.

**Workspace View**: File browser focused on documents (not source code). Preview pane for generated documents. Research panel with sources and citations. Same agent trace / plan view as eCode.

---

## 6. Multi-Agent Orchestration

### 6.1 TaskGraph

```rust
pub struct TaskGraph {
    pub root_task_id: TaskId,
    pub nodes: HashMap<TaskId, TaskNode>,
    pub edges: Vec<(TaskId, TaskId)>,  // (dependency, dependent)
}

pub struct TaskNode {
    pub id: TaskId,
    pub description: String,
    pub status: TaskStatus,
    pub assigned_agent: Option<AgentId>,
    pub assigned_provider: Option<ProviderId>,
    pub tools_allowed: Vec<ToolName>,
    pub constraints: TaskConstraints,
    pub result: Option<TaskResult>,
    pub trace: Vec<TraceEntry>,
}

pub enum TaskStatus {
    Pending,
    Ready,
    Scheduled,
    Running,
    AwaitingReview,
    Complete,
    Failed { error: String, retries: u32 },
    Cancelled { reason: String },
}
```

### 6.2 Scheduler

Topological sort engine:
1. Identify all Ready tasks (dependencies met)
2. Respect max concurrency limit — configurable globally (default 4) and per-provider via `max_concurrent_sessions`. A single API provider can run multiple concurrent agents; a local llama.cpp provider defaults to 1 concurrent session.
3. Assign tasks to agents based on provider availability, per-provider concurrency limits, and task type
4. Monitor running tasks for completion, failure, or timeout
5. On completion: mark dependents as Ready, trigger next scheduling pass
6. On failure: retry up to N times, then mark dependents as blocked, notify user

### 6.3 Conflict Resolution

The primary isolation strategy is **in-memory file snapshots** — each agent works against a virtual filesystem overlay that captures its mutations without touching the real workspace until the task completes. This avoids the Windows-specific performance and file-locking issues with git worktrees.

Git worktrees are available as an opt-in strategy for long-running tasks where agents need to run commands (e.g., `cargo build`, `npm test`) that require real files on disk. The user enables this per-task in the Plan View.

**Resolution flow:**
- **Default (in-memory)**: agents produce diffs against the workspace baseline. On completion, diffs are merged sequentially (in dependency order). Overlapping edits to the same file region trigger a 3-way merge via `similar`.
- **Conflict escalation**: if auto-merge fails, the user sees a 3-way diff in the Diff Review panel and manually resolves.
- **Lock escalation**: if the planner detects that two independent sub-tasks will modify the same file (via static analysis of the task descriptions + project graph), it serializes those tasks instead of running them in parallel.

### 6.4 Agent Lifecycle

Each agent gets:
- A fresh provider session (isolated context)
- A scoped tool set (only the tools the task needs)
- A system prompt tailored to the task + project context
- Access to the project graph for codebase understanding
- A budget (max tokens, max tool calls, max time)

### 6.5 TaskGraph Persistence

The existing `OrchestrationEngine` CQRS model (Thread/Turn events) is replaced by a new event schema designed for multi-agent TaskGraph state. The `EventStore` (SQLite) is retained but with new event types:

```rust
pub enum TaskGraphEvent {
    GraphCreated { graph_id: TaskGraphId, root_task: TaskNode, user_prompt: String },
    SubTasksProposed { graph_id: TaskGraphId, nodes: Vec<TaskNode>, edges: Vec<(TaskId, TaskId)> },
    PlanApproved { graph_id: TaskGraphId },
    TaskScheduled { graph_id: TaskGraphId, task_id: TaskId, agent_id: AgentId, provider_id: ProviderId },
    TaskStarted { graph_id: TaskGraphId, task_id: TaskId },
    TraceAppended { graph_id: TaskGraphId, task_id: TaskId, entry: TraceEntry },
    FileMutationRecorded { graph_id: TaskGraphId, task_id: TaskId, path: String, diff: String },
    OversightRequested { graph_id: TaskGraphId, task_id: TaskId, request: OversightRequest },
    OversightResolved { graph_id: TaskGraphId, task_id: TaskId, decision: OversightDecision },
    TaskCompleted { graph_id: TaskGraphId, task_id: TaskId, result: TaskResult },
    TaskFailed { graph_id: TaskGraphId, task_id: TaskId, error: String },
    TaskCancelled { graph_id: TaskGraphId, task_id: TaskId, reason: String },
    GraphCompleted { graph_id: TaskGraphId },
}
```

On startup, the harness replays all `TaskGraphEvent`s to reconstruct active TaskGraphs. Completed graphs are archived but remain queryable for history.

---

## 7. eMCPs — Connector Ecosystem

eMCPs are MCP-compatible connectors that extend the tool registry. They follow the standard MCP protocol (stdio or SSE transport).

### 7.1 Structure

```
emcp-gmail/
├── manifest.json     { name, version, transport, tools, auth }
├── server.js|py|rs   (MCP server implementation)
└── README.md
```

### 7.2 Manifest

```json
{
  "name": "emcp-gmail",
  "version": "1.0.0",
  "transport": "stdio",
  "tools": [
    { "name": "search_emails", "description": "...", "risk_level": "low" },
    { "name": "send_draft", "description": "...", "risk_level": "medium" }
  ],
  "auth": { "type": "oauth2", "provider": "google" }
}
```

### 7.3 Integration

- eMCPs register tools into the harness ToolRegistry
- Tools become available to agents like built-in tools
- Auth managed by the harness (OAuth flows in Tauri UI)
- Installable from local directories or future marketplace

---

## 8. eSkills — Reusable Agent Capabilities

eSkills are packaged prompts + tool configurations that give agents specialized capabilities.

### 8.1 Structure

```
eskill-code-review/
├── manifest.json     { name, version, description, trigger_patterns }
├── system_prompt.md  (the skill's system prompt)
├── tools.json        (which tools this skill needs)
└── examples/         (few-shot examples)
```

### 8.2 Manifest

```json
{
  "name": "eskill-code-review",
  "version": "1.0.0",
  "description": "Reviews code for bugs, security issues, and style",
  "trigger_patterns": ["review", "code review", "check my code"],
  "required_tools": ["read_file", "search_files", "git_diff"],
  "mode": "ecode"
}
```

### 8.3 Integration

- Auto-triggered by substring matching against `trigger_patterns` (case-insensitive). Patterns are plain strings, not regex — simplicity over cleverness. The planner can also explicitly select an eSkill by name. Future: semantic similarity matching when a local embedding model is available.
- Planner can select eSkills for sub-tasks
- eSkills compose: a "full PR review" eSkill invokes code review + test coverage + security scan
- Bundled eSkills ship with eAgent; users and third parties can create their own

---

## 9. UI/UX Architecture

### 9.1 Layout

```
┌──────────────────────────────────────────────────────────────┐
│  Top Bar: eAgent  [eCode] [eWork]  Provider  Model  Settings│
├────────┬─────────────────────────────────────┬───────────────┤
│        │                                     │               │
│ Side   │         Main Canvas                 │  Right Panel  │
│ bar    │                                     │               │
│        │  Agent Trace / Chat                 │  Plan View    │
│ Thread │  (swim lanes for parallel agents)   │  (DAG)        │
│ List   │                                     │               │
│        │  Shows: reasoning, tool calls,      │  Diff Review  │
│ Project│  file mutations, terminal output    │               │
│ Tree   │  inline                             │  Project      │
│        │                                     │  Graph        │
│        │  Composer / Prompt Input             │               │
│        │  [Oversight: Full Auto]              │               │
├────────┴─────────────────────────────────────┴───────────────┤
│  Terminal Drawer (collapsible, multi-tab)                     │
│  [Agent-1: npm test] [Agent-2: cargo build] [User Terminal]  │
└──────────────────────────────────────────────────────────────┘
```

### 9.2 Key Components

**Mode Switcher**: Toggle eCode / eWork. Changes available tools, right panel config, system prompts, file tree filtering.

**Agent Trace**: Primary view. Color-coded lanes per agent. Typed entries (Thinking, ToolCall, FileChange, TerminalOutput, Question, Error). Inline diffs and terminal output (collapsible). Oversight requests as action cards. Streaming token-by-token.

**Composer**: Natural language input. Oversight mode selector. Provider/model selector. File/folder drag-and-drop for context. @-mentions for files, functions, previous tasks. Slash commands powered by eSkills.

**Plan View**: Interactive DAG. Color-coded nodes by status. Click to jump to trace. Drag to reorder. Right-click to add/remove/edit.

**Diff Review**: Tabbed per-file, grouped by sub-task. Side-by-side and unified views. Per-hunk approve/reject. Accept All / Review Each modes.

**Project Graph**: Visual dependency graph. Highlights files touched by current task. Searchable symbol index.

**Terminal Drawer**: Multi-tab. Agent-owned and user-owned tabs. Full xterm.js. Tabs can be popped out, resized, closed. Agent terminals read-only by default.

**Settings**: Multi-provider configuration (endpoint, API key, model, context window). Agent defaults (oversight, concurrency, budgets). eMCP management. eSkill browsing. llama.cpp tuning. Codex configuration. Appearance.

### 9.3 Frontend State Management

The current Zustand store (single-thread model with `bootstrap`, `snapshot`, `selectedThreadId`) is replaced with a multi-agent-aware state architecture:

**Tauri Event Contract (Rust → React):**

| Event | Payload | Purpose |
|-------|---------|---------|
| `task-graph-update` | `{ graphId, nodes, edges, statuses }` | TaskGraph DAG changes (new tasks, status transitions) |
| `agent-trace` | `{ graphId, taskId, agentId, entry: TraceEntry }` | Real-time agent execution trace entries |
| `file-mutation` | `{ graphId, taskId, agentId, path, diff }` | Agent file change for diff review |
| `oversight-request` | `{ graphId, taskId, requestId, action, context, riskLevel }` | Agent asking for approval |
| `terminal-event` | `{ terminalId, agentId?, data }` | Terminal output (agent-owned or user-owned) |
| `provider-status` | `{ providerId, status, models? }` | Provider availability changes |

**Zustand Store Structure:**

```typescript
interface EAgentStore {
  // Mode
  mode: 'ecode' | 'ework';
  // Active TaskGraphs
  activeGraphs: Map<TaskGraphId, TaskGraphState>;
  // Per-graph: nodes, edges, per-task traces, per-task diffs
  // Selected/focused state
  selectedGraphId: TaskGraphId | null;
  selectedTaskId: TaskId | null;
  // Provider state
  providers: Map<ProviderId, ProviderStatus>;
  // Terminals
  terminals: Map<TerminalId, TerminalState>;
  // Settings
  config: AppConfig;
}
```

Each Tauri event is handled by a Zustand action that incrementally updates the relevant slice. The store is never replaced wholesale — only the affected TaskGraph node, trace entry, or terminal buffer is updated. React components subscribe to fine-grained selectors to avoid unnecessary re-renders during high-frequency streaming.

### 9.4 Responsive Behavior

- Right panel collapses to tab bar on smaller windows
- Terminal drawer collapses to status bar
- Sidebar collapses to icons only
- All panels resizable with drag handles

---

## 10. Crate Architecture

### 10.1 New Structure

```
crates/
├── eagent-protocol/       — AgentMessage enum (all harness↔agent messages from section 2.1), TaskGraph types, TaskGraphEvent types. This crate defines the "wire format" between agents and the runtime.
├── eagent-runtime/        — Harness: scheduler, agent pool, conflict resolver
├── eagent-providers/      — Provider trait + implementations
│   ├── codex.rs           (migrated from ecode-core)
│   ├── llama_cpp.rs       (migrated from ecode-core)
│   ├── api_key.rs         (new — generic OpenAI-compatible)
│   └── mod.rs             (provider registry)
├── eagent-tools/          — Tool trait + built-in tools
│   ├── filesystem.rs      (read, write, edit, list, search)
│   ├── terminal.rs        (migrated from ecode-core)
│   ├── git.rs             (migrated from ecode-core)
│   ├── web.rs             (search, fetch — migrated)
│   ├── documents.rs       (new — eWork document tools)
│   └── mod.rs             (tool registry)
├── eagent-skills/         — eSkill loader, manifest parser, invocation
├── eagent-mcp/            — eMCP client, MCP protocol bridge
├── eagent-index/          — Project graph, symbol index, tree-sitter
├── eagent-persistence/    — Event store, config, session state
├── eagent-contracts/      — Shared non-protocol domain types: IDs, config structs, provider metadata, UI DTOs. Does NOT contain AgentMessage or TaskGraph types (those live in eagent-protocol).
├── eagent-desktop-app/    — Tauri bridge
└── eagent-planner/        — Planner agent logic, TaskGraph generation

apps/
└── desktop/
    ├── src/               — React/TypeScript UI
    └── src-tauri/         — Tauri commands
```

### 10.2 Migration From Existing Code

| Existing | Destination | Change Level |
|----------|-------------|--------------|
| `ecode-contracts` types | `eagent-contracts` + `eagent-protocol` | Rename + extend |
| `CodexManager` | `eagent-providers/codex.rs` | Wrap in Provider trait |
| `LlamaCppManager` | `eagent-providers/llama_cpp.rs` | Wrap in Provider trait |
| `LocalAgentExecutor` tools | `eagent-tools/*` | Extract into Tool trait impls |
| `OrchestrationEngine` | `eagent-runtime` | The CQRS command/event dispatch pattern moves to the runtime, which manages TaskGraph state transitions. The EventStore (persistence) is a dependency, not the destination. |
| `TerminalManager` | `eagent-tools/terminal.rs` | Expose via Tool trait |
| `ConfigManager` | `eagent-persistence/config.rs` | Extend for multi-provider |
| `EventStore` | `eagent-persistence/events.rs` | Keep, extend schema |
| `AppRuntime` + `UiAction` | `eagent-desktop-app` | Major rework for multi-agent |
| Tauri commands | `apps/desktop/src-tauri` | Extend significantly |
| React components | `apps/desktop/src` | Major rework — new layout |
| `ecode-gui` (egui) | **Deleted** | No longer needed |

### 10.3 Key Traits

```rust
// eagent-protocol — Agent trait
// The Agent is the runtime's unit of execution. It receives a task and communicates
// with the harness via an `AgentChannel` — a bidirectional message channel that
// carries all AgentMessage variants from section 2.1. This is how agents emit
// StatusUpdates, ToolRequests, and OversightRequests during execution (not just
// at the end).
pub trait Agent: Send + Sync {
    async fn execute(
        &self,
        task: TaskAssignment,
        channel: AgentChannel,  // bidirectional: agent sends AgentMessage, receives OversightResponse/ContextUpdate
        ctx: AgentContext,      // read-only: project graph, workspace root, config
    ) -> Result<TaskResult, AgentError>;
    fn cancel(&self);
}

// AgentChannel wraps an mpsc sender (agent → harness) and receiver (harness → agent)
pub struct AgentChannel {
    pub tx: mpsc::UnboundedSender<AgentMessage>,   // agent emits protocol messages
    pub rx: mpsc::UnboundedReceiver<AgentMessage>,  // agent receives oversight responses, context updates
}

// eagent-providers — Provider trait (see section 3.1 for full definition)
// Providers translate between the LLM's raw streaming output and AgentMessage.
// The Provider::send() return type is a Stream<Item = ProviderEvent> where
// ProviderEvent includes token deltas, tool call requests, and completion signals.
// The Agent implementation (in eagent-runtime) converts ProviderEvents into
// AgentMessages and sends them through the AgentChannel.

// eagent-tools — Tool trait
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn risk_level(&self) -> RiskLevel;
    fn parameter_schema(&self) -> serde_json::Value;
    async fn execute(&self, params: serde_json::Value, ctx: ToolContext)
        -> Result<ToolResult, ToolError>;
}
```

---

## 11. Implementation Phases

| Phase | Scope | Key Deliverables |
|-------|-------|------------------|
| 1. Scaffolding | Create new crate structure, define traits | Compiling empty crate shells with trait definitions |
| 2. Extract | Migrate existing code into new crates | Codex, llama.cpp, terminal, git, file tools behind trait interfaces |
| 3. Orchestration | Build multi-agent runtime | TaskGraph scheduler, AgentPool, ConflictResolver; single-agent first, then parallel |
| 4. Providers | Add ApiKeyProvider | Generic OpenAI-compatible endpoints; extended settings UI |
| 5a. UI Core | Rebuild React shell layout and state | New layout structure, Zustand store, Tauri event bridge, mode switcher, composer |
| 5b. UI Agent Traces | Agent trace panel with swim lanes | Streaming trace entries, typed rendering (Thinking, ToolCall, FileChange, etc.) |
| 5c. UI Plan & Diff | Plan view DAG and diff review | Interactive DAG visualization, per-hunk diff review, conflict merge UI |
| 5d. UI Terminal | Multi-tab terminal drawer | Agent-owned and user-owned terminals, xterm.js, pop-out support |
| 6. eWork | Add general-purpose mode | Document/research tools, eWork mode, eWork-specific UI |
| 7. Ecosystem | eMCP and eSkill support | Loaders, manifest parsers, settings UI for extensions |
| 8. Project Index | Codebase understanding | tree-sitter integration, symbol index, persistent project graph |
