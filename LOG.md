# Implementation Log

## 2026-03-15
- Established project truth files for the `eCode` rewrite.
- Confirmed baseline test status before implementation: `cargo test` passed.
- Confirmed repo gaps: missing startup rebuild, missing Codex session establishment/resume flow, and missing thread-scoped settings persistence.
- Began implementation with the domain/core layer before UI work.
- Added startup rebuild coverage in the GUI crate and wired engine startup through `engine.rebuild()`.
- Added thread-scoped `ThreadSettings`, normalized provider session state, and runtime event storage in the shared orchestration domain.
- Reworked the GUI/background loop so thread selection restores per-thread drafts and per-thread model/runtime/provider settings.
- Hardened Codex send lifecycle by ensuring session spawn/resume before `send_turn`, persisting `TurnStarted`, and matching completions to provider turn IDs.
- Added baseline `llama.cpp` settings plus a managed local-model send path using `llama-server`'s OpenAI-compatible API.
- Re-ran verification: `cargo check` and `cargo test` passed; Trivy filesystem scan still fails at the MCP layer.
- Added session clearing/restart behavior when thread settings materially change, so Codex does not keep using stale session configuration.
- Added a bounded `llama.cpp` local-agent loop with workspace-rooted file tools, command execution in full-access mode, textual patch application, and DuckDuckGo-backed web search.
- Switched the app's storage resolution to a portable-first layout rooted at `eCode-data\` next to the executable when writable.
- Added Windows portable packaging assets: `scripts/build-portable.ps1` and `docs/portable-windows.md`.
- Verified the portable package script by producing `dist\eCode-portable` and `dist\eCode-portable-windows-x64.zip`.

## 2026-03-16
- Verified the local Codex environment end to end with `codex exec` and a live ignored smoke test that successfully spawns `codex app-server` and starts a thread from Rust.
- Hardened Windows Codex resolution to prefer spawnable `.cmd` and `.exe` shims instead of shell-only command resolution.
- Replaced the flat Codex settings block with a T3-style `Codex App Server` card, including binary-path overrides, `CODEX_HOME`, binary-source diagnostics, resolved-path display, and reset behavior.
- Made saved Codex override changes refresh the live manager for future sessions and fixed settings persistence to reuse the exact config path opened at startup.
- Hardened the local provider with request timeouts, redirect limits, HTML-only fetch checks, private-host guards, repo-search skip lists, file-size limits, and child-process cleanup on command timeout.
- Hardened `llama.cpp` supervision with request timeouts, stale-child detection, and stderr logging.
- Lowered default `llama.cpp` context size and made default thread count adapt to available CPU capacity to better fit lower-memory laptops.
- Cleared full-workspace `clippy -D warnings` issues to unblock release verification.
- Added a GUI-side Codex model catalog that refreshes from `model/list` and falls back to the current official Codex lineup when discovery is unavailable.
- Replaced the freeform Codex model text field with a dropdown for Codex threads while preserving `llama.cpp` model entry behavior.
- Removed the obsolete OpenAI API key config/UI path from the settings surface and persisted app config.
- Restyled the top bar, welcome state, thread header, transcript, composer, and Codex settings card to better match the current Codex and T3 Code visual direction.
- Updated the fallback Codex model lineup to `gpt-5.2`, `gpt-5.2-codex`, `gpt-5.3-codex`, `gpt-5.3-codex-spark`, and `gpt-5.4`, and made `gpt-5.4` the default fallback selection.
- Generated a PDF research brief with Codex and T3 Code UI/UX findings, embedded reference screenshots, and a handoff prompt for an AI UI designer under `output/pdf/`.
- Chose Tauri + React as the new desktop shell direction, keeping Rust for backend/runtime work and retiring `egui` as the long-term product UI path.
- Added an initial `apps/desktop` Tauri + React scaffold with a polished shell prototype, a live Rust `bootstrap` command, and workspace wiring for incremental migration.
- Wrote the migration plan at `docs/plans/2026-03-16-tauri-react-migration.md` and updated project state to treat the new shell as the active UI direction.
- Extracted the desktop runtime/controller into a new shared crate, `crates/ecode-desktop-app`, moving `UiAction`, shared `AppState`, background bootstrap, provider/session handling, and terminal event coordination out of the `egui` shell.
- Simplified `crates/ecode-gui/src/app.rs` into an `eframe` wrapper over the shared desktop runtime instead of owning the async backend loop directly.
- Replaced the Tauri bootstrap stub with live Rust-backed commands and events: the Tauri shell now loads real thread/bootstrap state and can create threads, select threads, and send messages through the shared runtime.
- Verified the new pieces with `cargo check -p ecode-desktop-app`, `cargo check -p ecode-desktop`, `cargo test`, `npm run build`, and targeted `cargo clippy -p ecode-desktop-app -p ecode-desktop --all-targets -- -D warnings`.

