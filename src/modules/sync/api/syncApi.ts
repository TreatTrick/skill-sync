import { invokeCmd } from '@/shared/lib'

import {
  applySyncRequestSchema,
  applySyncResponseSchema,
  syncPlanSchema,
  type ApplySyncRequest,
  type ApplySyncResponse,
  type SyncPlan,
} from '../schemas/syncPlan'

export const getSyncPlan = async (): Promise<SyncPlan> => {
  const raw = await invokeCmd<unknown>('get_sync_plan')
  return syncPlanSchema.parse(raw)
}

export const applySyncPlan = async (
  request: ApplySyncRequest,
): Promise<ApplySyncResponse> => {
  const parsedRequest = applySyncRequestSchema.parse(request)
  const raw = await invokeCmd<unknown>('apply_sync_plan', {
    request: parsedRequest,
  })
  return applySyncResponseSchema.parse(raw)
}

export const resumeSyncRecovery = async (
  taskId: string,
): Promise<ApplySyncResponse> => {
  const raw = await invokeCmd<unknown>('resume_sync_recovery', {
    taskId,
  })
  return applySyncResponseSchema.parse(raw)
}
