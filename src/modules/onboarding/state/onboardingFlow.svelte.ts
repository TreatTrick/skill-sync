import { errorMessage, isWorkspaceReady, SkillSyncError } from '@/shared/lib'
import { t } from '@/shared/i18n'
import {
  githubVaultCheckSchema,
  type AppState,
  type DeviceFlowPoll,
  type DeviceFlowStart,
  type GithubAppInfo,
  type GithubRepository,
  type GithubVaultCheck,
  type RemoteConfig,
} from '@/shared/schemas'

// Onboarding api port. The real implementation lives in ../api/onboardingApi;
// tests inject a fake. Two adapters (real Tauri in prod, in-memory in tests)
// justify the seam - it is not hypothetical.
export type OnboardingApiPort = typeof import('../api/onboardingApi')

export type OnboardingStage =
  | 'app_not_configured'
  | 'authorize'
  | 'device_pending'
  | 'create_repository'
  | 'install_app'
  | 'repository_scope_blocked'
  | 'confirm_public_repository'
  | 'select_branch'
  | 'checking_vault'
  | 'vault_unavailable'
  | 'confirm_initialize'
  | 'invalid_manifest'
  | 'rate_limited'
  | 'ready'

export type BindVaultResult = {
  // null when no bind happened (precondition failed, rebind guard, or error);
  // the flow has already set stage/message in those cases. Non-null lets the
  // page update the appState cache and toast baseline adoptions.
  appState: AppState | null
  baselineAdoptions: number
  navigateTo: string | null
}

// Per-mount onboarding orchestration: the 13-stage state machine, device-flow
// polling, vault checks, error dispatch, and the auto-discovery trigger. Owns
// all domain state. svelte-query (appState cache) and view side-effects (toast,
// goto) stay in the page, which forwards appState and bridges the stale signal.
// Constructed fresh per route mount so reconfigure / re-entry never inherit
// stale state; the api adapter is constructor-injected so the flow is testable
// without a DOM or Svelte.
export class OnboardingFlow {
  appInfo = $state<GithubAppInfo | null>(null)
  appInfoLoaded = $state(false)
  stage = $state<OnboardingStage>('authorize')
  message = $state('')
  // Depth counter so nested async steps (checkVault inside continueWith,
  // discover inside poll) never flip busy false while an outer step still runs.
  private busyDepth = $state(0)
  readonly busy = $derived(this.busyDepth > 0)
  deviceFlow = $state<DeviceFlowStart | null>(null)
  deviceExpiresAt = $state<number | null>(null)
  deviceInterval = $state(5)
  private pollTimer: ReturnType<typeof setTimeout> | undefined
  selectedRepository = $state<GithubRepository | null>(null)
  remote = $state<RemoteConfig | null>(null)
  branchNames = $state<string[]>([])
  selectedBranch = $state('')
  vaultCheck = $state<GithubVaultCheck | null>(null)
  confirmRebind = $state(false)
  private autoDiscoveryStarted = $state(false)
  // Monotonic signal bumped when device-flow authorize succeeds. The flow is
  // svelte-query-agnostic, so the page watches this and invalidates the
  // appState query (github_authorized flipped) to refresh the cache.
  appStateStaleSeq = $state(0)

  constructor(private readonly api: OnboardingApiPort) {}

  // Single funnel for every stage mutation (candidate B). Happy path and error
  // path share one writer -> one assertion surface for tests. message undefined
  // leaves the current message untouched (callers that clear it pass '').
  transition(next: OnboardingStage, message?: string): void {
    this.stage = next
    if (message !== undefined) this.message = message
  }

  bindingChanged(state: AppState | undefined): boolean {
    return (
      this.remote !== null &&
      state?.configured === true &&
      (state.installation_id !== this.remote.installation_id ||
        state.repository_id !== this.remote.repository_id ||
        state.remote_branch !== this.remote.branch)
    )
  }

  // Error -> stage/message dispatch. Routes through the transition funnel so
  // errors and happy flow share one mutation point instead of two.
  handleError(error: unknown): void {
    if (error instanceof SkillSyncError) {
      if (error.kind === 'reauthorization_required') {
        this.transition('authorize', t('github.reauthorizationRequired'))
        return
      }
      if (error.kind === 'rate_limited') {
        this.transition(
          'rate_limited',
          error.retryAfter
            ? t('github.rateLimitedWithRetry', { retryAfter: error.retryAfter })
            : t('github.rateLimited'),
        )
        return
      }
      if (
        error.kind === 'vault_state_changed' &&
        error.latestCheck !== undefined
      ) {
        const latest = githubVaultCheckSchema.safeParse(error.latestCheck)
        if (latest.success) {
          this.vaultCheck = latest.data
          this.message = t('github.vaultStateChanged')
          this.setVaultStage(latest.data)
          return
        }
      }
    }
    this.message = errorMessage(error)
  }

  stopPolling(): void {
    if (this.pollTimer) clearTimeout(this.pollTimer)
    this.pollTimer = undefined
  }

