# Sync Recovery Journal Fix

## Problem

The manifest validator currently treats `folder_name` as globally unique. Skills live under
namespace-specific roots, so entries such as `codex:ddd-scope` and
`claude-code:ddd-scope` do not share a filesystem path and must be allowed. Rejecting this
valid manifest causes `commit_changes` to fail before any GitHub request is made.

`apply_plan` persists a `remote_committing` journal before calling `commit_changes`, but it
only clears that journal for `RemoteChanged`. A preflight validation error therefore leaves
the journal behind. On the next application load, the unknown journal phase is displayed as
`remote_outcome_unknown`; resume returns the same journal forever.

## Design

Manifest folder collision keys will include the skill namespace. Folder names remain
case-insensitively unique within one namespace, while identical names across different
namespaces are valid.

Manifest validation remains in the GitHub adapter as defense in depth, and `apply_plan` will
validate the completed manifest before writing the pre-commit journal. A validation failure
therefore cannot create a recovery record.

Before the remote call, `apply_plan` will finish every pure state transition, including base
adoptions and removals, and construct the complete intended post-apply `SyncState`. Its remote
commit field temporarily remains the base commit until GitHub supplies or recovery discovers
the published commit. The journal stores this complete state, so recovery only substitutes the
reconciled commit SHA rather than reconstructing state from action metadata.

The journal will retain enough evidence to reconcile an interrupted remote commit. Optional
fields preserve compatibility with existing schema-1 journals:

- base remote commit;
- deterministic hash of the intended manifest;
- candidate commit when GitHub has returned one;
- completed and pending action IDs.

Writing the initial journal is fallible and must succeed before the remote call starts. The
remote commit transition will use these outcomes:

- Success: durably replace the journal with `state_saving`, including the candidate commit,
  then save local sync state.
- `RemoteChanged`: clear the pre-commit journal and return `PlanChanged`.
- `RemoteOutcomeUnknown`: update the journal with the explicit unknown phase and candidate
  commit, then return `RecoveryRequired`.
- Any other error returned before branch ref publication: clear the pre-commit journal and
  return the original error.

The GitHub adapter will enforce the outcome contract at the branch PATCH boundary. A 409 or
422 is a definite remote change. For a transport error or any other unsuccessful PATCH
response, the adapter reads HEAD: candidate means success, base means the original operation
was not published, another commit means `RemoteChanged`, and failure to read HEAD means
`RemoteOutcomeUnknown`. Consequently, ordinary errors returned to `apply_plan` are safe to
clear because they occurred before publication or were reconciled against HEAD.

If updating or clearing a journal fails, return that persistence error instead of hiding it.
The initial save, unknown-phase update, success-to-state-saving transition, and clear paths
must all propagate persistence failures. The existing local replacement and state-saving
recovery behavior otherwise remains unchanged.

Remote evidence must come from one immutable snapshot. `fetch_manifest` will read HEAD once,
then fetch and validate `manifest.json` at that exact commit SHA rather than through a mutable
branch ref. The returned `RemoteSnapshot` therefore associates one manifest with one commit
even if another device advances the branch during the request.

Resume will reconcile `remote_committing` and `remote_outcome_unknown` journals through that
authenticated immutable snapshot while synchronization remains exclusively gated:

- HEAD equals base: publication did not occur; clear the journal and rebuild the plan.
- HEAD equals candidate, or the fetched manifest hash equals the intended hash: update the
  journal state with the fetched HEAD, durably save sync state, then clear the journal.
- HEAD and manifest match neither known outcome: retain the journal and report the conflict.
- Remote evidence cannot be fetched: retain the journal and return recovery-required/error.

This reconciliation also closes the process-exit window between a successful remote commit
and the `state_saving` journal replacement.

## Current State Repair

The current journal predates the added evidence fields. Its embedded sync state supplies the
base commit. After the code fix is verified, run the same authenticated recovery path. Clear
the journal only when fetched HEAD still equals that base. If HEAD differs or cannot be
fetched, retain the journal and report that manual reconciliation is required. Before any
successful clear, preserve a timestamped diagnostic backup outside the active journal path.

## Tests

- A manifest accepts equal normalized folder names in different namespaces.
- A manifest still rejects equal normalized folder names in the same namespace.
- Invalid next-manifest preflight leaves no journal and makes no remote call.
- A definite commit error returns the error and leaves no pending journal.
- PATCH timeout/network and 5xx responses reconcile candidate/base/other/unavailable HEAD
  outcomes correctly.
- An unknown remote outcome persists the candidate commit, intended manifest hash, action
  IDs, and explicit recovery phase; the recovery response exposes the exact persisted ID sets.
- Initial-save, unknown-update, state-saving transition, and clear failures do not get
  silently discarded.
- Resume reconciles unpublished, published, conflicting, and unavailable remote outcomes,
  including a legacy journal without the new optional fields.
- Interrupted mixed applies preserve base adoptions/removals from the complete intended state.
- A concurrent branch advance cannot pair a manifest from one commit with another HEAD.

Run the focused Rust tests first, then the repository pre-completion checks.
