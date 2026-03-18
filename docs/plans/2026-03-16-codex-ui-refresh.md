## Codex UI Refresh

### Goal
- Align the native eCode shell more closely with the current Codex and T3 Code product direction.
- Remove the obsolete API key path from settings and persisted config.
- Replace the freeform Codex model field with a dropdown backed by the current Codex model catalog.

### Acceptance Criteria
- Settings no longer show an OpenAI API key section and the config no longer persists `openai_api_key`.
- Codex threads render a dropdown of available models instead of a freeform text field.
- The dropdown prefers a live `model/list` result from Codex and falls back to the current official Codex model set if discovery fails.
- New Codex threads default to the current preferred Codex model rather than the old `o4-mini` placeholder.
- The top bar, thread header, transcript, and composer better reflect the Codex/T3 visual direction without changing core workflow layout.

### Implementation Notes
- Keep the model catalog in GUI state for a minimal change set.
- Refresh the catalog during the existing Codex health check path.
- Use an ephemeral Codex app-server session for model discovery, then tear it down immediately.
- Preserve existing session restart behavior when provider/model/runtime settings change.
