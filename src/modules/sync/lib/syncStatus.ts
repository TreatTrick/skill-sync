import type {
  ConflictReason,
  SyncDecision,
  SyncSkillEntry,
  SyncStatus,
} from '../schemas/syncPlan'

export const SYNC_STATUS_FILTERS = [
  'all',
  'synced',
  'local_update',
  'remote_update',
  'delete_remote',
  'delete_local',
  'conflict',
] as const

export type SyncStatusFilter = (typeof SYNC_STATUS_FILTERS)[number]

// Filters that represent pending changes worth surfacing as badge counts.
type SyncChangeFilter =
  | 'local_update'
  | 'remote_update'
  | 'delete_remote'
  | 'delete_local'
  | 'conflict'

export type SyncChangeCounts = Record<SyncChangeFilter, number>

export const EMPTY_SYNC_CHANGE_COUNTS: SyncChangeCounts = {
  local_update: 0,
  remote_update: 0,
  delete_remote: 0,
  delete_local: 0,
  conflict: 0,
}

// Count pending-change entries per filter. Mirrors matchesStatusFilter so the
// badge totals stay consistent with what each filter actually shows.
export const countSyncChanges = (
  entries: SyncSkillEntry[],
): SyncChangeCounts => {
  const counts: SyncChangeCounts = { ...EMPTY_SYNC_CHANGE_COUNTS }
  for (const entry of entries) {
    if (entry.status === 'local_update') counts.local_update++
    else if (entry.status === 'remote_update') counts.remote_update++
    else if (entry.status === 'local_deleted') counts.delete_remote++
    else if (entry.status === 'remote_deleted') counts.delete_local++
    else if (entry.status === 'conflict') counts.conflict++
  }
  return counts
}

export const summarizeSyncSelection = (
  entries: SyncSkillEntry[],
  decisions: SyncDecision[],
) => {
  const actions: (SyncStatus | SyncDecision)[] = [
    ...entries.map((entry) => entry.status),
    ...decisions,
  ]
  const count = (...types: (SyncStatus | SyncDecision)[]): number =>
    actions.filter((action) => types.includes(action)).length
  const uploads = count('local_update', 'keep_local')
  const downloads = count('remote_update', 'use_remote', 'restore_remote')
  const deleteRemote = count('local_deleted', 'delete_remote')
  const deleteLocal = count('remote_deleted', 'accept_delete')

  return {
    selected: actions.length,
    uploads,
    downloads,
    deleteRemote,
    deleteLocal,
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
  if (filter === 'all') return true
  if (filter === 'delete_remote') return status === 'local_deleted'
  if (filter === 'delete_local') return status === 'remote_deleted'
  return status === filter
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

export const statusLabelKey = (
  status: SyncStatus,
): `sync.status.${SyncStatus}` => `sync.status.${status}`

export const statusTone = (
  status: SyncStatus,
): 'neutral' | 'success' | 'warning' | 'destructive' | 'info' => {
  if (status === 'synced') return 'success'
  if (status === 'conflict' || status === 'blocked') return 'destructive'
  if (status === 'local_deleted' || status === 'remote_deleted') {
    return 'warning'
  }
  if (status === 'unknown') return 'neutral'
  return 'info'
}

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
    return ['restore_remote', 'delete_remote', 'skip']
  }
  if (entry.status === 'remote_deleted') {
    return ['keep_local', 'accept_delete', 'skip']
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
