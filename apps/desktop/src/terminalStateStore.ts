import { create } from "zustand";

interface TerminalUiState {
  openByThreadId: Record<string, boolean>;
  heightByThreadId: Record<string, number>;
  setOpen: (threadId: string, isOpen: boolean) => void;
  setHeight: (threadId: string, height: number) => void;
}

const DEFAULT_HEIGHT = 260;

export const useTerminalStateStore = create<TerminalUiState>((set) => ({
  openByThreadId: {},
  heightByThreadId: {},
  setOpen: (threadId, isOpen) =>
    set((state) => ({
      openByThreadId: {
        ...state.openByThreadId,
        [threadId]: isOpen,
      },
    })),
  setHeight: (threadId, height) =>
    set((state) => ({
      heightByThreadId: {
        ...state.heightByThreadId,
        [threadId]: height,
      },
    })),
}));

export function getTerminalOpen(threadId: string) {
  return useTerminalStateStore.getState().openByThreadId[threadId] ?? false;
}

export function getTerminalHeight(threadId: string) {
  return useTerminalStateStore.getState().heightByThreadId[threadId] ?? DEFAULT_HEIGHT;
}
