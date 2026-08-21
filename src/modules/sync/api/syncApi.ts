import { invokeCmd, invokeWithProgress } from '@/shared/lib'
import type { SyncProgressEvent } from '@/shared/schemas'

import {
  applySyncRequestSchema,
  applySyncResponseSchema,
  syncPlanSchema,
  type ApplySyncRequest,
  type ApplySyncResponse,
  type SyncPlan,
} from '../schemas/syncPlan'

export const getSyncPlan = async (
  onProgress?: (event: SyncProgressEvent) => void,
): Promise<SyncPlan> => {
  const raw = onProgress
    ? await invokeWithProgress<unknown>('get_sync_plan', undefined, onProgress)
    : await invokeCmd<unknown>('get_sync_plan')
  return syncPlanSchema.parse(raw)
}

export const applySyncPlan = async (
  request: ApplySyncRequest,
  onProgress?: (event: SyncProgressEvent) => void,
): Promise<ApplySyncResponse> => {
  const parsedRequest = applySyncRequestSchema.parse(request)
  const args = { request: parsedRequest }
  const raw = onProgress
    ? await invokeWithProgress<unknown>('apply_sync_plan', args, onProgress)
    : await invokeCmd<unknown>('apply_sync_plan', args)
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
