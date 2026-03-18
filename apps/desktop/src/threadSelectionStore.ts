import { create } from "zustand";

interface ThreadSelectionState {
  selectedThreadIds: string[];
  toggleThread: (threadId: string) => void;
  clearSelection: () => void;
}

export const useThreadSelectionStore = create<ThreadSelectionState>((set) => ({
  selectedThreadIds: [],
  toggleThread: (threadId) =>
    set((state) => ({
      selectedThreadIds: state.selectedThreadIds.includes(threadId)
        ? state.selectedThreadIds.filter((value) => value !== threadId)
        : [...state.selectedThreadIds, threadId],
    })),
  clearSelection: () => set({ selectedThreadIds: [] }),
}));
