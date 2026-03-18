import { deriveChangedFiles, derivePlanSteps, deriveTimelineEntries } from "~/session-logic";
import type { OrchestrationSnapshot, ProjectState, TerminalRecord, ThreadState } from "~/types";

export function selectThread(snapshot: OrchestrationSnapshot | null, threadId: string) {
  if (!snapshot) return null;
  return snapshot.readModel.threads[threadId] ?? null;
}

export function selectProjectForThread(
  snapshot: OrchestrationSnapshot | null,
  thread: ThreadState | null,
) {
  if (!snapshot || !thread) return null;
  return snapshot.readModel.projects[thread.project_id] ?? null;
}

export function terminalsForThread(snapshot: OrchestrationSnapshot | null, threadId: string) {
  if (!snapshot) return [];
  return snapshot.terminals.filter((terminal) => terminal.threadId === threadId);
}

export function buildThreadViewModel(
  snapshot: OrchestrationSnapshot | null,
  threadId: string,
) {
  const thread = selectThread(snapshot, threadId);
  const project = selectProjectForThread(snapshot, thread);
  return {
    thread,
    project,
    timeline: deriveTimelineEntries(thread),
    planSteps: derivePlanSteps(thread),
    changedFiles: deriveChangedFiles(thread),
    terminals: terminalsForThread(snapshot, threadId),
  };
}

export function currentTerminal(terminals: TerminalRecord[], threadId: string) {
  return terminals.find((terminal) => terminal.threadId === threadId) ?? terminals[0] ?? null;
}