  private setVaultStage(check: GithubVaultCheck): void {
    if (check.status === 'ready') this.transition('ready')
    else if (
      check.status === 'empty_repository' ||
      check.status === 'missing_manifest'
    )
      this.transition('confirm_initialize')
    else if (check.status === 'invalid_manifest')
      this.transition('invalid_manifest')
    else if (check.status === 'branch_missing') this.transition('select_branch')
    else this.transition('vault_unavailable')
  }

  private vaultStatusMessage(check: GithubVaultCheck): string {
    if (check.status === 'repository_forbidden')
      return t('github.repositoryForbidden')
    if (check.status === 'repository_missing')
      return t('github.repositoryMissing')
    if (check.status === 'repository_unavailable')
      return t('github.repositoryUnavailable')
    return check.message ?? t('github.vaultUnavailable')
  }

  private schedulePoll(): void {
    this.stopPolling()
    if (
      !this.deviceFlow ||
      !this.deviceExpiresAt ||
      Date.now() >= this.deviceExpiresAt
    ) {
      this.transition('authorize', t('github.deviceExpired'))
      return
    }
    this.pollTimer = setTimeout(
      () => void this.pollDeviceFlow(),
      this.deviceInterval * 1000,
    )
  }

  async loadAppInfo(): Promise<void> {
    try {
      this.appInfo = await this.api.getGithubAppInfo()
      this.transition(
        this.appInfo.configured ? 'authorize' : 'app_not_configured',
      )
    } catch (error) {
      this.handleError(error)
    } finally {
      this.appInfoLoaded = true
    }
  }

  async startDeviceAuthorization(): Promise<void> {
    this.busyDepth++
    this.message = ''
    try {
      this.deviceFlow = await this.api.startGithubDeviceFlow()
      this.deviceInterval = this.deviceFlow.interval
      this.deviceExpiresAt = Date.now() + this.deviceFlow.expires_in * 1000
      this.transition('device_pending')
      this.schedulePoll()
    } catch (error) {
      this.handleError(error)
    } finally {
      this.busyDepth--
    }
  }

  async pollDeviceFlow(): Promise<void> {
    if (!this.deviceFlow) return
    this.busyDepth++
    try {
      const response: DeviceFlowPoll = await this.api.pollGithubDeviceFlow(
        this.deviceFlow.device_code,
        this.deviceInterval,
      )
      if (response.status === 'authorized') {
        this.stopPolling()
        this.deviceFlow = null
        this.deviceExpiresAt = null
        // github_authorized flipped - tell the page to refresh the appState
        // cache, then discover. discoverRepository does not read appState.
        this.appStateStaleSeq++
        await this.discoverRepository()
        return
      }
      if (response.status === 'expired' || response.status === 'denied') {
        this.stopPolling()
        this.transition(
          'authorize',
          response.message ?? t('github.deviceAuthorizationFailed'),
        )
        return
      }
      this.deviceInterval = response.interval
      this.transition('device_pending')
      this.schedulePoll()
    } catch (error) {
      this.stopPolling()
      this.handleError(error)
    } finally {
      this.busyDepth--
    }
  }

  private async continueWithSelectedRepository(): Promise<void> {
    const currentRemote = this.remote
    if (!currentRemote) return
    this.branchNames =
      await this.api.listGithubRepositoryBranches(currentRemote)
    if (this.branchNames.length > 0) {
      this.selectedBranch = this.branchNames.includes(currentRemote.branch)
        ? currentRemote.branch
        : this.branchNames[0]
      this.transition('select_branch')
    } else {
      this.transition('checking_vault')
      await this.checkVault()
    }
  }

  async continueWithPublicRepository(): Promise<void> {
    this.busyDepth++
    this.message = ''
    try {
      await this.continueWithSelectedRepository()
    } catch (error) {
      this.handleError(error)
    } finally {
      this.busyDepth--
    }
  }

  // `entry` distinguishes the initial auto-discovery (route "no usable repo" to
  // the create-repository step) from a re-check triggered from the install step
  // (stay on install_app with a hint instead of looping back).
  async discoverRepository(entry: 'auto' | 'recheck' = 'auto'): Promise<void> {
    this.busyDepth++
    this.message = ''
    try {
      const discovery = await this.api.discoverSingleGithubRepository()
      if (discovery.status === 'app_not_installed') {
        this.appInfo = this.appInfo
          ? { ...this.appInfo, install_url: discovery.install_url }
          : this.appInfo
        if (entry === 'recheck') {
          this.transition('install_app', t('github.appStillNotInstalled'))
        } else {
          this.transition('create_repository')
        }
        return
      }
      if (
        discovery.status === 'selection_all' ||
        discovery.status === 'multiple_repositories'
      ) {
        this.transition(
          'repository_scope_blocked',
          discovery.status === 'multiple_repositories'
            ? t('github.multipleRepositories', { count: discovery.count })
            : t('github.selectionAll'),
        )
        return
      }
      if (discovery.status === 'unavailable') {
        if (entry === 'recheck') {
          this.message = discovery.message
          this.transition('install_app')
        } else {
          this.transition('create_repository')
        }
        return
      }
      const repositories = await this.api.listInstallationRepositories(
        discovery.repository.installation_id,
      )
      const repository = repositories.find(
        (candidate) =>
          candidate.repository_id === discovery.repository.repository_id,
      )
      if (!repository) {
        this.transition('install_app', t('github.repositoryUnavailable'))
        return
      }
      this.selectedRepository = repository
      const defaultBranch = repository.default_branch || 'main'
      this.remote = { ...discovery.repository, branch: defaultBranch }
      this.selectedBranch = defaultBranch
      if (!repository.private) {
        this.transition('confirm_public_repository')
        return
      }
      await this.continueWithSelectedRepository()
    } catch (error) {
      this.handleError(error)
    } finally {
      this.busyDepth--
    }
  }

