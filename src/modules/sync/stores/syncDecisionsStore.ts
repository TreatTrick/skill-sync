import { create } from 'zustand'

export type ConflictChoice = 'local' | 'remote' | 'skip'

interface SyncDecisionsState {
  decisions: Record<string, string>
  setDecision: (id: string, choice: string) => void
  clear: () => void
}

export const useSyncDecisionsStore = create<SyncDecisionsState>((set) => ({
  decisions: {},
  setDecision: (id, choice) =>
    set((state) => ({ decisions: { ...state.decisions, [id]: choice } })),
  clear: () => set({ decisions: {} }),
}))
