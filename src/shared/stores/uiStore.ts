import { create } from 'zustand'

const STORAGE_KEY = 'skill-sync.sidebar-collapsed'

const readStoredCollapsed = (): boolean =>
  window.localStorage.getItem(STORAGE_KEY) === 'true'

interface UiStoreState {
  sidebarCollapsed: boolean
  setSidebarCollapsed: (sidebarCollapsed: boolean) => void
}

export const useUiStore = create<UiStoreState>((set) => ({
  sidebarCollapsed: readStoredCollapsed(),
  setSidebarCollapsed: (sidebarCollapsed) => {
    window.localStorage.setItem(STORAGE_KEY, String(sidebarCollapsed))
    set({ sidebarCollapsed })
  },
}))
