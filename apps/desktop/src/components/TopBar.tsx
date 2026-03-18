import { Cpu, Layers, Monitor, Settings, Zap } from "lucide-react";
import type { AgentMode, ProviderStatus } from "~/types";
import { useStore } from "~/store";

function providerStatusDot(status: ProviderStatus["status"]) {
  switch (status) {
    case "available":
      return "bg-emerald-400";
    case "starting":
      return "bg-amber-400 animate-pulse";
    case "error":
      return "bg-rose-400";
    default:
      return "bg-muted-foreground/50";
  }
}

function ModeButton(props: {
  label: string;
  value: AgentMode;
  active: boolean;
  icon: React.ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      className={`inline-flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-xs font-semibold tracking-wide transition ${
        props.active
          ? "border-foreground/20 bg-foreground text-background"
          : "border-border/70 bg-background/60 text-muted-foreground hover:text-foreground hover:bg-background/80"
      }`}
      onClick={props.onClick}
      type="button"
    >
      {props.icon}
      {props.label}
    </button>
  );
}

export function TopBar(props: {
  onSettingsClick: () => void;
}) {
  const mode = useStore((state) => state.mode);
  const setMode = useStore((state) => state.setMode);
  const providers = useStore((state) => state.providers);
  const bootstrap = useStore((state) => state.bootstrap);

  const providerList = Array.from(providers.values());
  const activeProvider = providerList.find((p) => p.status === "available") ?? providerList[0];

  return (
    <header className="drag-region flex h-12 items-center justify-between gap-4 border-b border-border/70 bg-[linear-gradient(180deg,color-mix(in_srgb,var(--card)_94%,black)_0%,color-mix(in_srgb,var(--background)_97%,black)_100%)] px-4">
      {/* Left: branding + mode switcher */}
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-2">
          <Zap className="size-4 text-foreground" />
          <span className="text-sm font-bold tracking-tight text-foreground">
            eAgent
          </span>
        </div>

        <div className="flex items-center gap-1.5">
          <ModeButton
            active={mode === "ecode"}
            icon={<Monitor className="size-3" />}
            label="eCode"
            onClick={() => setMode("ecode")}
            value="ecode"
          />
          <ModeButton
            active={mode === "ework"}
            icon={<Layers className="size-3" />}
            label="eWork"
            onClick={() => setMode("ework")}
            value="ework"
          />
        </div>
      </div>

      {/* Center: status */}
      <div className="flex items-center gap-3 text-xs text-muted-foreground">
        {bootstrap?.statusMessage ? (
          <span className="truncate max-w-[260px]">{bootstrap.statusMessage}</span>
        ) : null}
      </div>

      {/* Right: provider info + settings */}
      <div className="flex items-center gap-3">
        {activeProvider ? (
          <div className="flex items-center gap-2 rounded-full border border-border/70 bg-background/60 px-3 py-1.5 text-xs text-muted-foreground">
            <span className={`inline-block size-2 rounded-full ${providerStatusDot(activeProvider.status)}`} />
            <Cpu className="size-3" />
            <span className="font-medium text-foreground">{activeProvider.displayName}</span>
            {activeProvider.models.length > 0 ? (
              <span className="text-muted-foreground">/ {activeProvider.models[0]}</span>
            ) : null}
          </div>
        ) : (
          <div className="flex items-center gap-2 rounded-full border border-border/70 bg-background/60 px-3 py-1.5 text-xs text-muted-foreground">
            <span className="inline-block size-2 rounded-full bg-muted-foreground/50" />
            <span>No provider</span>
          </div>
        )}

        {providerList.length > 1 ? (
          <span className="text-[10px] text-muted-foreground">
            +{providerList.length - 1} more
          </span>
        ) : null}

        <button
          className="inline-flex size-8 items-center justify-center rounded-xl border border-border/70 bg-background/60 text-muted-foreground transition hover:text-foreground hover:bg-background/80"
          onClick={props.onSettingsClick}
          type="button"
        >
          <Settings className="size-3.5" />
        </button>
      </div>
    </header>
  );
}
