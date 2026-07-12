<script lang="ts">
  import { onDestroy, onMount } from 'svelte'
  import { page } from '$app/state'
  import { goto } from '$app/navigation'

  import { getAppState } from '@/modules/settings'
  import { errorMessage, isWorkspaceReady, openPath, SkillSyncError } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import { useQueryClient, createQuery } from '@tanstack/svelte-query'
  import {
    AlertTriangle,
    CheckCircle,
    ExternalLink,
    KeyRound,
    LoaderCircle,
  } from '@lucide/svelte'
  import {
    Button,
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
    Spinner,
  } from '@/shared/ui'

  import {
    bindGithubVault,
    checkGithubVault,
    discoverSingleGithubRepository,
    getGithubAppInfo,
    initializeGithubVault,
    listGithubRepositoryBranches,
    listInstallationRepositories,
    pollGithubDeviceFlow,
    startGithubDeviceFlow,
  } from '../api/onboardingApi'
  import type {
    DeviceFlowPoll,
    DeviceFlowStart,
    GithubAppInfo,
    GithubRepository,
    GithubVaultCheck,
    RemoteConfig,
  } from '../schemas/onboarding'
  import { githubVaultCheckSchema } from '../schemas/onboarding'

  type OnboardingStage =
    | 'app_not_configured'
    | 'authorize'
    | 'device_pending'
    | 'install_app'
    | 'repository_scope_blocked'
    | 'select_branch'
    | 'checking_vault'
    | 'vault_unavailable'
    | 'confirm_initialize'
    | 'invalid_manifest'
    | 'rate_limited'
    | 'ready'

  const appState = createQuery(() => ({
    queryKey: ['app-state'],
    queryFn: getAppState,
  }))
  const queryClient = useQueryClient()
  const reconfigure = $derived(page.url.searchParams.get('mode') === 'reconfigure')

  let appInfo = $state<GithubAppInfo | null>(null)
  let stage = $state<OnboardingStage>('authorize')
  let message = $state('')
  let busy = $state(false)
  let deviceFlow = $state<DeviceFlowStart | null>(null)
  let deviceExpiresAt = $state<number | null>(null)
  let deviceInterval = $state(5)
  let pollTimer: ReturnType<typeof setTimeout> | undefined
  let selectedRepository = $state<GithubRepository | null>(null)
  let remote = $state<RemoteConfig | null>(null)
  let branchNames = $state<string[]>([])
  let selectedBranch = $state('')
  let vaultCheck = $state<GithubVaultCheck | null>(null)
  let confirmRebind = $state(false)
  let appInfoLoaded = $state(false)
  let autoDiscoveryStarted = $state(false)

  const progressStep = (currentStage: OnboardingStage): number => {
    if (currentStage === 'app_not_configured' || currentStage === 'authorize' || currentStage === 'device_pending') return 1
    if (currentStage === 'install_app' || currentStage === 'repository_scope_blocked') return 2
    if (currentStage === 'select_branch' || currentStage === 'checking_vault' || currentStage === 'vault_unavailable') return 3
    if (currentStage === 'confirm_initialize' || currentStage === 'invalid_manifest' || currentStage === 'rate_limited') return 4
    return 5
  }

  const stageTitle = (currentStage: OnboardingStage): string => {
    if (currentStage === 'app_not_configured') return t('onboarding.stage.appNotConfigured')
    if (currentStage === 'authorize' || currentStage === 'device_pending') return t('onboarding.stage.authorize')
    if (currentStage === 'install_app' || currentStage === 'repository_scope_blocked') return t('onboarding.stage.installApp')
    if (currentStage === 'select_branch' || currentStage === 'checking_vault' || currentStage === 'vault_unavailable') return t('onboarding.stage.branch')
    if (currentStage === 'confirm_initialize' || currentStage === 'invalid_manifest' || currentStage === 'rate_limited') return t('onboarding.stage.vault')
    return t('onboarding.stage.ready')
  }

  const stopPolling = (): void => {
    if (pollTimer) clearTimeout(pollTimer)
    pollTimer = undefined
  }

  const setVaultStage = (check: GithubVaultCheck): void => {
    if (check.status === 'ready') stage = 'ready'
    else if (check.status === 'empty_repository' || check.status === 'missing_manifest') stage = 'confirm_initialize'
    else if (check.status === 'invalid_manifest') stage = 'invalid_manifest'
    else if (check.status === 'branch_missing') stage = 'select_branch'
    else stage = 'vault_unavailable'
  }

  const vaultStatusMessage = (check: GithubVaultCheck): string => {
    if (check.status === 'repository_forbidden') return t('github.repositoryForbidden')
    if (check.status === 'repository_missing') return t('github.repositoryMissing')
    if (check.status === 'repository_unavailable') return t('github.repositoryUnavailable')
    return check.message ?? t('github.vaultUnavailable')
  }

  const setError = (error: unknown): void => {
    if (error instanceof SkillSyncError) {
      if (error.kind === 'reauthorization_required') {
        stage = 'authorize'
        message = t('github.reauthorizationRequired')
        return
      }
      if (error.kind === 'rate_limited') {
        stage = 'rate_limited'
        message = error.retryAfter
          ? t('github.rateLimitedWithRetry', { retryAfter: error.retryAfter })
          : t('github.rateLimited')
        return
      }
      if (error.kind === 'vault_state_changed' && error.latestCheck !== undefined) {
        const latest = githubVaultCheckSchema.safeParse(error.latestCheck)
        if (latest.success) {
          vaultCheck = latest.data
          message = t('github.vaultStateChanged')
          setVaultStage(latest.data)
          return
        }
      }
    }
    message = errorMessage(error)
  }

  const openExternal = async (event: MouseEvent, url: string): Promise<void> => {
    event.preventDefault()
    try {
      await openPath(url)
    } catch (error) {
      setError(error)
    }
  }

  const loadAppInfo = async (): Promise<void> => {
    try {
      appInfo = await getGithubAppInfo()
      stage = appInfo.configured ? 'authorize' : 'app_not_configured'
    } catch (error) {
      setError(error)
    } finally {
      appInfoLoaded = true
    }
  }

  const schedulePoll = (): void => {
    stopPolling()
    if (!deviceFlow || !deviceExpiresAt || Date.now() >= deviceExpiresAt) {
      stage = 'authorize'
      message = t('github.deviceExpired')
      return
    }
    pollTimer = setTimeout(() => void pollDeviceFlow(), deviceInterval * 1000)
  }

  const discoverRepository = async (): Promise<void> => {
    busy = true
    message = ''
    try {
      const discovery = await discoverSingleGithubRepository()
      if (discovery.status === 'app_not_installed') {
        appInfo = appInfo ? { ...appInfo, install_url: discovery.install_url } : appInfo
        stage = 'install_app'
        return
      }
      if (discovery.status === 'selection_all' || discovery.status === 'multiple_repositories') {
        stage = 'repository_scope_blocked'
        message = discovery.status === 'multiple_repositories'
          ? t('github.multipleRepositories', { count: discovery.count })
          : t('github.selectionAll')
        return
      }
      if (discovery.status === 'unavailable') {
        message = discovery.message
        stage = 'install_app'
        return
      }
      const repositories = await listInstallationRepositories(discovery.repository.installation_id)
      selectedRepository = repositories.find(
        (repository) => repository.repository_id === discovery.repository.repository_id,
      ) ?? null
      const defaultBranch = selectedRepository?.default_branch || 'main'
      remote = { ...discovery.repository, branch: defaultBranch }
      selectedBranch = defaultBranch
      branchNames = await listGithubRepositoryBranches(remote)
      if (branchNames.length > 0) {
        selectedBranch = branchNames.includes(defaultBranch) ? defaultBranch : branchNames[0]
        stage = 'select_branch'
      } else {
        stage = 'checking_vault'
        await checkVault()
      }
    } catch (error) {
      setError(error)
    } finally {
      busy = false
    }
  }

  const pollDeviceFlow = async (): Promise<void> => {
    if (!deviceFlow) return
    busy = true
    try {
      const response: DeviceFlowPoll = await pollGithubDeviceFlow(
        deviceFlow.device_code,
        deviceInterval,
      )
      if (response.status === 'authorized') {
        stopPolling()
        deviceFlow = null
        deviceExpiresAt = null
        await queryClient.invalidateQueries({ queryKey: ['app-state'] })
        await discoverRepository()
        return
      }
      if (response.status === 'expired' || response.status === 'denied') {
        stopPolling()
        stage = 'authorize'
        message = response.message ?? t('github.deviceAuthorizationFailed')
        return
      }
      deviceInterval = response.interval
      stage = 'device_pending'
      schedulePoll()
    } catch (error) {
      stopPolling()
      setError(error)
    } finally {
      busy = false
    }
  }

  const startDeviceAuthorization = async (): Promise<void> => {
    busy = true
    message = ''
    try {
      deviceFlow = await startGithubDeviceFlow()
      deviceInterval = deviceFlow.interval
      deviceExpiresAt = Date.now() + deviceFlow.expires_in * 1000
      stage = 'device_pending'
      schedulePoll()
    } catch (error) {
      setError(error)
    } finally {
      busy = false
    }
  }

  const checkVault = async (): Promise<void> => {
    if (!remote) return
    busy = true
    message = ''
    try {
      vaultCheck = await checkGithubVault(remote)
      if (vaultCheck.status === 'branch_missing') {
        branchNames = await listGithubRepositoryBranches(remote)
      }
      setVaultStage(vaultCheck)
      if (vaultCheck.status !== 'ready' && vaultCheck.status !== 'empty_repository' && vaultCheck.status !== 'missing_manifest' && vaultCheck.status !== 'invalid_manifest' && vaultCheck.status !== 'branch_missing') {
        message = vaultStatusMessage(vaultCheck)
      }
    } catch (error) {
      setError(error)
    } finally {
      busy = false
    }
  }

  const chooseBranch = async (): Promise<void> => {
    if (!remote || !selectedBranch) return
    remote = { ...remote, branch: selectedBranch }
    await checkVault()
  }

  const initializeVault = async (): Promise<void> => {
    if (!remote || !vaultCheck) return
    busy = true
    message = ''
    try {
      vaultCheck = await initializeGithubVault({
        remote,
        expected_status: vaultCheck.status === 'empty_repository' ? 'empty_repository' : 'missing_manifest',
        expected_head_sha: vaultCheck.head_sha,
        expected_manifest_sha: vaultCheck.manifest_sha,
      })
      setVaultStage(vaultCheck)
    } catch (error) {
      setError(error)
    } finally {
      busy = false
    }
  }

  const bindingChanged = $derived(
    remote !== null &&
      appState.data?.configured === true &&
      (appState.data.installation_id !== remote.installation_id ||
        appState.data.repository_id !== remote.repository_id ||
        appState.data.remote_branch !== remote.branch),
  )

  const bindVault = async (): Promise<void> => {
    if (!remote || !vaultCheck?.head_sha || !vaultCheck.manifest_sha) return
    if (bindingChanged && !confirmRebind) {
      message = t('github.confirmRebindRequired')
      return
    }
    busy = true
    message = ''
    try {
      const nextState = await bindGithubVault({
        remote,
        expected_head_sha: vaultCheck.head_sha,
        expected_manifest_sha: vaultCheck.manifest_sha,
        expected_previous_binding:
          appState.data?.configured &&
          appState.data.installation_id !== null &&
          appState.data.repository_id !== null &&
          appState.data.remote_branch !== null
            ? {
                installation_id: appState.data.installation_id,
                repository_id: appState.data.repository_id,
                branch: appState.data.remote_branch,
              }
            : null,
        confirm_rebind: confirmRebind,
      })
      queryClient.setQueryData(['app-state'], nextState)
      if (isWorkspaceReady(nextState)) await goto('/app/sync', { replaceState: true })
      else message = t('github.bindingFailed')
    } catch (error) {
      setError(error)
    } finally {
      busy = false
    }
  }

  const cancelReconfigure = (): void => {
    void goto('/app/sync', { replaceState: true })
  }

  $effect(() => {
    const state = appState.data
    if (
      !appInfoLoaded ||
      !appInfo?.configured ||
      !state ||
      appState.isLoading ||
      stage !== 'authorize' ||
      deviceFlow ||
      autoDiscoveryStarted ||
      !state.github_authorized ||
      !['valid', 'refreshing'].includes(state.credential_status)
    ) return
    autoDiscoveryStarted = true
    void discoverRepository()
  })

  $effect(() => {
    if (appState.data && isWorkspaceReady(appState.data) && !reconfigure) {
      void goto('/app/sync', { replaceState: true })
    }
  })

  onMount(() => {
    void loadAppInfo()
    return stopPolling
  })

  onDestroy(stopPolling)
