<script lang="ts">
  import { createQuery, useIsMutating, useQueryClient } from '@tanstack/svelte-query'
  import { ArrowDownToLine, CircleArrowUp, ExternalLink, RefreshCw, RotateCcw, ShieldCheck, Star } from '@lucide/svelte'

  import { t } from '@/shared/i18n'
  import { errorMessage, openPath } from '@/shared/lib'
  import { languageState } from '@/shared/state'
  import { Button, Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, toast } from '@/shared/ui'

  import { appUpdateQueryOptions, getSoftwareInfo, PROJECT_URL } from '../api/updateApi'
  import { updateInstaller } from '../state/updateInstaller.svelte'

  const queryClient = useQueryClient()
  const pendingMutations = useIsMutating()
  const info = createQuery(() => ({ queryKey: ['software-info'], queryFn: getSoftwareInfo, staleTime: Infinity, retry: false }))
  const updates = createQuery(() => appUpdateQueryOptions)
  let confirmInstall = $state(false)
  const status = $derived(updateInstaller.status)
  const progress = $derived(updateInstaller.totalBytes
    ? Math.min(100, Math.round(updateInstaller.downloadedBytes / updateInstaller.totalBytes * 100))
    : undefined)
  const statusKey = $derived(status !== 'idle' ? `about.updateState.${status}` as const
    : updates.isFetching ? 'about.updateState.checking'
      : updates.isError ? 'about.updateState.failed'
        : updates.data ? 'about.updateState.available'
          : updates.isSuccess ? 'about.updateState.latest' : 'about.updateState.checking')
  const releaseDate = $derived(updates.data?.date
    ? new Date(updates.data.date).toLocaleDateString(languageState.language)
    : null)

  const openLink = async (url: string): Promise<void> => {
    try {
      await openPath(url)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const install = (): void => {
    confirmInstall = false
    void updateInstaller.install(() => queryClient.isMutating() > 0)
  }
</script>

<div class="view">
  <div class="page">
    <div class="mx-auto grid w-full max-w-4xl gap-6">
      <header class="flex flex-wrap items-center gap-4">
        <img src="/favicon.svg" alt="" class="size-14 rounded-xl border border-border bg-surface p-2 shadow-sm" />
        <div class="min-w-0 flex-1">
          <h1 class="text-2xl">{t('routes.about')}</h1>
          <p class="mt-1 text-sm text-muted-foreground">{t('about.description')}</p>
        </div>
      </header>

      <section aria-labelledby="software-heading" class="grid gap-3">
        <h2 id="software-heading" class="text-base">{t('about.softwareInfo')}</h2>
        {#if info.error}
          <div role="alert" class="flex flex-wrap items-center gap-3 rounded-lg border border-destructive-border bg-destructive-muted p-4 text-sm text-destructive">
            <span class="min-w-0 flex-1 break-words">{t('about.infoFailed', { message: errorMessage(info.error) })}</span>
            <Button variant="outline" onclick={() => void info.refetch()}>{t('common.actions.retry')}</Button>
          </div>
        {/if}
        <dl class="grid grid-cols-1 gap-px overflow-hidden rounded-lg border border-border bg-border sm:grid-cols-2">
          {#each [
            [t('about.name'), info.data?.name ?? t('common.status.loading')],
            [t('about.currentVersion'), info.data?.version ?? t('common.status.loading')],
            [t('about.platform'), info.data?.platform ?? t('common.status.loading')],
            [t('about.arch'), info.data?.arch ?? t('common.status.loading')],
            [t('about.channel'), t('about.stable')],
            [t('about.license'), 'MIT'],
          ] as [label, value] (label)}
            <div class="min-w-0 bg-surface px-4 py-3">
              <dt class="text-xs text-muted-foreground">{label}</dt>
              <dd class="mt-1 break-words text-sm font-medium">{value}</dd>
            </div>
          {/each}
        </dl>
      </section>

      <section aria-labelledby="updates-heading" class="grid gap-3">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <h2 id="updates-heading" class="text-base">{t('about.updates')}</h2>
          <Button variant="outline" size="sm" disabled={updates.isFetching || status !== 'idle'} onclick={() => void updates.refetch({ cancelRefetch: false })}>
            <RefreshCw class={updates.isFetching ? 'animate-spin' : ''} />
            {t('about.checkUpdates')}
          </Button>
        </div>
        <div class="grid gap-4 rounded-lg border border-border bg-surface p-4 sm:p-5">
          <div class="flex items-start gap-3" role="status" aria-live="polite">
            {#if updates.data && !updates.isError}
              <CircleArrowUp class="mt-0.5 size-5 text-success" />
            {:else}
              <ShieldCheck class="mt-0.5 size-5 text-muted-foreground" />
            {/if}
            <div class="min-w-0 flex-1">
              <h3 class="text-sm">{t(statusKey)}</h3>
              <p class="mt-1 text-sm text-muted-foreground">
                {#if updates.data && !updates.isError}
                  {t('about.versionChange', { current: updates.data.currentVersion, latest: updates.data.version })}
                {:else}
                  {t('about.updateSource')}
                {/if}
              </p>
            </div>
          </div>

          {#if updates.error}
            <div role="alert" class="grid gap-1 rounded-md border border-destructive-border bg-destructive-muted p-3 text-sm text-destructive">
              <p class="break-words">{t('about.checkFailed', { message: errorMessage(updates.error) })}</p>
              <p>{t('about.checkFailureHint')}</p>
            </div>
          {/if}
          {#if updateInstaller.errorStage}
            <div role="alert" class="break-words rounded-md border border-destructive-border bg-destructive-muted p-3 text-sm text-destructive">
              {#if updateInstaller.errorStage === 'busy'}
                {t('about.waitForOperations')}
              {:else}
                {t(`about.errors.${updateInstaller.errorStage}`, { message: errorMessage(updateInstaller.error) })}
              {/if}
            </div>
          {/if}

          {#if status === 'downloading'}
            <div class="grid gap-2">
              <progress class="h-2 w-full accent-primary" max="100" value={progress} aria-label={t('about.downloadProgress')}></progress>
              <p class="text-xs tabular-nums text-muted-foreground" aria-live="polite">
                {progress !== undefined ? t('about.downloadPercent', { percent: progress }) : t('about.downloadBytes', { size: (updateInstaller.downloadedBytes / 1_048_576).toFixed(1) })}
              </p>
            </div>
          {/if}

          {#if updates.data && !updates.isError}
            <div class="flex flex-wrap items-center gap-2">
              {#if status === 'idle'}
                <Button disabled={updates.isFetching} onclick={() => { if (updates.data) void updateInstaller.download(updates.data.resource) }}>
                  <ArrowDownToLine />{t('about.downloadUpdate')}
                </Button>
              {:else if status === 'downloaded'}
                <Button disabled={pendingMutations.current > 0} onclick={() => (confirmInstall = true)}><CircleArrowUp />{t('about.installUpdate')}</Button>
              {:else if status === 'installed'}
                <Button disabled={pendingMutations.current > 0} onclick={() => void updateInstaller.restart(() => queryClient.isMutating() > 0)}><RotateCcw />{t('about.restartNow')}</Button>
              {:else}
                <Button loading disabled>{t(statusKey)}</Button>
              {/if}
              <Button variant="outline" onclick={() => void openLink(updates.data?.releaseUrl ?? `${PROJECT_URL}/releases/latest`)}>
                <ExternalLink />{t('about.viewRelease')}
              </Button>
            </div>
            {#if status === 'downloaded' || status === 'installed'}
              <p class="text-xs text-muted-foreground">{pendingMutations.current > 0 ? t('about.waitForOperations') : status === 'downloaded' ? t('about.downloadVerified') : t('about.restartHint')}</p>
            {/if}
            <div class="grid gap-2 border-t border-border pt-4">
              <div class="flex flex-wrap items-baseline justify-between gap-2">
                <h3 class="text-sm">{t('about.releaseNotes')}</h3>
                {#if releaseDate}<time class="text-xs text-muted-foreground" datetime={updates.data.date ?? undefined}>{releaseDate}</time>{/if}
              </div>
              <p class="max-h-64 overflow-auto whitespace-pre-wrap break-words text-sm text-muted-foreground">{updates.data.body || t('about.noReleaseNotes')}</p>
            </div>
          {/if}
        </div>
      </section>

      <section aria-labelledby="project-heading" class="grid gap-3">
        <h2 id="project-heading" class="text-base">{t('about.project')}</h2>
        <div class="flex flex-wrap items-center justify-between gap-4 rounded-lg border border-border bg-surface p-4">
          <p class="min-w-0 flex-1 text-sm text-muted-foreground">{t('settings.supportProjectDescription')}</p>
          <div class="flex flex-wrap gap-2">
            <Button variant="outline" onclick={() => void openLink(PROJECT_URL)}><Star />{t('settings.supportOnGithub')}</Button>
            <Button variant="outline" onclick={() => void openLink(`${PROJECT_URL}/blob/main/LICENSE`)}><ExternalLink />{t('about.viewLicense')}</Button>
          </div>
        </div>
      </section>
    </div>
  </div>
</div>

<Dialog bind:open={confirmInstall}>
  <DialogContent>
    <DialogHeader>
      <DialogTitle>{t('about.installTitle')}</DialogTitle>
      <DialogDescription>{info.data?.platform === 'windows' ? t('about.installWindows') : t('about.installOther')}</DialogDescription>
    </DialogHeader>
    <DialogFooter>
      <Button variant="outline" onclick={() => (confirmInstall = false)}>{t('common.actions.cancel')}</Button>
      <Button disabled={pendingMutations.current > 0} onclick={install}>{t('about.installUpdate')}</Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
