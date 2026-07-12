# Sync Recovery Journal Fix Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow same folder names in different skill namespaces and make interrupted remote commits recover from durable, verifiable evidence without stale recovery loops.

**Architecture:** Keep validation in `vault_manifest`, remote publication semantics in `github_store`, durable evidence in `local_apply`, and orchestration in `sync_engine::vault`/`commands`. Extend the existing schema-1 journal with backward-compatible optional evidence, and reconcile it against one immutable remote snapshot before changing local state.

**Tech Stack:** Rust, Tauri 2, Tokio, serde, SHA-256, wiremock, existing `RemoteStore` abstraction.

---

## Chunk 1: Manifest And Immutable Snapshot

### Task 1: Scope folder collisions to a namespace

**Files:**

- Modify: `src-tauri/src/vault_manifest.rs`
- Test: `src-tauri/src/vault_manifest.rs`

- [ ] **Step 1: Write the failing cross-namespace test**

Add `manifest_allows_same_folder_name_in_different_namespaces`. Build two otherwise valid entries with `folder_name = "shared"`, one `codex:shared` and one `claude-code:shared`, then assert `VaultManifest::parse_validated` succeeds.

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml vault_manifest::tests::manifest_allows_same_folder_name_in_different_namespaces --locked`

Expected: FAIL with `manifest folder_name collision`.

- [ ] **Step 3: Implement namespace-scoped collision keys**

Change `folder_collision_key` to accept `SkillNamespace` and prefix the NFC/lowercase folder key with `namespace_ser_value(namespace)`. Pass `skill.namespace` from `VaultManifest::validate`.

- [ ] **Step 4: Preserve same-namespace rejection**

Add `manifest_rejects_same_folder_name_in_one_namespace`, using distinct IDs but case-equivalent folders within `codex`, and assert validation fails.

- [ ] **Step 5: Run manifest tests and verify GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml vault_manifest::tests --locked`

Expected: all manifest tests PASS.

### Task 2: Fetch manifest by immutable commit SHA

**Files:**

- Modify: `src-tauri/src/github_store.rs`
- Modify: `src-tauri/src/github_credentials.rs`
- Test: `src-tauri/src/github_store.rs`

- [ ] **Step 1: Update the manifest fetch test to require the exact SHA**

Change `fetch_manifest_reads_head_and_validated_manifest` so the content mock expects `ref=head` instead of `ref=main`. Keep the returned snapshot assertion at `commit_sha == "head"`. Also add the concurrent-advance test now: serve HEAD `head-a`, require manifest `ref=head-a`, and provide no branch-relative manifest mock.

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml github_store::tests::fetch_manifest_reads_head_and_validated_manifest --locked`

Expected: FAIL because the request still sends `ref=main`.

- [ ] **Step 3: Implement an explicit content ref**

Replace `contents_path(path)` with `contents_path(path, reference)`. In `fetch_manifest`, fetch HEAD once and request `manifest.json` with that SHA. Continue using the configured branch for content-addressed blob reads.

- [ ] **Step 4: Run GitHub store tests and verify GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml github_store::tests --locked`

Expected: all GitHub store tests PASS.

- [ ] **Step 5: Commit Chunk 1**

Run:

```powershell
git add src-tauri/src/vault_manifest.rs src-tauri/src/github_store.rs
git commit -m "fix: scope manifest paths by namespace"
```

## Chunk 2: Durable Journal And Commit Contract

### Task 3: Add backward-compatible recovery evidence

**Files:**

- Modify: `src-tauri/src/local_apply.rs`
- Modify: `src-tauri/src/sync_engine/vault.rs`
- Test: `src-tauri/src/local_apply.rs`

- [ ] **Step 1: Write journal compatibility and mapping tests**

Add tests that deserialize the current schema-1 fixture without new fields and that round-trip a journal containing `remote_base`, `next_manifest_hash`, `completed_action_ids`, and `pending_action_ids`.

- [ ] **Step 2: Run local journal tests and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml local_apply::tests --locked`

Expected: FAIL because the evidence fields do not exist.

- [ ] **Step 3: Extend `ApplyJournal`**

Add serde-defaulted fields:

```rust
pub remote_base: Option<String>,
pub next_manifest_hash: Option<String>,
#[serde(default)]
pub completed_action_ids: Vec<String>,
#[serde(default)]
pub pending_action_ids: Vec<String>,
```

Use `#[serde(default)]` on every new field so the current on-disk journal remains readable. Update every existing `ApplyJournal` literal in `sync_engine/vault.rs` with `None` and empty-vector defaults before the GREEN run; serde defaults do not initialize Rust struct literals.

