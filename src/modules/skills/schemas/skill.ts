import { z } from 'zod'

const namespaceSchema = z.enum(['agents', 'codex', 'claude-code'])

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
  // Transitional fields used only by legacy consumers until Task 16.
  host: z.string(),
  repo_path: z.string(),
  enabled: z.boolean(),
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
  // The legacy scan_skills command does not return roots/collisions yet; default([])
  // keeps parsing working until the new vault scanner is wired in (Task 16).
  roots: z.array(scanRootStatusSchema).default([]),
  collisions: z.array(scanCollisionSchema).default([]),
})

export type Skill = z.infer<typeof skillSchema>
export type ScanRootStatus = z.infer<typeof scanRootStatusSchema>
export type ScanCollision = z.infer<typeof scanCollisionSchema>
export type ScanResult = z.infer<typeof scanResultSchema>
