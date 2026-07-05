import { dashboardDataSchema } from '../schemas/dashboardData'
import type { DashboardData } from '../types/dashboardData'

const rawDashboardData: DashboardData = {
  metrics: [
    {
      id: 'health',
      labelKey: 'demoDashboard.metrics.health',
      value: '98%',
      detail: '+2.4%',
    },
    {
      id: 'tasks',
      labelKey: 'demoDashboard.metrics.tasks',
      value: '24',
      detail: '+6',
    },
    {
      id: 'latency',
      labelKey: 'demoDashboard.metrics.latency',
      value: '128ms',
      detail: '-12ms',
    },
    {
      id: 'users',
      labelKey: 'demoDashboard.metrics.users',
      value: '12',
      detail: '+3',
    },
  ],
  activities: [
    {
      id: 'activity-1',
      title: 'Design tokens synced',
      owner: 'Frontend',
      status: 'done',
      updatedAt: '09:30',
    },
    {
      id: 'activity-2',
      title: 'Responsive lint reviewed',
      owner: 'Platform',
      status: 'running',
      updatedAt: '10:15',
    },
    {
      id: 'activity-3',
      title: 'i18n keys prepared',
      owner: 'Experience',
      status: 'done',
      updatedAt: '11:20',
    },
  ],
}

export const getDashboardData = async (): Promise<DashboardData> =>
  dashboardDataSchema.parse(rawDashboardData)
