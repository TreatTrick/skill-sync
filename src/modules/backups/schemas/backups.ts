import { z } from 'zod'

export const backupEntrySchema = z.object({
  id: z.string(),
  skill_id: z.string(),
  original_path: z.string(),
  created_at: z.string(),
  size: z.number(),
})

export type BackupEntry = z.infer<typeof backupEntrySchema>
