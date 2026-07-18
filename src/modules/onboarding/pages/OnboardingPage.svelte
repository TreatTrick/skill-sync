<script lang="ts">
  import { onMount } from 'svelte'
  import { fade } from 'svelte/transition'
  import { page } from '$app/state'
  import { goto } from '$app/navigation'

  import { errorMessage, getAppState, openPath } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import { useQueryClient, createQuery } from '@tanstack/svelte-query'
  import { ExternalLink, LoaderCircle, TriangleAlert } from '@lucide/svelte'
  import {
    Button,
    Callout,
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
    Spinner,
    toast,
  } from '@/shared/ui'

  import CreateRepositoryStage from '../components/CreateRepositoryStage.svelte'
  import DeviceAuthorizationStage from '../components/DeviceAuthorizationStage.svelte'
  import FirstRunIntroDialog from '../components/FirstRunIntroDialog.svelte'
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

  const stageTitle = (currentStage: OnboardingStage): string => {
    if (currentStage === 'app_not_configured') return t('onboarding.stage.appNotConfigured')
    if (currentStage === 'authorize' || currentStage === 'device_pending') return t('onboarding.stage.authorize')
    if (currentStage === 'create_repository') return t('onboarding.stage.createRepository')
    if (currentStage === 'install_app' || currentStage === 'repository_scope_blocked') return t('onboarding.stage.installApp')
    if (currentStage === 'confirm_public_repository' || currentStage === 'select_branch' || currentStage === 'checking_vault' || currentStage === 'vault_unavailable') return t('onboarding.stage.branch')
    if (currentStage === 'confirm_initialize' || currentStage === 'invalid_manifest' || currentStage === 'rate_limited') return t('onboarding.stage.vault')
    return t('onboarding.stage.ready')
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
    void flow.loadAppInfo()
    return () => flow.stopPolling()
  })
</script>