## 2026-03-17
- Restored `crates/ecode-desktop-app` compilation by adding the missing `serde` workspace dependency for shared terminal state and fixing the terminal-event borrow issue in `handle_terminal_event`.
- Added a regression test covering terminal creation, output append, exit handling, and repaint notification flow in the shared desktop runtime.
- Restored the missing React chat shell subcomponents under `apps/desktop/src/components/chat/` and corrected the terminal drawer to use the installed `@xterm/xterm` package import path so the Tauri UI builds again.
- Re-ran verification: `cargo test -p ecode-desktop-app --lib`, `cargo check -p ecode-desktop-app`, `npm run build`, and full `cargo test` all passed.
- Retried Trivy filesystem scans at the repo root and `apps/desktop`; both still fail at the MCP layer with `failed to scan project`, so security scan results remain unavailable.

## 2026-03-18
- Renamed project direction from eCode to **eAgent** — an agentic engineering platform by Eptesicus Laboratories.
- eAgent brand hierarchy: eAgent (platform), eCode (coding), eWork (general-purpose), eMCPs (connectors), eSkills (capabilities).
- Created design spec at `docs/superpowers/specs/2026-03-18-eagent-platform-design.md` covering protocol-first architecture, multi-agent parallel orchestration, eMCPs, eSkills, and 10 implementation phases.
- Phase 1 Scaffolding complete — created 10 new crates:
  - `eagent-protocol`: AgentMessage (7 variants), HarnessMessage (5 variants), TaskGraph, TaskNode, TaskStatus, TraceEntry, TaskGraphEvent (13 variants), Agent trait with AgentChannel.
  - `eagent-contracts`: AgentConfig (multi-provider), ProviderKind (Codex/LlamaCpp/ApiKey), ProviderEvent, OversightMode (three-tier: FullAutonomy/ApproveRisky/ApproveAll).
  - `eagent-tools`: Tool trait (dyn-compatible async), ToolRegistry, ToolContext, ToolDef.
  - `eagent-providers`: Provider trait (dyn-compatible async), ProviderRegistry, SessionHandle.
  - `eagent-persistence`, `eagent-runtime`, `eagent-planner`, `eagent-skills`, `eagent-mcp`, `eagent-index`: shell crates for future phases.
- Switched to GNU toolchain (`rust-toolchain.toml`) to avoid POSIX link.exe shadowing MSVC linker.
- Installed MinGW-w64 (WinLibs) via winget for the GNU toolchain.
- All 55 tests pass across the full workspace (existing + new crates).
- Phase 2 Extract: migrated all business logic from ecode-core into eagent-* crates behind trait interfaces.
  - `eagent-persistence`: EventStore (SQLite, WAL mode, 7 tests) + ConfigManager (AgentConfig, portable-first, 5 tests).
  - `eagent-tools`: 16 built-in tools — filesystem (7: list, read, read_multiple, search, write, edit, patch), web (2: search, fetch), git (4: status, diff, commit, branch), terminal (2: create, write) + TerminalManager. 64 tool tests total.
  - `eagent-providers`: LlamaCpp (streaming SSE, 17 tests) + Codex (JSON-RPC, process management, 18 tests) wrapped in Provider trait.
  - `register_builtin_tools()` function wires all 16 tools into a ToolRegistry.
  - All 176 tests pass across the full workspace.
- Phase 3 Orchestration: built multi-agent runtime in eagent-runtime.
  - `scheduler.rs`: TaskGraph DAG scheduler with Kahn's cycle detection, dependency tracking, concurrency-aware task selection.
  - `agent_pool.rs`: AgentPool that spawns worker agents via Provider trait, translates ProviderEvents to AgentMessages.
  - `conflict.rs`: ConflictResolver for file-level merge conflict detection.
  - `engine.rs`: RuntimeEngine tying it all together — submit, run scheduling loop, cancel, persist TaskGraphEvents. 32 tests.
- Phase 4 Providers: added ApiKeyProvider for generic OpenAI-compatible endpoints.
  - Extracted shared SSE parsing into `sse.rs` module (reused by LlamaCpp and ApiKey providers).
  - Supports streaming, tool/function calling, model discovery. 72 provider tests total.
- Phase 5a UI Core: reworked React shell for multi-agent state management.
  - Replaced single-thread Zustand store with EAgentStore (mode, activeGraphs, providers, terminals).
  - Added Tauri event bridge (6 event types), TopBar with mode switcher, Composer with oversight selector.
  - Updated Sidebar with TaskGraph listing and status indicators.
