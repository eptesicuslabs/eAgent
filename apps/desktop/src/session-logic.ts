import type {
  PendingApproval,
  PendingUserInput,
  ProviderRuntimeEvent,
  ThreadError,
  ThreadState,
  TurnState,
} from "./types";

export type TimelineEntry =
  | {
      id: string;
      kind: "prompt";
      createdAt: string;
      title: string;
      body: string;
    }
  | {
      id: string;
      kind: "message";
      createdAt: string;
      role: "assistant" | "system" | "user";
      body: string;
    }
  | {
      id: string;
      kind: "event";
      createdAt: string;
      title: string;
      body: string;
    }
  | {
      id: string;
      kind: "approval";
      createdAt: string;
      title: string;
      body: string;
    }
  | {
      id: string;
      kind: "input";
      createdAt: string;
      title: string;
      body: string;
    }
  | {
      id: string;
      kind: "error";
      createdAt: string;
      title: string;
      body: string;
    };

function formatUnknown(value: unknown) {
  if (value == null) return "No details";
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function timelineForTurn(turn: TurnState, runtimeEvents: ProviderRuntimeEvent[]): TimelineEntry[] {
  const entries: TimelineEntry[] = [
    {
      id: `prompt-${turn.id}`,
      kind: "prompt",
      createdAt: turn.started_at,
      title: turn.status,
      body: turn.input,
    },
  ];

  for (const message of turn.messages) {
    entries.push({
      id: message.item_id,
      kind: "message",
      createdAt: message.timestamp,
      role: message.role,
      body: message.content,
    });
  }

  for (const event of runtimeEvents.filter((item) => item.turn_id === turn.id)) {
    entries.push({
      id: `event-${turn.id}-${event.timestamp}-${event.item_id ?? event.summary ?? "runtime"}`,
      kind: "event",
      createdAt: event.timestamp,
      title: event.summary ?? event.event_type,
      body: formatUnknown(event.data),
    });
  }

  return entries;
}

function approvalsToEntries(approvals: PendingApproval[]): TimelineEntry[] {
  return approvals.map((approval) => ({
    id: `approval-${approval.id}`,
    kind: "approval",
    createdAt: approval.requested_at,
    title: approval.kind.replaceAll("_", " "),
    body: formatUnknown(approval.details),
  }));
}

function inputsToEntries(inputs: PendingUserInput[]): TimelineEntry[] {
  return inputs.map((input) => ({
    id: `input-${input.id}`,
    kind: "input",
    createdAt: input.requested_at,
    title: "user input requested",
    body: formatUnknown(input.questions),
  }));
}

function errorsToEntries(errors: ThreadError[]): TimelineEntry[] {
  return errors.map((error) => ({
    id: `error-${error.timestamp}`,
    kind: "error",
    createdAt: error.timestamp,
    title: error.will_retry ? "runtime error, retrying" : "runtime error",
    body: error.message,
  }));
}

export function deriveTimelineEntries(thread: ThreadState | null) {
  if (!thread) return [];
  const entries = [
    ...thread.turns.flatMap((turn) => timelineForTurn(turn, thread.runtime_events)),
    ...approvalsToEntries(Object.values(thread.pending_approvals)),
    ...inputsToEntries(Object.values(thread.pending_inputs)),
    ...errorsToEntries(thread.errors),
  ];

  return entries.sort((left, right) => left.createdAt.localeCompare(right.createdAt));
}

export function derivePlanSteps(thread: ThreadState | null) {
  if (!thread) return [];
  return thread.turns.map((turn, index) => ({
    id: turn.id,
    label: `Step ${index + 1}`,
    detail: turn.input,
    status: turn.status,
  }));
}

export function deriveChangedFiles(thread: ThreadState | null) {
  if (!thread) return [];
  const paths = new Set<string>();
  for (const event of thread.runtime_events) {
    const payload = formatUnknown(event.data);
    const matches = payload.matchAll(/[A-Za-z0-9_\-/\\.]+\.[A-Za-z0-9]+/g);
    for (const match of matches) {
      if (match[0]) paths.add(match[0]);
    }
  }
  return [...paths].sort();
}
