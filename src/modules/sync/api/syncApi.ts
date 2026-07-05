import { invokeCmd } from '@/shared/lib'

import {
  applyResultSchema,
  syncPlanSchema,
  type ApplyResult,
  type SyncPlan,
} from '../schemas/syncPlan'

export const getSyncPlan = async (): Promise<SyncPlan> => {
  const raw = await invokeCmd<unknown>('get_sync_plan')
  return syncPlanSchema.parse(raw)
}

export const applySyncPlan = async (
  decisions: Record<string, string>,
): Promise<ApplyResult> => {
  const raw = await invokeCmd<unknown>('apply_sync_plan', { decisions })
  return applyResultSchema.parse(raw)
}
