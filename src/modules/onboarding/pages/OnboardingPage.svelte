<script lang="ts">
  import { onMount } from 'svelte'
  import { page } from '$app/state'
  import { goto } from '$app/navigation'

  import { errorMessage, getAppState, INTRO_SEEN_STORAGE_KEY, openPath } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import { useQueryClient, createQuery } from '@tanstack/svelte-query'
  import { Clock3, ExternalLink, LoaderCircle, TriangleAlert } from '@lucide/svelte'
  import {
    Button,
    Spinner,
    toast,
  } from '@/shared/ui'

  import CreateRepositoryStage from '../components/CreateRepositoryStage.svelte'
  import DeviceAuthorizationStage from '../components/DeviceAuthorizationStage.svelte'
  import FirstRunIntro from '../components/FirstRunIntro.svelte'
  import InstallAppStage from '../components/InstallAppStage.svelte'
  import LanguageToggle from '../components/LanguageToggle.svelte'
  import OnboardingStepper from '../components/OnboardingStepper.svelte'
  import PublicRepositoryWarningStage from '../components/PublicRepositoryWarningStage.svelte'
  import SelectBranchStage from '../components/SelectBranchStage.svelte'
  import VaultReadyStage from '../components/VaultReadyStage.svelte'

  import {
    startGithubDeviceFlow,
    pollGithubDeviceFlow,
    getGithubAppInfo,
    listInstallationRepositories,
    discoverSingleGithubRepository,
    listGithubRepositoryBranches,
    checkGithubVault,
    initializeGithubVault,
    bindGithubVault,
    establishBaseline,
  } from '../api/onboardingApi'
  import { OnboardingFlow, type OnboardingStage } from '../state/onboardingFlow.svelte'

  const CREATE_GITHUB_REPOSITORY_URL = 'https://github.com/new'

  const appState = createQuery(() => ({
    queryKey: ['app-state'],
    queryFn: getAppState,
  }))
  const queryClient = useQueryClient()
  const reconfigure = $derived(page.url.searchParams.get('mode') === 'reconfigure')
  let introOpen = $state(false)
  let introLoaded = $state(false)

  // Per-mount orchestration. The flow owns the state machine; this page is a
  // thin view that renders flow state, forwards appState, bridges the stale
  // signal to the query cache, and runs view side-effects (toast/goto).
  const flow = new OnboardingFlow({
    startGithubDeviceFlow,
    pollGithubDeviceFlow,
    getGithubAppInfo,
    listInstallationRepositories,
    discoverSingleGithubRepository,
    listGithubRepositoryBranches,
    checkGithubVault,
    initializeGithubVault,
    bindGithubVault,
    establishBaseline,
  })

  const bindingChanged = $derived(flow.bindingChanged(appState.data))

  const progressStep = (currentStage: OnboardingStage): number => {
    if (currentStage === 'app_not_configured' || currentStage === 'authorize' || currentStage === 'device_pending') return 1
    if (currentStage === 'create_repository') return 2
    if (currentStage === 'install_app' || currentStage === 'repository_scope_blocked') return 3
    if (currentStage === 'confirm_public_repository' || currentStage === 'select_branch' || currentStage === 'checking_vault' || currentStage === 'vault_unavailable') return 4
    if (currentStage === 'confirm_initialize' || currentStage === 'invalid_manifest' || currentStage === 'rate_limited') return 5
    return 6
  }

  const openExternal = async (event: MouseEvent, url: string): Promise<void> => {
    event.preventDefault()
    try {
      await openPath(url)
    } catch (error) {
      flow.handleError(error)
    }
  }

  const copyDeviceCode = async (): Promise<void> => {
    if (!flow.deviceFlow?.user_code) return
    try {
      await navigator.clipboard.writeText(flow.deviceFlow.user_code)
      toast.success(t('github.deviceCodeCopied'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  // Flow owns the bind + best-effort baseline sequence and returns a result; the
  // page wires the svelte-query cache, toast, and navigation from that result.
  const handleBindVault = async (): Promise<void> => {
    const result = await flow.bindVault(appState.data)
    if (result.appState) queryClient.setQueryData(['app-state'], result.appState)
    if (result.baselineAdoptions > 0) {
      toast.success(t('onboarding.baselineEstablished', { count: result.baselineAdoptions }))
    }
    if (result.navigateTo) await goto(result.navigateTo, { replaceState: true })
  }

  const cancelReconfigure = (): void => {
    void goto('/app/sync', { replaceState: true })
  }

  const continueSetup = (): void => {
    window.localStorage.setItem(INTRO_SEEN_STORAGE_KEY, 'true')
    introOpen = false
  }

  // Forward the appState cache to the flow so it can run the initial
  // auto-discovery guard (the decision lives in the flow; the subscription
  // lives here, next to svelte-query).
  $effect(() => {
    flow.onAppStateChanged(appState.data)
  })

  // Bridge: device-flow authorize bumped the stale signal -> refresh the
  // appState cache (github_authorized flipped). The flow is svelte-query-agnostic.
  $effect(() => {
    const seq = flow.appStateStaleSeq
    if (seq > 0) void queryClient.invalidateQueries({ queryKey: ['app-state'] })
  })

  onMount(() => {
    introOpen =
      !reconfigure && window.localStorage.getItem(INTRO_SEEN_STORAGE_KEY) !== 'true'
    introLoaded = true
    void flow.loadAppInfo()
    return () => flow.stopPolling()
  })
</script>
<div class="prototype-ui flex h-dvh min-h-0 flex-col overflow-hidden">
  <div class="ob">
    <aside class="ob-rail">
      <div><div class="brand"><img class="size-6.5 rounded-lg shadow-sm" src="/favicon.svg" alt="" /><span class="ob-brand-name">Skill Sync</span></div><h2>{reconfigure ? t('settings.reconfigureVault') : t('onboarding.description')}</h2></div>
      <OnboardingStepper current={introOpen ? 0 : progressStep(flow.stage)} ariaLabel={t('onboarding.progress', { current: progressStep(flow.stage), total: 6 })} summaries={[appState.data?.github_user ? '@' + appState.data.github_user : '', '', flow.remote?.repo ?? '', flow.selectedBranch, flow.vaultCheck?.status === 'ready' ? t('github.manifestReady') : '']} />
      <div class="foot"><LanguageToggle />{#if reconfigure}<Button class="btn ghost sm" onclick={cancelReconfigure} variant="ghost">{t('onboarding.cancelReconfigure')}</Button>{/if}</div>
    </aside>
    <main class="ob-main">
      <FirstRunIntro open={introOpen} onContinue={continueSetup} />
      {#if introLoaded && !introOpen}
        <div class="panel">
          {#if flow.message}<div class="callout danger" role="alert"><TriangleAlert class="ic" /><div class="grow">{flow.message}</div></div>{/if}
          {#if !flow.appInfoLoaded || appState.isLoading}<Spinner class="size-6" />
          {:else if flow.stage === 'app_not_configured'}
            <div class="statusico warn"><TriangleAlert class="ic lg" /></div><h1>{t('github.appNotConfigured')}</h1><p class="lede">{t('github.appNotConfiguredDescription')}</p>
          {:else if flow.stage === 'authorize' || flow.stage === 'device_pending'}
            <DeviceAuthorizationStage stage={flow.stage} busy={flow.busy} deviceFlow={flow.deviceFlow} deviceExpiresAt={flow.deviceExpiresAt} onStart={() => void flow.startDeviceAuthorization()} onCopyCode={() => void copyDeviceCode()} onOpenExternal={(event, url) => void openExternal(event, url)} />
          {:else if flow.stage === 'create_repository'}
            <CreateRepositoryStage createRepositoryUrl={CREATE_GITHUB_REPOSITORY_URL} onOpenExternal={(event, url) => void openExternal(event, url)} onContinue={() => flow.transition('install_app', '')} />
          {:else if flow.stage === 'install_app' || flow.stage === 'repository_scope_blocked'}
            <InstallAppStage stage={flow.stage} installUrl={flow.appInfo?.install_url ?? null} busy={flow.busy} onOpenExternal={(event, url) => void openExternal(event, url)} onCheckInstallation={() => void flow.discoverRepository('recheck')} />
          {:else if flow.stage === 'confirm_public_repository' && flow.selectedRepository}
            <PublicRepositoryWarningStage repository={flow.selectedRepository} installUrl={flow.appInfo?.install_url ?? null} createRepositoryUrl={CREATE_GITHUB_REPOSITORY_URL} busy={flow.busy} onContinue={() => void flow.continueWithPublicRepository()} onOpenExternal={(event, url) => void openExternal(event, url)} />
          {:else if flow.stage === 'select_branch'}
            <SelectBranchStage remote={flow.remote} branchNames={flow.branchNames} bind:selectedBranch={flow.selectedBranch} busy={flow.busy} onChooseBranch={() => void flow.chooseBranch()} />
          {:else if flow.stage === 'checking_vault'}
            <div class="eyebrow">{t('onboarding.progress', { current: 4, total: 6 })}</div><h1>{t('github.checkingVault')}</h1><div class="wait"><LoaderCircle class="ic spin" />{flow.remote?.owner}/{flow.remote?.repo}@{flow.selectedBranch} · manifest.json</div>
          {:else if flow.stage === 'vault_unavailable'}
            <div class="eyebrow">{t('onboarding.progress', { current: 4, total: 6 })}</div><div class="statusico warn"><TriangleAlert class="ic lg" /></div>
            <h1>{t('github.vaultUnavailableTitle')}</h1><p class="lede">{t('github.vaultUnavailableDescription', { repository: flow.remote ? flow.remote.owner + '/' + flow.remote.repo : '—' })}</p>
            <div class="acts"><Button class="btn primary lg" disabled={flow.busy} loading={flow.busy} onclick={() => void flow.checkVault()}>{t('common.actions.retry')}</Button>
              {#if flow.appInfo?.install_url}<Button class="btn lg" onclick={(event: MouseEvent) => void openExternal(event, flow.appInfo?.install_url ?? '')} variant="outline">{t('github.adjustInstallation')}<ExternalLink class="ic sm" /></Button>{/if}
            </div>
          {:else if flow.stage === 'confirm_initialize'}
            <div class="eyebrow">{t('onboarding.progress', { current: 5, total: 6 })}</div><h1>{t('github.initializeVault')}</h1><p class="lede">{t('github.initializeCreatesCommit')}</p>
            <div class="box mono init-files"><span class="muted">{t('github.willCreate')}</span><span>+ manifest.json</span></div>
            <div class="acts"><Button class="btn primary lg" disabled={flow.busy} loading={flow.busy} onclick={() => void flow.initializeVault()}>{t('github.confirmInitialize')}</Button></div>
          {:else if flow.stage === 'invalid_manifest'}
            <div class="eyebrow">{t('onboarding.progress', { current: 5, total: 6 })}</div><div class="statusico bad"><TriangleAlert class="ic lg" /></div><h1>{t('github.invalidManifest')}</h1><p class="lede">{t('github.invalidManifestDescription')}</p>
            <div class="acts">
              {#if flow.remote}<Button class="btn lg" onclick={(event: MouseEvent) => void openExternal(event, 'https://github.com/' + flow.remote?.owner + '/' + flow.remote?.repo)} variant="outline">{t('github.openRepository')}<ExternalLink class="ic sm" /></Button>{/if}
              <Button class="btn primary lg" disabled={flow.busy} loading={flow.busy} onclick={() => void flow.checkVault()}>{t('sync.recheck')}</Button>
            </div>
          {:else if flow.stage === 'rate_limited'}
            <div class="eyebrow">{t('onboarding.progress', { current: 5, total: 6 })}</div><div class="statusico warn"><Clock3 class="ic lg" /></div><h1>{t('github.rateLimitedTitle')}</h1><p class="lede">{t('github.rateLimitedDescription')}</p>
            <div class="acts"><Button class="btn primary lg" disabled={flow.busy} loading={flow.busy} onclick={() => void flow.checkVault()}>{t('common.actions.retry')}</Button></div>
          {:else if flow.stage === 'ready'}
            <VaultReadyStage remote={flow.remote} user={appState.data?.github_user} isPrivate={flow.selectedRepository?.private ?? null} {bindingChanged} bind:confirmRebind={flow.confirmRebind} busy={flow.busy} onBindVault={() => void handleBindVault()} />
          {/if}
        </div>
      {/if}
    </main>
  </div>
</div>
