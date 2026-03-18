import { useState } from "react";
import { readNativeApi } from "~/nativeApi";
import type { PendingApproval } from "~/types";

function formatDetails(value: unknown) {
  if (value == null) return "No details";
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function titleize(kind: string) {
  return kind.replaceAll("_", " ");
}

export function ComposerPendingApprovalPanel(props: { approval: PendingApproval }) {
  const [pending, setPending] = useState<"approve" | "deny" | null>(null);

  async function respond(type: "approve" | "deny") {
    const api = readNativeApi();
    if (!api) return;
    setPending(type);
    try {
      await api.orchestration.dispatch(
        type === "approve"
          ? { type: "approve", approvalId: props.approval.id }
          : { type: "deny", approvalId: props.approval.id },
      );
    } finally {
      setPending(null);
    }
  }

  return (
    <section className="rounded-[1.5rem] border border-rose-500/20 bg-rose-500/8 p-4">
      <div className="flex items-center justify-between gap-3">
        <div>
          <p className="text-[11px] font-semibold tracking-[0.22em] text-rose-200 uppercase">
            Approval requested
          </p>
          <h3 className="mt-1 text-sm font-medium text-foreground">
            {titleize(props.approval.kind)}
          </h3>
        </div>
        <span className="rounded-full border border-rose-500/20 bg-background/70 px-2.5 py-1 text-[10px] font-semibold tracking-[0.18em] text-rose-200 uppercase">
          {props.approval.requested_at}
        </span>
      </div>

      <pre className="mt-3 max-h-36 overflow-auto whitespace-pre-wrap break-words rounded-2xl border border-rose-500/15 bg-background/65 px-3 py-3 font-mono text-xs leading-6 text-muted-foreground">
        {formatDetails(props.approval.details)}
      </pre>

      <div className="mt-3 flex items-center justify-end gap-2">
        <button
          className="rounded-full border border-border/70 bg-background/75 px-3 py-2 text-xs font-medium text-muted-foreground transition hover:text-foreground disabled:opacity-60"
          disabled={pending !== null}
          onClick={() => void respond("deny")}
          type="button"
        >
          {pending === "deny" ? "Denying..." : "Deny"}
        </button>
        <button
          className="rounded-full border border-emerald-500/25 bg-emerald-500/12 px-3 py-2 text-xs font-medium text-emerald-100 transition hover:bg-emerald-500/18 disabled:opacity-60"
          disabled={pending !== null}
          onClick={() => void respond("approve")}
          type="button"
        >
          {pending === "approve" ? "Approving..." : "Approve"}
        </button>
      </div>
    </section>
  );
}
