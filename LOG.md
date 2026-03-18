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
