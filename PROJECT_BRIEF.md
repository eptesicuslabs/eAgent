# eCode Project Brief

## Goal
Build a desktop coding assistant with a T3 Code-style information architecture, robust Codex CLI integration, thread-scoped conversation settings, and a managed `llama.cpp` local-model path, using Rust for the backend/runtime and a Tauri-hosted React shell for the product UI.

## Current Focus
- Move more shell features from the legacy `egui` frontend into the new shared desktop runtime and Tauri surface.
- Expose richer typed Tauri DTOs for transcript content, settings, terminal state, and right-panel review surfaces.
- Preserve the dropdown-only Codex model flow and keep API key UI removed.
- Harden local command execution, local web search, and `llama.cpp` process supervision for portable Windows use.
- Keep the Windows build portable so users can run a downloaded `exe` directly.

## Constraints
- Keep Rust as the backend/runtime layer for orchestration, provider integration, persistence, and terminal work.
- Favor evidence-driven, minimal changes over speculative abstractions.
- Preserve security boundaries around local file, command, and web-search tools.
- The machine has limited RAM, so avoid heavy background services and oversized caches.

## Planning Decisions
- Move the shell to Tauri + React because it is more suitable for polished product UI than `egui` while preserving a Rust backend.
- Use T3 Code's information architecture, not its Electron stack.
- Persist thread settings as domain state rather than UI-only drafts.
- Treat Codex and `llama.cpp` as providers behind a shared runtime event model.
- Implement `llama.cpp` through a managed `llama-server` process configured in settings.
- Support the core local agent loop plus built-in web search for the local provider.
- Resolve app storage relative to the executable when the executable directory is writable, so the shipped Windows build behaves portably by default.
- Prefer spawnable Codex executable paths on Windows (`.cmd` / `.exe`) over shell-only resolutions.
- Treat Codex model choice as live provider metadata first, with an official fallback list only when discovery is unavailable.
- Hide direct API key configuration until a real product requirement justifies reintroducing it.
- Keep the existing `ecode-core` and `ecode-contracts` crates as the migration seam and replace the current shell incrementally rather than rewriting the backend.
- Extract the desktop runtime/controller into a toolkit-neutral crate so `egui` and Tauri can share one backend loop during migration.
