import { FolderKanban, GitBranch, PanelsTopLeft } from "lucide-react";
import { formatRelativeTime, titleFromPath } from "~/lib/utils";
import type { ProjectState, ThreadState } from "~/types";

function sessionTone(status: ThreadState["session"] extends infer T
  ? T extends { status: infer S }
    ? S
    : never
  : never) {
  switch (status) {
    case "running":
      return "border-emerald-500/30 bg-emerald-500/10 text-emerald-200";
    case "waiting":
      return "border-amber-500/30 bg-amber-500/10 text-amber-200";
    case "error":
      return "border-rose-500/30 bg-rose-500/10 text-rose-200";
    case "ready":
      return "border-sky-500/30 bg-sky-500/10 text-sky-200";
    default:
      return "border-border/70 bg-background/70 text-muted-foreground";
  }
}

export function ChatHeader(props: {
  thread: ThreadState;
  project: ProjectState | null;
  showPlan: boolean;
  showDiff: boolean;
  onTogglePlan: () => void;
  onToggleDiff: () => void;
}) {
  const sessionStatus = props.thread.session?.status ?? "stopped";

  return (
    <header className="drag-region flex min-h-18 items-center justify-between gap-4 border-b border-border/70 bg-[linear-gradient(180deg,color-mix(in_srgb,var(--card)_92%,black)_0%,color-mix(in_srgb,var(--background)_96%,black)_100%)] px-6 py-4">
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-2">
          <span className="inline-flex items-center gap-2 rounded-full border border-border/70 bg-background/70 px-3 py-1 text-[10px] font-semibold tracking-[0.18em] text-muted-foreground uppercase">
            <PanelsTopLeft className="size-3.5" />
            Transcript shell
          </span>
          <span
            className={`inline-flex items-center rounded-full border px-3 py-1 text-[10px] font-semibold tracking-[0.18em] uppercase ${sessionTone(sessionStatus)}`}
          >
            {sessionStatus}
          </span>
        </div>

        <h1 className="mt-3 truncate text-2xl font-semibold tracking-[-0.04em] text-foreground">
          {props.thread.name}
        </h1>

        <div className="mt-2 flex flex-wrap items-center gap-x-4 gap-y-2 text-xs text-muted-foreground">
          <span className="inline-flex items-center gap-1.5">
            <FolderKanban className="size-3.5" />
            {props.project?.name ?? titleFromPath(props.project?.path) ?? "Workspace"}
          </span>
          <span className="inline-flex items-center gap-1.5">
            <GitBranch className="size-3.5" />
            {props.thread.settings.provider} / {props.thread.settings.model}
          </span>
          <span>Updated {formatRelativeTime(props.thread.updated_at)}</span>
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-2">
        <button
          className={`rounded-full border px-3 py-2 text-xs font-medium transition ${
            props.showPlan
              ? "border-border bg-foreground text-background"
              : "border-border/70 bg-background/70 text-muted-foreground hover:text-foreground"
          }`}
          onClick={props.onTogglePlan}
          type="button"
        >
          Plan
        </button>
        <button
          className={`rounded-full border px-3 py-2 text-xs font-medium transition ${
            props.showDiff
              ? "border-border bg-foreground text-background"
              : "border-border/70 bg-background/70 text-muted-foreground hover:text-foreground"
          }`}
          onClick={props.onToggleDiff}
          type="button"
        >
          Diff
        </button>
      </div>
    </header>
  );
}
