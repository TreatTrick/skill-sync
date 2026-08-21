import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { error as logError } from '@tauri-apps/plugin-log'

import {
  appErrorSchema,
  syncProgressEventSchema,
  type AppError,
  type SyncProgressEvent,
} from '@/shared/schemas'

const redactLogMessage = (message: string): string =>
  message
    .replace(
      /((?:access_token|refresh_token|device_code|user_code|client_secret|private_key)\s*[:=]\s*)("[^"]*"|\S+)/gi,
      '$1[REDACTED]',
    )
    .replace(/Bearer\s+\S+/gi, 'Bearer [REDACTED]')

const logInvokeError = (
  command: string,
  kind: string,
  message: string,
): void => {
  void logError(
    `command=${command} kind=${kind} error=${redactLogMessage(message)}`,
  ).catch(() => undefined)
}

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
    const kind = parsed.success ? parsed.data.kind : 'other'
    const message = parsed.success ? parsed.data.message : String(raw)
    logInvokeError(cmd, kind, message)
    if (parsed.success) {
      throw new SkillSyncError(parsed.data)
    }
    throw new SkillSyncError({
      kind: 'other',
      message: String(raw),
    })
  }
}

export const invokeWithProgress = async <T>(
  cmd: string,
  args: Record<string, unknown> | undefined,
  onProgress: (event: SyncProgressEvent) => void,
): Promise<T> => {
  let operationId: string | null = null
  const unlisten = await listen<unknown>('sync-progress', (event) => {
    const parsed = syncProgressEventSchema.safeParse(event.payload)
    if (!parsed.success) return
    operationId ??= parsed.data.operation_id
    if (parsed.data.operation_id === operationId) onProgress(parsed.data)
  })
  try {
    return await invokeCmd<T>(cmd, args)
  } finally {
    unlisten()
  }
}

/** Open a path in the OS file manager. */
export const openPath = (path: string): Promise<void> =>
  invokeCmd('open_path', { path })