  async checkVault(): Promise<void> {
    if (!this.remote) return
    this.busyDepth++
    this.message = ''
    try {
      this.vaultCheck = await this.api.checkGithubVault(this.remote)
      if (this.vaultCheck.status === 'branch_missing') {
        this.branchNames = await this.api.listGithubRepositoryBranches(
          this.remote,
        )
      }
      this.setVaultStage(this.vaultCheck)
      if (
        this.vaultCheck.status !== 'ready' &&
        this.vaultCheck.status !== 'empty_repository' &&
        this.vaultCheck.status !== 'missing_manifest' &&
        this.vaultCheck.status !== 'invalid_manifest' &&
        this.vaultCheck.status !== 'branch_missing'
      ) {
        this.message = this.vaultStatusMessage(this.vaultCheck)
      }
    } catch (error) {
      this.handleError(error)
    } finally {
      this.busyDepth--
    }
  }

  async chooseBranch(): Promise<void> {
    if (!this.remote || !this.selectedBranch) return
    this.remote = { ...this.remote, branch: this.selectedBranch }
    await this.checkVault()
  }

  async initializeVault(): Promise<void> {
    if (!this.remote || !this.vaultCheck) return
    this.busyDepth++
    this.message = ''
    try {
      this.vaultCheck = await this.api.initializeGithubVault({
        remote: this.remote,
        expected_status:
          this.vaultCheck.status === 'empty_repository'
            ? 'empty_repository'
            : 'missing_manifest',
        expected_head_sha: this.vaultCheck.head_sha,
        expected_manifest_sha: this.vaultCheck.manifest_sha,
      })
      this.setVaultStage(this.vaultCheck)
    } catch (error) {
      this.handleError(error)
    } finally {
      this.busyDepth--
    }
  }

  async bindVault(appState: AppState | undefined): Promise<BindVaultResult> {
    if (
      !this.remote ||
      !this.vaultCheck?.head_sha ||
      !this.vaultCheck.manifest_sha
    ) {
      return { appState: null, baselineAdoptions: 0, navigateTo: null }
    }
    if (this.bindingChanged(appState) && !this.confirmRebind) {
      this.message = t('github.confirmRebindRequired')
      return { appState: null, baselineAdoptions: 0, navigateTo: null }
    }
    this.busyDepth++
    this.message = ''
    try {
      const nextState = await this.api.bindGithubVault({
        remote: this.remote,
        expected_head_sha: this.vaultCheck.head_sha,
        expected_manifest_sha: this.vaultCheck.manifest_sha,
        expected_previous_binding:
          appState?.configured &&
          appState.installation_id !== null &&
          appState.repository_id !== null &&
          appState.remote_branch !== null
            ? {
                installation_id: appState.installation_id,
                repository_id: appState.repository_id,
                branch: appState.remote_branch,
              }
            : null,
        confirm_rebind: this.confirmRebind,
      })
      // Best-effort: adopt local skills that already match the remote into the
      // base so "delete local => delete remote" works without a manual apply.
      // Failure must not block onboarding.
      let baselineAdoptions = 0
      try {
        const baseline = await this.api.establishBaseline()
        baselineAdoptions = baseline.adoptions
      } catch (baselineError) {
        console.warn('establish_baseline failed', baselineError)
      }
      if (isWorkspaceReady(nextState)) {
        return {
          appState: nextState,
          baselineAdoptions,
          navigateTo: '/app/sync',
        }
      }
      this.message = t('github.bindingFailed')
      return { appState: nextState, baselineAdoptions, navigateTo: null }
    } catch (error) {
      this.handleError(error)
      return { appState: null, baselineAdoptions: 0, navigateTo: null }
    } finally {
      this.busyDepth--
    }
  }

  // Initial auto-discovery: when appState arrives already-authorized with a valid
  // credential while we sit on the authorize stage, kick discovery once. The
  // page forwards appState.data here; the guard is the only transition trigger.
  onAppStateChanged(state: AppState | undefined): void {
    if (
      !this.appInfoLoaded ||
      !this.appInfo?.configured ||
      !state ||
      this.stage !== 'authorize' ||
      this.deviceFlow ||
      this.autoDiscoveryStarted ||
      !state.github_authorized ||
      !['valid', 'refreshing'].includes(state.credential_status)
    )
      return
    this.autoDiscoveryStarted = true
    void this.discoverRepository()
  }
}
