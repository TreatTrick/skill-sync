import { z } from 'zod'

export const demoStatusSchema = z.enum(['running', 'done'])

export const demoMetricLabelKeySchema = z.enum([
  'demoDashboard.metrics.health',
  'demoDashboard.metrics.latency',
  'demoDashboard.metrics.tasks',
  'demoDashboard.metrics.users',
])

export const demoMetricSchema = z.object({
  id: z.string(),
  labelKey: demoMetricLabelKeySchema,
  value: z.string(),
  detail: z.string(),
})

export const demoActivitySchema = z.object({
  id: z.string(),
  title: z.string(),
  owner: z.string(),
  status: demoStatusSchema,
  updatedAt: z.string(),
})

export const dashboardDataSchema = z.object({
  metrics: z.array(demoMetricSchema),
  activities: z.array(demoActivitySchema),
})