<div class="mx-auto grid min-h-screen w-full max-w-2xl gap-4 px-4 py-10 sm:py-16">
  <FirstRunIntroDialog />
  <div class="flex justify-end">
    <LanguageToggle />
  </div>
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
          <span>{t('onboarding.progress', { current: progressStep(flow.stage), total: 6 })}</span>
          <span>{stageTitle(flow.stage)}</span>
        </div>
        <OnboardingStepper
          ariaLabel={t('onboarding.progress', { current: progressStep(flow.stage), total: 6 })}
          current={progressStep(flow.stage)}
          total={6}
        />
      </div>
    </CardHeader>
  </Card>

  {#if flow.message}
    <Callout tone="danger">
      {#snippet icon()}
        <TriangleAlert class="size-4" />
      {/snippet}
      {flow.message}
    </Callout>
  {/if}

  {#key flow.stage}
    <div in:fade={{ duration: 120 }}>
      {#if !flow.appInfoLoaded || appState.isLoading}
    <Card><CardContent class="flex justify-center py-12"><Spinner class="size-6" /></CardContent></Card>
  {:else if flow.stage === 'app_not_configured'}
    <Card>
      <CardContent class="grid gap-4 pt-6">
        <CardTitle>{t('github.appNotConfigured')}</CardTitle>
        <p class="text-sm text-muted-foreground">{t('github.appNotConfiguredDescription')}</p>
      </CardContent>
    </Card>
  {:else if flow.stage === 'authorize' || flow.stage === 'device_pending'}
    <DeviceAuthorizationStage
      stage={flow.stage}
      busy={flow.busy}
      deviceFlow={flow.deviceFlow}
      deviceExpiresAt={flow.deviceExpiresAt}
      onStart={() => void flow.startDeviceAuthorization()}
      onCopyCode={() => void copyDeviceCode()}
      onOpenExternal={(event, url) => void openExternal(event, url)}
    />
  {:else if flow.stage === 'create_repository'}
    <CreateRepositoryStage
      createRepositoryUrl={CREATE_GITHUB_REPOSITORY_URL}
      onOpenExternal={(event, url) => void openExternal(event, url)}
      onContinue={() => flow.transition('install_app', '')}
    />
  {:else if flow.stage === 'install_app' || flow.stage === 'repository_scope_blocked'}
    <InstallAppStage
      stage={flow.stage}
      installUrl={flow.appInfo?.install_url ?? null}
      busy={flow.busy}
      onOpenExternal={(event, url) => void openExternal(event, url)}
      onCheckInstallation={() => void flow.discoverRepository('recheck')}
    />
  {:else if flow.stage === 'confirm_public_repository' && flow.selectedRepository}
    <PublicRepositoryWarningStage
      repository={flow.selectedRepository}
      installUrl={flow.appInfo?.install_url ?? null}
      createRepositoryUrl={CREATE_GITHUB_REPOSITORY_URL}
      busy={flow.busy}
      onContinue={() => void flow.continueWithPublicRepository()}
      onOpenExternal={(event, url) => void openExternal(event, url)}
    />
  {:else if flow.stage === 'select_branch'}
    <SelectBranchStage
      remote={flow.remote}
      branchNames={flow.branchNames}
      bind:selectedBranch={flow.selectedBranch}
      busy={flow.busy}
      onChooseBranch={() => void flow.chooseBranch()}
    />
  {:else if flow.stage === 'checking_vault'}
    <Card><CardContent class="flex items-center justify-center gap-2 py-12 text-sm text-muted-foreground"><LoaderCircle class="size-4 animate-spin" />{t('github.checkingVault')}</CardContent></Card>
  {:else if flow.stage === 'vault_unavailable'}
    <Card>
      <CardContent class="grid gap-4 pt-6">
        <h2 class="font-semibold text-warning">{t('github.vaultUnavailable')}</h2>
        <Button disabled={flow.busy} loading={flow.busy} onclick={() => void flow.checkVault()}>{t('common.actions.retry')}</Button>
      </CardContent>
    </Card>
  {:else if flow.stage === 'confirm_initialize'}
    <Card>
      <CardContent class="grid gap-4 pt-6">
        <h2 class="font-semibold text-strong-foreground">{t('github.initializeVault')}</h2>
        <p class="text-sm text-muted-foreground">{t('github.initializeCreatesCommit')}</p>
        <Button disabled={flow.busy} loading={flow.busy} onclick={() => void flow.initializeVault()}>
          {t('github.confirmInitialize')}
        </Button>
      </CardContent>
    </Card>
  {:else if flow.stage === 'invalid_manifest'}
    <Card>
      <CardContent class="grid gap-4 pt-6">
        <h2 class="font-bold text-destructive">{t('github.invalidManifest')}</h2>
        <p class="text-sm text-muted-foreground">{t('github.invalidManifestDescription')}</p>
        {#if flow.remote}
          <Button
            class="w-fit"
            onclick={(event: MouseEvent) => void openExternal(event, `https://github.com/${flow.remote?.owner ?? ''}/${flow.remote?.repo ?? ''}`)}
            variant="outline"
          >
            {t('github.openRepository')} <ExternalLink class="size-4" />
          </Button>
        {/if}
      </CardContent>
    </Card>
  {:else if flow.stage === 'rate_limited'}
    <Card>
      <CardContent class="grid gap-4 pt-6">
        <h2 class="font-semibold text-warning">{t('github.rateLimited')}</h2>
        <Button disabled={flow.busy} loading={flow.busy} onclick={() => void flow.checkVault()}>{t('common.actions.retry')}</Button>
      </CardContent>
    </Card>
  {:else if flow.stage === 'ready'}
    <VaultReadyStage
      remote={flow.remote}
      bindingChanged={bindingChanged}
      bind:confirmRebind={flow.confirmRebind}
      busy={flow.busy}
      onBindVault={() => void handleBindVault()}
    />
      {/if}
    </div>
  {/key}
</div>
