import { z } from 'zod'

import { invokeCmd } from '@/shared/lib'
import {
  appStateSchema,
  baselineResultSchema,
  deviceFlowPollSchema,
  deviceFlowStartSchema,
  githubAppInfoSchema,
  githubRepositoryDiscoverySchema,
  githubRepositorySchema,
  githubVaultCheckSchema,
  type AppState,
  type BaselineResult,
  type BindGithubVaultRequest,
  type DeviceFlowPoll,
  type DeviceFlowStart,
  type GithubAppInfo,
  type GithubRepository,
  type GithubRepositoryDiscovery,
  type GithubVaultCheck,
  type InitializeGithubVaultRequest,
  type RemoteConfig,
} from '@/shared/schemas'

const deviceFlowPollPayloadSchema = z.object({
  status: z.enum(['pending', 'slow_down', 'authorized', 'expired', 'denied']),
  message: z.string().optional(),
  interval: z.number().int().nonnegative().optional(),
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

// GitHub returns repository payloads in multiple shapes across endpoints
// (installation_id/repository_id vs id, owner string vs object). Normalize to
// the canonical GithubRepository before handing it to the rest of the app.
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

export const listInstallationRepositories = async (
  installationId: number,
): Promise<GithubRepository[]> => {
  const raw = await invokeCmd<unknown>('list_installation_repositories', {
    installationId,
  })
  return z
    .array(z.unknown())
    .parse(raw)
    .map((repository) => normalizeGithubRepository(repository, installationId))
}

export const discoverSingleGithubRepository =
  async (): Promise<GithubRepositoryDiscovery> =>
    githubRepositoryDiscoverySchema.parse(
      await invokeCmd<unknown>('discover_single_github_repository'),
    )

export const listGithubRepositoryBranches = async (
  remote: RemoteConfig,
): Promise<string[]> => {
  return z.array(z.string()).parse(
    await invokeCmd<unknown>('list_github_repository_branches', {
      remote,
    }),
  )
}

export const checkGithubVault = async (
  remote: RemoteConfig,
): Promise<GithubVaultCheck> => {
  return githubVaultCheckSchema.parse(
    await invokeCmd<unknown>('check_github_vault', { remote }),
  )
}

export const initializeGithubVault = async (
  request: InitializeGithubVaultRequest,
): Promise<GithubVaultCheck> => {
  return githubVaultCheckSchema.parse(
    await invokeCmd<unknown>('initialize_github_vault', {
      request,
    }),
  )
}

export const bindGithubVault = async (
  request: BindGithubVaultRequest,
): Promise<AppState> => {
  const response = z.union([appStateSchema, githubVaultCheckSchema]).parse(
    await invokeCmd<unknown>('bind_github_vault', {
      request,
    }),
  )
  if ('config' in response) {
    return response
  }
  return appStateSchema.parse(await invokeCmd<unknown>('get_app_state'))
}

// Adopt local skills that already match the remote into the base (sync_state) so
// that "delete local => delete remote" works right after onboarding, without a
// manual apply. Best-effort: callers should not block onboarding on failure.
export const establishBaseline = async (): Promise<BaselineResult> =>
  baselineResultSchema.parse(await invokeCmd<unknown>('establish_baseline'))
