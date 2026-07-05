<script lang="ts">
  import { createMutation, createQuery, useQueryClient } from '@tanstack/svelte-query'
  import { CheckCircle2, RefreshCw } from '@lucide/svelte'

  import { cn, errorMessage } from '@/shared/lib'
  import { hostLabel, t } from '@/shared/i18n'
  import {
    Badge,
    Button,
    Card,
    CardBody,
    CardHeader,
    EmptyState,
    Spinner,
  } from '@/shared/ui'

  import { applySyncPlan, getSyncPlan } from '../api/syncApi'
  import type { Conflict, SyncAction } from '../schemas/syncPlan'
  import { syncDecisions } from '../state/syncDecisions.svelte'

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
  const plan = createQuery(() => ({
    queryKey: ['sync-plan'],
    queryFn: getSyncPlan,
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

{#snippet actionRow(action: SyncAction)}
  <div class="grid gap-1 rounded-lg border border-border bg-surface p-3 text-sm">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <span class="font-bold text-strong-foreground">{action.name}</span>
      <Badge variant="default">{hostLabel(action.host)}</Badge>
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
        <Badge variant="default">{items.length}</Badge>
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
      <Badge variant="warning">{conflict.reason}</Badge>
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
        <Badge variant="warning">{conflicts.length}</Badge>
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
  <Card>
    <CardHeader description={t('sync.description')} title={t('sync.title')}>
      {#snippet action()}
        <div class="flex gap-2">
          <Button onclick={() => void plan.refetch()} variant="secondary">
            {#snippet icon()}
              <RefreshCw class="size-4" />
            {/snippet}
            {t('sync.recheck')}
          </Button>
          <Button disabled={isEmpty} loading={apply.isPending} onclick={handleApply}>
            {apply.isPending ? t('sync.applying') : t('common.actions.apply')}
          </Button>
        </div>
      {/snippet}
    </CardHeader>
  </Card>

  {#if plan.isLoading}
    <div class="flex justify-center py-12">
      <Spinner class="size-6" />
    </div>
  {/if}

  {#if plan.error}
    <Card class="border-destructive-border bg-destructive-muted">
      <CardBody class="text-sm text-destructive">
        {t('sync.loadError', { message: errorMessage(plan.error) })}
      </CardBody>
    </Card>
  {/if}

  {#if resultMsg}
    <Card class="border-success-muted bg-success-muted">
      <CardBody class="flex items-center gap-2 text-sm text-success">
        <CheckCircle2 class="size-4 shrink-0" />
        {resultMsg}
      </CardBody>
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
</div>
