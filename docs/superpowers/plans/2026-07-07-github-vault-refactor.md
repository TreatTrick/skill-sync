# GitHub Vault Refactor Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current Git CLI / local repo sync model with a GitHub API-backed vault using `manifest.json`, content-addressed skill zip blobs, local `sync_state`, Device Flow auth, and a unified Sync page.

**Architecture:** The Svelte UI stays as the interaction layer, while all filesystem, packing, hashing, GitHub API, conflict, and overwrite-protection logic lives in Rust Tauri commands. The backend is rebuilt around `SkillPacker`, `SyncStateStore`, `RemoteStore`, `GitHubVaultStore`, `LocalVaultStore`, and a rewritten `SyncEngine`.

**Tech Stack:** Tauri 2, Rust 1.77+, Svelte 5, SvelteKit static SPA, TypeScript strict, Zod, i18next, Tailwind CSS v4, GitHub REST/Git Database APIs.

---

## Source Of Truth

- Design doc: `docs/github-base-refactor-design.md`
- Follow repo rules in `AGENTS.md`.
- Manual edits must use `apply_patch`.
- UI text must go through `src/shared/i18n/locales`.
- After UI/layout/color/copy changes, run matching lint scripts.
- Before final handoff, run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm run typecheck
npm run format:check
npm run lint
npm run build
```

## Target File Map

Create:

- `src-tauri/src/ignore.rs` - built-in + user ignore matching for pack/hash.
- `src-tauri/src/pack.rs` - canonical zip creation, hash, size limit, zip-slip-safe unpack.
- `src-tauri/src/vault_manifest.rs` - cloud `manifest.json` DTOs and validation.
- `src-tauri/src/sync_state.rs` - local base hash / remote commit state.
- `src-tauri/src/remote_store.rs` - `RemoteStore` trait and shared remote DTOs.
- `src-tauri/src/local_vault_store.rs` - local directory vault for tests/dev.
- `src-tauri/src/github_auth.rs` - GitHub OAuth Device Flow and token storage boundary.
- `src-tauri/src/github_store.rs` - GitHub manifest/blob/commit implementation.
- `src-tauri/tests/github_vault_sync.rs` - end-to-end-ish Rust tests using `LocalVaultStore`.
- `src/modules/sync/lib/syncStatus.ts` - frontend status/filter helpers.
- `src/modules/sync/components/SyncSkillCard.svelte` - skill status card.
- `src/modules/sync/components/ConflictDetailDialog.svelte` - conflict summary/decision dialog.

Modify:

- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/config.rs`
- `src-tauri/src/detect.rs`
- `src-tauri/src/errors.rs`
- `src-tauri/src/skill.rs`
- `src-tauri/src/sync_engine.rs`
- `src/modules/sync/schemas/syncPlan.ts`
- `src/modules/sync/api/syncApi.ts`
- `src/modules/sync/pages/SyncPreviewPage.svelte`
- `src/modules/settings/schemas/config.ts`
- `src/modules/settings/pages/SettingsPage.svelte`
- `src/modules/onboarding/schemas/onboarding.ts`
- `src/modules/onboarding/api/onboardingApi.ts`
- `src/modules/onboarding/pages/OnboardingPage.svelte`
- `src/app/router/routeConfig.ts`
- `src/shared/i18n/locales/zh-CN.json`
- `src/shared/i18n/locales/en-US.json`
- `src/shared/lib/tauri.ts` only if command argument typing needs adjustment.

Delete:

- `src-tauri/src/git_store.rs`
- `src-tauri/src/backup.rs`
- `src/modules/backups/**`
- `src/modules/conflicts/**`
- `src/routes/app/backups/+page.svelte`
- `src/routes/app/conflicts/+page.svelte`
- `src/modules/onboarding/components/SshSetupDialog.svelte`
- `src/modules/onboarding/content/ssh-setup-prompt.md`

## Chunk 1: Backend Contracts And Config

### Task 1: Add Dependencies And Module Shells

**Files:**

- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/ignore.rs`
- Create: `src-tauri/src/pack.rs`
- Create: `src-tauri/src/vault_manifest.rs`
- Create: `src-tauri/src/sync_state.rs`
- Create: `src-tauri/src/remote_store.rs`
- Create: `src-tauri/src/local_vault_store.rs`
- Create: `src-tauri/src/github_auth.rs`
- Create: `src-tauri/src/github_store.rs`

- [ ] **Step 1: Add Rust dependencies**

Add dependencies needed for HTTP, zip, random task IDs, base64, and OS credential storage.

```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
zip = { version = "2", default-features = false, features = ["deflate"] }
uuid = { version = "1", features = ["v4", "serde"] }
base64 = "0.22"
keyring = "3"
```

Expected: `cargo test --manifest-path src-tauri/Cargo.toml --no-run` compiles dependency resolution or fails only because new modules are still empty.

- [ ] **Step 2: Register new modules**

In `src-tauri/src/lib.rs`, add:

```rust
mod github_auth;
mod github_store;
mod ignore;
mod local_vault_store;
mod pack;
mod remote_store;
mod sync_state;
mod vault_manifest;
```

Keep old modules temporarily until later tasks remove call sites:

```rust
mod backup;
mod git_store;
mod manifest;
```

- [ ] **Step 3: Add empty module files**

Each new file should compile with a short placeholder type or comment. Example:

```rust
// GitHub vault implementation lands in later tasks.
```

- [ ] **Step 4: Verify**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-run
```

