<script lang="ts">
  import { createMutation, createQuery, useQueryClient } from '@tanstack/svelte-query'
  import { goto } from '$app/navigation'
  import {
    AlertTriangle,
    ArrowDownToLine,
    ArrowUpFromLine,
    CheckCircle,
    Package,
    RefreshCw,
    Sparkles,
  } from '@lucide/svelte'
  import type { Component } from 'svelte'

  import { cn, errorMessage } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import type { RecoveryInfo } from '@/shared/schemas'
  import {
    Badge,
    Button,
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
    Checkbox,
    EmptyState,
    Input,
    Spinner,
  } from '@/shared/ui'
  import { getAppState } from '@/modules/settings'
  import { scanSkills } from '@/modules/skills'

  import {
    applySyncPlan,
    getSyncPlan,
    resumeSyncRecovery,
  } from '../api/syncApi'
  import ConflictDetailDialog from '../components/ConflictDetailDialog.svelte'
  import SyncSkillCard from '../components/SyncSkillCard.svelte'
  import {
    SYNC_STATUS_FILTERS,
    isDeleteEntry,
    matchesEntry,
    type SyncStatusFilter,
  } from '../lib/syncStatus'
  import type {
    ApplySyncRequest,
    Conflict,
    SyncDecision,
    SyncPlan,
    SyncSkillEntry,
  } from '../schemas/syncPlan'
  import { syncDecisions } from '../state/syncDecisions.svelte'

  const queryClient = useQueryClient()
  const appState = createQuery(() => ({
    queryKey: ['app-state'],
    queryFn: getAppState,
  }))
  const configured = $derived(appState.data?.configured ?? false)
  const pendingRecovery = $derived(appState.data?.pending_recovery ?? null)

  const scan = createQuery(() => ({
    queryKey: ['scan-skills'],
    queryFn: scanSkills,
    enabled: configured && pendingRecovery === null,
  }))
  const plan = createQuery(() => ({
    queryKey: ['sync-plan'],
    queryFn: getSyncPlan,
    enabled: configured && pendingRecovery === null,
  }))

  let search = $state('')
  let statusFilter = $state<SyncStatusFilter>('all')
  let selectedActionIds = $state<string[]>([])
  let deleteGuardAck = $state(false)
  let lastFingerprint = $state<string | null>(null)
  let defaultNextPlan = $state(false)
  let planNotice = $state('')
  let resultMsg = $state('')
  let resultStateMsg = $state('')
  let recoveryOverride = $state<RecoveryInfo | null>(null)
  let selectedConflict = $state<Conflict | null>(null)
  let conflictDialogOpen = $state(false)

  const recovery = $derived(
    recoveryOverride ?? appState.data?.pending_recovery ?? null,
  )
  const planData = $derived(plan.data)
  const skills = $derived(scan.data?.skills ?? [])
  const visibleEntries = $derived(
    planData?.entries.filter((entry) =>
      matchesEntry(entry, search, statusFilter),
    ) ?? [],
  )
  const selectedEntries = $derived(
    planData?.entries.filter((entry) =>
      selectedActionIds.includes(entry.action_id),
    ) ?? [],
  )
  const selectedDelete = $derived(selectedEntries.some(isDeleteEntry))
  const hasDecisions = $derived(
    Object.keys(syncDecisions.decisions).length > 0,
  )
  const hasLocalStateUpdates = $derived(
    (planData?.base_adoptions.length ?? 0) > 0 ||
      (planData?.base_removals.length ?? 0) > 0,
  )
  const canApply = $derived(
    planData !== undefined &&
      !recovery &&
      (selectedActionIds.length > 0 || hasDecisions || hasLocalStateUpdates) &&
      (!selectedDelete || !planData.delete_guard_tripped || deleteGuardAck),
  )
  const recheckLoading = $derived(plan.isFetching || scan.isFetching)

  const statusFilterLabel = (
    filter: SyncStatusFilter,
  ): `sync.filters.${SyncStatusFilter}` => `sync.filters.${filter}`

  const isSelectable = (entry: SyncSkillEntry): boolean =>
    entry.status === 'local_update' ||
    entry.status === 'remote_update' ||
    entry.status === 'local_deleted' ||
    entry.status === 'remote_deleted'

  const defaultSelectedActionIds = (data: SyncPlan): string[] =>
    data.entries
      .filter(
        (entry) =>
          isSelectable(entry) &&
          !isDeleteEntry(entry) &&
          (entry.status === 'local_update' || entry.status === 'remote_update'),
      )
      .map((entry) => entry.action_id)

  $effect(() => {
    const currentPlan = plan.data
    const fingerprint = currentPlan?.plan_fingerprint
    if (!currentPlan || !fingerprint) return
    if (lastFingerprint === null) {
      lastFingerprint = fingerprint
      selectedActionIds = defaultSelectedActionIds(currentPlan)
      return
    }
    if (fingerprint !== lastFingerprint) {
      lastFingerprint = fingerprint
      selectedActionIds = defaultNextPlan
        ? defaultSelectedActionIds(currentPlan)
        : []
      defaultNextPlan = false
      deleteGuardAck = false
      syncDecisions.clear()
      planNotice = t('sync.planChanged')
    }
  })

  const clearInteractionState = (): void => {
    selectedActionIds = []
    deleteGuardAck = false
    syncDecisions.clear()
  }

  const apply = createMutation(() => ({
    mutationFn: (request: ApplySyncRequest) => applySyncPlan(request),
    retry: false,
    onSuccess: (response) => {
      if (response.status === 'applied') {
        resultMsg = t('sync.applied', {
          count: response.result.applied.length,
        })
        resultStateMsg = response.result.state_updated.length
          ? t('sync.localBaseUpdated', {
              count: response.result.state_updated.length,
            })
          : ''
        recoveryOverride = null
        clearInteractionState()
        void queryClient.invalidateQueries({ queryKey: ['sync-plan'] })
        void queryClient.invalidateQueries({ queryKey: ['app-state'] })
        return
      }
      if (response.status === 'plan_changed') {
        defaultNextPlan = false
        queryClient.setQueryData(['sync-plan'], response.latest_plan)
        clearInteractionState()
        planNotice = t('sync.planChanged')
        return
      }
      clearInteractionState()
      recoveryOverride = response.recovery
    },
    onError: (error) => {
      resultMsg = t('sync.applyError', { message: errorMessage(error) })
    },
  }))

  const resume = createMutation(() => ({
    mutationFn: (taskId: string) => resumeSyncRecovery(taskId),
    retry: false,
    onSuccess: (response) => {
      if (response.status === 'recovery_required') {
        recoveryOverride = response.recovery
        return
      }
      recoveryOverride = null
      resultMsg = t('sync.recoveryCompleted')
      void queryClient.invalidateQueries({ queryKey: ['app-state'] })
      void queryClient.invalidateQueries({ queryKey: ['sync-plan'] })
    },
    onError: (error) => {
      resultMsg = t('sync.applyError', { message: errorMessage(error) })
    },
  }))

  const toggleAction = (actionId: string, selected: boolean): void => {
    selectedActionIds = selected
      ? [...selectedActionIds, actionId]
      : selectedActionIds.filter((id) => id !== actionId)
  }

  const openConflict = (entry: SyncSkillEntry): void => {
    selectedConflict =
      planData?.conflicts.find((conflict) => conflict.skill_id === entry.skill_id) ??
      null
    conflictDialogOpen = selectedConflict !== null
  }

  const handleDecision = (choice: SyncDecision): void => {
    if (selectedConflict) {
      syncDecisions.setDecision(selectedConflict.skill_id, choice)
    }
  }

  const handleApply = (): void => {
    if (!planData || !canApply) return
    resultMsg = ''
    resultStateMsg = ''
    apply.mutate({
      expected_remote_commit: planData.expected_remote_commit,
      plan_fingerprint: planData.plan_fingerprint,
      selected_action_ids: [...selectedActionIds],
      decisions: { ...syncDecisions.decisions },
      delete_guard_ack: deleteGuardAck,
    })
  }

  const handleRecheck = (): void => {
    defaultNextPlan = true
    planNotice = ''
    resultMsg = ''
    resultStateMsg = ''
    void plan.refetch()
    void scan.refetch()
  }
