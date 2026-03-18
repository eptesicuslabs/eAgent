import { ArrowUp, Loader2, Shield, ShieldAlert, ShieldCheck } from "lucide-react";
import type { KeyboardEvent } from "react";
import { useState } from "react";
import { readNativeApi } from "~/nativeApi";
import { useStore } from "~/store";
import type { OversightMode, OversightRequest } from "~/types";

function oversightIcon(mode: OversightMode) {
  switch (mode) {
    case "full-autonomy":
      return <ShieldCheck className="size-3.5" />;
    case "approve-risky":
      return <Shield className="size-3.5" />;
    case "approve-all":
      return <ShieldAlert className="size-3.5" />;
  }
}

function oversightLabel(mode: OversightMode) {
  switch (mode) {
    case "full-autonomy":
      return "Full Autonomy";
    case "approve-risky":
      return "Approve Risky";
    case "approve-all":
      return "Approve All";
  }
}

const OVERSIGHT_MODES: OversightMode[] = [
  "full-autonomy",
  "approve-risky",
  "approve-all",
];

function OversightRequestCard(props: {
  request: OversightRequest;
  onApprove: () => void;
  onDeny: () => void;
}) {
  return (
    <div className="rounded-[1.2rem] border border-amber-500/25 bg-amber-500/8 px-4 py-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="text-xs font-semibold tracking-[0.2em] text-amber-200 uppercase">
            Oversight Required
          </p>
          <p className="mt-1 text-sm font-medium text-foreground">
            {props.request.action}
          </p>
          <p className="mt-1 text-xs leading-5 text-muted-foreground line-clamp-2">
            {props.request.context}
          </p>
        </div>
        <span
          className={`rounded-full px-2 py-1 text-[10px] font-semibold tracking-[0.18em] uppercase ${
            props.request.riskLevel === "high"
              ? "bg-rose-500/15 text-rose-300"
              : props.request.riskLevel === "medium"
                ? "bg-amber-500/15 text-amber-300"
                : "bg-emerald-500/15 text-emerald-300"
          }`}
        >
          {props.request.riskLevel}
        </span>
      </div>
      <div className="mt-3 flex items-center gap-2">
        <button
          className="inline-flex items-center gap-1.5 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-3 py-1.5 text-xs font-medium text-emerald-200 transition hover:bg-emerald-500/20"
          onClick={props.onApprove}
          type="button"
        >
          Approve
        </button>
        <button
          className="inline-flex items-center gap-1.5 rounded-full border border-rose-500/30 bg-rose-500/10 px-3 py-1.5 text-xs font-medium text-rose-200 transition hover:bg-rose-500/20"
          onClick={props.onDeny}
          type="button"
        >
          Deny
        </button>
      </div>
    </div>
  );
}

export function Composer() {
  const [prompt, setPrompt] = useState("");
  const [sending, setSending] = useState(false);
  const [oversightMode, setOversightMode] = useState<OversightMode>("approve-risky");

  const selectedGraphId = useStore((state) => state.selectedGraphId);
  const activeGraphs = useStore((state) => state.activeGraphs);

  // Collect pending oversight requests from the selected graph
  const selectedGraph = selectedGraphId ? activeGraphs.get(selectedGraphId) : null;
  const oversightRequests = selectedGraph
    ? Object.values(selectedGraph.oversightRequests)
    : [];

  async function submitTask() {
    const message = prompt.trim();
    if (!message || sending) return;
    const api = readNativeApi();
    if (!api) return;
    setSending(true);
    try {
      const graphId = await api.eagent.submitTask(message);
      if (graphId) {
        useStore.getState().selectGraph(graphId);
      }
      setPrompt("");
    } catch (error) {
      console.error("[eAgent] Failed to submit task:", error);
    } finally {
      setSending(false);
    }
  }

  async function handleApprove(requestId: string) {
    const api = readNativeApi();
    if (!api) return;
    try {
      await api.eagent.approveOversight(requestId);
    } catch (error) {
      console.error("[eAgent] Failed to approve oversight:", error);
    }
  }

  async function handleDeny(requestId: string) {
    const api = readNativeApi();
    if (!api) return;
    try {
      await api.eagent.denyOversight(requestId);
    } catch (error) {
      console.error("[eAgent] Failed to deny oversight:", error);
    }
  }

  const isRunning = selectedGraph?.status === "running" || selectedGraph?.status === "planning";

  return (
    <div className="border-t border-border/70 bg-[linear-gradient(180deg,color-mix(in_srgb,var(--background)_96%,transparent)_0%,color-mix(in_srgb,var(--card)_96%,black)_100%)] px-6 py-4">
      {/* Oversight request cards */}
      {oversightRequests.length > 0 ? (
        <div className="mb-3 grid gap-2">
          {oversightRequests.map((request) => (
            <OversightRequestCard
              key={request.requestId}
              onApprove={() => void handleApprove(request.requestId)}
              onDeny={() => void handleDeny(request.requestId)}
              request={request}
            />
          ))}
        </div>
      ) : null}

      {/* Composer input */}
      <div className="rounded-[1.8rem] border border-border/70 bg-card/90 p-4 shadow-[0_18px_60px_rgba(0,0,0,0.22)]">
        <textarea
          className="min-h-[80px] w-full resize-none border-0 bg-transparent text-[15px] leading-7 text-foreground outline-none placeholder:text-muted-foreground/55"
          onChange={(event) => setPrompt(event.target.value)}
          onKeyDown={(event: KeyboardEvent<HTMLTextAreaElement>) => {
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
              event.preventDefault();
              void submitTask();
            }
          }}
          placeholder="Describe a task for eAgent to plan and execute..."
          value={prompt}
        />

        <div className="mt-3 flex flex-wrap items-center justify-between gap-3">
          {/* Left: oversight mode selector */}
          <div className="flex items-center gap-2">
            <div className="flex items-center gap-1 rounded-full border border-border/70 bg-background/70 px-1 py-1">
              {OVERSIGHT_MODES.map((m) => (
                <button
                  key={m}
                  className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1.5 text-[11px] font-medium transition ${
                    oversightMode === m
                      ? "bg-foreground text-background"
                      : "text-muted-foreground hover:text-foreground"
                  }`}
                  onClick={() => setOversightMode(m)}
                  type="button"
                >
                  {oversightIcon(m)}
                  {oversightLabel(m)}
                </button>
              ))}
            </div>
          </div>

          {/* Right: submit */}
          <div className="flex items-center gap-2">
            {isRunning ? (
              <div className="inline-flex items-center gap-2 rounded-full border border-sky-500/30 bg-sky-500/10 px-3 py-1.5 text-xs font-medium text-sky-200">
                <Loader2 className="size-3 animate-spin" />
                Running...
              </div>
            ) : null}

            <button
              className="inline-flex items-center gap-2 rounded-full bg-foreground px-4 py-2 text-sm font-medium text-background transition hover:opacity-90 disabled:opacity-60"
              disabled={!prompt.trim() || sending}
              onClick={() => void submitTask()}
              type="button"
            >
              <ArrowUp className="size-4" />
              {sending ? "Submitting..." : "Submit Task"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
