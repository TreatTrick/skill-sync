import type { z } from 'zod'

import type {
  dashboardDataSchema,
  demoActivitySchema,
  demoMetricSchema,
  demoStatusSchema,
} from '../schemas/dashboardData'

export type DemoStatus = z.infer<typeof demoStatusSchema>
export type DemoMetric = z.infer<typeof demoMetricSchema>
export type DemoActivity = z.infer<typeof demoActivitySchema>
export type DashboardData = z.infer<typeof dashboardDataSchema>
