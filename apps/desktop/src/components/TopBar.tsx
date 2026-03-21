import { useStore } from "~/store";

export function TopBar() {
  const mode = useStore((s) => s.mode);
  const setMode = useStore((s) => s.setMode);
  const providers = useStore((s) => s.providers);
  const active = Array.from(providers.values()).find((p) => p.status === "available");

  return (
    <header className="drag-region flex h-8 items-center justify-between border-b border-border px-3 text-xs">
      <div className="flex items-center gap-3">
        <span className="font-bold text-foreground">eAgent</span>
        <span className="text-muted-foreground">|</span>
        <button
          className={mode === "ecode" ? "text-foreground" : "text-muted-foreground hover:text-foreground"}
          onClick={() => setMode("ecode")}
          type="button"
        >eCode</button>
        <button
          className={mode === "ework" ? "text-foreground" : "text-muted-foreground hover:text-foreground"}
          onClick={() => setMode("ework")}
          type="button"
        >eWork</button>
      </div>
      <div className="text-muted-foreground">
        {active ? (
          <span><span className="text-green-500">●</span> {active.displayName}</span>
        ) : (
          <span className="text-neutral-600">no provider</span>
        )}
      </div>
    </header>
  );
}
