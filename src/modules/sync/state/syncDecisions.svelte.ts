import type { SyncDecision } from '../schemas/syncPlan'

// Reactive sync-conflict decisions, shared across the Sync Preview page.
class SyncDecisionsState {
  decisions = $state<Record<string, SyncDecision>>({})

  setDecision(id: string, choice: SyncDecision): void {
    this.decisions = { ...this.decisions, [id]: choice }
  }

  removeDecision(id: string): void {
    if (!(id in this.decisions)) return
    const next = { ...this.decisions }
    delete next[id]
    this.decisions = next
  }

  clear(): void {
    this.decisions = {}
  }
}

export const syncDecisions = new SyncDecisionsState()
