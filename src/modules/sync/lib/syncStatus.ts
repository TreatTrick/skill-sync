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
  'deleted',
  'conflict',
] as const

export type SyncStatusFilter = (typeof SYNC_STATUS_FILTERS)[number]

export const isDeleteEntry = (entry: SyncSkillEntry): boolean =>
  entry.delete_direction !== null ||
  entry.status === 'local_deleted' ||
  entry.status === 'remote_deleted'

const matchesStatusFilter = (
  status: SyncStatus,
  filter: SyncStatusFilter,
): boolean => {
  if (filter === 'all') return true
  if (filter === 'deleted') {
    return status === 'local_deleted' || status === 'remote_deleted'
  }
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

export const decisionLabelKey = (
  choice: SyncDecision,
): `sync.decisions.${SyncDecision}` => `sync.decisions.${choice}`
