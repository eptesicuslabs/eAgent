# eCode Tauri + React Migration Implementation Plan
> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.
**Goal:** Replace the `egui` desktop shell with a Tauri-hosted React/TypeScript shell while preserving the existing Rust core and contracts crates.
**Architecture:** Keep `ecode-core` and `ecode-contracts` as the durable backend/domain layer. Add a new Tauri app under `apps/desktop` that renders a React shell and communicates with Rust commands/events in `apps/desktop/src-tauri`.
**Tech Stack:** Rust, Tauri 2, React 18, TypeScript, Vite, custom CSS
---

### Task 1: Introduce the new desktop app shell
**Files:**
- Modify: `Cargo.toml`
- Modify: `.gitignore`
- Create: `apps/desktop/package.json`
- Create: `apps/desktop/tsconfig.json`
- Create: `apps/desktop/tsconfig.node.json`
- Create: `apps/desktop/vite.config.ts`
- Create: `apps/desktop/index.html`
- Create: `apps/desktop/src/main.tsx`
- Create: `apps/desktop/src/App.tsx`
- Create: `apps/desktop/src/styles.css`
- Create: `apps/desktop/src/vite-env.d.ts`
- Create: `apps/desktop/src-tauri/Cargo.toml`
- Create: `apps/desktop/src-tauri/build.rs`
- Create: `apps/desktop/src-tauri/tauri.conf.json`
- Create: `apps/desktop/src-tauri/capabilities/default.json`
- Create: `apps/desktop/src-tauri/src/main.rs`
**Step 1: Create the new Tauri and frontend scaffold**
Implement the frontend and Rust shell with a minimal live command so the app proves the new architecture works.
**Step 2: Install frontend dependencies**
Run: `npm install`
Expected: dependencies installed successfully in `apps/desktop`
**Step 3: Verify the frontend build**
Run: `npm run build`
Expected: Vite emits `dist/` with no TypeScript errors
**Step 4: Verify the Rust backend crate**
Run: `cargo check -p ecode-desktop`
Expected: the Tauri backend compiles inside the workspace

### Task 2: Record the architectural decision
**Files:**
- Modify: `PROJECT_BRIEF.md`
- Modify: `STATE.yaml`
- Modify: `LOG.md`
**Step 1: Update project records**
Record that `egui` is no longer the target shell and that Tauri + React is the chosen desktop surface while Rust remains the backend/runtime.

### Task 3: Define the migration seam
**Files:**
- Modify: `docs/plans/2026-03-16-tauri-react-migration.md`
- Future modify: `apps/desktop/src-tauri/src/main.rs`
- Future create: `apps/desktop/src-tauri/src/backend/*.rs`
**Step 1: Lock the seam**
The Tauri layer must own windowing, layout, and browser-facing state. Existing orchestration, provider, persistence, and terminal logic must remain in `ecode-core` / `ecode-contracts`.
**Step 2: Sequence the next move**
Next implementation pass should add typed Tauri commands for bootstrap state, threads, projects, transcript events, and send-message actions before porting every legacy feature.
