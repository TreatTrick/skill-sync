import { z } from 'zod'

// Skill namespace shared by scan results and sync plan entries. Backend adds a
// namespace by extending this enum; both sides must stay in sync.
export const namespaceSchema = z.enum(['agents', 'codex', 'claude-code'])

const githubVaultStatusSchema = z.enum([
  'app_not_installed',
  'repository_forbidden',
  'repository_missing',
  'repository_unavailable',
  'empty_repository',
  'branch_missing',
  'missing_manifest',
  'invalid_manifest',
  'ready',
])

export const recoveryInfoSchema = z.object({
  task_id: z.string(),
  phase: z.enum([
    'remote_outcome_unknown',
    'local_replace_failed',
    'state_save_failed',
  ]),
  remote_commit: z.string().nullable(),
  completed_action_ids: z.array(z.string()),
  pending_action_ids: z.array(z.string()),
  message: z.string(),
})

export type RecoveryInfo = z.infer<typeof recoveryInfoSchema>

const remoteConfigSchema = z.object({
  installation_id: z.number().int().nonnegative(),
  repository_id: z.number().int().nonnegative(),
  owner: z.string(),
  repo: z.string(),
  branch: z.string(),
})

export type RemoteConfig = z.infer<typeof remoteConfigSchema>

const limitsConfigSchema = z
  .object({
    max_skill_zip_bytes: z.number().int().positive(),
    max_skill_files: z.number().int().positive(),
    max_single_file_unpacked_bytes: z.number().int().positive(),
    max_skill_unpacked_bytes: z.number().int().positive(),
    max_auto_delete: z.number().int().nonnegative(),
  })
  .refine(
    (limits) =>
      limits.max_single_file_unpacked_bytes <= limits.max_skill_unpacked_bytes,
    {
      message:
        'max_single_file_unpacked_bytes must not exceed max_skill_unpacked_bytes',
      path: ['max_single_file_unpacked_bytes'],
    },
  )

export const appConfigSchema = z.object({
  version: z.number().int(),
  ignore: z.array(z.string()),
  remote: remoteConfigSchema.nullable(),
  limits: limitsConfigSchema,
  device_id: z.string(),
})

export type AppConfig = z.infer<typeof appConfigSchema>

export const githubVaultCheckSchema = z.object({
  status: githubVaultStatusSchema,
  installation_id: z.number().int().nonnegative().nullable(),
  repository_id: z.number().int().nonnegative().nullable(),
  owner: z.string().nullable(),
  repo: z.string().nullable(),
  branch: z.string().nullable(),
  head_sha: z.string().nullable(),
  manifest_sha: z.string().nullable(),
  retry_after: z.string().nullable(),
  message: z.string().nullable(),
})

export type GithubVaultCheck = z.infer<typeof githubVaultCheckSchema>

export const appStateSchema = z.object({
  configured: z.boolean(),
  config: appConfigSchema,
  github_authorized: z.boolean(),
  github_user: z.string().nullable(),
  github_app_slug: z.string().nullable(),
  credential_status: z.enum([
    'disconnected',
    'valid',
    'refreshing',
    'reauthorization_required',
  ]),
  installation_id: z.number().int().nonnegative().nullable(),
  repository_id: z.number().int().nonnegative().nullable(),
  remote_owner: z.string().nullable(),
  remote_repo: z.string().nullable(),
  remote_branch: z.string().nullable(),
  vault_status: githubVaultStatusSchema.nullable(),
  device_name: z.string(),
  remote_commit: z.string().nullable(),
  pending_recovery: recoveryInfoSchema.nullable(),
})

export type AppState = z.infer<typeof appStateSchema>

export const githubAppInfoSchema = z.object({
  configured: z.boolean(),
  app_slug: z.string().nullable(),
  install_url: z.string().nullable(),
})

export type GithubAppInfo = z.infer<typeof githubAppInfoSchema>

export const deviceFlowStartSchema = z.object({
  device_code: z.string(),
  user_code: z.string(),
  verification_uri: z.string(),
  expires_in: z.number().int().nonnegative(),
  interval: z.number().int().nonnegative(),
})

export type DeviceFlowStart = z.infer<typeof deviceFlowStartSchema>

export const deviceFlowPollSchema = z.object({
  status: z.enum(['pending', 'slow_down', 'authorized', 'expired', 'denied']),
  message: z.string().optional(),
  interval: z.number().int().nonnegative(),
})

export type DeviceFlowPoll = z.infer<typeof deviceFlowPollSchema>

const githubRepositorySelectionSchema = z.object({
  installation_id: z.number().int().nonnegative(),
  repository_id: z.number().int().nonnegative(),
  owner: z.string(),
  repo: z.string(),
})

export const githubRepositorySchema = githubRepositorySelectionSchema.extend({
  default_branch: z.string(),
  private: z.boolean(),
})

export type GithubRepository = z.infer<typeof githubRepositorySchema>

export const githubRepositoryDiscoverySchema = z.discriminatedUnion('status', [
  z.object({
    status: z.literal('app_not_installed'),
    install_url: z.string(),
  }),
  z.object({
    status: z.literal('single_repository'),
    repository: githubRepositorySelectionSchema,
  }),
  z.object({ status: z.literal('selection_all') }),
  z.object({
    status: z.literal('multiple_repositories'),
    count: z.number().int().nonnegative(),
  }),
  z.object({
    status: z.literal('unavailable'),
    message: z.string(),
  }),
])

export type GithubRepositoryDiscovery = z.infer<
  typeof githubRepositoryDiscoverySchema
>

export type InitializeGithubVaultRequest = {
  remote: RemoteConfig
  expected_status: 'empty_repository' | 'missing_manifest'
  expected_head_sha: string | null
  expected_manifest_sha: string | null
}

export type BindGithubVaultRequest = {
  remote: RemoteConfig
  expected_head_sha: string
  expected_manifest_sha: string
  expected_previous_binding: {
    installation_id: number
    repository_id: number
    branch: string
  } | null
  confirm_rebind: boolean
}

export const appErrorSchema = z.object({
  kind: z.string(),
  message: z.string(),
  retry_after: z.string().nullable().optional(),
  latest_check: z.unknown().optional(),
})

export type AppError = z.infer<typeof appErrorSchema>
