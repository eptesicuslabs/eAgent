import { useState } from "react";
import { readNativeApi } from "~/nativeApi";
import type { PendingUserInput } from "~/types";

function formatQuestions(value: unknown) {
  if (value == null) return "The runtime is waiting for an input response.";
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

export function ComposerPendingUserInputPanel(props: { input: PendingUserInput }) {
  const [response, setResponse] = useState("");
  const [sending, setSending] = useState(false);

  async function submit() {
    if (!response.trim()) return;
    const api = readNativeApi();
    if (!api) return;
    setSending(true);
    try {
      await api.orchestration.dispatch({
        type: "userInputResponse",
        approvalId: props.input.id,
        response,
      });
      setResponse("");
    } finally {
      setSending(false);
    }
  }

  return (
    <section className="rounded-[1.5rem] border border-violet-500/20 bg-violet-500/8 p-4">
      <div className="flex items-center justify-between gap-3">
        <div>
          <p className="text-[11px] font-semibold tracking-[0.22em] text-violet-200 uppercase">
            User input requested
          </p>
          <h3 className="mt-1 text-sm font-medium text-foreground">Respond to continue</h3>
        </div>
        <span className="rounded-full border border-violet-500/20 bg-background/70 px-2.5 py-1 text-[10px] font-semibold tracking-[0.18em] text-violet-200 uppercase">
          {props.input.requested_at}
        </span>
      </div>

      <pre className="mt-3 max-h-36 overflow-auto whitespace-pre-wrap break-words rounded-2xl border border-violet-500/15 bg-background/65 px-3 py-3 font-mono text-xs leading-6 text-muted-foreground">
        {formatQuestions(props.input.questions)}
      </pre>

      <div className="mt-3 flex items-center gap-2">
        <input
          className="h-10 min-w-0 flex-1 rounded-xl border border-violet-500/15 bg-background/75 px-4 text-sm text-foreground outline-none placeholder:text-muted-foreground/55"
          onChange={(event) => setResponse(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              void submit();
            }
          }}
          placeholder="Type the operator response"
          value={response}
        />
        <button
          className="rounded-full border border-violet-500/25 bg-violet-500/12 px-3 py-2 text-xs font-medium text-violet-100 transition hover:bg-violet-500/18 disabled:opacity-60"
          disabled={!response.trim() || sending}
          onClick={() => void submit()}
          type="button"
        >
          {sending ? "Sending..." : "Submit"}
        </button>
      </div>
    </section>
  );
}
