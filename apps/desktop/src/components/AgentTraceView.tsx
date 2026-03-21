import { useEffect, useRef } from "react";
import { useStore } from "~/store";
import type { TraceEntry } from "~/types";

function tag(kind: string) {
  switch (kind) {
    case "thinking": return { t: "THINK", c: "text-purple-400" };
    case "tool-call": return { t: "TOOL ", c: "text-yellow-500" };
    case "tool-result": return { t: "RSLT ", c: "text-green-500" };
    case "error": return { t: "ERROR", c: "text-red-500" };
    case "file-change": return { t: "FILE ", c: "text-cyan-500" };
    default: return { t: "INFO ", c: "text-neutral-500" };
  }
}

function TraceLine(props: { entry: TraceEntry }) {
  const { t, c } = tag(props.entry.kind);
  return (
    <div className="flex gap-2 py-px hover:bg-neutral-900">
      <span className={`shrink-0 ${c}`}>[{t}]</span>
      <span className="text-foreground/80 break-all">{props.entry.summary}</span>
    </div>
  );
}

export function AgentTraceView() {
  const selectedGraphId = useStore((s) => s.selectedGraphId);
  const activeGraphs = useStore((s) => s.activeGraphs);
  const selectedTaskId = useStore((s) => s.selectedTaskId);
  const selectTask = useStore((s) => s.selectTask);
  const bottomRef = useRef<HTMLDivElement>(null);

  const graph = selectedGraphId ? activeGraphs.get(selectedGraphId) : null;

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  });

  if (!graph) return null;

  const nodes = Object.values(graph.nodes);
  const activeId = selectedTaskId ?? nodes[0]?.id;

  return (
    <div className="flex flex-col h-full overflow-hidden text-xs">
      {/* prompt */}
      <div className="shrink-0 border-b border-border px-4 py-2">
        <span className="text-muted-foreground">task: </span>
        <span className="text-foreground">{graph.userPrompt}</span>
        <span className="ml-3 text-muted-foreground">[{graph.status}]</span>
      </div>

      {/* task list + trace */}
      <div className="flex flex-1 min-h-0">
        {/* task list (if multiple) */}
        {nodes.length > 1 ? (
          <div className="w-48 shrink-0 border-r border-border overflow-y-auto">
            {nodes.map((n) => (
              <button
                key={n.id}
                className={`block w-full text-left px-3 py-1.5 border-b border-border truncate ${
                  activeId === n.id ? "bg-neutral-800 text-foreground" : "text-muted-foreground hover:bg-neutral-900"
                }`}
                onClick={() => selectTask(n.id)}
                type="button"
              >
                {n.status === "complete" ? "✓ " : n.status === "failed" ? "✗ " : n.status === "running" ? "⟳ " : "· "}
                {n.description.slice(0, 30)}
              </button>
            ))}
          </div>
        ) : null}

        {/* trace output */}
        <div className="flex-1 overflow-y-auto px-4 py-2 font-mono">
          {activeId && graph.nodes[activeId] ? (
            <>
              <div className="text-muted-foreground mb-2">
                --- {graph.nodes[activeId].description} [{graph.nodes[activeId].status}] ---
              </div>
              {graph.nodes[activeId].error ? (
                <div className="text-red-500 mb-2">error: {graph.nodes[activeId].error}</div>
              ) : null}
              {(graph.traces[activeId] ?? []).map((entry) => (
                <TraceLine key={entry.id} entry={entry} />
              ))}
              {graph.nodes[activeId].status === "running" && (graph.traces[activeId] ?? []).length === 0 ? (
                <div className="text-muted-foreground animate-pulse">working...</div>
              ) : null}
              <div ref={bottomRef} />
            </>
          ) : (
            <div className="text-muted-foreground">select a task</div>
          )}
        </div>
      </div>
    </div>
  );
}
