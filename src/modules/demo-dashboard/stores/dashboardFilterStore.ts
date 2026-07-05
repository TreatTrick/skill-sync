import { create } from 'zustand'

import type { DemoStatus } from '../types/dashboardData'

export type DashboardStatusFilter = DemoStatus | 'all'

interface DashboardFilterStoreState {
  statusFilter: DashboardStatusFilter
  setStatusFilter: (statusFilter: DashboardStatusFilter) => void
}

export const useDashboardFilterStore = create<DashboardFilterStoreState>(
  (set) => ({
    statusFilter: 'all',
    setStatusFilter: (statusFilter) => set({ statusFilter }),
  }),
)
