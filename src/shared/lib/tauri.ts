import { invoke } from '@tauri-apps/api/core'

import { appErrorSchema, type AppError } from '@/shared/schemas'

/** Error thrown when a Tauri command returns a structured AppError. */
export class SkillSyncError extends Error {
  readonly kind: string
  readonly retryAfter: string | null | undefined
  readonly latestCheck: unknown

  constructor(err: AppError) {
    super(err.message)
    this.name = 'SkillSyncError'
    this.kind = err.kind
    this.retryAfter = err.retry_after
    this.latestCheck = err.latest_check
  }
}

/** Extract a human-readable message from any thrown value. */
export const errorMessage = (value: unknown): string =>
  value instanceof Error ? value.message : String(value)

/** Invoke a Tauri command and rethrow structured errors as SkillSyncError. */
export const invokeCmd = async <T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> => {
  try {
    return await invoke<T>(cmd, args)
  } catch (raw) {
    const parsed = appErrorSchema.safeParse(raw)
    if (parsed.success) {
      throw new SkillSyncError(parsed.data)
    }
    throw new SkillSyncError({
      kind: 'other',
      message: String(raw),
    })
  }
}

/** Open a path in the OS file manager. */
export const openPath = (path: string): Promise<void> =>
  invokeCmd('open_path', { path })
