import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useStore } from "./store";
import type {
  AgentTracePayload,
  FileMutationPayload,
  OversightRequestPayload,
  ProviderStatusPayload,
  TaskGraphUpdatePayload,
  TerminalEventPayload,
} from "./types";

/**
 * Sets up Tauri event listeners for the eAgent platform events.
 * Routes each event to the appropriate Zustand store action for
 * incremental state updates.
 *
 * Returns a cleanup function that unsubscribes all listeners.
 */
export async function setupEAgentEventBridge(): Promise<() => void> {
  const cleanups: UnlistenFn[] = [];

  try {
    // task-graph-update: TaskGraph DAG changes (new tasks, status transitions)
    cleanups.push(
      await listen<TaskGraphUpdatePayload>("eagent://task-graph-update", (event) => {
        useStore.getState().onTaskGraphUpdate(event.payload);
      }),
    );
  } catch {
    console.warn("[eAgent] Could not subscribe to task-graph-update events");
  }

  try {
    // agent-trace: Real-time agent execution trace entries
    cleanups.push(
      await listen<AgentTracePayload>("eagent://agent-trace", (event) => {
        useStore.getState().onAgentTrace(event.payload);
      }),
    );
  } catch {
    console.warn("[eAgent] Could not subscribe to agent-trace events");
  }

  try {
    // file-mutation: Agent file changes for diff review
    cleanups.push(
      await listen<FileMutationPayload>("eagent://file-mutation", (event) => {
        useStore.getState().onFileMutation(event.payload);
      }),
    );
  } catch {
    console.warn("[eAgent] Could not subscribe to file-mutation events");
  }

  try {
    // oversight-request: Agent asking for human approval
    cleanups.push(
      await listen<OversightRequestPayload>("eagent://oversight-request", (event) => {
        useStore.getState().onOversightRequest(event.payload);
      }),
    );
  } catch {
    console.warn("[eAgent] Could not subscribe to oversight-request events");
  }

  try {
    // terminal-event: Terminal output (agent-owned or user-owned)
    cleanups.push(
      await listen<TerminalEventPayload>("eagent://terminal-event", (event) => {
        useStore.getState().onTerminalEvent(event.payload);
      }),
    );
  } catch {
    console.warn("[eAgent] Could not subscribe to terminal-event events");
  }

  try {
    // provider-status: Provider availability changes
    cleanups.push(
      await listen<ProviderStatusPayload>("eagent://provider-status", (event) => {
        useStore.getState().onProviderStatus(event.payload);
      }),
    );
  } catch {
    console.warn("[eAgent] Could not subscribe to provider-status events");
  }

  return () => {
    for (const cleanup of cleanups) {
      cleanup();
    }
  };
}