</script>

<div class="mx-auto grid w-full max-w-2xl gap-4">
  <Card>
    <CardHeader>
      <div class="flex items-center justify-between gap-3">
        <div class="grid gap-1.5">
          <CardTitle>{t('onboarding.title')}</CardTitle>
          <CardDescription>{t('onboarding.description')}</CardDescription>
        </div>
        {#if reconfigure}
          <Button onclick={cancelReconfigure} size="sm" variant="ghost">
            {t('onboarding.cancelReconfigure')}
          </Button>
        {/if}
      </div>
      <div class="grid gap-2 pt-3">
        <div class="flex justify-between text-xs text-muted-foreground">
          <span>{t('onboarding.progress', { current: progressStep(stage), total: 5 })}</span>
          <span>{stageTitle(stage)}</span>
        </div>
        <progress
          aria-label={t('onboarding.progress', { current: progressStep(stage), total: 5 })}
          class="h-2 w-full accent-primary"
          max="5"
          value={progressStep(stage)}
        ></progress>
      </div>
    </CardHeader>
  </Card>

  {#if message}
    <Card class="border-destructive-border bg-destructive-muted">
      <CardContent class="flex items-start gap-2 pt-6 text-sm text-destructive">
        <AlertTriangle class="mt-0.5 size-4 shrink-0" />
        <span>{message}</span>
      </CardContent>
    </Card>
  {/if}

  {#if !appInfoLoaded || appState.isLoading}
    <Card><CardContent class="flex justify-center py-12"><Spinner class="size-6" /></CardContent></Card>
  {:else if stage === 'app_not_configured'}
    <Card>
      <CardContent class="grid gap-4 pt-6">
        <CardTitle>{t('github.appNotConfigured')}</CardTitle>
        <p class="text-sm text-muted-foreground">{t('github.appNotConfiguredDescription')}</p>
      </CardContent>
    </Card>
  {:else if stage === 'authorize' || stage === 'device_pending'}
    <Card>
      <CardContent class="grid gap-4 pt-6">
        <div class="flex items-center gap-3">
          <KeyRound class="size-6" />
          <div class="grid gap-1">
            <h2 class="font-bold text-strong-foreground">{t('github.authorizeTitle')}</h2>
            <p class="text-sm text-muted-foreground">{t('github.authorizeDescription')}</p>
          </div>
        </div>
        {#if stage === 'device_pending' && deviceFlow}
          <div class="grid gap-3 border border-border bg-surface p-4">
            <p class="text-sm text-muted-foreground">{t('github.deviceCodeLabel')}</p>
            <code class="text-2xl font-bold tracking-widest text-strong-foreground">{deviceFlow.user_code}</code>
            <a
              class="inline-flex items-center gap-2 text-sm text-primary underline"
              href={deviceFlow?.verification_uri ?? ''}
              onclick={(event) => void openExternal(event, deviceFlow?.verification_uri ?? '')}
              rel="noreferrer"
              target="_blank"
            >
              {t('github.openVerification')} <ExternalLink class="size-4" />
            </a>
            {#if deviceExpiresAt}
              <p class="text-xs text-muted-foreground">{t('github.waitingAuthorization')}</p>
            {/if}
          </div>
        {:else}
          <Button disabled={busy} loading={busy} onclick={() => void startDeviceAuthorization()}>
            {t('github.connectGithub')}
          </Button>
        {/if}
      </CardContent>
    </Card>
  {:else if stage === 'install_app' || stage === 'repository_scope_blocked'}
    <Card>
      <CardContent class="grid gap-4 pt-6">
        <h2 class="font-bold text-strong-foreground">{t('github.installTitle')}</h2>
        <p class="text-sm text-muted-foreground">
          {stage === 'repository_scope_blocked' ? t('github.adjustScope') : t('github.installDescription')}
        </p>
        {#if appInfo?.install_url}
          <a
            class="inline-flex items-center gap-2 text-sm text-primary underline"
            href={appInfo?.install_url ?? ''}
            onclick={(event) => void openExternal(event, appInfo?.install_url ?? '')}
            rel="noreferrer"
            target="_blank"
          >
            {t('github.installApp')} <ExternalLink class="size-4" />
          </a>
        {/if}
        <Button disabled={busy} loading={busy} onclick={() => void discoverRepository()} variant="outline">
          {t('github.checkInstallation')}
        </Button>
      </CardContent>
    </Card>
  {:else if stage === 'select_branch'}
    <Card>
      <CardContent class="grid gap-4 pt-6">
        <h2 class="font-bold text-strong-foreground">{t('github.selectBranch')}</h2>
        <p class="text-sm text-muted-foreground">
          {remote ? `${remote.owner}/${remote.repo}` : t('github.repositoryUnavailable')}
        </p>
        <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('onboarding.branch')}
          <select bind:value={selectedBranch} class="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground">
            {#each branchNames as branch (branch)}
              <option value={branch}>{branch}</option>
            {/each}
          </select>
        </label>
        <Button disabled={busy || !selectedBranch} loading={busy} onclick={() => void chooseBranch()}>
          {t('github.checkVault')}
        </Button>
      </CardContent>
    </Card>
  {:else if stage === 'checking_vault'}
    <Card><CardContent class="flex items-center justify-center gap-2 py-12 text-sm text-muted-foreground"><LoaderCircle class="size-4 animate-spin" />{t('github.checkingVault')}</CardContent></Card>
  {:else if stage === 'vault_unavailable'}
    <Card>
      <CardContent class="grid gap-4 pt-6">
        <h2 class="font-bold text-warning">{t('github.vaultUnavailable')}</h2>
        <Button disabled={busy} loading={busy} onclick={() => void checkVault()}>{t('common.actions.retry')}</Button>
      </CardContent>
    </Card>
  {:else if stage === 'confirm_initialize'}
    <Card>
      <CardContent class="grid gap-4 pt-6">
        <h2 class="font-bold text-strong-foreground">{t('github.initializeVault')}</h2>
        <p class="text-sm text-muted-foreground">{t('github.initializeCreatesCommit')}</p>
        <Button disabled={busy} loading={busy} onclick={() => void initializeVault()}>
          {t('github.confirmInitialize')}
        </Button>
      </CardContent>
    </Card>
  {:else if stage === 'invalid_manifest'}
    <Card>
      <CardContent class="grid gap-4 pt-6">
        <h2 class="font-bold text-destructive">{t('github.invalidManifest')}</h2>
        <p class="text-sm text-muted-foreground">{t('github.invalidManifestDescription')}</p>
        {#if remote}
          <a
            class="inline-flex items-center gap-2 text-sm text-primary underline"
            href={`https://github.com/${remote?.owner ?? ''}/${remote?.repo ?? ''}`}
            onclick={(event) => void openExternal(event, `https://github.com/${remote?.owner ?? ''}/${remote?.repo ?? ''}`)}
            rel="noreferrer"
            target="_blank"
          >
            {t('github.openRepository')} <ExternalLink class="size-4" />
          </a>
        {/if}
      </CardContent>
    </Card>
  {:else if stage === 'rate_limited'}
    <Card>
      <CardContent class="grid gap-4 pt-6">
        <h2 class="font-bold text-warning">{t('github.rateLimited')}</h2>
        <Button disabled={busy} loading={busy} onclick={() => void checkVault()}>{t('common.actions.retry')}</Button>
      </CardContent>
    </Card>
  {:else if stage === 'ready'}
    <Card>
      <CardContent class="grid gap-4 pt-6">
        <div class="flex items-center gap-2 text-success"><CheckCircle class="size-5" /><h2 class="font-bold">{t('github.readyTitle')}</h2></div>
        <p class="text-sm text-muted-foreground">{remote ? `${remote.owner}/${remote.repo} · ${remote.branch}` : ''}</p>
        {#if bindingChanged}
          <label class="flex items-start gap-2 border border-warning-border bg-warning-muted p-3 text-sm">
            <input bind:checked={confirmRebind} class="mt-0.5" type="checkbox" />
            <span>{t('github.confirmRebind')}</span>
          </label>
        {/if}
        <Button disabled={busy} loading={busy} onclick={() => void bindVault()}>
          {t('github.bindVault')}
        </Button>
      </CardContent>
    </Card>
  {/if}
</div>
