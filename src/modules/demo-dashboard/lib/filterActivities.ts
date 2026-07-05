import type { DemoActivity, DemoStatus } from '../types/dashboardData'

export const filterActivitiesByStatus = (
  activities: DemoActivity[],
  status: DemoStatus | 'all',
): DemoActivity[] => {
  if (status === 'all') {
    return activities
  }

  return activities.filter((activity) => activity.status === status)
}
