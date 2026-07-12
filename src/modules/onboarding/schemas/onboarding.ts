import { z } from 'zod'

export {
  appErrorSchema,
  appStateSchema,
  bindGithubVaultRequestSchema,
  deviceFlowPollSchema,
  deviceFlowStartSchema,
  githubAppInfoSchema,
  githubInstallationSchema,
  githubRepositoryDiscoverySchema,
  githubRepositorySchema,
  githubRepositorySelectionSchema,
  githubVaultCheckSchema,
  githubVaultStatusSchema,
  initializeGithubVaultRequestSchema,
  remoteBindingKeySchema,
  remoteConfigSchema,
} from '@/shared/schemas'

export type {
  AppState,
  BindGithubVaultRequest,
  DeviceFlowPoll,
  DeviceFlowStart,
  GithubAppInfo,
  GithubInstallation,
  GithubRepository,
  GithubRepositoryDiscovery,
  GithubRepositorySelection,
  GithubVaultCheck,
  InitializeGithubVaultRequest,
  RemoteBindingKey,
  RemoteConfig,
} from '@/shared/schemas'

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
