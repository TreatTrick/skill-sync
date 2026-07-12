<script lang="ts">
  import { createQuery, useQueryClient } from '@tanstack/svelte-query'
  import { goto } from '$app/navigation'
  import { Copy, ExternalLink, Monitor, Moon, RefreshCw, Sun } from '@lucide/svelte'
  import type { Component } from 'svelte'

  import { errorMessage, openPath } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import { languageState, themeState, type ThemeMode } from '@/shared/state'
  import {
    Button,
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
    Input,
    SegmentedControl,
    Skeleton,
    Spinner,
    Textarea,
    toast,
  } from '@/shared/ui'
  import { scanSkills } from '@/modules/skills'

  import { disconnectGithub, getAppState, saveConfig } from '../api/configApi'
  import type { AppConfig } from '../schemas/config'

  const toLines = (values: string[]): string => values.join('\n')
  const fromLines = (text: string): string[] =>
    text
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line.length > 0)

  const queryClient = useQueryClient()
  const appState = createQuery(() => ({
    queryKey: ['app-state'],
    queryFn: getAppState,
  }))
  const scan = createQuery(() => ({
    queryKey: ['scan-skills'],
    queryFn: scanSkills,
    enabled: appState.data?.configured ?? false,
  }))
  let config = $state<AppConfig | null>(null)
  let ignore = $state('')
  let prefilled = $state(false)
  let disconnectDialogOpen = $state(false)
  let lastSaved: string | null = null
  let saveTimer: ReturnType<typeof setTimeout> | undefined

  const namespaceLabelKeys = {
    agents: 'settings.namespace.agents',
    codex: 'settings.namespace.codex',
    'claude-code': 'settings.namespace.claudeCode',
  } as const
  const credentialStatusLabelKeys = {
    disconnected: 'settings.credentialDisconnected',
    valid: 'settings.credentialValid',
    refreshing: 'settings.credentialRefreshing',
    reauthorization_required: 'settings.credentialReauthorizationRequired',
  } as const

  $effect(() => {
    if (appState.data && !prefilled) {
      prefilled = true
      config = appState.data.config
      ignore = toLines(appState.data.config.ignore)
    }
  })

  const effectiveConfig = $derived(
    config
      ? {
          ...config,
          ignore: fromLines(ignore),
        }
      : null,
  )
  const limitsInvalid = $derived(
    config !== null &&
      config.limits.max_single_file_unpacked_bytes >
        config.limits.max_skill_unpacked_bytes,
  )

  $effect(() => {
    const current = effectiveConfig
    if (!current || limitsInvalid) return
    const snapshot = JSON.stringify(current)
    if (snapshot === lastSaved) return
    if (lastSaved === null) {
      lastSaved = snapshot
      return
    }
    clearTimeout(saveTimer)
    saveTimer = setTimeout(() => {
      void saveConfig(current)
        .then(() => {
          lastSaved = snapshot
          void queryClient.invalidateQueries({ queryKey: ['app-state'] })
        })
        .catch((error) => {
          toast.error(errorMessage(error))
        })
    }, 400)
  })

  const themeOptions = $derived<
    { value: ThemeMode; icon: Component<{ class?: string }>; label: string }[]
  >([
    { value: 'light', icon: Sun, label: t('settings.themeLight') },
    { value: 'dark', icon: Moon, label: t('settings.themeDark') },
    { value: 'system', icon: Monitor, label: t('settings.themeSystem') },
  ])
  const languageOptions = $derived<{ value: 'zh-CN' | 'en-US'; label: string }[]>([
    { value: 'zh-CN', label: t('settings.languageZh') },
    { value: 'en-US', label: t('settings.languageEn') },
  ])

  const formatBytes = (value: number): string => {
    if (value >= 1024 * 1024) return `${Math.round(value / 1024 / 1024)} MiB`
    if (value >= 1024) return `${Math.round(value / 1024)} KiB`
    return `${value} B`
  }

  const copyPath = async (path: string): Promise<void> => {
    try {
      await navigator.clipboard.writeText(path)
      toast.success(t('settings.pathCopied'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const openDisconnectDialog = (): void => {
    if (
      appState.data?.repository_id === null ||
      appState.data?.repository_id === undefined
    ) {
      return
    }
    disconnectDialogOpen = true
  }

  const handleDisconnect = async (): Promise<void> => {
    const repositoryId = appState.data?.repository_id
    if (repositoryId === null || repositoryId === undefined) return
    try {
      await disconnectGithub(repositoryId)
      await queryClient.invalidateQueries({ queryKey: ['app-state'] })
      await goto('/app/onboarding', { replaceState: true })
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      disconnectDialogOpen = false
    }
  }
</script>

<div class="grid gap-4">
  <Card>
    <CardHeader>
      <CardTitle>{t('settings.appearance')}</CardTitle>
      <CardDescription>{t('settings.appearanceDesc')}</CardDescription>
    </CardHeader>
    <CardContent>
      <SegmentedControl
        options={themeOptions}
        value={themeState.theme}
        onSelect={(v) => themeState.setTheme(v as ThemeMode)}
      />
    </CardContent>
  </Card>

  <Card>
    <CardHeader>
      <CardTitle>{t('settings.language')}</CardTitle>
      <CardDescription>{t('settings.languageDesc')}</CardDescription>
    </CardHeader>
    <CardContent>
      <SegmentedControl
        options={languageOptions}
        value={languageState.language}
        onSelect={(v) => void languageState.setLanguage(v as 'zh-CN' | 'en-US')}
      />
    </CardContent>
  </Card>

  {#if appState.error}
    <Card class="border-destructive-border bg-destructive-muted">
      <CardContent class="pt-6 text-sm text-destructive">{errorMessage(appState.error)}</CardContent>
    </Card>
  {/if}

  {#if !config}
    <div class="flex justify-center py-12"><Spinner class="size-6" /></div>
  {:else}
    <Card>
      <CardHeader>
        <CardTitle>{t('settings.githubVault')}</CardTitle>
        <CardDescription>{t('settings.githubVaultReadOnly')}</CardDescription>
      </CardHeader>
      <CardContent class="text-sm">
        <dl class="divide-y divide-border-muted">
          <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
            <dt class="text-muted-foreground">{t('settings.githubAppSlug')}</dt>
            <dd class="font-mono text-foreground">{appState.data?.github_app_slug ?? t('settings.remoteCommitEmpty')}</dd>
          </div>
          <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
            <dt class="text-muted-foreground">{t('settings.githubUser')}</dt>
            <dd class="text-foreground">{appState.data?.github_user ?? t('settings.remoteCommitEmpty')}</dd>
          </div>
          <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
            <dt class="text-muted-foreground">{t('settings.credentialStatus')}</dt>
            <dd class="text-foreground">
              {#if appState.data}
                {t(credentialStatusLabelKeys[appState.data.credential_status])}
              {:else}
                {t('settings.remoteCommitEmpty')}
              {/if}
            </dd>
          </div>
          {#if config.remote}
            <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
              <dt class="text-muted-foreground">{t('settings.installationId')}</dt>
              <dd class="font-mono text-foreground">{config.remote.installation_id}</dd>
            </div>
            <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
              <dt class="text-muted-foreground">{t('settings.repositoryId')}</dt>
              <dd class="font-mono text-foreground">{config.remote.repository_id}</dd>
            </div>
            <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
              <dt class="text-muted-foreground">{t('settings.repository')}</dt>
              <dd class="font-mono text-foreground">{config.remote.owner}/{config.remote.repo}:{config.remote.branch}</dd>
            </div>
          {/if}
          <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
            <dt class="text-muted-foreground">{t('settings.deviceName')}</dt>
            <dd class="text-foreground">{appState.data?.device_name ?? config.device_id}</dd>
          </div>
          <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
            <dt class="text-muted-foreground">{t('settings.remoteCommit')}</dt>
            <dd class="font-mono text-foreground">{appState.data?.remote_commit ?? t('settings.remoteCommitEmpty')}</dd>
          </div>
        </dl>
        <div class="flex flex-wrap gap-2 pt-2">
          <Button onclick={() => void goto('/app/onboarding?mode=reconfigure')} size="sm" variant="outline">
            <RefreshCw class="size-4" />{t('settings.reconfigureVault')}
          </Button>
          <Button disabled={appState.data?.repository_id === null} onclick={openDisconnectDialog} size="sm" variant="destructive">
            {t('settings.disconnectGithub')}
          </Button>
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle>{t('settings.limits')}</CardTitle>
        <CardDescription>{t('settings.limitsDescription')}</CardDescription>
      </CardHeader>
      <CardContent class="grid gap-4 sm:grid-cols-2">
        <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('settings.maxSkillZipBytes')}
          <Input bind:value={config.limits.max_skill_zip_bytes} min="1" step="1" type="number" />
          <span class="text-xs font-normal">{formatBytes(config.limits.max_skill_zip_bytes)}</span>
        </label>
        <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('settings.maxSkillFiles')}
          <Input bind:value={config.limits.max_skill_files} min="1" step="1" type="number" />
        </label>
        <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('settings.maxSingleFileUnpacked')}
          <Input bind:value={config.limits.max_single_file_unpacked_bytes} min="1" step="1" type="number" />
          <span class="text-xs font-normal">{formatBytes(config.limits.max_single_file_unpacked_bytes)}</span>
        </label>
        <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('settings.maxSkillUnpacked')}
          <Input bind:value={config.limits.max_skill_unpacked_bytes} min="1" step="1" type="number" />
          <span class="text-xs font-normal">{formatBytes(config.limits.max_skill_unpacked_bytes)}</span>
        </label>
        <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('settings.maxAutoDelete')}
          <Input bind:value={config.limits.max_auto_delete} min="0" step="1" type="number" />
        </label>
        {#if limitsInvalid}
          <p class="text-sm text-destructive sm:col-span-2">{t('settings.singleLimitTooLarge')}</p>
        {/if}
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle>{t('settings.ignore')}</CardTitle>
        <CardDescription>{t('settings.ignoreDescription')}</CardDescription>
      </CardHeader>
      <CardContent>
        <Textarea bind:value={ignore} class="min-h-[180px]" />
      </CardContent>
    </Card>

    <div class="grid gap-3">
      <div>
        <h2 class="text-lg font-semibold text-strong-foreground">{t('settings.skillRoots')}</h2>
        <p class="text-sm text-muted-foreground">{t('settings.skillRootsReadOnly')}</p>
      </div>
      {#if scan.isLoading}
        <div class="grid gap-3 lg:grid-cols-3">
          {#each Array(3) as _, i (i)}
            <div class="rounded-xl border border-border bg-card p-4">
              <Skeleton class="h-5 w-32" />
              <Skeleton class="mt-3 h-3 w-full" />
            </div>
          {/each}
        </div>
      {:else}
        <div class="grid gap-3 lg:grid-cols-3">
          {#each scan.data?.roots ?? [] as root (root.namespace)}
            <Card class="transition-shadow hover:shadow-md">
              <CardContent class="grid gap-3 p-4">
                <div class="flex items-start justify-between gap-2">
                  <div>
                    <h3 class="font-semibold text-strong-foreground">{t(namespaceLabelKeys[root.namespace])}</h3>
                    <p class="mt-1 break-all text-xs text-muted-foreground">{root.root_path}</p>
                  </div>
                  <span class="text-xs {root.exists && root.readable ? 'text-success' : 'text-warning'}">
                    {root.exists ? (root.readable ? t('common.status.ready') : t('settings.rootUnreadable')) : t('settings.rootNotFound')}
                  </span>
                </div>
                <div class="flex gap-2">
                  <Button onclick={() => void openPath(root.root_path)} size="sm" variant="outline">
                    <ExternalLink class="size-4" />{t('common.actions.open')}
                  </Button>
                  <Button onclick={() => void copyPath(root.root_path)} size="sm" variant="ghost">
                    <Copy class="size-4" />{t('settings.copyPath')}
                  </Button>
                </div>
              </CardContent>
            </Card>
          {/each}
        </div>
      {/if}
    </div>
  {/if}

  <Dialog bind:open={disconnectDialogOpen}>
    <DialogContent>
      <DialogHeader>
        <DialogTitle>{t('settings.disconnectTitle')}</DialogTitle>
        <DialogDescription>{t('settings.disconnectConfirm')}</DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button variant="outline" onclick={() => (disconnectDialogOpen = false)}>
          {t('common.actions.cancel')}
        </Button>
        <Button variant="destructive" onclick={() => void handleDisconnect()}>
          {t('settings.disconnectGithub')}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</div>
