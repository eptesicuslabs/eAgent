import { CheckCircle, ChevronRight, Circle, Loader2, XCircle } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useStore } from "~/store";
import type { TaskGraphState, TaskNode, TraceEntry } from "~/types";

// === Status helpers ===

function StatusIcon(props: { status: string; className?: string }) {
  const cls = props.className ?? "size-4";
  switch (props.status) {
    case "running":
    case "scheduled":
      return <Loader2 className={`${cls} animate-spin text-sky-400`} />;
    case "complete":
      return <CheckCircle className={`${cls} text-emerald-400`} />;
    case "failed":
      return <XCircle className={`${cls} text-rose-400`} />;
    default:
      return <Circle className={`${cls} text-zinc-600`} />;
  }
}

function statusLabel(s: string) {
  switch (s) {
    case "complete": return "text-emerald-400";
    case "failed": return "text-rose-400";
    case "running": case "scheduled": return "text-sky-400";
    default: return "text-zinc-500";
  }
}

// === Trace entry rendering ===

function kindTag(kind: string): { text: string; cls: string } {
  switch (kind) {
    case "thinking": return { text: "THINK", cls: "text-violet-400/80" };
    case "tool-call": return { text: "TOOL", cls: "text-amber-400/80" };
    case "tool-result": return { text: "RESULT", cls: "text-emerald-400/80" };
    case "error": return { text: "ERROR", cls: "text-rose-400" };
    case "status": return { text: "STATUS", cls: "text-zinc-500" };
    case "file-change": return { text: "FILE", cls: "text-sky-400/80" };
    default: return { text: kind.toUpperCase(), cls: "text-zinc-500" };
  }
}

function TraceRow(props: { entry: TraceEntry }) {
  const [open, setOpen] = useState(false);
  const tag = kindTag(props.entry.kind);

  return (
    <div className="group flex gap-2 py-[3px]">
      <span className={`mt-[2px] w-[52px] shrink-0 text-right font-mono text-[10px] font-bold ${tag.cls}`}>
        {tag.text}
      </span>
      <div className="min-w-0 flex-1">
        <p className="text-[12px] leading-[18px] text-foreground/75">{props.entry.summary}</p>
        {props.entry.detail ? (
          <>
            <button
              className="mt-0.5 inline-flex items-center gap-0.5 text-[10px] text-muted-foreground/60 hover:text-muted-foreground transition"
              onClick={() => setOpen((v) => !v)}
              type="button"
            >
              <ChevronRight className={`size-2.5 transition-transform ${open ? "rotate-90" : ""}`} />
              detail
            </button>
            {open ? (
              <pre className="mt-1 max-h-48 overflow-auto rounded-md bg-black/30 px-3 py-2 font-mono text-[10px] leading-4 text-zinc-400 border border-border/50">
                {props.entry.detail}
              </pre>
            ) : null}
          </>
        ) : null}
      </div>
    </div>
  );
}

// === Task card ===

function TaskCard(props: {
  node: TaskNode;
  traces: TraceEntry[];
  isSelected: boolean;
  onSelect: () => void;
}) {
  const traceEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (props.isSelected && traceEndRef.current) {
      traceEndRef.current.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
  }, [props.traces.length, props.isSelected]);

  return (
    <div
      className={`rounded-lg border transition cursor-pointer ${
        props.isSelected
          ? "border-border bg-foreground/[0.03]"
          : "border-transparent hover:border-border/50"
      }`}
      onClick={props.onSelect}
    >
      {/* Header */}
      <div className="flex items-center gap-2.5 px-3.5 py-2.5">
        <StatusIcon status={props.node.status} className="size-3.5" />
        <span className="flex-1 text-[13px] font-medium text-foreground/90 truncate">
          {props.node.description}
        </span>
        <span className={`text-[10px] font-semibold tracking-wider uppercase ${statusLabel(props.node.status)}`}>
          {props.node.status}
        </span>
      </div>

      {/* Error */}
      {props.node.error ? (
        <div className="mx-3.5 mb-2.5 rounded-md bg-rose-500/10 border border-rose-500/20 px-3 py-1.5 text-[11px] text-rose-300">
          {props.node.error}
        </div>
      ) : null}

      {/* Traces (only if selected) */}
      {props.isSelected && props.traces.length > 0 ? (
        <div className="border-t border-border/40 px-3.5 py-2 max-h-[400px] overflow-y-auto">
          {props.traces.map((entry) => (
            <TraceRow key={entry.id} entry={entry} />
          ))}
          <div ref={traceEndRef} />
        </div>
      ) : null}

      {/* Loading state */}
      {props.isSelected && props.traces.length === 0 && props.node.status === "running" ? (
        <div className="border-t border-border/40 px-3.5 py-3">
          <p className="text-[11px] text-muted-foreground/60 animate-pulse">Agent working...</p>
        </div>
      ) : null}
    </div>
  );
}

// === Main view ===

export function AgentTraceView() {
  const selectedGraphId = useStore((s) => s.selectedGraphId);
  const activeGraphs = useStore((s) => s.activeGraphs);
  const selectedTaskId = useStore((s) => s.selectedTaskId);
  const selectTask = useStore((s) => s.selectTask);

  if (!selectedGraphId) return null;
  const graph = activeGraphs.get(selectedGraphId);
  if (!graph) return null;

  const nodes = Object.values(graph.nodes);
  const activeTaskId = selectedTaskId ?? nodes[0]?.id ?? null;

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Header */}
      <div className="shrink-0 border-b border-border px-5 py-3">
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <p className="text-[13px] font-medium text-foreground truncate">
              {graph.userPrompt}
            </p>
          </div>
          <GraphStatusBadge status={graph.status} />
        </div>
      </div>

      {/* Task list */}
      <div className="flex-1 overflow-y-auto px-4 py-3">
        <div className="grid gap-1.5 max-w-3xl mx-auto">
          {nodes.map((node) => (
            <TaskCard
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

function GraphStatusBadge(props: { status: TaskGraphState["status"] }) {
  const cls = {
    complete: "bg-emerald-500/10 text-emerald-400 border-emerald-500/20",
    failed: "bg-rose-500/10 text-rose-400 border-rose-500/20",
    running: "bg-sky-500/10 text-sky-400 border-sky-500/20",
    planning: "bg-amber-500/10 text-amber-400 border-amber-500/20",
    paused: "bg-zinc-500/10 text-zinc-400 border-zinc-500/20",
  }[props.status] ?? "bg-zinc-500/10 text-zinc-400 border-zinc-500/20";

  return (
    <span className={`shrink-0 rounded-md border px-2 py-0.5 text-[10px] font-semibold tracking-wider uppercase ${cls}`}>
      {props.status}
    </span>
  );
}