</script>

{#snippet metric(
  label: string,
  value: number | string,
  Icon: Component<{ class?: string }>,
  tone: 'neutral' | 'warning' = 'neutral',
)}
  <div class="grid gap-2 border border-border bg-surface p-4">
    <div class="flex items-center justify-between gap-3">
      <div class="text-sm font-medium text-muted-foreground">{label}</div>
      <span class={tone === 'warning' ? 'text-warning' : 'text-muted-foreground'}>
        <Icon class="size-4" />
      </span>
    </div>
    <div class={cn('text-3xl font-bold', tone === 'warning' ? 'text-warning' : 'text-strong-foreground')}>
      {value}
    </div>
  </div>
{/snippet}

<ConflictDetailDialog
  bind:open={conflictDialogOpen}
  conflict={selectedConflict}
  decision={selectedConflict ? syncDecisions.decisions[selectedConflict.skill_id] ?? '' : ''}
  onDecision={handleDecision}
/>

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
  {:else if !configured && !recovery}
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
  {:else if recovery}
    <Card class="border-warning-border bg-warning-muted">
      <CardHeader>
        <CardTitle>{t('sync.recoveryRequired')}</CardTitle>
        <CardDescription>{recovery.message}</CardDescription>
      </CardHeader>
      <CardContent class="grid gap-3 text-sm">
        <div class="grid gap-1 text-muted-foreground sm:grid-cols-3">
          <span>{t('sync.recoveryPhase')}: {recovery.phase}</span>
          <span>{t('sync.recoveryCompletedCount')}: {recovery.completed_action_ids.length}</span>
          <span>{t('sync.recoveryPendingCount')}: {recovery.pending_action_ids.length}</span>
        </div>
        <div class="flex justify-end">
          <Button
            disabled={resume.isPending}
            loading={resume.isPending}
            onclick={() => resume.mutate(recovery.task_id)}
          >
            {t('sync.resumeRecovery')}
          </Button>
        </div>
      </CardContent>
    </Card>
  {:else}
    <div class="grid gap-4">
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h1 class="text-xl font-bold text-strong-foreground">{t('sync.title')}</h1>
          <p class="text-sm text-muted-foreground">{t('sync.description')}</p>
        </div>
        <div class="flex flex-wrap gap-2">
          <Button
            disabled={recovery !== null}
            loading={recheckLoading}
            onclick={handleRecheck}
            variant="outline"
          >
            {#snippet icon()}
              <RefreshCw class="size-4" />
            {/snippet}
            {t('sync.recheck')}
          </Button>
          <Button disabled={!canApply} loading={apply.isPending} onclick={handleApply}>
            {t('common.actions.apply')}
          </Button>
        </div>
      </div>

      <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {@render metric(t('dashboard.metrics.discovered'), skills.length, Package)}
        {@render metric(t('dashboard.metrics.toUpload'), planData?.uploads.length ?? 0, ArrowUpFromLine)}
        {@render metric(t('dashboard.metrics.toDownload'), planData?.downloads.length ?? 0, ArrowDownToLine)}
        {@render metric(t('dashboard.metrics.conflicts'), planData?.conflicts.length ?? 0, AlertTriangle, (planData?.conflicts.length ?? 0) > 0 ? 'warning' : 'neutral')}
      </div>

      {#if planNotice}
        <div class="border border-warning-border bg-warning-muted p-3 text-sm text-warning">
          {planNotice}
        </div>
      {/if}

      {#if planData?.delete_guard_tripped}
        <div class="flex items-start gap-3 border border-warning-border bg-warning-muted p-3 text-sm">
          <AlertTriangle class="mt-0.5 size-4 shrink-0 text-warning" />
          <div class="grid gap-1">
            <strong class="text-warning">{t('sync.deleteGuard.title')}</strong>
            <span class="text-muted-foreground">{t('sync.deleteGuard.description')}</span>
            <label class="mt-2 flex items-center gap-2 text-foreground">
              <Checkbox bind:checked={deleteGuardAck} />
              {t('sync.confirmDelete')}
            </label>
          </div>
        </div>
      {/if}

      <div class="flex flex-col gap-3 border-b border-border pb-4 sm:flex-row">
        <Input bind:value={search} placeholder={t('sync.searchPlaceholder')} />
        <select
          aria-label={t('sync.filterLabel')}
          bind:value={statusFilter}
          class="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40 sm:w-52"
        >
          {#each SYNC_STATUS_FILTERS as filter (filter)}
            <option value={filter}>{statusFilterLabel(filter)}</option>
          {/each}
        </select>
      </div>

      {#if plan.isLoading}
        <div class="flex justify-center py-12">
          <Spinner class="size-6" />
        </div>
      {:else if plan.error}
        <Card class="border-destructive-border bg-destructive-muted">
          <CardContent class="pt-6 text-sm text-destructive">
            {t('sync.loadError', { message: errorMessage(plan.error) })}
          </CardContent>
        </Card>
      {:else if visibleEntries.length === 0}
        <EmptyState title={t('sync.empty')} description={t('sync.emptyDescription')}>
          {#snippet icon()}
            <CheckCircle class="size-10" />
          {/snippet}
        </EmptyState>
      {:else}
        <div class="grid grid-cols-1 gap-3 lg:grid-cols-2">
          {#each visibleEntries as entry (entry.action_id)}
            <SyncSkillCard
              entry={entry}
              onOpenConflict={entry.conflict_reason ? () => openConflict(entry) : undefined}
              onToggle={isSelectable(entry) ? (selected) => toggleAction(entry.action_id, selected) : undefined}
              requiresConfirmation={isDeleteEntry(entry)}
              selected={selectedActionIds.includes(entry.action_id)}
            />
          {/each}
        </div>
      {/if}

      {#if planData}
        <div class="grid gap-2 border-t border-border pt-4 text-sm">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <span class="font-bold text-strong-foreground">{t('sync.commitSummary')}</span>
            <Badge variant="secondary">
              {planData.will_create_commit ? t('sync.commitWillBeCreated') : t('sync.commitNone')}
            </Badge>
          </div>
          <div class="grid gap-1 text-muted-foreground sm:grid-cols-2 lg:grid-cols-5">
            <span>{t('sync.commitSummaryUploads')}: {planData.commit_summary.uploads}</span>
            <span>{t('sync.commitSummaryDownloads')}: {planData.commit_summary.downloads}</span>
            <span>{t('sync.commitSummaryDeleteRemote')}: {planData.commit_summary.delete_remote}</span>
            <span>{t('sync.commitSummaryDeleteLocal')}: {planData.commit_summary.delete_local}</span>
            <span>{t('sync.commitSummaryState')}: {planData.commit_summary.local_state_updates}</span>
          </div>
          {#if hasLocalStateUpdates && !planData.will_create_commit}
            <p class="text-xs text-muted-foreground">{t('sync.localBaseOnly')}</p>
          {/if}
        </div>
      {/if}

      {#if resultMsg}
        <div class="flex items-center gap-2 border border-success-muted bg-success-muted p-3 text-sm text-success">
          <CheckCircle class="size-4 shrink-0" />
          {resultMsg}
        </div>
      {/if}
      {#if resultStateMsg}
        <div class="border border-primary-muted bg-primary-muted p-3 text-sm text-primary-muted-foreground">
          {resultStateMsg}
        </div>
      {/if}
    </div>
  {/if}
</div>
