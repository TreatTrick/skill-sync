<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query'
  import { goto } from '$app/navigation'
  import {
    AlertTriangle,
    ArrowDownToLine,
    ArrowRight,
    ArrowUpFromLine,
    Package,
    Sparkles,
  } from '@lucide/svelte'
  import type { Component } from 'svelte'

  import { errorMessage } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import { getSyncPlan } from '@/modules/sync'
  import { scanSkills } from '@/modules/skills'
  import { getAppState } from '@/modules/settings'
  import { Button, Card, CardContent, EmptyState, Spinner } from '@/shared/ui'

  const state = createQuery(() => ({
    queryKey: ['app-state'],
    queryFn: getAppState,
  }))
  const configured = $derived(state.data?.configured ?? false)
  const scan = createQuery(() => ({
    queryKey: ['scan-skills'],
    queryFn: scanSkills,
    enabled: configured,
  }))
  const plan = createQuery(() => ({
    queryKey: ['sync-plan'],
    queryFn: getSyncPlan,
    enabled: configured,
  }))
  const conflictCount = $derived(plan.data?.conflicts.length ?? 0)
</script>

{#snippet metric(
  label: string,
  value: number | string,
  Icon: Component<{ class?: string }>,
  tone: 'neutral' | 'warning' = 'neutral',
)}
  <Card class="p-4">
    <div class="flex items-center justify-between gap-3">
      <div class="text-sm font-medium text-muted-foreground">{label}</div>
      <span class={tone === 'warning' ? 'text-warning' : 'text-muted-foreground'}>
        <Icon class="size-4" />
      </span>
    </div>
    <div
      class="mt-3 text-3xl font-bold {tone === 'warning'
        ? 'text-warning'
        : 'text-strong-foreground'}"
    >
      {value}
    </div>
  </Card>
{/snippet}

<div class="grid gap-4">
  {#if state.isLoading}
    <div class="flex justify-center py-12">
      <Spinner class="size-6" />
    </div>
  {:else if state.error}
    <Card>
      <CardContent>
        <p class="text-sm text-destructive">{errorMessage(state.error)}</p>
      </CardContent>
    </Card>
  {:else if !configured}
    <Card>
      <EmptyState title={t('dashboard.notConfigured')}>
        {#snippet icon()}
          <Sparkles class="size-10" />
        {/snippet}
        {#snippet action()}
          <Button onclick={() => void goto('/app/onboarding')}>
            {#snippet icon()}
              <Sparkles class="size-4" />
            {/snippet}
            {t('dashboard.goToOnboarding')}
          </Button>
        {/snippet}
      </EmptyState>
    </Card>
  {:else}
    <Card>
      <CardContent
        class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
      >
        <div>
          <h2 class="text-lg font-bold text-strong-foreground">
            {t('dashboard.title')}
          </h2>
          <p class="mt-1 text-sm text-muted-foreground">
            {t('dashboard.description')}
          </p>
        </div>
        <Button onclick={() => void goto('/app/sync')}>
          {#snippet icon()}
            <ArrowRight class="size-4" />
          {/snippet}
          {t('dashboard.preview')}
        </Button>
      </CardContent>
    </Card>

    {#if state.data && !state.data.git_available}
      <Card>
        <CardContent class="flex items-center gap-2 text-sm text-warning">
          <AlertTriangle class="size-4 shrink-0" />
          {t('dashboard.gitUnavailable')}
        </CardContent>
      </Card>
    {/if}

    <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
      {@render metric(
        t('dashboard.metrics.discovered'),
        scan.data?.skills.length ?? 0,
        Package,
      )}
      {@render metric(
        t('dashboard.metrics.toUpload'),
        plan.data?.uploads.length ?? 0,
        ArrowUpFromLine,
      )}
      {@render metric(
        t('dashboard.metrics.toDownload'),
        plan.data?.downloads.length ?? 0,
        ArrowDownToLine,
      )}
      {@render metric(
        t('dashboard.metrics.conflicts'),
        conflictCount,
        AlertTriangle,
        conflictCount > 0 ? 'warning' : 'neutral',
      )}
    </div>

    {#if plan.error}
      <Card>
        <CardContent class="text-sm text-warning">
          {t('sync.loadError', { message: errorMessage(plan.error) })}
        </CardContent>
      </Card>
    {/if}
  {/if}
</div>