- [ ] **Step 4: Run local journal tests and verify GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml local_apply::tests --locked`

Expected: PASS.

### Task 4: Reconcile every ambiguous branch update

**Files:**

- Modify: `src-tauri/src/github_store.rs`
- Test: `src-tauri/src/github_store.rs`

- [ ] **Step 1: Write PATCH outcome tests**

Cover these cases after a candidate commit exists: 5xx plus HEAD=candidate returns success; 5xx plus HEAD=base returns the original error; 5xx plus HEAD=other returns `RemoteChanged`; 5xx plus unreadable HEAD returns `RemoteOutcomeUnknown`. Repeat the four outcome assertions by calling the same reconciliation path with a transport-style `AppError::Auth("http error: timeout")`, proving response and network errors share the contract. Assert the candidate/base values in the unknown error.

Add one full `commit_changes` transport test. Add a `#[cfg(test)]` constructor in `github_credentials.rs` that accepts an already built `reqwest::Client`, leaving production construction unchanged. Inject a short-timeout client, delay only the PATCH wiremock response beyond that timeout, and use a stateful mock responder so the two pre-PATCH HEAD reads return base while the reconciliation read returns candidate. Assert `commit_changes` succeeds with the candidate. This proves a real PATCH transport failure reaches reconciliation; direct helper tests cover the other HEAD outcomes without repeated slow timeouts.

- [ ] **Step 2: Run the new tests and verify RED**

Run each new test with `cargo test --manifest-path src-tauri/Cargo.toml github_store::tests::<name> --locked`.

Expected: the 5xx candidate case returns a vault error, and transport errors bypass reconciliation or produce the wrong outcome.

- [ ] **Step 3: Implement the publication outcome contract**

After PATCH, keep 409/422 as `RemoteChanged`. Convert any transport error or other unsuccessful response into an `AppError`, then call one reconciliation method with the base, candidate, and original error. Return success for candidate HEAD, the original error for base HEAD, `RemoteChanged` for another HEAD, and `RemoteOutcomeUnknown` if HEAD cannot be read.

- [ ] **Step 4: Run GitHub store tests and verify GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml github_store::tests --locked`

Expected: PASS.

### Task 5: Persist the complete intended state before publication

**Files:**

- Modify: `src-tauri/src/sync_engine/vault.rs`
- Modify: `src-tauri/src/vault_manifest.rs`
- Test: `src-tauri/src/sync_engine/vault.rs`

- [ ] **Step 1: Write failing apply tests**

Add a definite-error mock mode and assert: the returned error is preserved, `load_pending(config_dir)` is `None`, and one commit attempt occurred. Add an outcome-unknown assertion that the pending journal contains exact action IDs, base commit, candidate commit, intended manifest hash, explicit phase, and a complete intended `SyncState`. Add an initial-journal-save failure test using an invalid config directory and assert commit count remains zero.

Add persistence-transition tests using a mock-store journal mutation enum. During `commit_changes`, mutate only the temporary test config path after the initial journal has succeeded:

- Replace `apply-transaction.json` with a directory before returning a definite error, so `clear_journal` fails; assert the clear error is returned and the path remains.
- Replace the config directory with a file before returning `RemoteOutcomeUnknown`, so the unknown-phase `save_journal` fails; assert the persistence error wins and no local state is saved.
- Replace `apply-transaction.json` with a directory before returning success, so the `state_saving` replacement fails; assert remote commit count is one, local sync state remains at the base, and recovery evidence is not silently cleared.

These mutations produce deterministic `IsADirectory`/`AlreadyExists` failures without relying on Unix or Windows permission semantics.

- [ ] **Step 2: Run each focused test and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sync_engine::vault::tests::<name> --locked`

Expected: stale journal remains, evidence fields are empty, or commit is attempted despite save failure.

- [ ] **Step 3: Expose validated deterministic manifest bytes**

Add a crate-visible `validated_bytes(&self) -> Result<Vec<u8>>` on `VaultManifest` that serializes and validates through the existing parser. Use it both for GitHub commit preflight and the intended manifest SHA-256.

- [ ] **Step 4: Build complete intended state before commit**

Move base adoption/removal state changes before the remote call. Keep `working.remote.commit_sha` at the base until a candidate is known. Construct and save a `remote_committing` journal with complete state, manifest hash, base commit, and exact IDs; propagate save failure before calling `commit_changes`.

- [ ] **Step 5: Make every journal transition durable**