- Phase 6 eWork: added document tools (create_document, summarize, read_pdf stub, research). 85 tool tests total.
- Phase 7 Ecosystem: added eSkill manifest parser/loader (10 tests) and eMCP manifest parser/loader (12 tests).
- Phase 8 Project Index: added file tree indexing with language detection and project graph summary generation. 17 tests.
- Added SimplePlanner for single-task graph creation and 5 Tauri eAgent commands (submit, cancel, providers, oversight).
- Fixed critical self-critique issues: tool execution loop in RuntimeEngine, provider routing bug, SSE true streaming, UTF-8 safety in limit_output, session_status fallback.
- Final state: 292 tests passing, React build clean, 28 commits on master.

## 2026-03-20
- Conducted strategic brainstorm for eAgent platform direction: recursive agents, conversational+dashboard UI, all providers day one, bundled eMCPs/eSkills, Foundation First implementation strategy.
- Wrote design spec at `docs/superpowers/specs/2026-03-20-eagent-foundation-first-design.md`.
- **Phase 1 — Wire the Loop**: Connected eagent-runtime to Tauri shell end-to-end.
  - Added eagent-runtime, eagent-providers, eagent-tools, eagent-persistence, eagent-contracts as Tauri dependencies.
  - Created `EAgentState` in main.rs: instantiates EventStore, ToolRegistry (16 builtin tools), ProviderRegistry (ApiKeyProvider from env vars), RuntimeEngine. Spawns scheduling loop + event bridge.
  - Added eAgent event DTOs in dto.rs: `EAgentTaskGraphUpdatePayload`, `EAgentTaskNodePayload`, `EAgentAgentTracePayload`, `EAgentTraceEntryPayload` + `task_graph_to_update_payload()` conversion.
  - Added `eagent_event_bridge()` in events.rs: async loop translating RuntimeEvents into Tauri events (`eagent://task-graph-update`, `eagent://agent-trace`).
  - Wired all 5 eagent Tauri commands to real RuntimeEngine: submit (via planner + engine.submit()), cancel, get_providers, approve, deny.
  - Frontend event bridge already set up from prior work — zero React changes needed.
- **Phase 2 partial — Legacy cleanup**:
  - Deleted `crates/ecode-gui/` (dead code, zero references).
  - Audited ecode-core, ecode-desktop-app, ecode-contracts for migration readiness. Key finding: legacy crates stay for now — they power the existing chat UI (thread/turn model). eagent-runtime handles the new multi-agent flow. The two systems coexist without cross-dependencies.
- **Phase 3 — LLM-powered planner**:
  - Created `LlmPlanner` in `crates/eagent-planner/src/llm.rs`: calls a provider with a planner system prompt, receives JSON task decomposition, parses into TaskGraph DAG.
  - Handles markdown code blocks in LLM responses, unknown dependencies, empty tool lists (defaults to common set).
  - Falls back to SimplePlanner if LLM response can't be parsed.
  - Wired into Tauri: `eagent_submit_task` uses LlmPlanner when provider available, SimplePlanner otherwise.
  - 10 new tests covering JSON parsing, dependency edges, code block extraction, fallback behavior.
- **Critical fix — Agentic tool loop**: Rewrote `AgentPool::spawn_agent` with a full multi-turn tool loop. Previously agents did a single provider call; now they: call provider → collect tool requests → execute tools locally → feed results back → call provider again → repeat up to 20 rounds. Moved tool execution from RuntimeEngine into the agent worker task. Engine now just relays messages to UI.
- **Protocol fix — ProviderMessage tool fields**: Extended `ProviderMessage` with `tool_call_id` (for tool results) and `tool_calls` (for assistant messages with function calls). Updated `ApiKeyProvider::build_messages_payload()` to emit correct OpenAI format: tool messages include top-level `tool_call_id`, assistant messages include `tool_calls` array with `id`, `type: "function"`, `function: {name, arguments}`. Without this, multi-turn tool conversations would be rejected by the API.
- **UI — AgentTraceView component**: Created `AgentTraceView.tsx` React component that reads from the eAgent store and renders task nodes with status icons, color-coded trace entries (thinking/tool-call/tool-result/error/status), expandable detail sections, and error display. Wired into `_chat.index.tsx` — shows the trace view when a graph is selected, welcome screen when not.
- **Critical bug fixes from code review** (5 issues):
  1. Max-round hit in agent loop now returns failure (was silently succeeding with truncated work).
  2. `AwaitingReview` status included in active tasks check (was causing permanent deadlock).
  3. Completed graphs evicted from `RuntimeEngine::graphs` (was emitting `GraphCompleted` every 100ms forever).
  4. Task submission requires an open project (was defaulting to `"."` or `""`, breaking sandbox).
  5. Removed `run_command` from planner defaults (not registered as a tool, was causing persistent LLM errors).
- Current state: 302 tests passing, React build clean. Full end-to-end pipeline with UI: user prompt → LLM planner → TaskGraph DAG → RuntimeEngine → AgentPool → Provider → multi-turn tool loop → Tauri events → React store → AgentTraceView renders execution trace.
