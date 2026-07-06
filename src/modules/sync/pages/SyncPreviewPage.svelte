<script lang="ts">
  import { createMutation, createQuery, useQueryClient } from '@tanstack/svelte-query'
  import { goto } from '$app/navigation'
  import {
    AlertTriangle,
    ArrowDownToLine,
    ArrowUpFromLine,
    CheckCircle2,
    FolderOpen,
    Package,
    RefreshCw,
    Sparkles,
  } from '@lucide/svelte'
  import type { Component } from 'svelte'

  import { cn, errorMessage, openPath } from '@/shared/lib'
  import { hostLabel, t } from '@/shared/i18n'
  import {
    Badge,
    Button,
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
    EmptyState,
    Spinner,
    StatusBadge,
  } from '@/shared/ui'

  import { applySyncPlan, getSyncPlan } from '../api/syncApi'
  import type { Conflict, SyncAction } from '../schemas/syncPlan'
  import { syncDecisions } from '../state/syncDecisions.svelte'
  import { scanSkills } from '@/modules/skills'
  import { getAppState } from '@/modules/settings'

  const shortHash = (hash: string) =>
    hash.length > 12 ? hash.slice(0, 12) : hash

  const directionLabel = (direction: string): string =>
    direction === 'upload'
      ? t('sync.direction.upload')
      : direction === 'download'
        ? t('sync.direction.download')
        : direction

  const CHOICES = [
    { key: 'local', labelKey: 'conflicts.keepLocal' },
    { key: 'remote', labelKey: 'conflicts.useRemote' },
    { key: 'skip', labelKey: 'conflicts.skip' },
  ] as const

  const queryClient = useQueryClient()
  const appState = createQuery(() => ({
    queryKey: ['app-state'],
    queryFn: getAppState,
  }))
  const configured = $derived(appState.data?.configured ?? false)

  const scan = createQuery(() => ({
    queryKey: ['scan-skills'],
    queryFn: scanSkills,
    enabled: configured,
  }))
  const skills = $derived(scan.data?.skills ?? [])
  const warnings = $derived(scan.data?.warnings ?? [])

  const plan = createQuery(() => ({
    queryKey: ['sync-plan'],
    queryFn: getSyncPlan,
    enabled: configured,
  }))
  let resultMsg = $state('')

  const apply = createMutation(() => ({
    mutationFn: (vars: Record<string, string>) => applySyncPlan(vars),
    onSuccess: (data) => {
      resultMsg = t('sync.applied', {
        count: data.applied.length,
        backups: data.backups.length,
      })
      syncDecisions.clear()
      void queryClient.invalidateQueries({ queryKey: ['sync-plan'] })
    },
    onError: (error) => {
      resultMsg = t('sync.applyError', { message: errorMessage(error) })
    },
  }))

  const planData = $derived(plan.data)
  const totalActions = $derived(
    planData
      ? planData.uploads.length +
        planData.downloads.length +
        planData.updates.length +
        planData.deletes.length
      : 0,
  )
  const conflictCount = $derived(planData?.conflicts.length ?? 0)
  const isEmpty = $derived(totalActions === 0 && conflictCount === 0)

  const handleApply = () => {
    resultMsg = ''
    apply.mutate(syncDecisions.decisions)
  }
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

