<script lang="ts">
  import { createMutation, createQuery, useQueryClient } from '@tanstack/svelte-query'
  import { Monitor, Moon, Sun } from '@lucide/svelte'
  import type { Component } from 'svelte'

  import { cn, errorMessage } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import { languageState, themeState, type ThemeMode } from '@/shared/state'
  import {
    Button,
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
    Checkbox,
    Spinner,
    Textarea,
  } from '@/shared/ui'

  import { getAppState, saveConfig } from '../api/configApi'
  import type { AppConfig } from '../schemas/config'

  const toLines = (paths: string[]) => paths.join('\n')
  const fromLines = (text: string) =>
    text
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line.length > 0)

  const queryClient = useQueryClient()
  const appState = createQuery(() => ({
    queryKey: ['app-state'],
    queryFn: getAppState,
  }))
  let config = $state<AppConfig | null>(null)
  let codexPaths = $state('')
  let claudePaths = $state('')
  let ignore = $state('')
  let msg = $state('')
  let prefilled = $state(false)

  // Prefill the form once the loaded app state arrives.
  $effect(() => {
    if (appState.data && !prefilled) {
      prefilled = true
      config = appState.data.config
      codexPaths = toLines(appState.data.config.hosts.codex.paths)
      claudePaths = toLines(appState.data.config.hosts.claude.paths)
      ignore = toLines(appState.data.config.ignore)
    }
  })

  const save = createMutation(() => ({
    mutationFn: (cfg: AppConfig) => saveConfig(cfg),
    onSuccess: () => {
      msg = t('settings.saved')
      void queryClient.invalidateQueries({ queryKey: ['app-state'] })
    },
    onError: (error) => {
      msg = t('settings.saveError', { message: errorMessage(error) })
    },
  }))

  const handleSave = () => {
    if (!config) {
      return
    }
    msg = ''
    save.mutate({
      ...config,
      hosts: {
        codex: { ...config.hosts.codex, paths: fromLines(codexPaths) },
        claude: { ...config.hosts.claude, paths: fromLines(claudePaths) },
      },
      ignore: fromLines(ignore),
    })
  }

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
</script>

<div class="grid gap-4">
  <Card>
    <CardHeader class="flex-row items-center justify-between space-y-0">
      <div class="space-y-1.5">
        <CardTitle>{t('settings.title')}</CardTitle>
        <CardDescription>{t('settings.description')}</CardDescription>
      </div>
      <Button disabled={!config} loading={save.isPending} onclick={handleSave}>
        {t('settings.save')}
      </Button>
    </CardHeader>
  </Card>

  <Card>
    <CardHeader>
      <CardTitle>{t('settings.appearance')}</CardTitle>
      <CardDescription>{t('settings.appearanceDesc')}</CardDescription>
    </CardHeader>
    <CardContent>
      <div class="flex gap-2">
        {#each themeOptions as { mode, icon: Icon, label } (mode)}
          <button
            class={cn(
              'flex h-9 flex-1 items-center justify-center gap-1.5 rounded-lg border text-sm font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/40',
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
      </div>
    </CardContent>
  </Card>

  <Card>
    <CardHeader>
      <CardTitle>{t('settings.language')}</CardTitle>
      <CardDescription>{t('settings.languageDesc')}</CardDescription>
    </CardHeader>
    <CardContent>
      <div class="flex gap-2">
        {#each languageOptions as { code, label } (code)}
          <button
            class={cn(
              'flex h-9 flex-1 items-center justify-center gap-1.5 rounded-lg border text-sm font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/40',
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
      </div>
    </CardContent>
  </Card>

  {#if msg}
    <Card class="border-success-muted bg-success-muted">
      <CardContent class="pt-6 text-sm text-success">{msg}</CardContent>
    </Card>
  {/if}

  {#if appState.error}
    <Card class="border-destructive-border bg-destructive-muted">
      <CardContent class="pt-6 text-sm text-destructive">
        {errorMessage(appState.error)}
      </CardContent>
    </Card>
  {/if}

  {#if !config}
    <div class="flex justify-center py-12">
      <Spinner class="size-6" />
    </div>
  {:else}
    <Card>
      <CardHeader><CardTitle>{t('settings.hosts')}</CardTitle></CardHeader>
      <CardContent class="grid gap-4">
        <label class="flex items-center gap-2 text-sm text-foreground">
          <Checkbox bind:checked={config.hosts.codex.enabled} />
          {t('settings.codexEnabled')}
        </label>
        <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('settings.codexPaths')}
          <Textarea bind:value={codexPaths} class="min-h-[180px]" />
        </label>
        <label class="flex items-center gap-2 text-sm text-foreground">
          <Checkbox bind:checked={config.hosts.claude.enabled} />
          {t('settings.claudeEnabled')}
        </label>
        <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('settings.claudePaths')}
          <Textarea bind:value={claudePaths} class="min-h-[180px]" />
        </label>
      </CardContent>
    </Card>

    <Card>
      <CardContent class="grid gap-4 pt-6">
        <label class="flex items-center gap-2 text-sm text-foreground">
          <Checkbox bind:checked={config.defaults.backup} />
          {t('settings.backup')}
        </label>
        <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('settings.ignore')}
          <Textarea bind:value={ignore} />
        </label>
      </CardContent>
    </Card>
  {/if}
</div>
