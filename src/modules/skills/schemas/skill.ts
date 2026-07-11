import { z } from 'zod'

const skillSchema = z.object({
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

export const scanResultSchema = z.object({
  skills: z.array(skillSchema),
  warnings: z.array(z.string()),
})

export type ScanResult = z.infer<typeof scanResultSchema>
