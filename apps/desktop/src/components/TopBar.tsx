import { Cpu, Layers, Monitor, Zap } from "lucide-react";
import type { AgentMode } from "~/types";
import { useStore } from "~/store";

export function TopBar() {
  const mode = useStore((s) => s.mode);
  const setMode = useStore((s) => s.setMode);
  const providers = useStore((s) => s.providers);

  const providerList = Array.from(providers.values());
  const active = providerList.find((p) => p.status === "available");

  return (
    <header className="drag-region flex h-11 shrink-0 items-center justify-between border-b border-border bg-card/80 px-4">
      {/* Brand + mode */}
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-1.5 text-foreground">
          <Zap className="size-3.5" />
          <span className="text-[13px] font-bold tracking-tight">eAgent</span>
        </div>

        <div className="flex items-center rounded-md border border-border bg-background/60 p-0.5">
          <ModeTab active={mode === "ecode"} onClick={() => setMode("ecode")}>
            <Monitor className="size-3" /> eCode
          </ModeTab>
          <ModeTab active={mode === "ework"} onClick={() => setMode("ework")}>
            <Layers className="size-3" /> eWork
          </ModeTab>
        </div>
      </div>

      {/* Provider */}
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        {active ? (
          <>
            <span className="inline-block size-1.5 rounded-full bg-emerald-400" />
            <Cpu className="size-3" />
            <span className="font-medium text-foreground/80">{active.displayName}</span>
          </>
        ) : (
          <span className="text-muted-foreground/60">No provider configured</span>
        )}
      </div>
    </header>
  );
}

function ModeTab(props: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      className={`inline-flex items-center gap-1 rounded-[5px] px-2.5 py-1 text-[11px] font-semibold transition-colors ${
        props.active
          ? "bg-foreground text-background"
          : "text-muted-foreground hover:text-foreground"
      }`}
      onClick={props.onClick}
      type="button"
    >
      {props.children}
    </button>
  );
}
