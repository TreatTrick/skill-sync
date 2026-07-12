import { z } from 'zod'

import { invokeCmd } from '@/shared/lib'

import {
  applySyncRequestSchema,
  applySyncResponseSchema,
  syncPlanSchema,
  type ApplySyncRequest,
  type ApplySyncResponse,
  type SyncPlan,
} from '../schemas/syncPlan'

const skillIdsSchema = z.array(z.string())

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

export const uploadSkills = async (
  skillIds: string[],
): Promise<ApplySyncResponse> => {
  const parsedSkillIds = skillIdsSchema.parse(skillIds)
  const raw = await invokeCmd<unknown>('upload_skills', {
    skillIds: parsedSkillIds,
  })
  return applySyncResponseSchema.parse(raw)
}

export const downloadSkills = async (
  skillIds: string[],
): Promise<ApplySyncResponse> => {
  const parsedSkillIds = skillIdsSchema.parse(skillIds)
  const raw = await invokeCmd<unknown>('download_skills', {
    skillIds: parsedSkillIds,
  })
  return applySyncResponseSchema.parse(raw)
}

export const resumeSyncRecovery = async (
  taskId: string,
): Promise<ApplySyncResponse> => {
  const parsedTaskId = z.string().min(1).parse(taskId)
  const raw = await invokeCmd<unknown>('resume_sync_recovery', {
    taskId: parsedTaskId,
  })
  return applySyncResponseSchema.parse(raw)
}
