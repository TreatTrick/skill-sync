import { invoke } from '@tauri-apps/api/core'

import type { AppError } from '@/shared/schemas'

/** Error thrown when a Tauri command returns a structured AppError. */
class SkillSyncError extends Error {
  readonly kind: string

  constructor(err: AppError) {
    super(err.message)
    this.name = 'SkillSyncError'
    this.kind = err.kind
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
    if (
      raw !== null &&
      typeof raw === 'object' &&
      'kind' in raw &&
      'message' in raw
    ) {
      throw new SkillSyncError(raw as AppError)
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
