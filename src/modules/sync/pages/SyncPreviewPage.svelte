<script lang="ts">
  import { createMutation, createQuery, useQueryClient } from '@tanstack/svelte-query'
  import { goto } from '$app/navigation'
  import { ArrowDownToLine, ArrowUpFromLine, CheckCircle, Package, RefreshCw, Sparkles, Trash2, TriangleAlert } from '@lucide/svelte'
  import { fade, fly } from 'svelte/transition'
  import { flip } from 'svelte/animate'

  import { errorMessage, getAppState, openPath, scanSkills } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import type { RecoveryInfo } from '@/shared/schemas'
  import {
    Button,
    Callout,
    Card,
    CardContent,
    Checkbox,
    EmptyState,
    Skeleton,
    Spinner,
    toast,
  } from '@/shared/ui'

  import {
    applySyncPlan,
    getSyncPlan,
    resumeSyncRecovery,
  } from '../api/syncApi'
  import ConflictDetailDialog from '../components/ConflictDetailDialog.svelte'
  import RecoveryCard from '../components/RecoveryCard.svelte'
  import SyncApplyBar from '../components/SyncApplyBar.svelte'
  import SyncBulkActionBar from '../components/SyncBulkActionBar.svelte'
  import SyncCommitSummary from '../components/SyncCommitSummary.svelte'
  import SyncFilterBar from '../components/SyncFilterBar.svelte'
  import SyncMetric from '../components/SyncMetric.svelte'
  import SyncSkillCard from '../components/SyncSkillCard.svelte'
  import {
    bulkConflictDecision,
    deleteDecisionOptions,
    isDeleteEntry,
    matchesEntry,
    summarizeSyncSelection,
    type ConflictBias,
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
  const selectionSummary = $derived(
    summarizeSyncSelection(
      selectedEntries,
      Object.values(syncDecisions.decisions),
    ),
  )
  const syncedCount = $derived(
    (planData?.entries ?? []).filter((entry) => entry.status === 'synced').length,
  )
  const hasLocalStateUpdates = $derived(
    (planData?.base_adoptions.length ?? 0) > 0 ||
      (planData?.base_removals.length ?? 0) > 0,
  )
  const canApply = $derived(
    planData !== undefined &&
      !recovery &&
      (selectionSummary.selected > 0 || hasLocalStateUpdates) &&
      (!selectionSummary.hasDelete ||
        !planData.delete_guard_tripped ||
        deleteGuardAck),
  )
  const recheckLoading = $derived(plan.isFetching || scan.isFetching)

  const isSelectable = (entry: SyncSkillEntry): boolean =>
    entry.status === 'local_update' ||
    entry.status === 'remote_update' ||
    entry.status === 'local_deleted' ||
    entry.status === 'remote_deleted'

  const isRecoveryDecision = (decision?: SyncDecision): boolean =>
    decision === 'restore_remote' || decision === 'keep_local'

  const defaultSelectedActionIds = (data: SyncPlan): string[] =>
    data.entries
      .filter(
        (entry) =>
          isSelectable(entry) &&
          !isDeleteEntry(entry) &&
          (entry.status === 'local_update' || entry.status === 'remote_update'),
      )
      .map((entry) => entry.action_id)

  const visibleSelectableActionIds = $derived(
    visibleEntries
      .filter(
        (entry) =>
          isSelectable(entry) &&
          !isRecoveryDecision(syncDecisions.decisions[entry.skill_id]),
      )
      .map((entry) => entry.action_id),
  )
  const canSelectAll = $derived(
    visibleSelectableActionIds.some((id) => !selectedActionIds.includes(id)),
  )
  const canSelectNone = $derived(
    visibleSelectableActionIds.some((id) => selectedActionIds.includes(id)),
  )

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
    } else {
      // Recheck returned an unchanged fingerprint: discard the default-apply
      // flag so a later, non-user-initiated plan change never auto-applies
      // defaults over the user's manual selection.
      defaultNextPlan = false
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
        toast.success(
          t('sync.applied', { count: response.result.applied.length }),
        )
        if (response.result.state_updated.length) {
          toast.info(
            t('sync.localBaseUpdated', {
              count: response.result.state_updated.length,
            }),
          )
        }
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
      toast.error(t('sync.applyError', { message: errorMessage(error) }))
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
      toast.success(t('sync.recoveryCompleted'))
      void queryClient.invalidateQueries({ queryKey: ['app-state'] })
      void queryClient.invalidateQueries({ queryKey: ['sync-plan'] })
    },
    onError: (error) => {
      toast.error(t('sync.applyError', { message: errorMessage(error) }))
    },
  }))

  const toggleAction = (actionId: string, selected: boolean): void => {
    selectedActionIds = selected
      ? [...selectedActionIds, actionId]
      : selectedActionIds.filter((id) => id !== actionId)
  }

  const selectAllVisible = (): void => {
    const next = [...selectedActionIds]
    for (const id of visibleSelectableActionIds) {
      if (!next.includes(id)) next.push(id)
    }
    selectedActionIds = next
  }

  const selectNoneVisible = (): void => {
    selectedActionIds = selectedActionIds.filter(
      (id) => !visibleSelectableActionIds.includes(id),
    )
  }

  const openConflict = (entry: SyncSkillEntry): void => {
    selectedConflict =
      planData?.conflicts.find((conflict) => conflict.skill_id === entry.skill_id) ??
      null
    conflictDialogOpen = selectedConflict !== null
  }

  const openSkillFolder = (entry: SyncSkillEntry): void => {
    if (entry.local_path) void openPath(entry.local_path)
  }

  const handleDecision = (choice: SyncDecision): void => {
    if (selectedConflict) {
      syncDecisions.setDecision(selectedConflict.skill_id, choice)
    }
  }

  const handleDeleteDecision = (
    entry: SyncSkillEntry,
    choice: SyncDecision,
  ): void => {
    if (!deleteDecisionOptions(entry).includes(choice)) return
    if (syncDecisions.decisions[entry.skill_id] === choice) {
      syncDecisions.removeDecision(entry.skill_id)
      return
    }
    if (
      isRecoveryDecision(choice) &&
      selectedActionIds.includes(entry.action_id)
    ) {
      toggleAction(entry.action_id, false)
    }
    syncDecisions.setDecision(entry.skill_id, choice)
  }

  // Bulk-resolve every visible conflict by adopting one side's state. The
  // mapping in bulkConflictDecision covers all conflict reasons, so no conflict
  // in the current view is left for manual handling.
  const applyBulkConflictDecision = (bias: ConflictBias): void => {
    for (const entry of visibleEntries) {
      if (entry.status !== 'conflict' || !entry.conflict_reason) continue
      syncDecisions.setDecision(
        entry.skill_id,
        bulkConflictDecision(entry.conflict_reason, bias),
      )
    }
  }

  // Bulk-handle every visible delete entry on the active delete filter. "delete"
  // clears recovery decisions and selects every action_id so apply runs the
  // deletions; "recover" sets the side's recovery decision and deselects.
  const applyBulkDeleteDecision = (action: 'delete' | 'recover'): void => {
    const entries = visibleEntries
    const ids = new Set(entries.map((entry) => entry.action_id))
    if (action === 'delete') {
      for (const entry of entries) syncDecisions.removeDecision(entry.skill_id)
      selectedActionIds = [
        ...new Set([...selectedActionIds, ...entries.map((entry) => entry.action_id)]),
      ]
      return
    }
    const decision: SyncDecision =
      statusFilter === 'delete_remote' ? 'restore_remote' : 'keep_local'
    for (const entry of entries) syncDecisions.setDecision(entry.skill_id, decision)
    selectedActionIds = selectedActionIds.filter((id) => !ids.has(id))
  }

  const handleApply = (): void => {
    if (!planData || !canApply) return
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
    void plan.refetch()
    void scan.refetch()
  }
