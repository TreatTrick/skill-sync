import { z } from 'zod'

import { recoveryInfoSchema } from '@/shared/schemas'

const namespaceSchema = z.enum(['agents', 'codex', 'claude-code'])

const syncStatusSchema = z.enum([
  'synced',
  'local_update',
  'remote_update',
  'local_deleted',
  'remote_deleted',
  'both_deleted',
  'conflict',
  'blocked',
  'unknown',
])

export type SyncStatus = z.infer<typeof syncStatusSchema>

const conflictReasonSchema = z.enum([
  'same_name_first_seen',
  'both_changed',
  'local_deleted_remote_changed',
  'remote_deleted_local_changed',
])

export type ConflictReason = z.infer<typeof conflictReasonSchema>

const deleteDirectionSchema = z.enum(['delete_local', 'delete_remote'])

const syncDecisionSchema = z.enum([
  'keep_local',
  'use_remote',
  'delete_remote',
  'restore_remote',
  'accept_delete',
  'skip',
])

export type SyncDecision = z.infer<typeof syncDecisionSchema>

const syncSkillEntrySchema = z.object({
  action_id: z.string(),
  skill_id: z.string(),
  name: z.string(),
  namespace: namespaceSchema,
  folder_name: z.string(),
  relative_dir: z.string().nullable(),
  status: syncStatusSchema,
  local_hash: z.string().nullable(),
  remote_hash: z.string().nullable(),
  base_hash: z.string().nullable(),
  local_path: z.string().nullable(),
  remote_blob: z.string().nullable(),
  conflict_reason: conflictReasonSchema.nullable(),
  delete_direction: deleteDirectionSchema.nullable(),
  blocked_reason: z.string().nullable(),
  warnings: z.array(z.string()),
})

export type SyncSkillEntry = z.infer<typeof syncSkillEntrySchema>

const conflictSchema = z.object({
  skill_id: z.string(),
  name: z.string(),
  namespace: namespaceSchema,
  folder_name: z.string(),
  relative_dir: z.string().nullable(),
  conflict_reason: conflictReasonSchema,
  local_hash: z.string().nullable(),
  remote_hash: z.string().nullable(),
  base_hash: z.string().nullable(),
  local_path: z.string().nullable(),
  remote_blob: z.string().nullable(),
  warnings: z.array(z.string()),
})

export type Conflict = z.infer<typeof conflictSchema>

const blockedSkillSchema = z.object({
  skill_id: z.string(),
  name: z.string(),
  namespace: namespaceSchema,
  folder_name: z.string(),
  reason: z.string(),
})

const commitSummarySchema = z.object({
  uploads: z.number().int().nonnegative(),
  downloads: z.number().int().nonnegative(),
  delete_remote: z.number().int().nonnegative(),
  delete_local: z.number().int().nonnegative(),
  local_state_updates: z.number().int().nonnegative(),
})

const baseAdoptionSchema = z.object({
  skill_id: z.string(),
  hash: z.string(),
})

export const syncPlanSchema = z.object({
  entries: z.array(syncSkillEntrySchema),
  uploads: z.array(z.string()),
  downloads: z.array(z.string()),
  delete_remote: z.array(z.string()),
  delete_local: z.array(z.string()),
  conflicts: z.array(conflictSchema),
  blocked: z.array(blockedSkillSchema),
  warnings: z.array(z.string()),
  delete_guard_tripped: z.boolean(),
  expected_remote_commit: z.string(),
  plan_fingerprint: z.string(),
  base_adoptions: z.array(baseAdoptionSchema),
  base_removals: z.array(z.string()),
  will_create_commit: z.boolean(),
  commit_summary: commitSummarySchema,
})

export type SyncPlan = z.infer<typeof syncPlanSchema>

const applyResultSchema = z.object({
  applied: z.array(z.string()),
  state_updated: z.array(z.string()),
  warnings: z.array(z.string()),
  remote_commit: z.string().nullable(),
})

export const applySyncRequestSchema = z.object({
  expected_remote_commit: z.string(),
  plan_fingerprint: z.string(),
  selected_action_ids: z.array(z.string()),
  decisions: z.record(z.string(), syncDecisionSchema),
  delete_guard_ack: z.boolean(),
})

export type ApplySyncRequest = z.infer<typeof applySyncRequestSchema>

export const applySyncResponseSchema = z.discriminatedUnion('status', [
  z.object({ status: z.literal('applied'), result: applyResultSchema }),
  z.object({
    status: z.literal('plan_changed'),
    reason: z.enum(['remote_changed', 'plan_changed']),
    latest_plan: syncPlanSchema,
  }),
  z.object({
    status: z.literal('recovery_required'),
    recovery: recoveryInfoSchema,
  }),
])

export type ApplySyncResponse = z.infer<typeof applySyncResponseSchema>
