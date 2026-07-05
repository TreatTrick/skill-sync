// Reactive UI state backed by localStorage. Replaces the Zustand uiStore.
const STORAGE_KEY = 'skill-sync.sidebar-collapsed'

const readStoredCollapsed = (): boolean =>
  window.localStorage.getItem(STORAGE_KEY) === 'true'

class UiState {
  sidebarCollapsed = $state<boolean>(readStoredCollapsed())

  setSidebarCollapsed(sidebarCollapsed: boolean): void {
    window.localStorage.setItem(STORAGE_KEY, String(sidebarCollapsed))
    this.sidebarCollapsed = sidebarCollapsed
  }
}

export const uiState = new UiState()
