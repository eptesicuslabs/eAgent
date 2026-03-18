import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { TimelineEntry } from "~/session-logic";
import type { ThreadState } from "~/types";

function cardTone(entry: TimelineEntry) {
  switch (entry.kind) {
    case "prompt":
      return "border-sky-500/20 bg-sky-500/8";
    case "message":
      return entry.role === "assistant"
        ? "border-border/70 bg-card/75"
        : entry.role === "system"
          ? "border-amber-500/20 bg-amber-500/8"
          : "border-emerald-500/20 bg-emerald-500/8";
    case "approval":
      return "border-rose-500/20 bg-rose-500/8";
    case "input":
      return "border-violet-500/20 bg-violet-500/8";
    case "error":
      return "border-rose-500/25 bg-rose-500/10";
    default:
      return "border-border/70 bg-background/75";
  }
}

function heading(entry: TimelineEntry) {
  switch (entry.kind) {
    case "prompt":
      return "Prompt";
    case "message":
      return entry.role;
    case "approval":
      return entry.title;
    case "input":
      return entry.title;
    case "error":
      return entry.title;
    case "event":
      return entry.title;
  }
}

function body(entry: TimelineEntry) {
  if (entry.kind === "event" || entry.kind === "approval" || entry.kind === "input") {
    return (
      <pre className="overflow-x-auto whitespace-pre-wrap break-words font-mono text-xs leading-6 text-muted-foreground">
        {entry.body}
      </pre>
    );
  }

  return (
    <div className="prose prose-invert max-w-none text-sm leading-7 text-foreground prose-p:my-0 prose-pre:my-0 prose-code:text-[0.95em]">
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{entry.body}</ReactMarkdown>
    </div>
  );
}

export function MessagesTimeline(props: {
  thread: ThreadState;
  timeline: TimelineEntry[];
}) {
  if (props.timeline.length === 0) {
    return (
      <div className="flex h-full items-center justify-center bg-[radial-gradient(circle_at_top,rgba(255,255,255,0.06),transparent_42%)] px-6">
        <div className="max-w-xl rounded-[1.8rem] border border-border/70 bg-card/75 p-8 text-center shadow-[0_24px_80px_rgba(0,0,0,0.18)]">
          <p className="text-xs font-semibold tracking-[0.24em] text-muted-foreground uppercase">
            No transcript yet
          </p>
          <h2 className="mt-3 text-2xl font-semibold tracking-[-0.03em] text-foreground">
            Start the thread with a prompt below.
          </h2>
          <p className="mt-3 text-sm leading-7 text-muted-foreground">
            Provider events, assistant output, approvals, and runtime notes will collect here as
            the session unfolds.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="overflow-auto bg-[radial-gradient(circle_at_top,rgba(255,255,255,0.05),transparent_38%)] px-6 py-6">
      <div className="mx-auto grid max-w-4xl gap-4">
        {props.timeline.map((entry) => (
          <article
            key={entry.id}
            className={`rounded-[1.7rem] border px-5 py-4 shadow-[0_16px_50px_rgba(0,0,0,0.14)] ${cardTone(entry)}`}
          >
            <div className="mb-3 flex items-center justify-between gap-3">
              <div className="min-w-0">
                <p className="truncate text-[11px] font-semibold tracking-[0.22em] text-muted-foreground uppercase">
                  {heading(entry)}
                </p>
                <p className="mt-1 text-xs text-muted-foreground">{entry.createdAt}</p>
              </div>
              {entry.kind === "prompt" ? (
                <span className="rounded-full border border-border/70 bg-background/75 px-2.5 py-1 text-[10px] font-semibold tracking-[0.18em] text-muted-foreground uppercase">
                  {props.thread.settings.interaction_mode}
                </span>
              ) : null}
            </div>

            {body(entry)}
          </article>
        ))}
      </div>
    </div>
  );
}
