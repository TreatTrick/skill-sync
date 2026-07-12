<script lang="ts">
  import { createQuery, useQueryClient } from '@tanstack/svelte-query'
  import { goto } from '$app/navigation'
  import { Copy, ExternalLink, Monitor, Moon, RefreshCw, Sun } from '@lucide/svelte'
  import type { Component } from 'svelte'

  import { cn, errorMessage, openPath } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import { languageState, themeState, type ThemeMode } from '@/shared/state'
  import {
    Button,
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
    Input,
    Spinner,
    Textarea,
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
  let saveError = $state('')
  let actionMessage = $state('')
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
          saveError = ''
          void queryClient.invalidateQueries({ queryKey: ['app-state'] })
        })
        .catch((error) => {
          saveError = errorMessage(error)
        })
    }, 400)
  })

  const themeOptions = $derived<
    { mode: ThemeMode; icon: Component<{ class?: string }>; label: string }[]
  >([
    { mode: 'light', icon: Sun, label: t('settings.themeLight') },
    { mode: 'dark', icon: Moon, label: t('settings.themeDark') },
    { mode: 'system', icon: Monitor, label: t('settings.themeSystem') },
  ])
  const languageOptions = $derived<{ code: 'zh-CN' | 'en-US'; label: string }[]>([
    { code: 'zh-CN', label: t('settings.languageZh') },
    { code: 'en-US', label: t('settings.languageEn') },
  ])

  const formatBytes = (value: number): string => {
    if (value >= 1024 * 1024) return `${Math.round(value / 1024 / 1024)} MiB`
    if (value >= 1024) return `${Math.round(value / 1024)} KiB`
    return `${value} B`
  }

  const copyPath = async (path: string): Promise<void> => {
    try {
      await navigator.clipboard.writeText(path)
      actionMessage = t('settings.pathCopied')
    } catch (error) {
      actionMessage = errorMessage(error)
    }
  }

  const handleDisconnect = async (): Promise<void> => {
    const repositoryId = appState.data?.repository_id
    if (repositoryId === null || repositoryId === undefined) return
    if (!window.confirm(t('settings.disconnectConfirm'))) return
    try {
      await disconnectGithub(repositoryId)
      await queryClient.invalidateQueries({ queryKey: ['app-state'] })
      await goto('/app/onboarding', { replaceState: true })
    } catch (error) {
      actionMessage = errorMessage(error)
    }
  }
</script>

<div class="grid gap-4">
  <Card>
    <CardHeader>
      <CardTitle>{t('settings.appearance')}</CardTitle>
      <CardDescription>{t('settings.appearanceDesc')}</CardDescription>
    </CardHeader>
    <CardContent class="flex flex-wrap gap-2">
      {#each themeOptions as { mode, icon: Icon, label } (mode)}
        <button
          class={cn(
            'flex h-9 min-w-32 flex-1 items-center justify-center gap-1.5 rounded-lg border text-sm font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/40',
            themeState.theme === mode
              ? 'border-primary bg-primary-muted text-primary-muted-foreground'
              : 'border-border bg-surface text-foreground hover:bg-surface-hover',
          )}
          onclick={() => themeState.setTheme(mode)}
          type="button"
        >
          <Icon class="size-4" />
          {label}
        </button>
      {/each}
    </CardContent>
  </Card>

  <Card>
    <CardHeader>
      <CardTitle>{t('settings.language')}</CardTitle>
      <CardDescription>{t('settings.languageDesc')}</CardDescription>
    </CardHeader>
    <CardContent class="flex gap-2">
      {#each languageOptions as { code, label } (code)}
        <button
          class={cn(
            'flex h-9 flex-1 items-center justify-center rounded-lg border text-sm font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/40',
            languageState.language === code
              ? 'border-primary bg-primary-muted text-primary-muted-foreground'
              : 'border-border bg-surface text-foreground hover:bg-surface-hover',
          )}
          onclick={() => void languageState.setLanguage(code)}
          type="button"
        >
          {label}
        </button>
      {/each}
    </CardContent>
  </Card>

  {#if saveError || actionMessage}
    <Card class={saveError ? 'border-destructive-border bg-destructive-muted' : 'border-success-muted bg-success-muted'}>
      <CardContent class="pt-6 text-sm {saveError ? 'text-destructive' : 'text-success'}">
        {saveError || actionMessage}
      </CardContent>
    </Card>
  {/if}

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
      <CardContent class="grid gap-3 text-sm">
        <div class="grid gap-1 sm:grid-cols-2">
          <span class="text-muted-foreground">{t('settings.githubAppSlug')}</span>
          <span>{appState.data?.github_app_slug ?? t('settings.remoteCommitEmpty')}</span>
        </div>
        <div class="grid gap-1 sm:grid-cols-2">
          <span class="text-muted-foreground">{t('settings.githubUser')}</span>
          <span>{appState.data?.github_user ?? t('settings.remoteCommitEmpty')}</span>
        </div>
        <div class="grid gap-1 sm:grid-cols-2">
          <span class="text-muted-foreground">{t('settings.credentialStatus')}</span>
          {#if appState.data}
            <span>{t(credentialStatusLabelKeys[appState.data.credential_status])}</span>
          {:else}
            <span>{t('settings.remoteCommitEmpty')}</span>
          {/if}
        </div>
        {#if config.remote}
          <div class="grid gap-1 sm:grid-cols-2"><span class="text-muted-foreground">{t('settings.installationId')}</span><span>{config.remote.installation_id}</span></div>
          <div class="grid gap-1 sm:grid-cols-2"><span class="text-muted-foreground">{t('settings.repositoryId')}</span><span>{config.remote.repository_id}</span></div>
          <div class="grid gap-1 sm:grid-cols-2"><span class="text-muted-foreground">{t('settings.repository')}</span><span>{config.remote.owner}/{config.remote.repo}:{config.remote.branch}</span></div>
        {/if}
        <div class="grid gap-1 sm:grid-cols-2"><span class="text-muted-foreground">{t('settings.deviceName')}</span><span>{appState.data?.device_name ?? config.device_id}</span></div>
        <div class="grid gap-1 sm:grid-cols-2"><span class="text-muted-foreground">{t('settings.remoteCommit')}</span><span>{appState.data?.remote_commit ?? t('settings.remoteCommitEmpty')}</span></div>
        <div class="flex flex-wrap gap-2 pt-2">
          <Button onclick={() => void goto('/app/onboarding?mode=reconfigure')} size="sm" variant="outline">
            <RefreshCw class="size-4" />{t('settings.reconfigureVault')}
          </Button>
          <Button disabled={appState.data?.repository_id === null} onclick={() => void handleDisconnect()} size="sm" variant="destructive">
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
        <h2 class="text-lg font-bold text-strong-foreground">{t('settings.skillRoots')}</h2>
        <p class="text-sm text-muted-foreground">{t('settings.skillRootsReadOnly')}</p>
      </div>
      {#if scan.isLoading}
        <div class="flex justify-center py-8"><Spinner class="size-6" /></div>
      {:else}
        <div class="grid gap-3 lg:grid-cols-3">
          {#each scan.data?.roots ?? [] as root (root.namespace)}
            <Card>
              <CardContent class="grid gap-3 p-4">
                <div class="flex items-start justify-between gap-2">
                  <div>
                    <h3 class="font-bold text-strong-foreground">{t(namespaceLabelKeys[root.namespace])}</h3>
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
</div>
