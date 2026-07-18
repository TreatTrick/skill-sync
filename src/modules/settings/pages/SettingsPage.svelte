<script lang="ts">
  import { createQuery, useQueryClient } from '@tanstack/svelte-query'
  import { goto } from '$app/navigation'
  import { Monitor, Moon, Star, Sun } from '@lucide/svelte'
  import type { Component } from 'svelte'

  import { errorMessage, getAppState, openPath, resetIntroSeen, scanSkills } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import { languageState, themeState, type ThemeMode } from '@/shared/state'
  import {
    Button,
    Card,
    CardContent,
    CardDescription,
    CardFooter,
    CardHeader,
    CardTitle,
    SegmentedControl,
    Spinner,
    Textarea,
    toast,
  } from '@/shared/ui'

  import { disconnectGithub, saveConfig } from '../api/configApi'
  import DisconnectGithubDialog from '../components/DisconnectGithubDialog.svelte'
  import GithubVaultCard from '../components/GithubVaultCard.svelte'
  import LimitsCard from '../components/LimitsCard.svelte'
  import SkillRootsSection from '../components/SkillRootsSection.svelte'
  import type { AppConfig } from '@/shared/schemas'

  const toLines = (values: string[]): string => values.join('\n')
  const fromLines = (text: string): string[] =>
    text
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line.length > 0)
  const PROJECT_REPOSITORY_URL = 'https://github.com/TreatTrick/skill-sync'

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
    // Cancel any pending save before deciding, so a "change then revert" within
    // the debounce window never persists the intermediate value.
    clearTimeout(saveTimer)
    if (snapshot === lastSaved) return
    if (lastSaved === null) {
      lastSaved = snapshot
      return
    }
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
    { value: 'zh-CN', label: t('common.languageZh') },
    { value: 'en-US', label: t('common.languageEn') },
  ])

  const copyPath = async (path: string): Promise<void> => {
    try {
      await navigator.clipboard.writeText(path)
      toast.success(t('settings.pathCopied'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const openProjectRepository = async (): Promise<void> => {
    try {
      await openPath(PROJECT_REPOSITORY_URL)
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
      // Reset the intro-seen flag so the first-run intro dialog re-shows on
      // the fresh onboarding entry (now, and after an app restart).
      resetIntroSeen()
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
      <CardTitle>{t('settings.preferences')}</CardTitle>
      <CardDescription>{t('settings.preferencesDesc')}</CardDescription>
    </CardHeader>
    <CardContent class="grid gap-5">
      <div class="grid gap-2">
        <span class="text-sm font-medium text-foreground">{t('settings.appearance')}</span>
        <SegmentedControl
          options={themeOptions}
          value={themeState.theme}
          onSelect={(v) => themeState.setTheme(v as ThemeMode)}
        />
      </div>
      <div class="grid gap-2">
        <span class="text-sm font-medium text-foreground">{t('common.language')}</span>
        <SegmentedControl
          options={languageOptions}
          value={languageState.language}
          onSelect={(v) => void languageState.setLanguage(v as 'zh-CN' | 'en-US')}
        />
      </div>
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
    <GithubVaultCard
      config={config}
      appState={appState.data}
      onReconfigure={() => void goto('/app/onboarding?mode=reconfigure')}
      onDisconnect={openDisconnectDialog}
    />

    <LimitsCard bind:limits={config.limits} limitsInvalid={limitsInvalid} />

    <Card>
      <CardHeader>
        <CardTitle>{t('settings.ignore')}</CardTitle>
        <CardDescription>{t('settings.ignoreDescription')}</CardDescription>
      </CardHeader>
      <CardContent>
        <Textarea bind:value={ignore} class="min-h-[180px]" />
      </CardContent>
    </Card>

    <SkillRootsSection
      isLoading={scan.isLoading}
      roots={scan.data?.roots ?? []}
      onOpenPath={(path) => void openPath(path)}
      onCopyPath={(path) => void copyPath(path)}
    />

    <Card>
      <CardHeader>
        <CardTitle>{t('settings.supportProject')}</CardTitle>
        <CardDescription>{t('settings.supportProjectDescription')}</CardDescription>
      </CardHeader>
      <CardFooter>
        <Button variant="outline" onclick={() => void openProjectRepository()}>
          <Star />
          {t('settings.supportOnGithub')}
        </Button>
      </CardFooter>
    </Card>
  {/if}

  <DisconnectGithubDialog bind:open={disconnectDialogOpen} onConfirm={() => void handleDisconnect()} />
</div>
