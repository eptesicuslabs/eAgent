import { ChevronDown, ChevronRight, Circle, CheckCircle, XCircle, Loader2 } from "lucide-react";
import { useState } from "react";
import { useStore } from "~/store";
import type { TaskGraphState, TaskNode, TraceEntry } from "~/types";

function statusColor(status: string): string {
  switch (status) {
    case "running":
    case "scheduled":
      return "text-sky-400";
    case "complete":
      return "text-emerald-400";
    case "failed":
      return "text-rose-400";
    case "cancelled":
      return "text-zinc-500";
    case "pending":
    case "ready":
      return "text-zinc-400";
    default:
      return "text-zinc-400";
  }
}

function StatusIcon(props: { status: string }) {
  switch (props.status) {
    case "running":
    case "scheduled":
      return <Loader2 className="size-3.5 animate-spin text-sky-400" />;
    case "complete":
      return <CheckCircle className="size-3.5 text-emerald-400" />;
    case "failed":
      return <XCircle className="size-3.5 text-rose-400" />;
    default:
      return <Circle className="size-3.5 text-zinc-500" />;
  }
}

function traceKindStyle(kind: string): { label: string; color: string } {
  switch (kind) {
    case "thinking":
      return { label: "think", color: "text-violet-400" };
    case "tool-call":
      return { label: "tool", color: "text-amber-400" };
    case "tool-result":
      return { label: "result", color: "text-emerald-400" };
    case "file-change":
      return { label: "edit", color: "text-sky-400" };
    case "error":
      return { label: "error", color: "text-rose-400" };
    case "status":
      return { label: "status", color: "text-zinc-400" };
    default:
      return { label: kind, color: "text-zinc-400" };
  }
}

function TraceEntryRow(props: { entry: TraceEntry }) {
  const style = traceKindStyle(props.entry.kind);
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="group flex items-start gap-2 py-1">
      <span className={`mt-0.5 w-12 shrink-0 text-right text-[10px] font-mono font-semibold ${style.color}`}>
        {style.label}
      </span>
      <div className="min-w-0 flex-1">
        <p className="text-xs leading-5 text-foreground/80">
          {props.entry.summary}
        </p>
        {props.entry.detail ? (
          <>
            <button
              className="mt-0.5 text-[10px] text-muted-foreground hover:text-foreground"
              onClick={() => setExpanded((v) => !v)}
              type="button"
            >
              {expanded ? (
                <ChevronDown className="inline size-3" />
              ) : (
                <ChevronRight className="inline size-3" />
              )}{" "}
              detail
            </button>
            {expanded ? (
              <pre className="mt-1 max-h-40 overflow-auto rounded bg-zinc-900/60 p-2 text-[10px] leading-4 text-zinc-400">
                {props.entry.detail}
              </pre>
            ) : null}
          </>
        ) : null}
      </div>
    </div>
  );
}

function TaskNodeCard(props: { node: TaskNode; traces: TraceEntry[]; isSelected: boolean; onSelect: () => void }) {
  return (
    <div
      className={`rounded-lg border p-3 transition cursor-pointer ${
        props.isSelected
          ? "border-sky-500/40 bg-sky-500/5"
          : "border-border/50 bg-card/40 hover:border-border"
      }`}
      onClick={props.onSelect}
    >
      <div className="flex items-center gap-2">
        <StatusIcon status={props.node.status} />
        <span className="flex-1 text-sm font-medium text-foreground truncate">
          {props.node.description}
        </span>
        <span className={`text-[10px] font-semibold tracking-wider uppercase ${statusColor(props.node.status)}`}>
          {props.node.status}
        </span>
      </div>

      {props.node.error ? (
        <p className="mt-2 text-xs text-rose-300 bg-rose-500/10 rounded px-2 py-1">
          {props.node.error}
        </p>
      ) : null}

      {props.isSelected && props.traces.length > 0 ? (
        <div className="mt-3 border-t border-border/30 pt-2">
          {props.traces.map((entry) => (
            <TraceEntryRow key={entry.id} entry={entry} />
          ))}
        </div>
      ) : null}

      {props.isSelected && props.traces.length === 0 && props.node.status === "running" ? (
        <p className="mt-3 text-xs text-muted-foreground animate-pulse">
          Agent working...
        </p>
      ) : null}
    </div>
  );
}

export function AgentTraceView() {
  const selectedGraphId = useStore((state) => state.selectedGraphId);
  const activeGraphs = useStore((state) => state.activeGraphs);
  const selectedTaskId = useStore((state) => state.selectedTaskId);
  const selectTask = useStore((state) => state.selectTask);

  if (!selectedGraphId) return null;
  const graph = activeGraphs.get(selectedGraphId);
  if (!graph) return null;

  const nodes = Object.values(graph.nodes);
  const firstTaskId = nodes[0]?.id ?? null;
  const activeTaskId = selectedTaskId ?? firstTaskId;

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Header */}
      <div className="shrink-0 border-b border-border/50 bg-card/50 px-5 py-3">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-[10px] font-semibold tracking-[0.2em] text-muted-foreground uppercase">
              Task Graph
            </p>
            <p className="mt-1 text-sm font-medium text-foreground truncate max-w-[500px]">
              {graph.userPrompt}
            </p>
          </div>
          <span
            className={`rounded-full px-2.5 py-1 text-[10px] font-semibold tracking-wider uppercase ${
              graph.status === "complete"
                ? "bg-emerald-500/15 text-emerald-300"
                : graph.status === "failed"
                  ? "bg-rose-500/15 text-rose-300"
                  : graph.status === "running"
                    ? "bg-sky-500/15 text-sky-300"
                    : "bg-zinc-500/15 text-zinc-300"
            }`}
          >
            {graph.status}
          </span>
        </div>
      </div>

      {/* Task list with traces */}
      <div className="flex-1 overflow-auto px-5 py-4">
        <div className="grid gap-3">
          {nodes.map((node) => (
            <TaskNodeCard
              key={node.id}
              node={node}
              traces={graph.traces[node.id] ?? []}
              isSelected={activeTaskId === node.id}
              onSelect={() => selectTask(node.id)}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
