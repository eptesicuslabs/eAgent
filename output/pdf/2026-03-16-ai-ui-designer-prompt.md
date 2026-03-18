# AI UI Designer Prompt for eCode

You are redesigning eCode, a Rust-native desktop coding assistant built with egui. Your job is to analyze the current product deeply, absorb the strongest UI and UX patterns from Codex and T3 Code, and refine the whole program into a coherent, production-grade desktop experience.

## Context
- eCode already has a working shell with a top bar, left sidebar, center chat/workspace panel, right plan panel, settings modal, status bar, and optional terminal.
- The product is local-first and desktop-native. Keep that identity.
- The current app recently moved toward a Codex/T3 direction, but it still needs a more complete systems-level refinement pass.
- Codex model selection is now dropdown-based and API key UI is intentionally removed. Do not reintroduce API key entry.

## Reference taste to absorb
### From Codex
- Treat the app like a command center for real engineering work, not a generic chatbot.
- Preserve strong progress reporting, review/change visibility, and explicit execution states.
- Use calmer cards, clearer sectioning, and more trustworthy operational affordances.
- Make multi-step agent workflows feel manageable, not overwhelming.

### From T3 Code
- Preserve a dense, high-focus dark shell with a strong left rail and a transcript-first center pane.
- Keep chrome minimal and useful.
- Make the composer and current thread feel like the natural center of gravity.
- Use compact inline controls and subtle separators instead of bulky forms.

## What to redesign
- Top bar
- Sidebar / thread navigation
- Main thread header
- Transcript / tool-call blocks / approvals / waiting states
- Composer
- Right-side plan or context panel
- Settings modal
- Status bar
- Terminal panel
- Empty states and onboarding moments

## Design goals
- Make the whole app feel like one intentional product, not a collection of panels.
- Improve information hierarchy and reduce ambiguity about status, next action, and ownership.
- Increase perceived trust and quality.
- Keep the app fast, local, and desktop-native.
- Preserve accessibility, legibility, and responsive behavior for smaller desktop windows.

## Non-negotiable constraints
- No API key entry UI.
- Codex model choice must remain a dropdown-based flow.
- Do not turn the app into a browser-clone or marketing mockup.
- Do not add decorative complexity that hurts scan speed.
- Respect the current Rust/egui implementation reality and propose changes that can plausibly be built in this codebase.

## Working method
1. Audit each existing surface.
2. Identify the strongest UI/UX patterns worth keeping.
3. Identify the weakest or most incoherent areas.
4. Propose a unified visual direction: layout logic, density model, spacing rhythm, color strategy, card treatment, status chips, and composer behavior.
5. Refine the whole program screen by screen, not just the main chat view.

## Deliverables
- A thorough UI/UX analysis of the current eCode shell.
- A refined design direction statement.
- Concrete redesign recommendations for every major surface.
- Suggested component-level changes and interaction rules.
- A prioritized implementation plan for the redesign.

## Required final report format
1. Overall design direction
2. What changed in each surface
3. Why each change improves UX
4. Which Codex patterns were borrowed
5. Which T3 Code patterns were borrowed
6. What was intentionally not copied
7. Risks or tradeoffs
8. Next implementation priorities

Be opinionated, specific, and exhaustive. I do not want a shallow 'make it cleaner' pass. I want a designer who understands the product taste, internalizes it, and refines the entire program like a real operating environment for coding agents.
