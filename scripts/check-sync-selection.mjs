import assert from 'node:assert/strict'

import { summarizeSyncSelection } from '../src/modules/sync/lib/syncStatus.ts'

const selected = [
  { status: 'local_update', skill_id: 'upload' },
  { status: 'remote_update', skill_id: 'download' },
  { status: 'local_deleted', skill_id: 'remote-delete' },
  { status: 'remote_deleted', skill_id: 'local-delete' },
]
const decisions = {
  download: 'use_remote',
  'remote-delete': 'delete_remote',
  skipped: 'skip',
  restored: 'restore_remote',
  kept: 'keep_local',
  accepted: 'accept_delete',
}
const conflicts = [
  { skill_id: 'skipped' },
  { skill_id: 'kept' },
  { skill_id: 'unresolved' },
]

assert.deepEqual(summarizeSyncSelection(selected, decisions, conflicts), {
  selected: 7,
  uploads: 2,
  downloads: 2,
  deleteRemote: 1,
  deleteLocal: 2,
  skipped: 1,
  unresolvedConflicts: 1,
  hasDelete: true,
  willCreateCommit: true,
})

const skipped = summarizeSyncSelection([], { skipped: 'skip' }, [
  { skill_id: 'skipped' },
])
assert.equal(skipped.selected, 0)
assert.equal(skipped.skipped, 1)
assert.equal(skipped.unresolvedConflicts, 0)
assert.equal(skipped.willCreateCommit, false)
assert.equal(
  summarizeSyncSelection([], { download: 'use_remote' }).willCreateCommit,
  false,
)
console.log('Sync selection checks passed.')
