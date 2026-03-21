import { ArrowUp, Loader2, Square, Shield, ShieldCheck, ShieldAlert } from "lucide-react";
import type { KeyboardEvent } from "react";
import { useState } from "react";
import { readNativeApi } from "~/nativeApi";
import { useStore } from "~/store";
import type { OversightMode } from "~/types";

const OVERSIGHT: Array<{ mode: OversightMode; icon: typeof Shield; tip: string }> = [
  { mode: "full-autonomy", icon: ShieldCheck, tip: "Full autonomy" },
  { mode: "approve-risky", icon: Shield, tip: "Approve risky" },
  { mode: "approve-all", icon: ShieldAlert, tip: "Approve all" },
];

export function Composer() {
  const [prompt, setPrompt] = useState("");
  const [sending, setSending] = useState(false);
  const [oversight, setOversight] = useState<OversightMode>("approve-risky");

  const selectedGraphId = useStore((s) => s.selectedGraphId);
  const activeGraphs = useStore((s) => s.activeGraphs);
  const graph = selectedGraphId ? activeGraphs.get(selectedGraphId) : null;
  const isRunning = graph?.status === "running" || graph?.status === "planning";

  async function submit() {
    const text = prompt.trim();
    if (!text || sending) return;
    const api = readNativeApi();
    setSending(true);
    try {
      const result = await api.eagent.submitTask(text);
      if (result?.graphId) {
        useStore.getState().selectGraph(result.graphId);
      }
      setPrompt("");
    } catch (e) {
      console.error("[eAgent] submit failed:", e);
    } finally {
      setSending(false);
    }
  }

  async function cancel() {
    if (!selectedGraphId) return;
    const api = readNativeApi();
    try {
      await api.eagent.cancelGraph(selectedGraphId);
    } catch (e) {
      console.error("[eAgent] cancel failed:", e);
    }
  }

  return (
    <div className="shrink-0 border-t border-border bg-card/60 px-4 py-3">
      <div className="mx-auto max-w-3xl">
        <div className="rounded-xl border border-border bg-background/80">
          {/* Textarea */}
          <textarea
            className="block w-full resize-none bg-transparent px-4 pt-3 pb-2 text-[13px] leading-6 text-foreground outline-none placeholder:text-muted-foreground/40"
            rows={3}
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            onKeyDown={(e: KeyboardEvent<HTMLTextAreaElement>) => {
              if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
                e.preventDefault();
                void submit();
              }
            }}
            placeholder={isRunning ? "Agent is working... type to add context" : "Describe what you want to build..."}
          />

          {/* Bottom bar */}
          <div className="flex items-center justify-between px-3 pb-2">
            {/* Left: oversight toggle */}
            <div className="flex items-center gap-0.5 rounded-md border border-border/60 p-0.5">
              {OVERSIGHT.map(({ mode, icon: Icon, tip }) => (
                <button
                  key={mode}
                  className={`rounded-[5px] p-1.5 transition ${
                    oversight === mode
                      ? "bg-foreground/10 text-foreground"
                      : "text-muted-foreground/50 hover:text-muted-foreground"
                  }`}
                  onClick={() => setOversight(mode)}
                  title={tip}
                  type="button"
                >
                  <Icon className="size-3" />
                </button>
              ))}
            </div>

            {/* Right: actions */}
            <div className="flex items-center gap-2">
              {isRunning ? (
                <button
                  className="inline-flex items-center gap-1.5 rounded-lg border border-rose-500/30 bg-rose-500/10 px-3 py-1.5 text-[11px] font-medium text-rose-300 transition hover:bg-rose-500/20"
                  onClick={() => void cancel()}
                  type="button"
                >
                  <Square className="size-2.5" />
                  Cancel
                </button>
              ) : null}
              <button
                className="inline-flex items-center gap-1.5 rounded-lg bg-foreground px-3 py-1.5 text-[11px] font-semibold text-background transition hover:opacity-90 disabled:opacity-40"
                disabled={!prompt.trim() || sending}
                onClick={() => void submit()}
                type="button"
              >
                {sending ? (
                  <Loader2 className="size-3 animate-spin" />
                ) : (
                  <ArrowUp className="size-3" />
                )}
                {sending ? "Sending" : "Submit"}
              </button>
            </div>
          </div>
        </div>

        <p className="mt-1.5 text-center text-[10px] text-muted-foreground/40">
          Ctrl+Enter to submit
        </p>
      </div>
    </div>
  );
}