Expected: compiles, or reports only warnings from unused placeholders.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src
git commit -m "build: add github vault backend modules"
```

### Task 2: Replace App Config Shape

**Files:**

- Modify: `src-tauri/src/config.rs`
- Modify: `src/modules/settings/schemas/config.ts`
- Modify: `src/modules/onboarding/schemas/onboarding.ts`
- Modify: `src/shared/i18n/locales/zh-CN.json`
- Modify: `src/shared/i18n/locales/en-US.json`

- [ ] **Step 1: Write config tests first**

Add or update Rust tests in `src-tauri/src/config.rs` for:

```rust
#[test]
fn default_config_uses_github_vault_remote() {
    let cfg = AppConfig::default_config();
    assert_eq!(cfg.remote.provider, "github");
    assert_eq!(cfg.remote.branch, "main");
    assert_eq!(cfg.limits.max_skill_zip_bytes, 20 * 1024 * 1024);
    assert!(cfg.ignore.iter().any(|p| p == "**/cache/**"));
}

#[test]
fn config_roundtrip_preserves_ignore_and_limits() {
    let cfg = AppConfig::default_config();
    let text = serde_yaml::to_string(&cfg).unwrap();
    let back: AppConfig = serde_yaml::from_str(&text).unwrap();
    assert_eq!(back.ignore, cfg.ignore);
    assert_eq!(back.limits.max_skill_zip_bytes, cfg.limits.max_skill_zip_bytes);
}
```

- [ ] **Step 2: Update Rust config DTOs**

Replace `RepositoryConfig` with provider-aware config:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default = "default_branch")]
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsConfig {
    #[serde(default = "default_max_skill_zip_bytes")]
    pub max_skill_zip_bytes: u64,
}
```

`AppConfig` should contain:

```rust
pub remote: RemoteConfig,
pub limits: LimitsConfig,
pub hosts: HostsConfig,
pub custom_paths: Vec<String>,
pub ignore: Vec<String>,
```

Remove `repository.local_path`, `repository.remote`, and `defaults.backup`.

- [ ] **Step 3: Expand default ignore**

Use:

```rust
fn default_ignore() -> Vec<String> {
    vec![
        "**/.git/**".into(),
        "**/node_modules/**".into(),
        "**/.env".into(),
        "**/.DS_Store".into(),
        "**/cache/**".into(),
        "**/.cache/**".into(),
        "**/tmp/**".into(),
        "**/temp/**".into(),
        "**/*.log".into(),
    ]
}
```

- [ ] **Step 4: Update TypeScript config schema**

In `src/modules/settings/schemas/config.ts`, replace `repositoryConfigSchema` and `defaults.backup` with:

```ts
export const remoteConfigSchema = z.object({
  provider: z.literal('github'),
  owner: z.string(),
  repo: z.string(),
  branch: z.string(),
})

export const limitsConfigSchema = z.object({
  max_skill_zip_bytes: z.number(),
})
```

`appConfigSchema` should include `remote`, `limits`, `hosts`, `custom_paths`, and `ignore`.

- [ ] **Step 5: Update locale keys**

Add keys for:

```json
{
  "settings.github": "...",
  "settings.githubOwner": "...",
  "settings.githubRepo": "...",
  "settings.githubBranch": "...",
  "settings.maxSkillSize": "...",
  "settings.maxSkillSizeDesc": "..."
}
```

Remove or stop using `settings.backup`.

- [ ] **Step 6: Verify**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml config
npm run typecheck
```

Expected: config tests pass; TypeScript may fail only where pages still reference removed fields. Fix those references in later UI tasks, or temporarily keep compatibility fields with `#[serde(default)]` if needed.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/config.rs src/modules/settings/schemas/config.ts src/modules/onboarding/schemas/onboarding.ts src/shared/i18n/locales
git commit -m "refactor: define github vault config"
```

### Task 3: Define Vault Manifest And Sync State

**Files:**

- Create/Modify: `src-tauri/src/vault_manifest.rs`
- Create/Modify: `src-tauri/src/sync_state.rs`
- Modify: `src-tauri/src/errors.rs`

- [ ] **Step 1: Write manifest tests**

In `vault_manifest.rs`, add tests:

```rust
#[test]
fn manifest_roundtrip_preserves_skill_entry() {
    let mut manifest = VaultManifest::empty("device-a");
    manifest.skills.insert(
        "codex:ponytail".into(),
        VaultSkill {
            id: "codex:ponytail".into(),
            name: "ponytail".into(),
            description: "desc".into(),
            platform: "codex".into(),
            platforms: vec!["codex".into()],
            hash: "sha256:abc".into(),
            blob: "blobs/sha256/abc.skill.zip".into(),
            size: 123,
            updated_at: "2026-07-07T13:00:00Z".into(),
            updated_by: "device-a".into(),
        },
    );
    let text = serde_json::to_string(&manifest).unwrap();
    let back: VaultManifest = serde_json::from_str(&text).unwrap();
    assert_eq!(back.skills["codex:ponytail"].hash, "sha256:abc");
}
```

- [ ] **Step 2: Implement manifest DTOs**

Use `BTreeMap` for deterministic serialization:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultManifest {
    pub schema: u32,
    pub updated_at: String,
    pub updated_by: String,
    pub skills: BTreeMap<String, VaultSkill>,
}
```

Add `VaultManifest::empty(device_id: &str) -> Self`.

- [ ] **Step 3: Write sync state tests**

In `sync_state.rs`, test:

