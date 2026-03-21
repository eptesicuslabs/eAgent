import {
  CheckCircle,
  Circle,
  FolderOpen,
  Loader2,
  Plus,
  Settings,
  XCircle,
} from "lucide-react";
import { useStore } from "~/store";
import { readNativeApi } from "~/nativeApi";
import type { TaskGraphState } from "~/types";

function statusIcon(status: TaskGraphState["status"]) {
  switch (status) {
    case "running":
    case "planning":
      return <Loader2 className="size-3 shrink-0 animate-spin text-sky-400" />;
    case "complete":
      return <CheckCircle className="size-3 shrink-0 text-emerald-400" />;
    case "failed":
      return <XCircle className="size-3 shrink-0 text-rose-400" />;
    default:
      return <Circle className="size-3 shrink-0 text-zinc-500" />;
  }
}

function timeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const m = Math.floor(diff / 60000);
  if (m < 1) return "now";
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h`;
  return `${Math.floor(h / 24)}d`;
}

export function Sidebar() {
  const activeGraphs = useStore((s) => s.activeGraphs);
  const selectedGraphId = useStore((s) => s.selectedGraphId);
  const selectGraph = useStore((s) => s.selectGraph);
  const bootstrap = useStore((s) => s.bootstrap);

  const graphList = Array.from(activeGraphs.values()).sort(
    (a, b) => b.updatedAt.localeCompare(a.updatedAt),
  );

  const currentProject = bootstrap?.currentProject;

  return (
    <aside className="flex h-full flex-col border-r border-border bg-card/40">
      {/* Project header */}
      <div className="flex items-center justify-between border-b border-border px-3 py-2.5">
        {currentProject ? (
          <div className="flex items-center gap-1.5 text-xs text-foreground/80 truncate">
            <FolderOpen className="size-3 shrink-0 text-muted-foreground" />
            <span className="truncate font-medium">
              {currentProject.split(/[/\\]/).pop()}
            </span>
          </div>
        ) : (
          <button
            className="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition"
            onClick={async () => {
              const api = readNativeApi();
              const path = await api.app.pickFolder();
              if (path) await api.projects.open(path);
            }}
            type="button"
          >
            <FolderOpen className="size-3" />
            Open project
          </button>
        )}
        <button
          className="text-muted-foreground hover:text-foreground transition"
          onClick={() => selectGraph(null)}
          type="button"
          title="New task"
        >
          <Plus className="size-3.5" />
        </button>
      </div>

      {/* Task list */}
      <div className="flex-1 overflow-y-auto px-2 py-2">
        {graphList.length === 0 ? (
          <p className="px-2 py-6 text-center text-xs text-muted-foreground/60">
            No tasks yet
          </p>
        ) : (
          <div className="grid gap-0.5">
            {graphList.map((graph) => {
              const isActive = selectedGraphId === graph.graphId;
              const taskCount = Object.keys(graph.nodes).length;
              return (
                <button
                  key={graph.graphId}
                  className={`group flex w-full items-start gap-2 rounded-lg px-2.5 py-2 text-left transition ${
                    isActive
                      ? "bg-foreground/[0.07]"
                      : "hover:bg-foreground/[0.04]"
                  }`}
                  onClick={() => selectGraph(graph.graphId)}
                  type="button"
                >
                  <div className="mt-0.5">{statusIcon(graph.status)}</div>
                  <div className="min-w-0 flex-1">
                    <p className="text-[12px] leading-[18px] font-medium text-foreground/90 truncate">
                      {graph.userPrompt || "Untitled task"}
                    </p>
                    <p className="mt-0.5 text-[10px] text-muted-foreground/70">
                      {taskCount} {taskCount === 1 ? "task" : "tasks"} · {timeAgo(graph.updatedAt)}
                    </p>
                  </div>
                </button>
              );
            })}
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="border-t border-border px-3 py-2">
        <button
          className="flex w-full items-center gap-1.5 rounded-md px-2 py-1.5 text-[11px] text-muted-foreground hover:text-foreground hover:bg-foreground/[0.04] transition"
          onClick={async () => {
            const api = readNativeApi();
            const path = await api.app.pickFolder();
            if (path) await api.projects.open(path);
          }}
          type="button"
        >
          <Settings className="size-3" />
          Settings
        </button>
      </div>
    </aside>
  );
}
