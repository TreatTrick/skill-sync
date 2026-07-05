import { invokeCmd } from '@/shared/lib'

import { backupEntrySchema, type BackupEntry } from '../schemas/backups'

export const listBackups = async (): Promise<BackupEntry[]> => {
  const raw = await invokeCmd<unknown>('list_backups')
  return backupEntrySchema.array().parse(raw)
}

export const restoreBackup = async (
  backupId: string,
  targetPath: string,
): Promise<void> => {
  await invokeCmd('restore_backup', { backupId, targetPath })
}
