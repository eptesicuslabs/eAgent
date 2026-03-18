import { useState } from "react";
import { useStore } from "~/store";
import { buildThreadViewModel } from "./ChatView.logic";
import { ComposerPromptEditor } from "./ComposerPromptEditor";
import { DiffPanelShell } from "./DiffPanelShell";
import { PlanSidebar } from "./PlanSidebar";
import { ThreadTerminalDrawer } from "./ThreadTerminalDrawer";
import { ChatHeader } from "./chat/ChatHeader";
import { MessagesTimeline } from "./chat/MessagesTimeline";

export function ChatView(props: { threadId: string }) {
  const snapshot = useStore((state) => state.snapshot);
  const [showPlan, setShowPlan] = useState(true);
  const [showDiff, setShowDiff] = useState(true);

  const view = buildThreadViewModel(snapshot, props.threadId);

  if (!view.thread) {
    return (
      <section className="flex h-full items-center justify-center bg-background">
        <div className="rounded-[1.8rem] border border-border/70 bg-card/75 px-8 py-10 text-center">
          <p className="text-xs font-semibold tracking-[0.24em] text-muted-foreground uppercase">
            Thread missing
          </p>
          <h1 className="mt-3 text-2xl font-semibold text-foreground">
            This thread no longer exists.
          </h1>
        </div>
      </section>
    );
  }

  return (
    <section className="grid h-full min-w-0 grid-rows-[auto_1fr_auto] overflow-hidden bg-background">
      <ChatHeader
        onToggleDiff={() => setShowDiff((value) => !value)}
        onTogglePlan={() => setShowPlan((value) => !value)}
        showDiff={showDiff}
        showPlan={showPlan}
        thread={view.thread}
        project={view.project}
      />

      <div className="grid min-h-0 min-w-0 grid-cols-[minmax(0,1fr)_380px] overflow-hidden max-[1220px]:grid-cols-[minmax(0,1fr)]">
        <div className="grid min-h-0 min-w-0 grid-rows-[1fr_auto] overflow-hidden border-r border-border/60 max-[1220px]:border-r-0">
          <MessagesTimeline thread={view.thread} timeline={view.timeline} />
          <ComposerPromptEditor thread={view.thread} />
        </div>

        <div className="min-h-0 overflow-hidden max-[1220px]:hidden">
          <div className="grid h-full min-h-0 grid-rows-[minmax(0,1fr)_minmax(0,1fr)]">
            {showPlan ? (
              <PlanSidebar
                project={view.project}
                steps={view.planSteps}
                thread={view.thread}
              />
            ) : (
              <div className="border-b border-border/60 bg-card/30" />
            )}
            {showDiff ? (
              <DiffPanelShell
                changedFiles={view.changedFiles}
                project={view.project}
                thread={view.thread}
              />
            ) : (
              <div className="bg-card/20" />
            )}
          </div>
        </div>
      </div>

      <ThreadTerminalDrawer terminals={view.terminals} thread={view.thread} />
    </section>
  );
}