```rust
#[test]
fn sync_state_roundtrip_preserves_base_hash() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = SyncState::empty(RemoteIdentity {
        provider: "github".into(),
        owner: "example".into(),
        repo: "agent-skills".into(),
        branch: "main".into(),
        commit_sha: "sha".into(),
    });
    state.skills.insert(
        "codex:ponytail".into(),
        SkillSyncState {
            base_hash: "sha256:abc".into(),
            last_remote_hash: "sha256:abc".into(),
            last_synced_at: "2026-07-07T13:00:00Z".into(),
        },
    );
    state.save_to(dir.path()).unwrap();
    let back = SyncState::load_from(dir.path()).unwrap();
    assert_eq!(back.skills["codex:ponytail"].base_hash, "sha256:abc");
}
```

- [ ] **Step 4: Implement sync state storage**

Store under app config dir:

```text
<config-dir>/skill-sync/sync_state.json
```

Add:

```rust
impl SyncState {
    pub fn load() -> Result<Self>;
    pub fn save(&self) -> Result<()>;
    pub fn empty(remote: RemoteIdentity) -> Self;
}
```

- [ ] **Step 5: Add error variants**

In `errors.rs`, add variants:

```rust
Vault(String)
RemoteChanged(String)
Auth(String)
```

Ensure they serialize cleanly through existing command error handling.

- [ ] **Step 6: Verify**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml vault_manifest sync_state
```

Expected: manifest and sync state tests pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/vault_manifest.rs src-tauri/src/sync_state.rs src-tauri/src/errors.rs
git commit -m "feat: add vault manifest and sync state"
```

## Chunk 2: Canonical Packing, Ignore, And Local Vault

### Task 4: Implement Ignore Matching

**Files:**

- Create/Modify: `src-tauri/src/ignore.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Move ignore matching tests**

Copy or move existing glob tests from `manifest.rs` into `ignore.rs`, then add:

```rust
#[test]
fn built_in_ignore_excludes_cache_and_logs() {
    assert!(is_ignored("skillA/cache/data.bin", &[]));
    assert!(is_ignored("skillA/.cache/data.bin", &[]));
    assert!(is_ignored("skillA/tmp/file", &[]));
    assert!(is_ignored("skillA/output.log", &[]));
}

#[test]
fn user_ignore_excludes_specific_skill_cache() {
    let rules = vec!["**/skillA/cache/**".to_string()];
    assert!(is_ignored("skills/skillA/cache/big.bin", &rules));
    assert!(!is_ignored("skills/skillB/cache/big.bin", &rules));
}
```

- [ ] **Step 2: Implement built-in + user ignore**

Expose:

```rust
pub fn is_ignored(rel_path: &str, user_ignore: &[String]) -> bool;
pub fn glob_match(pattern: &str, text: &str) -> bool;
pub fn default_ignore() -> Vec<String>;
```

`is_ignored` should apply built-ins first, then user rules.

- [ ] **Step 3: Verify**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml ignore
```

Expected: all ignore tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/ignore.rs src-tauri/src/lib.rs
git commit -m "feat: add vault ignore rules"
```

### Task 5: Implement SkillPacker

**Files:**

- Create/Modify: `src-tauri/src/pack.rs`
- Modify: `src-tauri/src/errors.rs`
- Test fixtures under: `src-tauri/tests/fixtures/skills/`

- [ ] **Step 1: Write packing tests first**

In `pack.rs`, add tests for:

```rust
#[test]
fn pack_hash_is_stable() { /* same files, different write order */ }

#[test]
fn ignored_cache_does_not_affect_hash_or_size() { /* mutate cache file */ }

#[test]
fn oversized_pack_is_blocked() { /* max bytes lower than zip size */ }

#[test]
fn unpack_rejects_zip_slip_paths() { /* zip entry ../evil */ }
```

- [ ] **Step 2: Implement pack result DTO**

```rust
pub struct PackedSkill {
    pub skill_id: String,
    pub hash: String,
    pub zip_path: PathBuf,
    pub zip_size: u64,
    pub ignored_files: usize,
    pub ignored_bytes: u64,
}

pub enum PackOutcome {
    Packed(PackedSkill),
    Blocked(PackBlocked),
}

pub struct PackBlocked {
    pub skill_id: String,
    pub reason: String,
    pub zip_size: u64,
    pub max_size: u64,
}
```

- [ ] **Step 3: Implement temp task directory**

Create temp dirs under:

```text
std::env::temp_dir()/skill-sync/<uuid>
```

Expose:

```rust
pub struct PackTaskDir { pub path: PathBuf }
impl PackTaskDir {
    pub fn new() -> Result<Self>;
    pub fn cleanup(self) -> Result<()>;
}
```

- [ ] **Step 4: Implement canonical zip**

Rules:

- Walk recursively.
- Normalize relative paths to `/`.
- Apply built-in + user ignore.
- Sort included files by normalized relative path.
- Set stable zip metadata where the crate allows it.
- Hash the final zip bytes as `sha256:<hex>`.
- Check `zip_size > max_skill_zip_bytes` after writing zip.

- [ ] **Step 5: Implement safe unpack**

Expose:

```rust
pub fn unpack_skill_zip(zip_bytes: &[u8], target_dir: &Path, expected_hash: &str) -> Result<()>;
```

Reject absolute paths and any path containing `..`.

- [ ] **Step 6: Verify**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml pack
```

