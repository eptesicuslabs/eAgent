import { ArrowUp, Square, TerminalSquare } from "lucide-react";
import type { KeyboardEvent } from "react";
import { useState } from "react";
import { useComposerDraftStore } from "~/composerDraftStore";
import { readNativeApi } from "~/nativeApi";
import type { ThreadState } from "~/types";
import { ProviderModelPicker } from "./chat/ProviderModelPicker";
import { ComposerPendingApprovalPanel } from "./chat/ComposerPendingApprovalPanel";
import { ComposerPendingUserInputPanel } from "./chat/ComposerPendingUserInputPanel";

export function ComposerPromptEditor(props: { thread: ThreadState }) {
  const [sending, setSending] = useState(false);
  const prompt =
    useComposerDraftStore((state) => state.promptByThreadId[props.thread.id]) ?? "";
  const setPrompt = useComposerDraftStore((state) => state.setPrompt);
  const clearPrompt = useComposerDraftStore((state) => state.clearPrompt);

  const pendingApprovals = Object.values(props.thread.pending_approvals);
  const pendingInputs = Object.values(props.thread.pending_inputs);

  async function submitPrompt() {
    const message = prompt.trim();
    if (!message || sending) return;
    const api = readNativeApi();
    if (!api) return;
    setSending(true);
    try {
      await api.orchestration.dispatch({ type: "sendMessage", message });
      clearPrompt(props.thread.id);
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="border-t border-border/70 bg-[linear-gradient(180deg,color-mix(in_srgb,var(--background)_96%,transparent)_0%,color-mix(in_srgb,var(--card)_96%,black)_100%)] px-6 py-5">
      {pendingApprovals.length > 0 ? (
        <div className="mb-3 grid gap-2">
          {pendingApprovals.map((approval) => (
            <ComposerPendingApprovalPanel key={approval.id} approval={approval} />
          ))}
        </div>
      ) : null}

      {pendingInputs.length > 0 ? (
        <div className="mb-3 grid gap-2">
          {pendingInputs.map((input) => (
            <ComposerPendingUserInputPanel key={input.id} input={input} />
          ))}
        </div>
      ) : null}

      <div className="rounded-[1.8rem] border border-border/70 bg-card/90 p-4 shadow-[0_18px_60px_rgba(0,0,0,0.22)]">
        <textarea
          className="min-h-[112px] w-full resize-none border-0 bg-transparent text-[15px] leading-7 text-foreground outline-none placeholder:text-muted-foreground/55"
          onChange={(event) => setPrompt(props.thread.id, event.target.value)}
          onKeyDown={(event: KeyboardEvent<HTMLTextAreaElement>) => {
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
              event.preventDefault();
              void submitPrompt();
            }
          }}
          placeholder="Ask anything, attach context, or continue the current thread..."
          value={prompt}
        />

        <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
          <div className="flex flex-wrap items-center gap-2">
            <ProviderModelPicker thread={props.thread} />
            <button
              className="inline-flex items-center gap-2 rounded-full border border-border/70 bg-background/70 px-3 py-2 text-xs font-medium text-muted-foreground transition hover:text-foreground"
              onClick={async () => {
                const api = readNativeApi();
                if (!api) return;
                await api.orchestration.dispatch({ type: "openTerminal" });
              }}
              type="button"
            >
              <TerminalSquare className="size-3.5" />
              Terminal
            </button>
          </div>

          <div className="flex items-center gap-2">
            {props.thread.active_turn ? (
              <button
                className="inline-flex items-center gap-2 rounded-full border border-rose-500/35 bg-rose-500/10 px-4 py-2 text-sm font-medium text-rose-200 transition hover:bg-rose-500/15"
                onClick={async () => {
                  const api = readNativeApi();
                  if (!api) return;
                  await api.orchestration.dispatch({ type: "interruptTurn" });
                }}
                type="button"
              >
                <Square className="size-3 fill-current" />
                Interrupt
              </button>
            ) : null}

            <button
              className="inline-flex items-center gap-2 rounded-full bg-foreground px-4 py-2 text-sm font-medium text-background transition hover:opacity-90 disabled:opacity-60"
              disabled={!prompt.trim() || sending}
              onClick={() => void submitPrompt()}
              type="button"
            >
              <ArrowUp className="size-4" />
              {sending ? "Sending..." : "Send"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
