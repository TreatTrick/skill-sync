<script lang="ts">
  import { createQuery, useQueryClient } from '@tanstack/svelte-query'
  import { goto } from '$app/navigation'
  import { Check, LoaderCircle, Monitor, Moon, Star, Sun } from '@lucide/svelte'
  import type { Component } from 'svelte'
  import { version } from '../../../../package.json'

import { errorMessage, getAppState, openPath, resetIntroSeen, scanSkills } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import { languageState, themeState, type ThemeMode } from '@/shared/state'
  import {
    Button,
    Spinner,
    SegmentedControl,
    Textarea,
    toast,
  } from '@/shared/ui'

import { clearSkillPackCache, disconnectGithub, getCacheStats, saveConfig } from '../api/configApi'
  import DisconnectGithubDialog from '../components/DisconnectGithubDialog.svelte'
  import GithubVaultCard from '../components/GithubVaultCard.svelte'
  import LimitsCard from '../components/LimitsCard.svelte'
import SkillRootsSection from '../components/SkillRootsSection.svelte'
import SkillPackCacheCard from '../components/SkillPackCacheCard.svelte'
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
  const cache = createQuery(() => ({
    queryKey: ['skill-pack-cache'],
    queryFn: getCacheStats,
  }))
  let config = $state<AppConfig | null>(null)
  let ignore = $state('')
  let prefilled = $state(false)
  let disconnectDialogOpen = $state(false)
  let clearCachePending = $state(false)
  let saving = $state(false)
  let saveFailed = $state(false)
  let activeSection = $state('preferences')
  let lastSaved: string | null = null
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  const sections = [
    { id: 'preferences', label: 'settings.preferences' },
    { id: 'vault', label: 'settings.githubVault' },
    { id: 'ignore', label: 'settings.ignoreTitle' },
    { id: 'limits', label: 'settings.limits' },
    { id: 'cache', label: 'settings.cache' },
    { id: 'roots', label: 'settings.skillRoots' },
    { id: 'about', label: 'settings.about' },
  ] as const

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
    clearTimeout(saveTimer)
    if (!current || limitsInvalid) {
      saving = false
      return
    }
    const snapshot = JSON.stringify(current)
    // Cancel any pending save before deciding, so a "change then revert" within
    // the debounce window never persists the intermediate value.
    if (snapshot === lastSaved) {
      saving = false
      return
    }
    if (lastSaved === null) {
      lastSaved = snapshot
      return
    }
    saving = true
    saveFailed = false
    saveTimer = setTimeout(() => {
      void saveConfig(current)
        .then(() => {
          saving = false
          lastSaved = snapshot
          void queryClient.invalidateQueries({ queryKey: ['app-state'] })
        })
        .catch((error) => {
          saving = false
          saveFailed = true
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

  const handleClearCache = async (): Promise<void> => {
    if (clearCachePending) return
    clearCachePending = true
    try {
      await clearSkillPackCache()
      toast.success(t('settings.cacheCleared'))
      await cache.refetch()
    } catch (error) {
      toast.error(t('settings.cacheClearError', { message: errorMessage(error) }))
    } finally {
      clearCachePending = false
    }
  }
  const updateActiveSection = (event: Event): void => {
    const view = event.currentTarget as HTMLDivElement
    const top = view.getBoundingClientRect().top
    let current: string = sections[0].id
    for (const section of sections) {
      const element = view.querySelector('#settings-' + section.id)
      if (element && element.getBoundingClientRect().top - top < 80) current = section.id
    }
    if (view.scrollTop + view.clientHeight >= view.scrollHeight - 4) current = 'about'
    activeSection = current
  }
</script>
<div class="view" onscroll={updateActiveSection}>
  <div class="page">
    <div class="settings">
      <nav aria-label={t('settings.sectionNav')} class="snav">
        {#each sections as section (section.id)}
          <button class:on={activeSection === section.id} onclick={() => document.getElementById('settings-' + section.id)?.scrollIntoView({ behavior: 'smooth', block: 'start' })} type="button">{t(section.label)}</button>
        {/each}
        <div class="saved" aria-live="polite">
          {#if saving}<LoaderCircle class="ic sm animate-spin" />{t('settings.saving')}
          {:else if saveFailed}<span class="text-destructive">{t('settings.saveFailed')}</span>
          {:else}<Check class="ic sm" />{t('settings.autoSave')}{/if}
        </div>
      </nav>
      <div class="secs">
        <section id="settings-preferences" class="sec">
          <h2>{t('settings.preferences')}</h2>
          <div class="srows">
            <div class="srow"><div class="lab"><b>{t('settings.appearance')}</b></div><div class="ctl">
              <SegmentedControl class="seg lg" ariaLabel={t('settings.appearance')} options={themeOptions} value={themeState.theme} onSelect={(value) => themeState.setTheme(value as ThemeMode)} />
            </div></div>
            <div class="srow"><div class="lab"><b>{t('common.language')}</b></div><div class="ctl">
              <SegmentedControl class="seg lg" ariaLabel={t('common.language')} options={languageOptions} value={languageState.language} onSelect={(value) => void languageState.setLanguage(value as 'zh-CN' | 'en-US')} />
            </div></div>
          </div>
        </section>
        {#if appState.error}<div class="callout danger" role="alert">{errorMessage(appState.error)}</div>{/if}
        {#if !config}<div class="flex justify-center py-12"><Spinner class="size-6" /></div>
        {:else}
          <section id="settings-vault" class="sec">
            <h2>{t('settings.githubVault')}</h2><p>{t('settings.githubVaultReadOnly')}</p>
            <GithubVaultCard config={config} appState={appState.data} onReconfigure={() => void goto('/app/onboarding?mode=reconfigure')} />
            <div class="danger-zone"><div class="srow"><div class="lab"><b>{t('settings.disconnectGithub')}</b><small>{t('settings.dangerDescription')}</small></div><div class="ctl">
              <Button class="btn danger" disabled={appState.data?.repository_id === null || appState.data?.repository_id === undefined} onclick={openDisconnectDialog} variant="outline">{t('settings.disconnectGithub')}</Button>
            </div></div></div>
          </section>
          <section id="settings-ignore" class="sec">
            <h2>{t('settings.ignoreTitle')}</h2><p>{t('settings.ignoreDescription')}</p>
            <div class="mt-2.5">
              <Textarea aria-label={t('settings.ignoreTitle')} bind:value={ignore} class="textarea" spellcheck="false"></Textarea>
              <small class="muted num">{t('settings.ignoreCount', { count: fromLines(ignore).length })} · <span class="mono">*</span> {t('settings.ignoreGlobHint')}</small>
            </div>
          </section>
          <section id="settings-limits" class="sec">
            <h2>{t('settings.limits')}</h2><p>{t('settings.limitsDescription')}</p>
            <LimitsCard bind:limits={config.limits} {limitsInvalid} />
          </section>
          <section id="settings-cache" class="sec">
            <h2>{t('settings.cache')}</h2><p>{t('settings.cacheDescription')}</p>
            <SkillPackCacheCard clearPending={clearCachePending} disabled={false} loading={cache.isLoading} onClear={() => void handleClearCache()} stats={cache.data} />
          </section>
          <section id="settings-roots" class="sec">
            <h2>{t('settings.skillRoots')}</h2><p>{t('settings.skillRootsReadOnly')}</p>
            <SkillRootsSection isLoading={scan.isLoading} roots={scan.data?.roots ?? []} onOpenPath={(path) => void openPath(path)} onCopyPath={(path) => void copyPath(path)} />
          </section>
          <section id="settings-about" class="sec">
            <h2>{t('settings.about')}</h2>
            <div class="srows"><div class="srow"><div class="lab"><b>{t('settings.appVersion', { version })}</b><small>{t('settings.supportProjectDescription')}</small></div><div class="ctl">
              <Button class="btn" variant="outline" onclick={() => void openProjectRepository()}><Star class="size-4" />{t('settings.supportOnGithub')}</Button>
            </div></div></div>
          </section>
        {/if}
      </div>
    </div>
  </div>
</div>
<DisconnectGithubDialog bind:open={disconnectDialogOpen} onConfirm={() => void handleDisconnect()} />