Expected: pack tests pass, including ignored cache and zip-slip.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/pack.rs src-tauri/src/errors.rs src-tauri/tests
git commit -m "feat: pack skills as canonical zip"
```

### Task 6: Implement LocalVaultStore

**Files:**

- Create/Modify: `src-tauri/src/remote_store.rs`
- Create/Modify: `src-tauri/src/local_vault_store.rs`
- Modify: `src-tauri/src/vault_manifest.rs`

- [ ] **Step 1: Define `RemoteStore` trait**

Use synchronous methods for `LocalVaultStore`; make the trait object-safe if possible:

```rust
pub trait RemoteStore {
    fn fetch_manifest(&self) -> Result<RemoteSnapshot>;
    fn fetch_blob(&self, blob_path: &str, expected_hash: &str) -> Result<Vec<u8>>;
    fn commit_changes(&self, changes: RemoteChanges) -> Result<RemoteCommit>;
}
```

DTOs:

```rust
pub struct RemoteSnapshot {
    pub manifest: VaultManifest,
    pub commit_sha: String,
    pub branch: String,
}

pub struct RemoteChanges {
    pub base_commit_sha: String,
    pub blobs: Vec<BlobWrite>,
    pub next_manifest: VaultManifest,
    pub commit_message: String,
}
```

- [ ] **Step 2: Write LocalVaultStore tests**

Test:

```rust
#[test]
fn empty_local_vault_returns_empty_manifest() { ... }

#[test]
fn commit_changes_writes_blob_and_manifest_once() { ... }

#[test]
fn changed_base_commit_returns_remote_changed() { ... }
```

- [ ] **Step 3: Implement local vault layout**

Use:

```text
manifest.json
blobs/sha256/<hash>.skill.zip
.local-vault-commit
```

`.local-vault-commit` can be a UUID string updated after each commit to simulate GitHub branch HEAD.

- [ ] **Step 4: Verify**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml local_vault_store remote_store
```

Expected: local remote tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/remote_store.rs src-tauri/src/local_vault_store.rs src-tauri/src/vault_manifest.rs
git commit -m "feat: add local vault store"
```

## Chunk 3: Sync Engine Rewrite

### Task 7: Extend Skill Identity And Detection

**Files:**

- Modify: `src-tauri/src/skill.rs`
- Modify: `src-tauri/src/detect.rs`
- Modify: `src/modules/skills/schemas/skill.ts`

- [ ] **Step 1: Add identity tests**

In `skill.rs`:

```rust
#[test]
fn normalizes_skill_id_with_platform_prefix() {
    assert_eq!(skill_id("codex", "Pony Tail"), "codex:pony-tail");
}
```

- [ ] **Step 2: Implement normalization**

Rules:

- Lowercase.
- Trim.
- Convert spaces and underscores to `-`.
- Keep ASCII letters, numbers, and `-`.
- Collapse repeated `-`.
- Fallback to folder name if frontmatter name is missing in future parser changes.

- [ ] **Step 3: Update `Skill` DTO**

Use `platform` naming but keep `host` only if needed for frontend compatibility. Target DTO:

```rust
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub platform: String,
    pub source_path: String,
    pub hash: String,
    pub zip_size: u64,
    pub modified_at: String,
    pub enabled: bool,
}
```

- [ ] **Step 4: Update frontend skill schema**

In `src/modules/skills/schemas/skill.ts`, add `platform`, `zip_size`, and keep `host` temporarily optional if existing UI still needs it:

```ts
platform: z.string(),
host: z.string().optional(),
zip_size: z.number().optional(),
```

- [ ] **Step 5: Verify**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml skill detect
npm run typecheck
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/skill.rs src-tauri/src/detect.rs src/modules/skills/schemas/skill.ts
git commit -m "refactor: normalize skill identity"
```

### Task 8: Define New Sync Plan DTOs

**Files:**

- Modify: `src-tauri/src/sync_engine.rs`
- Modify: `src/modules/sync/schemas/syncPlan.ts`
- Modify: `src/modules/sync/api/syncApi.ts`

- [ ] **Step 1: Write schema tests manually via typecheck**

Update TS schema first and run `npm run typecheck` expecting Rust command shape mismatches to surface later.

- [ ] **Step 2: Define Rust DTOs**

