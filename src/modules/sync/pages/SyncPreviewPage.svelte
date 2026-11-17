<script lang="ts">
  import { createMutation, createQuery, useQueryClient } from '@tanstack/svelte-query'
  import { goto } from '$app/navigation'
  import { ArrowDownToLine, ArrowUpFromLine, Ban, Check, ChevronRight, Clock3, GitCommitHorizontal, Info, Laptop, RefreshCw, Trash2, TriangleAlert, Undo2, X } from '@lucide/svelte'

  import { errorMessage, getAppState, openPath, scanSkills } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import type { RecoveryInfo, SyncProgressEvent } from '@/shared/schemas'
  import {
    Button,
    Checkbox,
    Skeleton,
    Spinner,
    toast,
  } from '@/shared/ui'

  import {
    applySyncPlan,
    getSyncPlan,
    resumeSyncRecovery,
  } from '../api/syncApi'
  import RecoveryCard from '../components/RecoveryCard.svelte'
  import SyncApplyBar from '../components/SyncApplyBar.svelte'
  import SyncFilterBar from '../components/SyncFilterBar.svelte'
  import SyncSkillCard from '../components/SyncSkillCard.svelte'
  import {
    bulkConflictDecision,
    conflictDecisionOptions,
    deleteDecisionOptions,
    isDeleteEntry,
    matchesEntry,
    summarizeSyncSelection,
    type ConflictBias,
    type SyncStatusFilter,
  } from '../lib/syncStatus'
  import type {
    ApplySyncRequest,
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
  let syncProgress = $state<SyncProgressEvent | null>(null)
  const updateProgress = (event: SyncProgressEvent): void => {
    syncProgress = event
  }
  const progressPhaseKeys = {
    scan: 'sync.progress.phase.scan',
    pack: 'sync.progress.phase.pack',
    download: 'sync.progress.phase.download',
    delete_local: 'sync.progress.phase.delete_local',
    upload: 'sync.progress.phase.upload',
    delete_remote: 'sync.progress.phase.delete_remote',
    remote_commit: 'sync.progress.phase.remote_commit',
    state_save: 'sync.progress.phase.state_save',
    cleanup: 'sync.progress.phase.cleanup',
    complete: 'sync.progress.phase.complete',
    failed: 'sync.progress.phase.failed',
    recovery: 'sync.progress.phase.recovery',
  } as const
  const progressPhaseLabel = (phase: string): string =>
    t(progressPhaseKeys[phase as keyof typeof progressPhaseKeys] ?? 'sync.progress.working')
  const configured = $derived(appState.data?.configured ?? false)
  const pendingRecovery = $derived(appState.data?.pending_recovery ?? null)

  const scan = createQuery(() => ({
    queryKey: ['scan-skills'],
    queryFn: scanSkills,
    enabled: configured && pendingRecovery === null,
  }))
  const plan = createQuery(() => ({
    queryKey: ['sync-plan'],
    queryFn: () => getSyncPlan(updateProgress),
    enabled: configured && pendingRecovery === null,
  }))

  let search = $state('')
  let statusFilter = $state<SyncStatusFilter>('pending')
  let selectedActionIds = $state<string[]>([])
  let deleteGuardAck = $state(false)
  let lastFingerprint = $state<string | null>(null)
  let defaultNextPlan = $state(false)
  let planNotice = $state('')
  let recoveryOverride = $state<RecoveryInfo | null>(null)
  let syncedExpanded = $state(false)

  const recovery = $derived(
    recoveryOverride ?? appState.data?.pending_recovery ?? null,
  )
  const planData = $derived(plan.data)
  const visibleEntries = $derived(
    planData?.entries.filter((entry) =>
      matchesEntry(entry, search, statusFilter) || (statusFilter === 'pending' && matchesEntry(entry, search, 'synced')),
    ) ?? [],
  )
  const entryGroups = $derived([
    {
      id: 'conflicts',
      kind: 'conflict',
      label: 'sync.needsDecision',
      hint: 'sync.groupHints.conflicts',
      entries: visibleEntries.filter((entry) => entry.status === 'conflict'),
    },
    {
      id: 'deletes',
      kind: 'del',
      label: 'sync.groups.deletes',
      hint: 'sync.groupHints.deletes',
      entries: visibleEntries.filter(
        (entry) =>
          entry.status === 'local_deleted' || entry.status === 'remote_deleted',
      ),
    },
    {
      id: 'uploads',
      kind: 'up',
      label: 'sync.groups.uploads',
      hint: 'sync.direction.upload',
      entries: visibleEntries.filter((entry) => entry.status === 'local_update'),
    },
    {
      id: 'downloads',
      kind: 'down',
      label: 'sync.groups.downloads',
      hint: 'sync.direction.download',
      entries: visibleEntries.filter((entry) => entry.status === 'remote_update'),
    },
    {
      id: 'blocked',
      kind: 'blocked',
      label: 'sync.groups.blocked',
      hint: 'sync.groupHints.blocked',
      entries: visibleEntries.filter(
        (entry) => entry.status === 'blocked' || entry.status === 'unknown',
      ),
    },
    {
      id: 'synced',
      kind: 'synced',
      label: 'sync.groups.synced',
      hint: 'sync.groupHints.synced',
      entries: visibleEntries.filter(
        (entry) => entry.status === 'synced' || entry.status === 'both_deleted',
      ),
    },
  ] as const)
  const filterCounts = $derived<Record<SyncStatusFilter, number>>({
    pending:
      planData?.entries.filter(
        (entry) => entry.status === 'conflict' || entry.status === 'local_deleted' || entry.status === 'remote_deleted' || entry.status === 'local_update' || entry.status === 'remote_update',
      ).length ?? 0,
    conflict: planData?.entries.filter((entry) => entry.status === 'conflict').length ?? 0,
    delete:
      planData?.entries.filter(
        (entry) => entry.status === 'local_deleted' || entry.status === 'remote_deleted',
      ).length ?? 0,
    up: planData?.entries.filter((entry) => entry.status === 'local_update').length ?? 0,
    down: planData?.entries.filter((entry) => entry.status === 'remote_update').length ?? 0,
    synced:
      planData?.entries.filter(
        (entry) => entry.status === 'synced' || entry.status === 'both_deleted',
      ).length ?? 0,
  })
  const blockedCount = $derived(
    planData?.entries.filter((entry) => entry.status === 'blocked' || entry.status === 'unknown').length ?? 0,
  )
  const undecidedCount = $derived(
    planData?.entries.filter((entry) =>
      (entry.status === 'conflict' || isDeleteEntry(entry)) &&
      syncDecisions.decisions[entry.skill_id] === undefined &&
      !selectedActionIds.includes(entry.action_id),
    ).length ?? 0,
  )
  const selectedEntries = $derived(
    planData?.entries.filter((entry) =>
      selectedActionIds.includes(entry.action_id),
    ) ?? [],
  )
  const selectionSummary = $derived(
    summarizeSyncSelection(
      selectedEntries,
      syncDecisions.decisions,
      planData?.entries.filter((entry) => entry.status === 'conflict') ?? [],
    ),
  )
  const showSyncedEntries = $derived(
    syncedExpanded || statusFilter === 'synced' || search.trim().length > 0,
  )
  const hasLocalStateUpdates = $derived(
    (planData?.base_adoptions.length ?? 0) > 0 ||
      (planData?.base_removals.length ?? 0) > 0,
  )
  const canApply = $derived(
    planData !== undefined &&
      !recovery &&
      !plan.isFetching &&
      !plan.error &&
      (selectionSummary.selected > 0 || hasLocalStateUpdates) &&
      (!selectionSummary.hasDelete ||
        !planData.delete_guard_tripped ||
        deleteGuardAck),
  )
  const recheckLoading = $derived.by(() => {
    const planFetching = plan.isFetching
    const scanFetching = scan.isFetching
    return planFetching || scanFetching
  })
  const progressPercent = $derived(
    syncProgress?.determinate && syncProgress.total
      ? Math.min(100, Math.round((syncProgress.current / syncProgress.total) * 100))
      : 0,
  )

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
    mutationFn: (request: ApplySyncRequest) => applySyncPlan(request, updateProgress),
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

  const showProgress = $derived(
    !apply.isPending && syncProgress !== null &&
      (recheckLoading ||
        syncProgress.status === 'failed' ||
        syncProgress.status === 'recovery'),
  )

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

  const toggleGroup = (entries: SyncSkillEntry[], selected: boolean): void => {
    const ids = entries.map((entry) => entry.action_id)
    selectedActionIds = selected
      ? [...new Set([...selectedActionIds, ...ids])]
      : selectedActionIds.filter((id) => !ids.includes(id))
  }

  const openSkillFolder = (entry: SyncSkillEntry): void => {
    if (entry.local_path) void openPath(entry.local_path)
  }

  const handleDeleteDecision = (
    entry: SyncSkillEntry,
    choice: SyncDecision | '',
  ): void => {
    if (choice === '') {
      syncDecisions.removeDecision(entry.skill_id)
      toggleAction(entry.action_id, false)
      return
    }
    if (!deleteDecisionOptions(entry).includes(choice)) return
    toggleAction(entry.action_id, false)
    syncDecisions.setDecision(entry.skill_id, choice)
  }

  const handleConflictDecision = (
    entry: SyncSkillEntry,
    choice: SyncDecision | '',
  ): void => {
    if (choice === '') {
      syncDecisions.removeDecision(entry.skill_id)
      return
    }
    if (!conflictDecisionOptions(entry.conflict_reason).includes(choice)) return
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
  const applyBulkDeleteDecision = (action: 'delete' | 'recover' | 'none'): void => {
    const entries = visibleEntries.filter(
      (entry) => entry.status === 'local_deleted' || entry.status === 'remote_deleted',
    )
    const ids = new Set(entries.map((entry) => entry.action_id))
    if (action === 'delete') {
      for (const entry of entries) syncDecisions.removeDecision(entry.skill_id)
      selectedActionIds = [
        ...new Set([...selectedActionIds, ...entries.map((entry) => entry.action_id)]),
      ]
      return
    }
    for (const entry of entries) {
      if (action === 'none') syncDecisions.removeDecision(entry.skill_id)
      else syncDecisions.setDecision(entry.skill_id, entry.status === 'local_deleted' ? 'restore_remote' : 'keep_local')
    }
    selectedActionIds = selectedActionIds.filter((id) => !ids.has(id))
  }

  const handleApply = (): void => {
    if (!planData || !canApply || apply.isPending) return
    syncProgress = null
    apply.mutate({
      expected_remote_commit: planData.expected_remote_commit,
      plan_fingerprint: planData.plan_fingerprint,
      selected_action_ids: selectedEntries
        .filter((entry) => syncDecisions.decisions[entry.skill_id] === undefined)
        .map((entry) => entry.action_id),
      decisions: { ...syncDecisions.decisions },
      delete_guard_ack: deleteGuardAck,
    })
  }

  const handleRecheck = (): void => {
    syncProgress = null
    defaultNextPlan = true
    planNotice = ''
    void plan.refetch()
    void scan.refetch()
  }
</script>

<div class="flex min-h-0 flex-1 flex-col">
  <div class="view">
    <div class="page">
      {#if appState.isLoading}
        <div class="scan"><Spinner class="size-5" /></div>
      {:else if appState.error}
        <div class="callout danger">{errorMessage(appState.error)}</div>
      {:else if !configured && !recovery}
        <div class="empty">
          <p>{t('sync.notConfigured')}</p>
          <Button class="btn primary" onclick={() => void goto('/app/onboarding')}>{t('sync.goToOnboarding')}</Button>
        </div>
      {:else if recovery}
        <RecoveryCard recovery={recovery} resumePending={resume.isPending} onResume={() => resume.mutate(recovery.task_id)} />
      {:else}
        <header class="sync-head">
          <div>
            <h1>
              {#if recheckLoading}
                {t('sync.checking')}
              {:else if plan.error}
                {t('sync.loadTitle')}
              {:else if filterCounts.pending === 0 && blockedCount > 0}
                {t('sync.blockedTitle', { count: blockedCount })}
              {:else if filterCounts.pending === 0}
                {t('sync.emptyAllSynced')}
              {:else}
                {t('sync.pendingTitle', { count: filterCounts.pending })}{#if undecidedCount > 0}<em>{t('sync.awaitingTitle', { count: undecidedCount })}</em>{/if}
              {/if}
            </h1>
            <div class="meta">
              <span><Laptop class="ic sm" />{t('sync.deviceMeta', { device: appState.data?.device_name ?? '—', count: planData?.entries.length ?? 0 })}</span>
              <span><GitCommitHorizontal class="ic sm" />{t('sync.remoteVersion')} <span class="mono">{planData?.expected_remote_commit.slice(0, 7) ?? '—'}</span></span>
              <span><Clock3 class="ic sm" />{recheckLoading ? t('sync.checking') : t('sync.lastChecked', { time: plan.dataUpdatedAt ? new Date(plan.dataUpdatedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : '—' })}</span>
            </div>
          </div>
          <Button class="btn" disabled={apply.isPending || recheckLoading} loading={recheckLoading} onclick={handleRecheck} variant="outline">
            {#if !recheckLoading}<RefreshCw class="ic" />{/if}{t('sync.recheck')}
          </Button>
        </header>

        {#if showProgress && syncProgress}
          <div class="scan" aria-live="polite">
            <div class="line"><b>{progressPhaseLabel(syncProgress.phase)}</b><span>{syncProgress.determinate ? syncProgress.current + '/' + syncProgress.total : t('sync.progress.working')}</span></div>
            <div class="bar" role="progressbar" aria-label={t('sync.progress.working')} aria-valuemin="0" aria-valuemax="100" aria-valuenow={syncProgress.determinate ? progressPercent : undefined}><i style={'width: ' + (syncProgress.determinate ? progressPercent : 33) + '%'}></i></div>
            {#if syncProgress.skill_id}<span class="mono muted">{syncProgress.skill_id}</span>{/if}
            <small class="muted">{t('sync.progress.cache', { hits: syncProgress.cache_hits, misses: syncProgress.cache_misses })}</small>
            {#if syncProgress.status === 'failed' || syncProgress.status === 'recovery'}<p class="text-destructive">{syncProgress.error ?? t('sync.progress.status.failed')}</p>{/if}
          </div>
        {/if}

        {#if scan.error}<div class="callout"><TriangleAlert class="ic" /><div class="grow">{t('sync.scanError', { message: errorMessage(scan.error) })}</div></div>{/if}

        {#if recheckLoading}
          <div class="rows">{#each [62, 48, 70, 40, 56] as width, i (i)}<div class="skel"><Skeleton class="size-4" /><Skeleton style={'width:' + width * 2 + 'px'} /><Skeleton style={'width:' + width + 'px'} /><span class="flex-1"></span><Skeleton class="w-36" /></div>{/each}</div>
        {:else if plan.error}
          <div class="callout danger"><TriangleAlert class="ic" /><div class="grow">{t('sync.loadError', { message: errorMessage(plan.error) })}</div></div>
        {:else}
          {#if planNotice}<div class="callout"><Info class="ic" /><div class="grow">{planNotice}</div><button aria-label={t('common.actions.close')} class="btn ghost sm icon" onclick={() => (planNotice = '')} type="button"><X class="ic" /></button></div>{/if}
          {#if planData?.delete_guard_tripped}
            <div class="callout"><TriangleAlert class="ic" /><div class="grow"><strong>{t('sync.deleteGuard.title')}</strong>{t('sync.deleteGuard.detail', { count: filterCounts.delete, limit: appState.data?.config.limits.max_auto_delete ?? 0 })}</div></div>
          {/if}
          {#if filterCounts.pending > 0 || blockedCount > 0 || search || statusFilter !== 'pending'}
            <SyncFilterBar bind:search {statusFilter} counts={filterCounts} onStatusFilter={(filter) => (statusFilter = filter)} />
          {/if}

          <div class:busy={apply.isPending} class="groups">
            {#if filterCounts.pending === 0 && blockedCount === 0 && !search && statusFilter === 'pending'}
              <div class="hero-ok"><span class="badge"><Check class="ic lg" /></span><div><h2>{t('sync.allSyncedTitle', { count: filterCounts.synced })}</h2><p class="muted">{t('sync.allSyncedHint')}</p></div></div>
            {/if}
            {#each entryGroups as group (group.id)}
              {@const isSynced = group.id === 'synced'}
              {#if group.entries.length > 0}
                {#if isSynced && !showSyncedEntries}
                  <button class="fold" aria-expanded="false" onclick={() => (syncedExpanded = true)} type="button"><Check class="ic" /><span>{t('sync.syncedFold', { count: group.entries.length })}</span><ChevronRight class="ic ml-auto" /></button>
                {:else}
                  <section class={'g-' + group.kind}>
                    <div class="group-head">
                      <span class="lead">
                        {#if group.id === 'conflicts'}<TriangleAlert class="ic" />
                        {:else if group.id === 'deletes'}<Trash2 class="ic" />
                        {:else if group.id === 'uploads' || group.id === 'downloads'}
                          <Checkbox class="row-check" aria-label={t('sync.selectGroup', { group: t(group.id === 'uploads' ? 'sync.groups.uploads' : 'sync.groups.downloads') })} checked={group.entries.every((entry) => selectedActionIds.includes(entry.action_id))} onCheckedChange={(checked) => toggleGroup(group.entries, checked === true)} />
                        {:else if group.id === 'blocked'}<Ban class="ic" />{:else}<Check class="ic" />{/if}
                      </span>
                      <h2>{t(group.label)}<span class="n">{group.entries.length}</span></h2>
                      <span class="hint">{t(group.hint)}</span>
                      <div class="bulk">
                        {#if group.id === 'conflicts'}
                          <Button class="btn sm" variant="outline" onclick={() => applyBulkConflictDecision('local')}><ArrowUpFromLine class="ic sm" />{t('sync.bulk.keepAllLocal')}</Button>
                          <Button class="btn sm" variant="outline" onclick={() => applyBulkConflictDecision('remote')}><ArrowDownToLine class="ic sm" />{t('sync.bulk.useAllRemote')}</Button>
                        {:else if group.id === 'deletes'}
                          <Button class="btn sm danger" variant="outline" onclick={() => applyBulkDeleteDecision('delete')}><Trash2 class="ic sm" />{t('sync.bulk.confirmAllDelete')}</Button>
                          <Button class="btn sm" variant="outline" onclick={() => applyBulkDeleteDecision('recover')}><Undo2 class="ic sm" />{t('sync.bulk.restoreAll')}</Button>
                          <Button class="btn sm ghost" variant="ghost" onclick={() => applyBulkDeleteDecision('none')}>{t('sync.bulk.deferAll')}</Button>
                        {:else if isSynced && !search && statusFilter === 'pending'}
                          <Button class="btn sm ghost" variant="ghost" onclick={() => (syncedExpanded = false)}>{t('sync.collapseSynced')}</Button>
                        {/if}
                      </div>
                    </div>
                    <div class="rows">
                      {#each group.entries as entry (entry.action_id)}
                        {@const decisionOptions = entry.status === 'conflict' ? conflictDecisionOptions(entry.conflict_reason) : deleteDecisionOptions(entry)}
                        <SyncSkillCard
                          decision={syncDecisions.decisions[entry.skill_id]}
                          {decisionOptions}
                          {entry}
                          onDecision={entry.status === 'conflict'
                            ? (choice) => handleConflictDecision(entry, choice)
                            : decisionOptions.length > 0
                              ? (choice) => handleDeleteDecision(entry, choice)
                              : undefined}
                          onOpenFolder={entry.local_path ? () => openSkillFolder(entry) : undefined}
                          onToggle={isSelectable(entry) ? (selected) => toggleAction(entry.action_id, selected) : undefined}
                          selected={selectedActionIds.includes(entry.action_id)}
                        />
                      {/each}
                    </div>
                  </section>
                {/if}
              {/if}
            {/each}
            {#if visibleEntries.length === 0 && (search || statusFilter !== 'pending')}<div class="empty"><p>{t('sync.empty')}</p><Button class="btn sm" variant="outline" onclick={() => { search = ''; statusFilter = 'pending' }}>{t('sync.clearFilters')}</Button></div>{/if}
          </div>
        {/if}
      {/if}
    </div>
  </div>
  {#if configured && !recovery && !appState.error && !plan.error && !recheckLoading && (filterCounts.pending > 0 || hasLocalStateUpdates || apply.isPending)}
    <SyncApplyBar summary={selectionSummary} showCommitHint={planData !== undefined} {canApply} applyPending={apply.isPending} progress={syncProgress} phaseLabel={syncProgress ? progressPhaseLabel(syncProgress.phase) : t('sync.progress.working')} deleteGuardTripped={planData?.delete_guard_tripped ?? false} {deleteGuardAck} localStateUpdates={planData?.commit_summary.local_state_updates ?? 0} onDeleteGuardAck={(ack) => (deleteGuardAck = ack)} onApply={handleApply} />
  {/if}
</div>
