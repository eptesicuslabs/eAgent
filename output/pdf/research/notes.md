# UI Research Notes

## Sources
- OpenAI Codex product page: https://openai.com/codex/
- OpenAI Codex developer docs: https://developers.openai.com/codex/
- OpenAI models docs: https://platform.openai.com/docs/models
- T3 Code landing page: https://t3.codes/
- T3 Code README: https://github.com/pingdotgg/t3code/blob/main/README.md

## Codex UI/UX
- Information architecture: a workspace-oriented shell with a persistent navigation rail, an active task/thread list, and a large central work canvas. The marketing page repeatedly frames Codex as a "command center" for multi-agent coding.
- Navigation: emphasis on workspaces, projects, tasks, and cross-surface continuity between app, editor, and terminal rather than a single chat-only stack.
- Density: moderate-to-high information density. The UI shows progress logs, tool status, file changes, approvals, branch/commit surfaces, and task chips without collapsing into a sparse chatbot.
- Transcript/composer behavior: the main canvas behaves like an execution transcript more than a messaging feed. Inputs sit at the bottom with model/context controls nearby, while the stream above mixes agent reasoning summaries, tool steps, and outcomes.
- Status affordances: strong use of step labels, turn states, approvals, commit summaries, and change-review surfaces. Codex makes system state legible through structured cards rather than hidden modal flows.
- Visual language: soft neutrals, editorial whitespace, rounded cards, light surfaces for settings/review flows, and product screenshots with clear hierarchy. The visual tone is calm and precise, not neon or playful.
- Interaction model: the product language centers on parallelism, background work, automations, and skills. UX favors orchestration and trust-building over pure conversational novelty.

## T3 Code UI/UX
- Information architecture: minimal shell with a compact left rail for threads/projects and a dominant central transcript. The README describes it as a minimal web GUI for coding agents and the landing page reinforces a single focused shell.
- Navigation: extremely lean. Primary navigation is thread/project selection on the left, transcript in the middle, and a compact top action bar.
- Density: high transcript density with narrow gutters, dense activity rows, and visible tool-call blocks. It feels terminal-adjacent rather than document-oriented.
- Transcript/composer behavior: bottom composer stays anchored and paired with compact runtime controls. The transcript favors rapid scanability over prose comfort.
- Status affordances: inline work indicators, completion markers, active thread highlighting, action buttons in the transcript header, and visible run-state signals.
- Visual language: very dark background, subdued chrome, soft borders, white primary typography, muted gray secondary copy, and a slight glass/console feel. The UI is intentionally minimal and lets the transcript carry most of the visual weight.
- Interaction model: optimized for staying in flow with agent execution, not for complex configuration. It feels faster, more operational, and more stripped down than Codex.

## Shared patterns worth borrowing
- A persistent left-side thread/workspace rail.
- A strong central transcript with visible tool/activity blocks.
- A bottom-anchored composer with nearby model/runtime controls.
- Visible system state through pills, chips, and inline statuses instead of hidden dialogs.
- Clear separation between conversation, review, and settings surfaces.

## eCode-specific refinement targets
- Shell layout: keep the left sidebar and right panel, but make the information hierarchy more explicit: workspace > thread > active run > output.
- Sidebar: tighten spacing, improve thread metadata, and make active/completed/waiting states more legible.
- Thread header: treat it as a compact control deck with current provider, model, runtime mode, and live session state.
- Transcript: distinguish user asks, assistant output, tool activity, approvals, and errors more clearly.
- Composer: keep it pinned, simplify its mental model, and make the primary action feel stronger.
- Settings: preserve the lighter Codex card pattern, but reduce generic form clutter and group by operational intent.
- Right panel: the plan/diff/git side still reads as placeholder compared with both references and should be upgraded into a more useful review surface.