Replace old action-centric DTO with card/status-centric DTO:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncStatus {
    Synced,
    LocalUpdate,
    RemoteUpdate,
    Conflict,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSkillEntry {
    pub skill_id: String,
    pub name: String,
    pub platform: String,
    pub status: SyncStatus,
    pub local_hash: Option<String>,
    pub remote_hash: Option<String>,
    pub base_hash: Option<String>,
    pub local_path: Option<String>,
    pub remote_blob: Option<String>,
    pub zip_size: Option<u64>,
    pub warnings: Vec<String>,
    pub block_reason: Option<String>,
    pub conflict: Option<Conflict>,
}
```

`SyncPlan` should contain:

```rust
pub struct SyncPlan {
    pub skills: Vec<SyncSkillEntry>,
    pub uploads: Vec<String>,
    pub downloads: Vec<String>,
    pub conflicts: Vec<Conflict>,
    pub blocked: Vec<BlockedSkill>,
    pub warnings: Vec<String>,
    pub expected_remote_commit: String,
    pub will_create_commit: bool,
    pub commit_summary: String,
    pub remote_changed: bool,
}
```

- [ ] **Step 3: Update TS schema**

Use matching string enum values:

```ts
export const syncStatusSchema = z.enum([
  'synced',
  'local_update',
  'remote_update',
  'conflict',
  'blocked',
])
```

- [ ] **Step 4: Update API functions**

In `syncApi.ts`, expose:

```ts
export const uploadSkills = async (skillIds: string[]): Promise<ApplyResult>
export const downloadSkills = async (targets: DownloadTarget[]): Promise<ApplyResult>
```

Do not wire UI yet.

- [ ] **Step 5: Verify**

Run:

```bash
npm run typecheck
```

Expected: schema compiles; page errors are allowed only if old page still expects old DTOs. If errors are broad, add temporary adapters in the page task instead of weakening schema.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/sync_engine.rs src/modules/sync/schemas/syncPlan.ts src/modules/sync/api/syncApi.ts
git commit -m "feat: define vault sync plan contracts"
```

### Task 9: Implement Three-Way Plan With LocalVaultStore

**Files:**

- Modify: `src-tauri/src/sync_engine.rs`
- Create: `src-tauri/tests/github_vault_sync.rs`

- [ ] **Step 1: Write failing integration tests**

Cover:

- Local only -> `local_update`
- Remote only -> `remote_update`
- Same local/remote hash -> `synced`
- Local changed from base -> `local_update`
- Remote changed from base -> `remote_update`
- Both changed from base -> `conflict`
- Oversized local pack -> `blocked`
- Ignored cache does not cause `local_update`

- [ ] **Step 2: Implement `build_plan` against `RemoteStore`**

Signature target:

```rust
pub fn build_plan<S: RemoteStore>(
    config: &AppConfig,
    state: &SyncState,
    store: &S,
) -> Result<SyncPlan>
```

The function should:

- Fetch remote manifest.
- Scan local skills.
- Pack/hash each local skill using temp task dir.
- Compare `base/local/remote`.
- Clean temp dir before returning.

- [ ] **Step 3: Implement status mapping**

Use design table exactly:

```text
local == remote -> synced
local != remote && remote == base -> local_update
local != remote && local == base -> remote_update
both changed -> conflict
local only -> local_update
remote only -> remote_update
```

- [ ] **Step 4: Verify**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml github_vault_sync
```

Expected: all plan-generation tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/sync_engine.rs src-tauri/tests/github_vault_sync.rs
git commit -m "feat: build github vault sync plan"
```

### Task 10: Implement Apply, Batch Upload, And Batch Download

**Files:**

- Modify: `src-tauri/src/sync_engine.rs`
- Modify: `src-tauri/tests/github_vault_sync.rs`

- [ ] **Step 1: Write apply tests**

Cover:

- Applying all local updates writes all blobs and manifest in one remote commit.
- Download-only sync does not create remote commit.
- Batch upload of 3 skills creates one remote commit.
- Batch download checks target path safety.
- Remote changed between preview and apply returns `RemoteChanged`.

- [ ] **Step 2: Implement `apply_plan`**

Target:

```rust
pub fn apply_plan<S: RemoteStore>(
    config: &AppConfig,
    decisions: &HashMap<String, String>,
    store: &S,
) -> Result<ApplyResult>
```

`ApplyResult`:

```rust
pub struct ApplyResult {
    pub applied: Vec<String>,
    pub warnings: Vec<String>,
    pub remote_commit: Option<String>,
}
```

- [ ] **Step 3: Implement batch commands internally**

Add engine functions:

```rust
pub fn upload_skills<S: RemoteStore>(skill_ids: &[String], ... ) -> Result<ApplyResult>;
pub fn download_skills<S: RemoteStore>(targets: &[DownloadTarget], ... ) -> Result<ApplyResult>;
```

Both should reuse the same plan/apply internals, not create a parallel sync path.

- [ ] **Step 4: Implement safe overwrite check**

Rules:

- If target does not exist, download can write.
- If target exists and current local hash equals base hash, download can overwrite.
- If target exists and has no base hash, return conflict/blocked.
- If target exists and current local hash differs from base, return conflict/blocked.

- [ ] **Step 5: Verify**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml github_vault_sync
```

Expected: apply, batch upload, and batch download tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/sync_engine.rs src-tauri/tests/github_vault_sync.rs
git commit -m "feat: apply github vault sync"
```

## Chunk 4: GitHub Auth And GitHub Store

### Task 11: Implement GitHub Device Flow Boundary

**Files:**

- Create/Modify: `src-tauri/src/github_auth.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add DTO tests for parsing**

In `github_auth.rs`, test deserialization of:

```rust
DeviceCodeResponse {
    device_code,
    user_code,
    verification_uri,
    expires_in,
    interval,
}
```

and token polling errors:

```text
authorization_pending
slow_down
expired_token
access_denied
```

- [ ] **Step 2: Implement commands**

Commands:

```rust
#[tauri::command]
pub async fn start_github_device_flow() -> Result<DeviceFlowStart>

#[tauri::command]
pub async fn poll_github_device_flow(device_code: String) -> Result<DeviceFlowPoll>
```

`DeviceFlowStart` should include:

```rust
pub user_code: String,
pub verification_uri: String,
pub expires_in: u64,
pub interval: u64,
pub device_code: String,
```

- [ ] **Step 3: Add client id handling**

Read OAuth client id from a development env var first:

```text
SKILL_SYNC_GITHUB_CLIENT_ID
```

If missing, return an actionable `Auth` error. Do not hardcode a fake client id.

- [ ] **Step 4: Implement token storage boundary**

Use `keyring`:

```rust
const SERVICE: &str = "skill-sync";
const GITHUB_TOKEN_USER: &str = "github-access-token";
```

Expose:

```rust
pub fn save_github_token(token: &str) -> Result<()>;
pub fn load_github_token() -> Result<Option<String>>;
pub fn clear_github_token() -> Result<()>;
```

Never log the token.

- [ ] **Step 5: Verify**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml github_auth
```

Expected: DTO and token storage tests pass where keyring is available. If CI cannot use keyring, feature-gate only storage tests.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/github_auth.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add github device flow auth"
```

### Task 12: Implement GitHubVaultStore

**Files:**

- Create/Modify: `src-tauri/src/github_store.rs`
- Modify: `src-tauri/src/remote_store.rs`
- Modify: `src-tauri/src/errors.rs`

- [ ] **Step 1: Define API client wrapper**

Create:

```rust
pub struct GitHubVaultStore {
    client: reqwest::Client,
    token: String,
    owner: String,
    repo: String,
    branch: String,
}
```

Set required headers:

```text
Authorization: Bearer <token>
Accept: application/vnd.github+json
X-GitHub-Api-Version: 2022-11-28
User-Agent: skill-sync
```

- [ ] **Step 2: Implement manifest fetch**

Use GitHub Contents or Git Data APIs to fetch:

```text
manifest.json
```

If 404, return `VaultManifest::empty(device_id)`.

- [ ] **Step 3: Implement blob fetch**

Fetch `blobs/sha256/<hash>.skill.zip`, verify downloaded bytes match expected hash, and return bytes.

- [ ] **Step 4: Implement commit changes**

Use GitHub Git Database APIs:

1. Get branch ref.
2. Get base commit/tree.
3. Create blob objects for zip files and manifest.
4. Create a tree with all changed entries.
5. Create one commit.
6. Before updating ref, ensure current branch SHA equals `base_commit_sha`.
7. Update branch ref.

Return `RemoteChanged` if branch head changed.

- [ ] **Step 5: Add integration test shape without real network**

Do not hit GitHub in automated tests. Keep `GitHubVaultStore` unit tests limited to DTO serialization and error mapping. Real API tests should be manual or gated behind:

```text
SKILL_SYNC_GITHUB_TEST_REPO
```

- [ ] **Step 6: Verify**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml github_store
```

Expected: no network required by default.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/github_store.rs src-tauri/src/remote_store.rs src-tauri/src/errors.rs
git commit -m "feat: add github vault store"
```

## Chunk 5: Tauri Commands And Legacy Removal

### Task 13: Rewire Commands To Vault Engine

**Files:**

- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/config.rs`

- [ ] **Step 1: Remove old commands**

Delete command functions and handler entries for:

```text
check_git
check_remote
prepare_repo
list_backups
restore_backup
```

- [ ] **Step 2: Add new commands**

Expose:

```rust
get_app_state()
save_config(config)
scan_skills()
get_sync_plan()
apply_sync_plan(decisions)
open_path(path)
start_github_device_flow()
poll_github_device_flow(device_code)
check_github_vault(config)
list_remote_skills()
upload_skills(skill_ids)
download_skills(targets)
```

- [ ] **Step 3: Make `get_app_state` return GitHub auth state**

Remove `git_available` and `git_version`. Add:

```rust
pub github_authorized: bool,
pub github_user: Option<String>,
```

If user lookup is not implemented yet, return `github_user: None`.

- [ ] **Step 4: Verify**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml commands
```

Expected: commands compile and old command names are gone.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/src/config.rs
git commit -m "refactor: route commands through github vault"
```

### Task 14: Delete Legacy Backend Modules

**Files:**

- Delete: `src-tauri/src/git_store.rs`
- Delete: `src-tauri/src/backup.rs`
- Delete or stop using: `src-tauri/src/manifest.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/detect.rs`
- Modify: `src-tauri/src/sync_engine.rs`

- [ ] **Step 1: Remove module declarations**

Delete:

```rust
mod backup;
mod git_store;
mod manifest;
```

- [ ] **Step 2: Replace imports**

Replace old `crate::manifest::hash_dir` usage with `crate::pack` or the new pack/hash path.

- [ ] **Step 3: Delete files**

Use `apply_patch` delete hunks for the legacy files.

- [ ] **Step 4: Verify no references remain**

Run:

```bash
rg -n "git_store|GitStore|backup::|list_backups|restore_backup|manifest::hash_dir|check_git|check_remote|prepare_repo" src-tauri src
```

Expected: no matches except documentation or migration plan.

- [ ] **Step 5: Run tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src
git commit -m "refactor: remove git cli sync backend"
```

## Chunk 6: Frontend Contracts And Sync Page

### Task 15: Update Frontend API Schemas

**Files:**

- Modify: `src/shared/schemas/apiResponse.ts`
- Modify: `src/modules/settings/schemas/config.ts`
- Modify: `src/modules/onboarding/schemas/onboarding.ts`
- Modify: `src/modules/sync/schemas/syncPlan.ts`
- Modify: `src/modules/sync/api/syncApi.ts`
- Modify: `src/modules/settings/api/configApi.ts`
- Modify: `src/modules/onboarding/api/onboardingApi.ts`

- [ ] **Step 1: Update `AppState` schema**

Remove:

```ts
git_available
git_version
```

Add:

```ts
github_authorized: z.boolean(),
github_user: z.string().nullable(),
```

- [ ] **Step 2: Add Device Flow schemas**

In onboarding schema:

```ts
export const deviceFlowStartSchema = z.object({
  device_code: z.string(),
  user_code: z.string(),
  verification_uri: z.string(),
  expires_in: z.number(),
  interval: z.number(),
})

export const deviceFlowPollSchema = z.object({
  status: z.enum(['pending', 'slow_down', 'authorized', 'expired', 'denied']),
  message: z.string().optional(),
})
```

- [ ] **Step 3: Add API functions**

In onboarding API:

```ts
export const startGithubDeviceFlow = async (): Promise<DeviceFlowStart>
export const pollGithubDeviceFlow = async (deviceCode: string): Promise<DeviceFlowPoll>
export const checkGithubVault = async (config: AppConfig): Promise<GithubVaultCheck>
```

- [ ] **Step 4: Verify**

Run:

```bash
npm run typecheck
```

Expected: schemas compile. Page errors are resolved in following tasks.

- [ ] **Step 5: Commit**

```bash
git add src/shared/schemas src/modules/settings src/modules/onboarding src/modules/sync
git commit -m "feat: add github vault frontend contracts"
```

### Task 16: Consolidate Routes

**Files:**

- Modify: `src/app/router/routeConfig.ts`
- Delete: `src/routes/app/backups/+page.svelte`
- Delete: `src/routes/app/conflicts/+page.svelte`
- Delete: `src/modules/backups/**`
- Delete: `src/modules/conflicts/**`
- Modify: `src/shared/i18n/locales/zh-CN.json`
- Modify: `src/shared/i18n/locales/en-US.json`

- [ ] **Step 1: Update route config**

Remove route entries for:

```text
/app/backups
/app/conflicts
```

Keep:

```text
/app/sync
/app/settings
/app/onboarding
```

- [ ] **Step 2: Delete modules and routes**

Delete backups and conflicts module folders and route files.

- [ ] **Step 3: Remove locale route keys**

Remove or stop using:

```json
"routes.backups"
"routes.conflicts"
"backups"
```

Keep conflict action copy under `conflicts` if Sync dialog still uses those labels, or move them under `sync.conflictDetail`.

- [ ] **Step 4: Verify references**

Run:

```bash
rg -n "modules/backups|modules/conflicts|routes.backups|routes.conflicts|/app/backups|/app/conflicts" src
```

Expected: no matches.

- [ ] **Step 5: Run checks**

```bash
npm run lint:i18n
npm run typecheck
```

- [ ] **Step 6: Commit**

```bash
git add src
git commit -m "refactor: remove standalone conflict and backup pages"
```

### Task 17: Build Sync Skill Cards And Filters

**Files:**

- Create: `src/modules/sync/lib/syncStatus.ts`
- Create: `src/modules/sync/components/SyncSkillCard.svelte`
- Create: `src/modules/sync/components/ConflictDetailDialog.svelte`
- Modify: `src/modules/sync/pages/SyncPreviewPage.svelte`
- Modify: `src/shared/i18n/locales/zh-CN.json`
- Modify: `src/shared/i18n/locales/en-US.json`

- [ ] **Step 1: Add filter helper**

`syncStatus.ts`:

```ts
export const SYNC_STATUS_FILTERS = [
  'all',
  'synced',
  'local_update',
  'remote_update',
  'conflict',
] as const

export type SyncStatusFilter = (typeof SYNC_STATUS_FILTERS)[number]
```

Add `matchesSyncFilter(entry, filter)` and `matchesSearch(entry, query)`.

- [ ] **Step 2: Create card component**

Card props:

```ts
entry: SyncSkillEntry
selected?: boolean
```

UI must show:

- Skill name.
- Platform badge.
- Status badge.
- Local hash / remote hash short values.
- Local path or remote blob.
- Warnings / blocked reason.

- [ ] **Step 3: Create conflict detail dialog**

Props:

```ts
conflict: Conflict | null
decision: string
onDecision(choice: 'local' | 'remote' | 'skip'): void
```

Do not implement file diff.

- [ ] **Step 4: Update Sync page**

Add:

- Search input.
- Status segmented control.
- Skill card grid.
- Conflict card click opens detail dialog.
- Apply button still calls `applySyncPlan(syncDecisions.decisions)`.

- [ ] **Step 5: Add locale keys**

Add keys for:

```json
{
  "sync.filters.all": "...",
  "sync.filters.synced": "...",
  "sync.filters.localUpdate": "...",
  "sync.filters.remoteUpdate": "...",
  "sync.filters.conflict": "...",
  "sync.searchPlaceholder": "...",
  "sync.blocked": "...",
  "sync.conflictDetail.title": "..."
}
```

- [ ] **Step 6: Run UI checks**

```bash
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
npm run typecheck
```

- [ ] **Step 7: Commit**

```bash
git add src/modules/sync src/shared/i18n/locales
git commit -m "feat: consolidate sync status UI"
```

## Chunk 7: Onboarding And Settings

### Task 18: Replace Onboarding With Device Flow

**Files:**

- Modify: `src/modules/onboarding/pages/OnboardingPage.svelte`
- Modify: `src/modules/onboarding/api/onboardingApi.ts`
- Modify: `src/modules/onboarding/schemas/onboarding.ts`
- Delete: `src/modules/onboarding/components/SshSetupDialog.svelte`
- Delete: `src/modules/onboarding/content/ssh-setup-prompt.md`
- Modify: `src/shared/i18n/locales/zh-CN.json`
- Modify: `src/shared/i18n/locales/en-US.json`

- [ ] **Step 1: Remove SSH UI**

Delete SSH setup dialog import and usage.

- [ ] **Step 2: Add Device Flow state**

Use local `$state`:

```ts
let deviceCode = $state('')
let userCode = $state('')
let verificationUri = $state('')
let authStatus = $state<
  'idle' | 'pending' | 'authorized' | 'expired' | 'denied' | 'error'
>('idle')
```

- [ ] **Step 3: Add connect button**

Button calls `startGithubDeviceFlow()`, displays `userCode`, and offers link/button to open `verificationUri`.

- [ ] **Step 4: Add polling**

Poll using returned interval until:

- `authorized`
- `expired`
- `denied`
- component unmount

- [ ] **Step 5: Add repo fields**

Fields:

- owner
- repo
- branch

Save into `config.remote`.

- [ ] **Step 6: Add vault check**

After authorization and repo fields are present, call `checkGithubVault(config)`. Show:

- repo accessible
- manifest exists or will be created
- branch found

- [ ] **Step 7: Run checks**

```bash
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
npm run typecheck
```

- [ ] **Step 8: Commit**

```bash
git add src/modules/onboarding src/shared/i18n/locales
git commit -m "feat: connect github vault in onboarding"
```

### Task 19: Update Settings For GitHub Vault

**Files:**

- Modify: `src/modules/settings/pages/SettingsPage.svelte`
- Modify: `src/modules/settings/schemas/config.ts`
- Modify: `src/shared/i18n/locales/zh-CN.json`
- Modify: `src/shared/i18n/locales/en-US.json`

- [ ] **Step 1: Remove backup checkbox**

Delete UI bound to `config.defaults.backup`.

- [ ] **Step 2: Add GitHub remote fields**

Add fields bound to:

```ts
config.remote.owner
config.remote.repo
config.remote.branch
```

- [ ] **Step 3: Add max size field**

Bind to:

```ts
config.limits.max_skill_zip_bytes
```

UI may show MiB and convert to bytes at save time. Keep the underlying config in bytes.

- [ ] **Step 4: Keep ignore textarea**

Continue using existing `ignore` textarea. Update label/description to mention cache examples:

```text
**/cache/**
**/skillA/cache/**
```

- [ ] **Step 5: Run checks**

```bash
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
npm run typecheck
```

- [ ] **Step 6: Commit**

```bash
git add src/modules/settings src/shared/i18n/locales
git commit -m "feat: configure github vault settings"
```

## Chunk 8: Final Cleanup And Verification

### Task 20: Remove Dead Imports, Docs Drift, And Old Copy

**Files:**

- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `docs/github-base-refactor-design.md` only if implementation details changed during execution.
- Modify: `docs/development-plan.md` if it still describes Git CLI or Backups as MVP.

- [ ] **Step 1: Search old Git CLI language**

Run:

```bash
rg -n "Git CLI|git_store|GitStore|check_git|check_remote|prepare_repo|Backups|backup|restore|SSH" README.md README.zh-CN.md docs src src-tauri
```

Expected: only historical docs or explicitly non-MVP notes remain. Remove product-facing stale references.

- [ ] **Step 2: Search old routes/modules**

```bash
rg -n "modules/backups|modules/conflicts|/app/backups|/app/conflicts" src
```

Expected: no matches.

- [ ] **Step 3: Search old command names**

```bash
rg -n "check_git|check_remote|prepare_repo|list_backups|restore_backup|upload_skill\\(|download_skill\\(" src src-tauri
```

Expected: no matches.

- [ ] **Step 4: Commit cleanup**

```bash
git add README.md README.zh-CN.md docs src src-tauri
git commit -m "docs: update github vault behavior"
```

### Task 21: Full Verification

**Files:**

- No new files unless fixes are needed.

- [ ] **Step 1: Rust tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: all Rust tests pass.

- [ ] **Step 2: Frontend typecheck**

Run:

```bash
npm run typecheck
```

Expected: typecheck passes.

- [ ] **Step 3: Formatting check**

Run:

```bash
npm run format:check
```

Expected: Prettier reports all matched files use configured style.

- [ ] **Step 4: Full lint**

Run:

```bash
npm run lint
```

Expected: typecheck, ESLint, responsive, colors, and i18n checks all pass.

- [ ] **Step 5: Build**

Run:

```bash
npm run build
```

Expected: SvelteKit/Vite build passes.

- [ ] **Step 6: Manual smoke test**

Run:

```bash
npm run tauri dev
```

Check:

- App opens to Sync.
- No Conflicts or Backups nav item exists.
- Onboarding shows GitHub Device Flow.
- Settings has GitHub repo, max size, roots, and ignore rules.
- Sync page shows search/filter/card UI.

- [ ] **Step 7: Final commit if fixes were needed**

```bash
git add .
git commit -m "fix: complete github vault verification"
```

## Execution Notes For Agents

- Do not create a local Git clone/repo as part of sync.
- Do not keep Git CLI as a fallback path.
- Do not implement Backups page or restore flow in this refactor.
- Do not implement file-level conflict diff in the first pass.
- Do not add per-skill `.skill-syncignore` in the first pass.
- Do not store GitHub tokens in YAML, JSON config, logs, or frontend state longer than necessary.
- Batch upload must produce at most one GitHub commit per operation.
- Download-only sync must produce zero GitHub commits.
- Re-run plan steps after any `RemoteChanged` handling changes; concurrency is a core invariant.
