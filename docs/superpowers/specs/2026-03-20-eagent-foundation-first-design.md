# eAgent Foundation First — Design Specification

> **Product**: eAgent by Eptesicus Laboratories
> **Date**: 2026-03-20
> **Status**: Approved
> **Scope**: Wiring eagent-runtime to Tauri, then layering depth through 7 phases

---

## 1. Vision Update

eAgent is a **mission control for AI work** — not just a coding tool. Users orchestrate recursive agent hierarchies through a conversational interface with an expanding mission dashboard.

### Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Agent depth | Recursive (unbounded with budget) | Maximum power; any agent can spawn children |
| UI paradigm | Conversational + expanding dashboard | Simple for vibe coders, observable for power users |
| Provider strategy | All day one (API, local, Codex) | Maximum flexibility from launch |
| Ecosystem | Bundled, no marketplace | Users build their own eMCPs/eSkills |
| eWork priority | Equal to eCode | Both are first-class workstation modes |
| Onboarding | 30-second setup wizard | Balance between instant gratification and understanding |
| Performance | Core brand differentiator | <1s startup, 8+ concurrent agents, fast time-to-result |

### Three UI States

1. **Conversational**: Clean chat, no dashboard. User talks to root agent.
2. **Agents Active (Compact)**: Inline dashboard in chat flow, one row per agent.
3. **Expanded Dashboard**: Full mission control with execution tree + agent traces. Pop-out capable.

---

## 2. Implementation Strategy: Foundation First

Get one prompt flowing end-to-end before adding depth.

| Phase | Goal | Key Deliverable |
|-------|------|-----------------|
| 1. Wire the Loop | Single agent end-to-end | Prompt → RuntimeEngine → Provider → Tool → UI trace |
| 2. Kill Legacy | Single architecture | Delete ecode-* crates |
| 3. LLM Planner | Multi-agent parallel | LLM decomposes prompts into DAGs |
| 4. Recursive Spawning | Agent trees | Any agent spawns children |
| 5. UI Polish | Production UX | Conversational + dashboard + setup wizard |
| 6. eWork + Ecosystem | Full platform | Document tools, bundled eMCPs/eSkills |
| 7. Performance | Brand differentiator | Startup, throughput, memory optimization |

---

## 3. Phase 1 Architecture

### Integration Points

```
┌─────────────────────────────────────────────────────┐
│ React Frontend (no changes needed)                   │
│  eventBridge.ts → store.ts (Zustand)                │
│  Listens: eagent://task-graph-update                │
│           eagent://agent-trace                       │
│           eagent://file-mutation                     │
│           eagent://oversight-request                 │
│           eagent://terminal-event                    │
│           eagent://provider-status                   │
└─────────────┬───────────────────────────────────────┘
              │ Tauri Events
┌─────────────▼───────────────────────────────────────┐
│ Tauri Shell (main.rs)                                │
│  EAgentState {                                       │
│    engine: Arc<RuntimeEngine>                        │
│    provider_registry: Arc<ProviderRegistry>          │
│    tool_registry: Arc<ToolRegistry>                  │
│  }                                                   │
│                                                      │
│  eagent_event_bridge: RuntimeEvent → Tauri emit      │
│  eagent_submit_task: plan + engine.submit()          │
│  eagent_cancel_graph: engine.cancel_graph()          │
│  eagent_get_providers: registry.names()              │
└─────────────┬───────────────────────────────────────┘
              │ Rust async
┌─────────────▼───────────────────────────────────────┐
│ eagent-runtime (RuntimeEngine)                       │
│  Scheduler → AgentPool → Provider → Tool execution   │
│  Emits RuntimeEvent to mpsc channel                  │
│  Persists TaskGraphEvent to EventStore               │
└─────────────────────────────────────────────────────┘
```

### Files Modified (Phase 1)

| File | Change |
|------|--------|
| `apps/desktop/src-tauri/Cargo.toml` | Add eagent-* dependencies |
| `apps/desktop/src-tauri/src/main.rs` | Add EAgentState, instantiate stack, spawn loops |
| `apps/desktop/src-tauri/src/dto.rs` | Add eAgent event payload structs |
| `apps/desktop/src-tauri/src/events.rs` | Add eagent_event_bridge function |
| `apps/desktop/src-tauri/src/commands/eagent.rs` | Wire commands to RuntimeEngine |

---

## 4. Recursive Agent Architecture (Phase 4)

### Tree-Shaped Execution

```
TaskNode {
    id: TaskId,
    parent_task_id: Option<TaskId>,  // NEW: enables tree structure
    description: String,
    status: TaskStatus,
    assigned_agent: Option<AgentId>,
    assigned_provider: Option<ProviderId>,
    tools_allowed: Vec<ToolName>,
    constraints: TaskConstraints,
    result: Option<TaskResult>,
    trace: Vec<TraceEntry>,
    budget: Budget,  // NEW: propagated from parent
}
```

### Budget Propagation

- Root task gets full budget (tokens, tool calls, time, depth)
- When an agent spawns children, parent's remaining budget splits
- `max_depth` (default 5) prevents infinite recursion
- `max_children` (default 10) per agent prevents explosion

### Dynamic Graph Growth

- `RuntimeEngine::run()` handles `SubTaskProposal` from any running agent
- New nodes and edges added to the active TaskGraph
- Scheduler immediately considers new Ready tasks
- Parent task status changes to `AwaitingChildren` until all children complete