{#snippet actionRow(action: SyncAction)}
  <div class="grid gap-1 rounded-lg border border-border bg-surface p-3 text-sm">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <span class="font-bold text-strong-foreground">{action.name}</span>
      <Badge variant="secondary">{hostLabel(action.host)}</Badge>
    </div>
    <div class="truncate text-xs text-muted-foreground">
      {directionLabel(action.direction)} · {action.repo_path}
    </div>
  </div>
{/snippet}

{#snippet groupSection(title: string, items: SyncAction[])}
  {#if items.length > 0}
    <div class="grid gap-2">
      <h3 class="flex items-center gap-2 text-sm font-bold text-strong-foreground">
        {title}
        <Badge variant="secondary">{items.length}</Badge>
      </h3>
      <div class="grid grid-cols-1 gap-2 lg:grid-cols-2">
        {#each items as action (action.skill_id)}
          {@render actionRow(action)}
        {/each}
      </div>
    </div>
  {/if}
{/snippet}

{#snippet conflictCard(conflict: Conflict)}
  {@const decision = syncDecisions.decisions[conflict.skill_id] ?? ''}
  <div class="grid gap-2 rounded-lg border border-warning-border bg-warning-muted p-3 text-sm">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <span class="font-bold text-strong-foreground">{conflict.name}</span>
      <StatusBadge tone="warning">{conflict.reason}</StatusBadge>
    </div>
    <div class="grid grid-cols-1 gap-1 text-xs text-muted-foreground sm:grid-cols-2">
      <div class="truncate">
        {t('conflicts.localHash')}: {shortHash(conflict.local_hash)}
      </div>
      <div class="truncate">
        {t('conflicts.remoteHash')}: {shortHash(conflict.remote_hash)}
      </div>
    </div>
    <div class="flex flex-wrap gap-2">
      {#each CHOICES as choice (choice.key)}
        <button
          class={cn(
            'h-8 rounded-lg border px-2.5 text-xs font-medium transition-colors',
            decision === choice.key
              ? 'border-primary bg-primary-muted text-primary-muted-foreground'
              : 'border-border bg-surface text-foreground hover:bg-surface-hover',
          )}
          onclick={() => syncDecisions.setDecision(conflict.skill_id, choice.key)}
          type="button"
        >
          {t(choice.labelKey)}
        </button>
      {/each}
    </div>
  </div>
{/snippet}

{#snippet conflictList(conflicts: Conflict[])}
  {#if conflicts.length > 0}
    <div class="grid gap-2">
      <h3 class="flex items-center gap-2 text-sm font-bold text-strong-foreground">
        {t('sync.groups.conflicts')}
        <StatusBadge tone="warning">{conflicts.length}</StatusBadge>
      </h3>
      <div class="grid grid-cols-1 gap-2 lg:grid-cols-2">
        {#each conflicts as conflict (conflict.skill_id)}
          {@render conflictCard(conflict)}
        {/each}
      </div>
    </div>
  {/if}
{/snippet}

<div class="grid gap-4">
  {#if appState.isLoading}
    <div class="flex justify-center py-12">
      <Spinner class="size-6" />
    </div>
  {:else if appState.error}
    <Card>
      <CardContent class="pt-6">
        <p class="text-sm text-destructive">{errorMessage(appState.error)}</p>
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
      <CardHeader class="flex-row items-center justify-between space-y-0">
        <div class="space-y-1.5">
          <CardTitle>{t('sync.title')}</CardTitle>
          <CardDescription>{t('sync.description')}</CardDescription>
        </div>
        <div class="flex gap-2">
          <Button onclick={() => void plan.refetch()} variant="outline">
            {#snippet icon()}
              <RefreshCw class="size-4" />
            {/snippet}
            {t('sync.recheck')}
          </Button>
          <Button disabled={isEmpty} loading={apply.isPending} onclick={handleApply}>
            {apply.isPending ? t('sync.applying') : t('common.actions.apply')}
          </Button>
        </div>
      </CardHeader>
    </Card>

    <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
      {@render metric(
        t('dashboard.metrics.discovered'),
        skills.length,
        Package,
      )}
      {@render metric(
        t('dashboard.metrics.toUpload'),
        planData?.uploads.length ?? 0,
        ArrowUpFromLine,
      )}
      {@render metric(
        t('dashboard.metrics.toDownload'),
        planData?.downloads.length ?? 0,
        ArrowDownToLine,
      )}
      {@render metric(
        t('dashboard.metrics.conflicts'),
        conflictCount,
        AlertTriangle,
        conflictCount > 0 ? 'warning' : 'neutral',
      )}
    </div>

    <Card>
      <CardHeader class="flex-row items-center justify-between space-y-0">
        <div class="space-y-1.5">
          <CardTitle>{t('skills.title')}</CardTitle>
          <CardDescription>{t('skills.description')}</CardDescription>
        </div>
        <Button
          loading={scan.isFetching}
          onclick={() => void scan.refetch()}
          variant="outline"
        >
          {#snippet icon()}
            <RefreshCw class="size-4" />
          {/snippet}
          {t('skills.rescan')}
        </Button>
      </CardHeader>
    </Card>

    {#if scan.isLoading}
      <div class="flex justify-center py-12">
        <Spinner class="size-6" />
      </div>
    {/if}

    {#if scan.error}
      <Card>
        <CardContent class="pt-6 text-sm text-destructive">
          {errorMessage(scan.error)}
        </CardContent>
      </Card>
    {/if}

    {#if warnings.length > 0}
      <Card class="border-warning-border bg-warning-muted">
        <CardContent class="pt-6 text-sm text-warning">
          <div class="font-bold">{t('skills.warnings')}</div>
          <ul class="mt-1 grid gap-1">
            {#each warnings as warning, index (index)}
              <li>{warning}</li>
            {/each}
          </ul>
        </CardContent>
      </Card>
    {/if}

    {#if skills.length === 0 && !scan.isLoading}
      <Card>
        <EmptyState title={t('skills.empty')}>
          {#snippet icon()}
            <Package class="size-10" />
          {/snippet}
        </EmptyState>
      </Card>
    {/if}

    <div class="grid grid-cols-1 gap-3 lg:grid-cols-2">
      {#each skills as skill (skill.id)}
        <Card class="p-4">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <div class="grid gap-1">
              <div class="text-base font-bold text-strong-foreground">
                {skill.name}
              </div>
              <Badge variant="secondary">{hostLabel(skill.host)}</Badge>
            </div>
            <StatusBadge tone="success">{t('skills.enabled')}</StatusBadge>
          </div>
          <p class="mt-2 text-sm text-muted-foreground">{skill.description}</p>
          <div
            class="mt-3 grid grid-cols-1 gap-1 text-xs text-muted-foreground sm:grid-cols-2"
          >
            <div class="truncate">
              <span class="text-foreground">{t('skills.columns.path')}:</span>
              {skill.source_path}
            </div>
            <div class="truncate">
              <span class="text-foreground">{t('skills.columns.modified')}:</span>
              {skill.modified_at || '—'}
            </div>
            <div class="truncate sm:col-span-2">
              <span class="text-foreground">{t('skills.columns.hash')}:</span>
              {shortHash(skill.hash)}
            </div>
          </div>
          <div class="mt-3 flex justify-end">
            <Button
              onclick={() => void openPath(skill.source_path)}
              size="sm"
              variant="outline"
            >
              {#snippet icon()}
                <FolderOpen class="size-3.5" />
              {/snippet}
              {t('skills.openFolder')}
            </Button>
          </div>
        </Card>
      {/each}
    </div>

    {#if plan.isLoading}
      <div class="flex justify-center py-12">
        <Spinner class="size-6" />
      </div>
    {/if}

    {#if plan.error}
      <Card class="border-destructive-border bg-destructive-muted">
        <CardContent class="pt-6 text-sm text-destructive">
          {t('sync.loadError', { message: errorMessage(plan.error) })}
        </CardContent>
      </Card>
    {/if}

    {#if resultMsg}
      <Card class="border-success-muted bg-success-muted">
        <CardContent class="flex items-center gap-2 pt-6 text-sm text-success">
          <CheckCircle2 class="size-4 shrink-0" />
          {resultMsg}
        </CardContent>
      </Card>
    {/if}

    {#if isEmpty && !plan.isLoading && !plan.error}
      <Card>
        <EmptyState title={t('sync.empty')}>
          {#snippet icon()}
            <CheckCircle2 class="size-10" />
          {/snippet}
        </EmptyState>
      </Card>
    {/if}

    {#if planData}
      {@render groupSection(t('sync.groups.uploads'), planData.uploads)}
      {@render groupSection(t('sync.groups.downloads'), planData.downloads)}
      {@render groupSection(t('sync.groups.updates'), planData.updates)}
      {@render groupSection(t('sync.groups.deletes'), planData.deletes)}
      {@render conflictList(planData.conflicts)}
    {/if}
  {/if}
</div>
