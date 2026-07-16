import { z } from 'zod'

import { namespaceSchema } from './apiResponse'

const skillSchema = z.object({
  id: z.string(),
  name: z.string(),
  folder_name: z.string(),
  description: z.string(),
  namespace: namespaceSchema,
  relative_dir: z.string(),
  source_path: z.string(),
  hash: z.string(),
  zip_size: z.number().optional(),
  modified_at: z.string(),
})

const scanRootStatusSchema = z.object({
  namespace: namespaceSchema,
  root_path: z.string(),
  exists: z.boolean(),
  readable: z.boolean(),
  scan_complete: z.boolean(),
  error: z.string().nullable(),
})

const scanCollisionSchema = z.object({
  namespace: namespaceSchema,
  collision_key: z.string(),
  kind: z.enum(['normalized_id', 'folded_folder_name']),
  skill_ids: z.array(z.string()),
  paths: z.array(z.string()),
})

export const scanResultSchema = z.object({
  skills: z.array(skillSchema),
  warnings: z.array(z.string()),
  roots: z.array(scanRootStatusSchema),
  collisions: z.array(scanCollisionSchema),
})

export type ScanResult = z.infer<typeof scanResultSchema>
