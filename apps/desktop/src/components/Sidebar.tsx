import { useStore } from "~/store";
import { readNativeApi } from "~/nativeApi";

export function Sidebar() {
  const activeGraphs = useStore((s) => s.activeGraphs);
  const selectedGraphId = useStore((s) => s.selectedGraphId);
  const selectGraph = useStore((s) => s.selectGraph);
  const bootstrap = useStore((s) => s.bootstrap);
  const project = bootstrap?.currentProject;

  const graphs = Array.from(activeGraphs.values()).sort(
    (a, b) => b.updatedAt.localeCompare(a.updatedAt),
  );

  return (
    <aside className="flex h-full flex-col border-r border-border text-xs">
      <div className="border-b border-border px-3 py-2 text-muted-foreground">
        {project ? (
          <span title={project}>📁 {project.split(/[/\\]/).pop()}</span>
        ) : (
          <button
            className="hover:text-foreground"
            onClick={async () => {
              const api = readNativeApi();
              const p = await api.app.pickFolder();
              if (p) await api.projects.open(p);
            }}
            type="button"
          >open project...</button>
        )}
      </div>

      <div className="flex-1 overflow-y-auto">
        {graphs.map((g) => (
          <button
            key={g.graphId}
            className={`block w-full text-left px-3 py-1.5 border-b border-border truncate ${
              selectedGraphId === g.graphId ? "bg-neutral-800 text-foreground" : "text-muted-foreground hover:bg-neutral-900"
            }`}
            onClick={() => selectGraph(g.graphId)}
            type="button"
          >
            <span className="mr-1.5">{
              g.status === "complete" ? "✓" :
              g.status === "failed" ? "✗" :
              g.status === "running" ? "⟳" : "·"
            }</span>
            {g.userPrompt.slice(0, 40) || "untitled"}
          </button>
        ))}
      </div>

      <div className="border-t border-border px-3 py-2">
        <button
          className="text-muted-foreground hover:text-foreground"
          onClick={() => selectGraph(null)}
          type="button"
        >+ new task</button>
      </div>
    </aside>
  );
}
