import { z } from 'zod'

export const gitCheckSchema = z.object({
  available: z.boolean(),
  version: z.string(),
})

export type GitCheck = z.infer<typeof gitCheckSchema>

export const remoteCheckSchema = z.object({
  ok: z.boolean(),
  message: z.string(),
})

export type RemoteCheck = z.infer<typeof remoteCheckSchema>
