import type { KeyboardEvent } from "react";
import { useState } from "react";
import { readNativeApi } from "~/nativeApi";
import { useStore } from "~/store";

export function Composer() {
  const [prompt, setPrompt] = useState("");
  const [sending, setSending] = useState(false);
  const selectedGraphId = useStore((s) => s.selectedGraphId);
  const graph = selectedGraphId ? useStore.getState().activeGraphs.get(selectedGraphId) : null;
  const isRunning = graph?.status === "running" || graph?.status === "planning";

  async function submit() {
    const text = prompt.trim();
    if (!text || sending) return;
    setSending(true);
    try {
      const api = readNativeApi();
      const result = await api.eagent.submitTask(text);
      if (result?.graphId) useStore.getState().selectGraph(result.graphId);
      setPrompt("");
    } catch (e) {
      console.error("[eAgent]", e);
    } finally {
      setSending(false);
    }
  }

  async function cancel() {
    if (!selectedGraphId) return;
    try {
      await readNativeApi().eagent.cancelGraph(selectedGraphId);
    } catch (e) {
      console.error("[eAgent]", e);
    }
  }

  return (
    <div className="shrink-0 border-t border-border px-4 py-2">
      <div className="flex gap-2 items-end">
        <textarea
          className="flex-1 resize-none bg-neutral-900 border border-border px-3 py-2 text-xs text-foreground outline-none focus:border-neutral-600 placeholder:text-neutral-600"
          rows={2}
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          onKeyDown={(e: KeyboardEvent<HTMLTextAreaElement>) => {
            if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
              e.preventDefault();
              void submit();
            }
          }}
          placeholder={isRunning ? "agent working..." : "describe task (ctrl+enter to submit)"}
        />
        <div className="flex flex-col gap-1">
          <button
            className="px-3 py-2 text-xs bg-foreground text-background font-bold hover:opacity-90 disabled:opacity-30"
            disabled={!prompt.trim() || sending}
            onClick={() => void submit()}
            type="button"
          >
            {sending ? "..." : "run"}
          </button>
          {isRunning ? (
            <button
              className="px-3 py-1 text-xs text-red-500 border border-red-500/30 hover:bg-red-500/10"
              onClick={() => void cancel()}
              type="button"
            >stop</button>
          ) : null}
        </div>
      </div>
    </div>
  );
}
