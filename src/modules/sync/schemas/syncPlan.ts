import { z } from 'zod'

export const syncActionSchema = z.object({
  skill_id: z.string(),
  name: z.string(),
  host: z.string(),
  source_path: z.string(),
  repo_path: z.string(),
  direction: z.string(),
  local_hash: z.string(),
  remote_hash: z.string(),
})

export type SyncAction = z.infer<typeof syncActionSchema>

export const conflictSchema = z.object({
  skill_id: z.string(),
  name: z.string(),
  host: z.string(),
  local_path: z.string(),
  repo_path: z.string(),
  local_hash: z.string(),
  remote_hash: z.string(),
  reason: z.string(),
})

export type Conflict = z.infer<typeof conflictSchema>

export const syncPlanSchema = z.object({
  uploads: z.array(syncActionSchema),
  downloads: z.array(syncActionSchema),
  updates: z.array(syncActionSchema),
  deletes: z.array(syncActionSchema),
  conflicts: z.array(conflictSchema),
  warnings: z.array(z.string()),
})

export type SyncPlan = z.infer<typeof syncPlanSchema>

export const applyResultSchema = z.object({
  applied: z.array(z.string()),
  backups: z.array(z.string()),
  warnings: z.array(z.string()),
})

export type ApplyResult = z.infer<typeof applyResultSchema>
