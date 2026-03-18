import { create } from "zustand";

interface ComposerDraftState {
  promptByThreadId: Record<string, string>;
  setPrompt: (threadId: string, prompt: string) => void;
  clearPrompt: (threadId: string) => void;
}

export const useComposerDraftStore = create<ComposerDraftState>((set) => ({
  promptByThreadId: {},
  setPrompt: (threadId, prompt) =>
    set((state) => ({
      promptByThreadId: {
        ...state.promptByThreadId,
        [threadId]: prompt,
      },
    })),
  clearPrompt: (threadId) =>
    set((state) => {
      const next = { ...state.promptByThreadId };
      delete next[threadId];
      return { promptByThreadId: next };
    }),
}));
