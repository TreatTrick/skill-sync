import type {
  ConflictReason,
  SyncDecision,
  SyncSkillEntry,
  SyncStatus,
} from '../schemas/syncPlan'

export const SYNC_STATUS_FILTERS = [
  'pending',
  'conflict',
  'delete',
  'up',
  'down',
  'synced',
] as const

export type SyncStatusFilter = (typeof SYNC_STATUS_FILTERS)[number]

export const summarizeSyncSelection = (
  entries: SyncSkillEntry[],
  decisions: Record<string, SyncDecision>,
  conflicts: SyncSkillEntry[] = [],
) => {
  const selectedEntries = entries.filter((entry) => !decisions[entry.skill_id])
  const decisionValues = Object.values(decisions)
  const uploads =
    selectedEntries.filter((entry) => entry.status === 'local_update').length +
    decisionValues.filter((decision) => decision === 'keep_local').length
  const downloads =
    selectedEntries.filter((entry) => entry.status === 'remote_update').length +
    decisionValues.filter(
      (decision) => decision === 'use_remote' || decision === 'restore_remote',
    ).length
  const deleteRemote =
    selectedEntries.filter((entry) => entry.status === 'local_deleted').length +
    decisionValues.filter((decision) => decision === 'delete_remote').length
  const deleteLocal =
    selectedEntries.filter((entry) => entry.status === 'remote_deleted')
      .length +
    decisionValues.filter((decision) => decision === 'accept_delete').length
  const skipped = decisionValues.filter(
    (decision) => decision === 'skip',
  ).length
  const unresolvedConflicts = conflicts.filter(
    (entry) => decisions[entry.skill_id] === undefined,
  ).length
  const selected = uploads + downloads + deleteRemote + deleteLocal

  return {
    selected,
    uploads,
    downloads,
    deleteRemote,
    deleteLocal,
    skipped,
    unresolvedConflicts,
    hasDelete: deleteRemote > 0 || deleteLocal > 0,
    willCreateCommit: uploads > 0 || deleteRemote > 0,
  }
}

export const isDeleteEntry = (entry: SyncSkillEntry): boolean =>
  entry.delete_direction !== null ||
  entry.status === 'local_deleted' ||
  entry.status === 'remote_deleted'

const matchesStatusFilter = (
  status: SyncStatus,
  filter: SyncStatusFilter,
): boolean => {
  if (filter === 'pending')
    return status !== 'synced' && status !== 'both_deleted'
  if (filter === 'delete') {
    return status === 'local_deleted' || status === 'remote_deleted'
  }
  if (filter === 'up') return status === 'local_update'
  if (filter === 'down') return status === 'remote_update'
  if (filter === 'synced')
    return status === 'synced' || status === 'both_deleted'
  return status === 'conflict'
}

const matchesSearch = (entry: SyncSkillEntry, search: string): boolean => {
  const query = search.trim().toLocaleLowerCase()
  if (!query) return true
  return [
    entry.name,
    entry.skill_id,
    entry.namespace,
    entry.folder_name,
    entry.relative_dir,
    entry.local_path,
  ]
    .filter((value): value is string => value !== null)
    .some((value) => value.toLocaleLowerCase().includes(query))
}

export const matchesEntry = (
  entry: SyncSkillEntry,
  search: string,
  filter: SyncStatusFilter,
): boolean =>
  matchesStatusFilter(entry.status, filter) && matchesSearch(entry, search)

export const conflictDecisionOptions = (
  reason: ConflictReason | null,
): readonly SyncDecision[] => {
  if (reason === 'local_deleted_remote_changed') {
    return ['delete_remote', 'restore_remote', 'skip']
  }
  if (reason === 'remote_deleted_local_changed') {
    return ['keep_local', 'accept_delete', 'skip']
  }
  return ['keep_local', 'use_remote', 'skip']
}

export const deleteDecisionOptions = (
  entry: SyncSkillEntry,
): readonly SyncDecision[] => {
  if (entry.status === 'local_deleted') {
    return ['delete_remote', 'restore_remote', 'skip']
  }
  if (entry.status === 'remote_deleted') {
    return ['accept_delete', 'keep_local', 'skip']
  }
  return []
}

export const decisionLabelKey = (
  choice: SyncDecision,
): `sync.decisions.${SyncDecision}` => `sync.decisions.${choice}`

// Bias for bulk conflict resolution: adopt the remote state or the local state.
export type ConflictBias = 'remote' | 'local'

// Map a conflict reason to the decision that adopts the chosen side's state.
// Covers every reason so bulk actions leave no conflict unhandled:
//   both_changed / same_name_first_seen -> use_remote | keep_local
//   local_deleted_remote_changed (local gone, remote changed) ->
//     remote: restore_remote (pull remote back) | local: delete_remote (keep local deletion)
//   remote_deleted_local_changed (remote gone, local changed) ->
//     remote: accept_delete (accept remote deletion) | local: keep_local (re-upload)
export const bulkConflictDecision = (
  reason: ConflictReason,
  bias: ConflictBias,
): SyncDecision => {
  if (bias === 'remote') {
    if (reason === 'local_deleted_remote_changed') return 'restore_remote'
    if (reason === 'remote_deleted_local_changed') return 'accept_delete'
    return 'use_remote'
  }
  if (reason === 'local_deleted_remote_changed') return 'delete_remote'
  return 'keep_local'
}
