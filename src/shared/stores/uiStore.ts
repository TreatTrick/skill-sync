import { create } from 'zustand'

interface UiStoreState {
  sidebarCollapsed: boolean
  setSidebarCollapsed: (sidebarCollapsed: boolean) => void
}

export const useUiStore = create<UiStoreState>((set) => ({
  sidebarCollapsed: false,
  setSidebarCollapsed: (sidebarCollapsed) => set({ sidebarCollapsed }),
}))