On `RemoteChanged` or definite error, propagate `clear_journal`. On `RemoteOutcomeUnknown`, update phase and candidate then propagate `save_journal`. On success, substitute the candidate in `working`, write `state_saving`, save sync state, and propagate final clear. Make trash recovery persistence fallible as well.

- [ ] **Step 6: Run sync engine tests and verify GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sync_engine::vault::tests --locked`

Expected: PASS.

- [ ] **Step 7: Commit Chunk 2**

Run:

```powershell
git add src-tauri/src/local_apply.rs src-tauri/src/vault_manifest.rs src-tauri/src/github_credentials.rs src-tauri/src/github_store.rs src-tauri/src/sync_engine/vault.rs
git commit -m "fix: persist remote commit recovery evidence"
```

## Chunk 3: Evidence-Based Resume And Current Repair

### Task 6: Reconcile pending remote journals

**Files:**

- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/local_apply.rs`
- Test: `src-tauri/src/commands.rs`

- [ ] **Step 1: Write pure reconciliation decision tests**

Add a small semantic helper that accepts a journal and immutable `RemoteSnapshot`. Test unpublished (HEAD=base), published by candidate, published by intended manifest hash, conflicting, and legacy journal cases. Assert the published state preserves adoption/removal entries and changes only `remote.commit_sha`.

Extract `resume_remote_recovery<S: RemoteStore>` as the recovery orchestrator. It accepts the store, config, current state, journal, config directory, and home path; it owns immutable snapshot fetch, backup/clear/save, and latest-plan rebuilding. `resume_sync_recovery_impl` remains the concrete Tauri adapter that acquires the write gate, creates `GitHubVaultStore`, and delegates.

Add integration-level tests for this generic orchestrator with the existing mock `RemoteStore`: for unpublished evidence, assert the active journal is backed up and cleared and the response is `PlanChanged` with a freshly rebuilt plan. For published evidence, assert the saved state and `Applied` response. For conflict/unavailable evidence, assert the active journal remains.

Add backup ordering tests around a real `backup_journal` function in `local_apply`: successful backup bytes exactly match the active journal before clear; when `recovery-backups` is pre-created as a file, backup fails and the active journal remains untouched. Include the legacy journal fixture in the successful-backup test.

- [ ] **Step 2: Run focused command tests and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml commands::tests::remote_recovery --locked`

Expected: FAIL because remote journals are always returned unchanged.

- [ ] **Step 3: Implement reconciliation under the existing write gate**

In `resume_sync_recovery_impl`, retain the current local `state_saving` recovery. For remote phases, load config/store, fetch one immutable snapshot, and apply the decision: back up then clear on unpublished and rebuild/return the latest plan; save intended state with the snapshot SHA then back up/clear on published; retain and return `RecoveryRequired` on conflict or unavailable evidence. A backup failure aborts before clear.

- [ ] **Step 4: Map exact journal evidence to UI recovery info**

Update `journal_to_recovery` to explicitly map `remote_committing` and `remote_outcome_unknown`, candidate commit, and the exact completed/pending ID vectors.

- [ ] **Step 5: Run command and local recovery tests and verify GREEN**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml commands::tests --locked
cargo test --manifest-path src-tauri/Cargo.toml local_apply::tests --locked
```

Expected: PASS.

- [ ] **Step 6: Commit Chunk 3 implementation**

Run:

```powershell
git add src-tauri/src/local_apply.rs src-tauri/src/commands.rs
git commit -m "fix: reconcile pending remote sync recovery"
```

### Task 7: Repair the current pending journal and verify the repository

**Files:**

- Runtime state: `%APPDATA%/skill-sync/apply-transaction.json`
- Backup: `%APPDATA%/skill-sync/recovery-backups/`

- [ ] **Step 1: Run Rust formatting and focused regression tests**

Run:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml --all-features --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
```

Expected: all commands exit 0 without warnings.

- [ ] **Step 2: Run repository pre-completion checks**

Run:

```powershell
npm run typecheck
npm run format:check
npm run lint
npm run build
```

Expected: all commands exit 0.

- [ ] **Step 3: Exercise authenticated recovery**

Start the Tauri dev app, invoke the existing “Continue recovery” action, and inspect the log. Expected: the immutable snapshot HEAD equals the legacy journal base, the journal is copied into `recovery-backups`, active recovery clears, and the sync preview loads. If evidence differs or cannot be fetched, leave the active journal untouched and report the exact blocker.

- [ ] **Step 4: Review final diff and commit verification updates if any**

Run `git diff --check` and review only the scoped Rust/docs changes. If verification required a scoped correction, commit it separately with an appropriate Conventional Commit message; otherwise leave the already reviewed chunk commits unchanged.
