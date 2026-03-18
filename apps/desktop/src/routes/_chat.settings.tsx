import { createRoute } from "@tanstack/react-router";
import { useState } from "react";
import { readNativeApi } from "~/nativeApi";
import { useStore } from "~/store";
import { Route as ChatRoute } from "./_chat";

export const Route = createRoute({
  getParentRoute: () => ChatRoute,
  path: "/settings",
  component: SettingsRouteView,
});

function SettingsRouteView() {
  const bootstrap = useStore((state) => state.bootstrap);
  const [saving, setSaving] = useState(false);

  if (!bootstrap) return null;

  const { config } = bootstrap;

  return (
    <section className="flex h-full min-w-0 flex-col overflow-hidden bg-background">
      <header className="drag-region flex h-14 items-center border-b border-border/70 px-6">
        <div>
          <p className="text-xs font-semibold tracking-[0.24em] text-muted-foreground uppercase">
            Settings
          </p>
          <h1 className="text-sm font-medium text-foreground">Desktop runtime defaults</h1>
        </div>
      </header>
      <div className="overflow-auto px-6 py-6">
        <div className="mx-auto grid max-w-4xl gap-6">
          <section className="rounded-[1.5rem] border border-border/70 bg-card/80 p-6">
            <h2 className="text-lg font-semibold text-foreground">Codex App Server</h2>
            <dl className="mt-5 grid gap-4 text-sm text-muted-foreground sm:grid-cols-2">
              <div>
                <dt className="text-xs font-semibold tracking-[0.2em] uppercase">
                  Binary path
                </dt>
                <dd className="mt-2 rounded-2xl border border-border/70 bg-background/80 px-4 py-3 font-mono text-xs text-foreground">
                  {config.codex.binary_path || "Auto-resolve"}
                </dd>
              </div>
              <div>
                <dt className="text-xs font-semibold tracking-[0.2em] uppercase">CODEX_HOME</dt>
                <dd className="mt-2 rounded-2xl border border-border/70 bg-background/80 px-4 py-3 font-mono text-xs text-foreground">
                  {config.codex.home_dir || "Default"}
                </dd>
              </div>
              <div>
                <dt className="text-xs font-semibold tracking-[0.2em] uppercase">
                  Default model
                </dt>
                <dd className="mt-2 rounded-2xl border border-border/70 bg-background/80 px-4 py-3 text-foreground">
                  {config.codex.default_model}
                </dd>
              </div>
              <div>
                <dt className="text-xs font-semibold tracking-[0.2em] uppercase">
                  Runtime mode
                </dt>
                <dd className="mt-2 rounded-2xl border border-border/70 bg-background/80 px-4 py-3 text-foreground">
                  {config.codex.default_runtime_mode}
                </dd>
              </div>
            </dl>
          </section>

          <section className="rounded-[1.5rem] border border-border/70 bg-card/80 p-6">
            <h2 className="text-lg font-semibold text-foreground">Local model</h2>
            <dl className="mt-5 grid gap-4 text-sm text-muted-foreground sm:grid-cols-2">
              <div>
                <dt className="text-xs font-semibold tracking-[0.2em] uppercase">Enabled</dt>
                <dd className="mt-2 rounded-2xl border border-border/70 bg-background/80 px-4 py-3 text-foreground">
                  {config.llama_cpp.enabled ? "Yes" : "No"}
                </dd>
              </div>
              <div>
                <dt className="text-xs font-semibold tracking-[0.2em] uppercase">Host</dt>
                <dd className="mt-2 rounded-2xl border border-border/70 bg-background/80 px-4 py-3 text-foreground">
                  {config.llama_cpp.host}:{config.llama_cpp.port}
                </dd>
              </div>
              <div className="sm:col-span-2">
                <dt className="text-xs font-semibold tracking-[0.2em] uppercase">Model path</dt>
                <dd className="mt-2 rounded-2xl border border-border/70 bg-background/80 px-4 py-3 font-mono text-xs text-foreground">
                  {config.llama_cpp.model_path || "Not configured"}
                </dd>
              </div>
            </dl>
          </section>

          <div className="flex justify-end">
            <button
              className="inline-flex items-center rounded-full border border-border/70 bg-muted/50 px-4 py-2 text-sm font-medium text-foreground transition hover:bg-muted/80 disabled:opacity-60"
              disabled={saving}
              onClick={async () => {
                const api = readNativeApi();
                if (!api) return;
                setSaving(true);
                try {
                  await api.settings.save(config);
                } finally {
                  setSaving(false);
                }
              }}
              type="button"
            >
              {saving ? "Saving..." : "Persist current settings"}
            </button>
          </div>
        </div>
      </div>
    </section>
  );
}
