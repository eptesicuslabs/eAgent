import { useEffect, useRef, useState } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { Eraser, TerminalSquare, X } from "lucide-react";
import { getTerminalHeight, useTerminalStateStore } from "~/terminalStateStore";
import { readNativeApi } from "~/nativeApi";
import type { TerminalRecord, ThreadState } from "~/types";

export function ThreadTerminalDrawer(props: {
  thread: ThreadState;
  terminals: TerminalRecord[];
}) {
  const [input, setInput] = useState("");
  const terminal =
    props.terminals.find((item) => item.threadId === props.thread.id) ?? props.terminals[0] ?? null;
  const isOpen = useTerminalStateStore(
    (state) => state.openByThreadId[props.thread.id] ?? props.terminals.length > 0,
  );
  const height = useTerminalStateStore(
    (state) => state.heightByThreadId[props.thread.id] ?? getTerminalHeight(props.thread.id),
  );
  const setOpen = useTerminalStateStore((state) => state.setOpen);
  const setHeight = useTerminalStateStore((state) => state.setHeight);
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const xtermRef = useRef<XTerm | null>(null);

  useEffect(() => {
    if (!viewportRef.current || !terminal || !isOpen) return;

    const instance =
      xtermRef.current ??
      new XTerm({
        convertEol: true,
        disableStdin: true,
        fontFamily: '"SF Mono", Consolas, monospace',
        fontSize: 12,
        theme: {
          background: "#111315",
          foreground: "#f2f4f8",
          cursor: "#f2f4f8",
          selectionBackground: "rgba(255,255,255,0.15)",
        },
      });

    if (!xtermRef.current) {
      instance.open(viewportRef.current);
      xtermRef.current = instance;
    }

    instance.reset();
    instance.write((terminal.buffer || "").replace(/\n/g, "\r\n"));
  }, [isOpen, terminal]);

  useEffect(() => {
    if (!terminal || !isOpen) return;
    const node = viewportRef.current;
    if (!node) return;

    const resizeObserver = new ResizeObserver(() => {
      const cols = Math.max(40, Math.floor(node.clientWidth / 8));
      const rows = Math.max(10, Math.floor(node.clientHeight / 18));
      const api = readNativeApi();
      if (!api) return;
      void api.terminal.resize(terminal.id, cols, rows);
    });

    resizeObserver.observe(node);
    return () => resizeObserver.disconnect();
  }, [isOpen, terminal]);

  if (!terminal && !isOpen) {
    return null;
  }

  return (
    <section className="thread-terminal-drawer border-t border-border/70 bg-[#101214]/95">
      <button
        className="flex h-11 w-full items-center justify-between px-6 text-left"
        onClick={() => setOpen(props.thread.id, !isOpen)}
        type="button"
      >
        <span className="inline-flex items-center gap-2 text-sm font-medium text-foreground">
          <TerminalSquare className="size-4 text-emerald-300" />
          {terminal?.title ?? "Terminal"}
        </span>
        <span className="text-xs text-muted-foreground">
          {isOpen ? "Collapse" : "Expand"}
        </span>
      </button>

      {isOpen ? (
        <div className="grid grid-rows-[1fr_auto_auto]" style={{ height }}>
          <div className="border-t border-border/70 bg-[#111315] p-3">
            <div className="h-full rounded-2xl border border-white/8 bg-[#111315]">
              <div ref={viewportRef} className="h-full w-full px-2 py-2" />
            </div>
          </div>
          <div className="flex items-center gap-2 border-t border-white/8 bg-[#0d0f10] px-4 py-3">
            <button
              className="inline-flex size-9 items-center justify-center rounded-xl border border-white/10 bg-white/5 text-muted-foreground transition hover:text-foreground"
              onClick={async () => {
                if (!terminal) return;
                const api = readNativeApi();
                if (!api) return;
                await api.terminal.clear(terminal.id);
              }}
              type="button"
            >
              <Eraser className="size-4" />
            </button>
            <input
              className="h-10 min-w-0 flex-1 rounded-xl border border-white/10 bg-white/5 px-4 text-sm text-foreground outline-none placeholder:text-muted-foreground/55"
              onChange={(event) => setInput(event.target.value)}
              onKeyDown={async (event) => {
                if (event.key !== "Enter" || !terminal || !input.trim()) return;
                const api = readNativeApi();
                if (!api) return;
                await api.terminal.write(terminal.id, `${input}\n`);
                setInput("");
              }}
              placeholder="Write to the active terminal"
              value={input}
            />
            <button
              className="inline-flex size-9 items-center justify-center rounded-xl border border-white/10 bg-white/5 text-muted-foreground transition hover:text-foreground"
              onClick={async () => {
                if (!terminal) {
                  setOpen(props.thread.id, false);
                  return;
                }
                const api = readNativeApi();
                if (!api) return;
                await api.terminal.close(terminal.id);
                setOpen(props.thread.id, false);
              }}
              type="button"
            >
              <X className="size-4" />
            </button>
          </div>
          <div className="flex justify-center border-t border-white/8 bg-[#0d0f10] py-1">
            <div
              className="h-1.5 w-16 cursor-row-resize rounded-full bg-white/10"
              onMouseDown={(event) => {
                event.preventDefault();
                const startY = event.clientY;
                const startHeight = height;
                const onMove = (moveEvent: MouseEvent) => {
                  setHeight(props.thread.id, Math.max(180, startHeight - (moveEvent.clientY - startY)));
                };
                const onUp = () => {
                  window.removeEventListener("mousemove", onMove);
                  window.removeEventListener("mouseup", onUp);
                };
                window.addEventListener("mousemove", onMove);
                window.addEventListener("mouseup", onUp);
              }}
            />
          </div>
        </div>
      ) : null}
    </section>
  );
}
