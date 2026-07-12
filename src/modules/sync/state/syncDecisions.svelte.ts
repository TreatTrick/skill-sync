import type { SyncDecision } from '../schemas/syncPlan'

// Reactive sync-conflict decisions. Replaces the Zustand syncDecisionsStore;
// shared between the Conflicts and Sync Preview pages.
class SyncDecisionsState {
  decisions = $state<Record<string, SyncDecision>>({})

  setDecision(id: string, choice: SyncDecision): void {
    this.decisions = { ...this.decisions, [id]: choice }
  }

  clear(): void {
    this.decisions = {}
  }
}

export const syncDecisions = new SyncDecisionsState()
