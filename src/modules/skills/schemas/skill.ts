import { z } from 'zod'

export const skillSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string(),
  host: z.string(),
  source_path: z.string(),
  repo_path: z.string(),
  hash: z.string(),
  modified_at: z.string(),
  enabled: z.boolean(),
})

export type Skill = z.infer<typeof skillSchema>

export const scanResultSchema = z.object({
  skills: z.array(skillSchema),
  warnings: z.array(z.string()),
})

export type ScanResult = z.infer<typeof scanResultSchema>