</script>

<ConflictDetailDialog
  bind:open={conflictDialogOpen}
  conflict={selectedConflict}
  decision={selectedConflict ? syncDecisions.decisions[selectedConflict.skill_id] ?? '' : ''}
  onDecision={handleDecision}
/>

<div class="flex flex-1 flex-col gap-4">
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
      <EmptyState title={t('sync.notConfigured')}>
        {#snippet icon()}
          <Sparkles class="size-10" />
        {/snippet}
        {#snippet action()}
          <Button onclick={() => void goto('/app/onboarding')}>
            {#snippet icon()}
              <Sparkles class="size-4" />
            {/snippet}
            {t('sync.goToOnboarding')}
          </Button>
        {/snippet}
      </EmptyState>
    </Card>
  {:else if recovery}
    <RecoveryCard
      recovery={recovery}
      resumePending={resume.isPending}
      onResume={() => resume.mutate(recovery.task_id)}
    />
  {:else}
    <div class="flex flex-1 flex-col gap-4">
      <div class="flex flex-wrap items-start justify-between gap-3">
        <p class="text-sm text-muted-foreground">{t('sync.description')}</p>
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
      </div>

      {#if scan.error}
        <Callout tone="warning">
          {#snippet icon()}
            <TriangleAlert class="size-4" />
          {/snippet}
          {t('sync.scanError', { message: errorMessage(scan.error) })}
        </Callout>
      {/if}

      <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
        <SyncMetric
          label={t('sync.metrics.discovered')}
          value={skills.length}
          icon={Package}
          filter="all"
          activeFilter={statusFilter}
          onFilter={(f) => { statusFilter = f }}
        />
        <SyncMetric
          label={t('sync.metrics.synced')}
          value={syncedCount}
          icon={CheckCircle}
          tone="primary"
          filter="synced"
          activeFilter={statusFilter}
          onFilter={(f) => { statusFilter = f }}
        />
        <SyncMetric
          label={t('sync.metrics.toUpload')}
          value={planData?.uploads.length ?? 0}
          icon={ArrowUpFromLine}
          tone="info"
          filter="local_update"
          activeFilter={statusFilter}
          onFilter={(f) => { statusFilter = f }}
        />
        <SyncMetric
          label={t('sync.metrics.toDownload')}
          value={planData?.downloads.length ?? 0}
          icon={ArrowDownToLine}
          tone="success"
          filter="remote_update"
          activeFilter={statusFilter}
          onFilter={(f) => { statusFilter = f }}
        />
        <SyncMetric
          label={t('sync.metrics.deleteRemote')}
          value={planData?.delete_remote.length ?? 0}
          icon={Trash2}
          tone="destructive"
          filter="delete_remote"
          activeFilter={statusFilter}
          onFilter={(f) => { statusFilter = f }}
        />
        <SyncMetric
          label={t('sync.metrics.deleteLocal')}
          value={planData?.delete_local.length ?? 0}
          icon={Trash2}
          tone="destructiveSoft"
          filter="delete_local"
          activeFilter={statusFilter}
          onFilter={(f) => { statusFilter = f }}
        />
        <SyncMetric
          label={t('sync.metrics.conflicts')}
          value={planData?.conflicts.length ?? 0}
          icon={TriangleAlert}
          tone="warning"
          filter="conflict"
          activeFilter={statusFilter}
          onFilter={(f) => { statusFilter = f }}
        />
      </div>

      {#if planNotice}
        <div transition:fly={{ y: -6, duration: 150 }}>
          <Callout tone="warning">{planNotice}</Callout>
        </div>
      {/if}

      {#if planData?.delete_guard_tripped}
        <Callout tone="warning">
          {#snippet icon()}
            <TriangleAlert class="size-4" />
          {/snippet}
          <div class="grid gap-1">
            <strong class="font-semibold">{t('sync.deleteGuard.title')}</strong>
            <span class="text-muted-foreground">{t('sync.deleteGuard.description')}</span>
            <label class="mt-2 flex items-center gap-2 text-foreground">
              <Checkbox bind:checked={deleteGuardAck} />
              {t('sync.confirmDelete')}
            </label>
          </div>
        </Callout>
      {/if}

      <SyncFilterBar
        bind:search
        canSelectAll={canSelectAll}
        canSelectNone={canSelectNone}
        onSelectAll={selectAllVisible}
        onSelectNone={selectNoneVisible}
      />

      {#if statusFilter === 'conflict' || statusFilter === 'delete_remote' || statusFilter === 'delete_local'}
        <SyncBulkActionBar
          statusFilter={statusFilter}
          visibleCount={visibleEntries.length}
          onBulkConflict={applyBulkConflictDecision}
          onBulkDelete={applyBulkDeleteDecision}
        />
      {/if}

      {#if plan.isLoading}
        <div class="grid grid-cols-1 gap-3 lg:grid-cols-2">
          {#each Array(4) as _, i (i)}
            <div class="rounded-xl border border-border bg-card p-4">
              <Skeleton class="h-5 w-40" />
              <Skeleton class="mt-3 h-3 w-full" />
              <Skeleton class="mt-2 h-3 w-2/3" />
            </div>
          {/each}
        </div>
      {:else if plan.error}
        <Card class="border-destructive-border bg-destructive-muted">
          <CardContent class="pt-6 text-sm text-destructive">
            {t('sync.loadError', { message: errorMessage(plan.error) })}
          </CardContent>
        </Card>
      {:else if visibleEntries.length === 0}
        {#if (planData?.entries.length ?? 0) === 0}
          <EmptyState title={t('sync.emptyAllSynced')} description={t('sync.emptyAllSyncedDescription')}>
            {#snippet icon()}
              <CheckCircle class="size-10" />
            {/snippet}
          </EmptyState>
        {:else}
          <EmptyState title={t('sync.empty')} description={t('sync.emptyDescription')}>
            {#snippet icon()}
              <CheckCircle class="size-10" />
            {/snippet}
            {#snippet action()}
              <Button onclick={() => { search = ''; statusFilter = 'all' }} variant="outline">
                {t('sync.clearFilters')}
              </Button>
            {/snippet}
          </EmptyState>
        {/if}
      {:else}
        <div class="grid grid-cols-1 gap-3 lg:grid-cols-2">
          {#each visibleEntries as entry (entry.action_id)}
            <div in:fade={{ duration: 100 }} animate:flip={{ duration: 150 }}>
              <SyncSkillCard
                decision={syncDecisions.decisions[entry.skill_id]}
                entry={entry}
                onDecision={deleteDecisionOptions(entry).length > 0 ? (choice) => handleDeleteDecision(entry, choice) : undefined}
                onOpenConflict={entry.conflict_reason ? () => openConflict(entry) : undefined}
                onOpenFolder={entry.local_path ? () => openSkillFolder(entry) : undefined}
                onToggle={isSelectable(entry) ? (selected) => toggleAction(entry.action_id, selected) : undefined}
                requiresConfirmation={isDeleteEntry(entry)}
                selected={selectedActionIds.includes(entry.action_id)}
              />
            </div>
          {/each}
        </div>
      {/if}

      {#if planData}
        <SyncCommitSummary
          summary={selectionSummary}
          localStateUpdates={planData.commit_summary.local_state_updates}
          hasLocalStateUpdates={hasLocalStateUpdates}
        />
      {/if}

      <SyncApplyBar
        selectedCount={selectionSummary.selected}
        willCreateCommit={selectionSummary.willCreateCommit}
        showCommitHint={planData !== undefined}
        canApply={canApply}
        applyPending={apply.isPending}
        onApply={handleApply}
      />
    </div>
  {/if}
</div>
