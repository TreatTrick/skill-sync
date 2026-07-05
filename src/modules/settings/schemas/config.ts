import { z } from 'zod'

export const repositoryConfigSchema = z.object({
  local_path: z.string(),
  remote: z.string(),
  branch: z.string(),
})

export const hostConfigSchema = z.object({
  enabled: z.boolean(),
  paths: z.array(z.string()),
})

export const appConfigSchema = z.object({
  version: z.number(),
  repository: repositoryConfigSchema,
  defaults: z.object({
    backup: z.boolean(),
    install_mode: z.string(),
  }),
  hosts: z.object({
    codex: hostConfigSchema,
    claude: hostConfigSchema,
  }),
  custom_paths: z.array(z.string()),
  ignore: z.array(z.string()),
})

export type AppConfig = z.infer<typeof appConfigSchema>

export const appStateSchema = z.object({
  configured: z.boolean(),
  config: appConfigSchema,
  git_available: z.boolean(),
  git_version: z.string(),
})

export type AppState = z.infer<typeof appStateSchema>
