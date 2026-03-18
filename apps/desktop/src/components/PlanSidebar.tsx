import type { ProjectState, ThreadState, TurnStatus } from "~/types";

function toneForStatus(status: TurnStatus) {
  switch (status) {
    case "completed":
      return "bg-emerald-500/12 text-emerald-300";
    case "running":
      return "bg-sky-500/12 text-sky-300";
    case "waiting":
      return "bg-amber-500/12 text-amber-300";
    case "failed":
      return "bg-rose-500/12 text-rose-300";
    default:
      return "bg-muted/60 text-muted-foreground";
  }
}

export function PlanSidebar(props: {
  project: ProjectState | null;
  thread: ThreadState;
  steps: Array<{ id: string; label: string; detail: string; status: TurnStatus }>;
}) {
  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden bg-[linear-gradient(180deg,color-mix(in_srgb,var(--card)_92%,transparent)_0%,color-mix(in_srgb,var(--background)_98%,black)_100%)] px-4 py-4">
      <div className="mb-3">
        <p className="text-[11px] font-semibold tracking-[0.24em] text-muted-foreground uppercase">
          Plan
        </p>
        <h2 className="text-sm font-medium text-foreground">
          {props.project?.name ?? "Workspace"} execution
        </h2>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        {props.steps.length === 0 ? (
          <div className="rounded-[1.4rem] border border-border/70 bg-card/55 p-4 text-sm text-muted-foreground">
            No proposed plan or turn history yet.
          </div>
        ) : (
          <div className="grid gap-3">
            {props.steps.map((step) => (
              <article
                key={step.id}
                className="rounded-[1.4rem] border border-border/70 bg-card/55 p-4"
              >
                <div className="flex items-center justify-between gap-3">
                  <p className="text-sm font-medium text-foreground">{step.label}</p>
                  <span
                    className={`rounded-full px-2.5 py-1 text-[10px] font-semibold tracking-[0.18em] uppercase ${toneForStatus(step.status)}`}
                  >
                    {step.status}
                  </span>
                </div>
                <p className="mt-3 text-sm leading-6 text-muted-foreground">{step.detail}</p>
              </article>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
