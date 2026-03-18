# eAgent Project Brief

## Goal
Build **eAgent** — an agentic engineering platform by Eptesicus Laboratories. eAgent is a desktop application where developers and knowledge workers orchestrate AI agents that perform complex, multi-step tasks autonomously. It includes **eCode** (coding workstation) and **eWork** (general-purpose workstation), with support for local models, API keys, and Codex as interchangeable providers.

## Current Focus
- Phase 1 (Scaffolding): New `eagent-*` crate architecture with protocol types, traits, and registries.
- Next: Phase 2 (Extract) — migrate existing ecode-core providers, tools, and persistence into new crates behind trait interfaces.
- Design spec: `docs/superpowers/specs/2026-03-18-eagent-platform-design.md`

## Architecture
- **Protocol-first**: All agent↔harness communication uses `eagent-protocol` types (AgentMessage, TaskGraph, TaskGraphEvent).
- **Multi-agent orchestration**: Planner decomposes tasks into a DAG, parallel Worker Agents execute sub-tasks concurrently with conflict resolution.
- **Provider-agnostic**: Codex, llama.cpp, and any OpenAI-compatible API behind a shared `Provider` trait.
- **Rust backend + Tauri/React frontend**: Rust for orchestration, providers, persistence, terminal. React for the product UI shell.

## Constraints
- Keep Rust as the backend/runtime layer for orchestration, provider integration, persistence, and terminal work.
- Favor evidence-driven, minimal changes over speculative abstractions.
- Preserve security boundaries around local file, command, and web-search tools.
- The machine has limited RAM, so avoid heavy background services and oversized caches.
- Windows-first portable distribution.

## Brand Hierarchy
- **eAgent** — the platform/harness
- **eCode** — coding workstation mode
- **eWork** — general-purpose workstation mode
- **eMCPs** — MCP-compatible connectors
- **eSkills** — reusable agent capabilities
