import { z } from 'zod'

import { invokeCmd } from '@/shared/lib'
import {
  appStateSchema,
  bindGithubVaultRequestSchema,
  deviceFlowPollSchema,
  deviceFlowStartSchema,
  githubAppInfoSchema,
  githubInstallationSchema,
  githubRepositoryDiscoverySchema,
  githubRepositorySchema,
  githubVaultCheckSchema,
  initializeGithubVaultRequestSchema,
  remoteConfigSchema,
  type AppState,
  type BindGithubVaultRequest,
  type DeviceFlowPoll,
  type DeviceFlowStart,
  type GithubAppInfo,
  type GithubInstallation,
  type GithubRepository,
  type GithubRepositoryDiscovery,
  type GithubVaultCheck,
  type InitializeGithubVaultRequest,
  type RemoteConfig,
} from '../schemas/onboarding'

const deviceFlowPollPayloadSchema = z.object({
  status: z.enum(['pending', 'slow_down', 'authorized', 'expired', 'denied']),
  message: z.string().optional(),
  interval: z.number().int().nonnegative().optional(),
})

const githubInstallationPayloadSchema = z.object({
  id: z.number().int().nonnegative(),
  account_login: z.string().optional(),
  account: z.object({ login: z.string() }).optional(),
  repository_selection: z.enum(['all', 'selected']),
})

const githubRepositoryPayloadSchema = z.object({
  installation_id: z.number().int().nonnegative().optional(),
  repository_id: z.number().int().nonnegative().optional(),
  id: z.number().int().nonnegative().optional(),
  owner: z.union([z.string(), z.object({ login: z.string() })]).optional(),
  repo: z.string().optional(),
  name: z.string().optional(),
  default_branch: z.string(),
  private: z.boolean(),
})

const normalizeGithubInstallation = (value: unknown): GithubInstallation => {
  const payload = githubInstallationPayloadSchema.parse(value)
  return githubInstallationSchema.parse({
    id: payload.id,
    account_login: payload.account_login ?? payload.account?.login,
    repository_selection: payload.repository_selection,
  })
}

const normalizeGithubRepository = (
  value: unknown,
  installationId: number,
): GithubRepository => {
  const payload = githubRepositoryPayloadSchema.parse(value)
  const owner =
    typeof payload.owner === 'string' ? payload.owner : payload.owner?.login
  return githubRepositorySchema.parse({
    installation_id: payload.installation_id ?? installationId,
    repository_id: payload.repository_id ?? payload.id,
    owner,
    repo: payload.repo ?? payload.name,
    default_branch: payload.default_branch,
    private: payload.private,
  })
}

export const startGithubDeviceFlow = async (): Promise<DeviceFlowStart> =>
  deviceFlowStartSchema.parse(
    await invokeCmd<unknown>('start_github_device_flow'),
  )

export const pollGithubDeviceFlow = async (
  deviceCode: string,
  interval: number,
): Promise<DeviceFlowPoll> => {
  const payload = deviceFlowPollPayloadSchema.parse(
    await invokeCmd<unknown>('poll_github_device_flow', {
      deviceCode,
      interval,
    }),
  )
  return deviceFlowPollSchema.parse({
    ...payload,
    interval: payload.interval ?? interval,
  })
}

export const getGithubAppInfo = async (): Promise<GithubAppInfo> =>
  githubAppInfoSchema.parse(await invokeCmd<unknown>('get_github_app_info'))

export const listGithubInstallations = async (): Promise<
  GithubInstallation[]
> => {
  const raw = await invokeCmd<unknown>('list_github_installations')
  return z.array(z.unknown()).parse(raw).map(normalizeGithubInstallation)
}

export const listInstallationRepositories = async (
  installationId: number,
): Promise<GithubRepository[]> => {
  const parsedInstallationId = z
    .number()
    .int()
    .nonnegative()
    .parse(installationId)
  const raw = await invokeCmd<unknown>('list_installation_repositories', {
    installationId: parsedInstallationId,
  })
  return z
    .array(z.unknown())
    .parse(raw)
    .map((repository) =>
      normalizeGithubRepository(repository, parsedInstallationId),
    )
}

export const discoverSingleGithubRepository =
  async (): Promise<GithubRepositoryDiscovery> =>
    githubRepositoryDiscoverySchema.parse(
      await invokeCmd<unknown>('discover_single_github_repository'),
    )

export const listGithubRepositoryBranches = async (
  remote: RemoteConfig,
): Promise<string[]> => {
  const parsedRemote = remoteConfigSchema.parse(remote)
  return z.array(z.string()).parse(
    await invokeCmd<unknown>('list_github_repository_branches', {
      remote: parsedRemote,
    }),
  )
}

export const checkGithubVault = async (
  remote: RemoteConfig,
): Promise<GithubVaultCheck> => {
  const parsedRemote = remoteConfigSchema.parse(remote)
  return githubVaultCheckSchema.parse(
    await invokeCmd<unknown>('check_github_vault', { remote: parsedRemote }),
  )
}

export const initializeGithubVault = async (
  request: InitializeGithubVaultRequest,
): Promise<GithubVaultCheck> => {
  const parsedRequest = initializeGithubVaultRequestSchema.parse(request)
  return githubVaultCheckSchema.parse(
    await invokeCmd<unknown>('initialize_github_vault', {
      request: parsedRequest,
    }),
  )
}

export const bindGithubVault = async (
  request: BindGithubVaultRequest,
): Promise<AppState> => {
  const parsedRequest = bindGithubVaultRequestSchema.parse(request)
  const response = z.union([appStateSchema, githubVaultCheckSchema]).parse(
    await invokeCmd<unknown>('bind_github_vault', {
      request: parsedRequest,
    }),
  )
  if ('config' in response) {
    return response
  }
  return appStateSchema.parse(await invokeCmd<unknown>('get_app_state'))
}
