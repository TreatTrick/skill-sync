# GitHub Vault 重构实施计划

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将当前 Git CLI / 本地中转仓库同步模型替换为 GitHub API backed vault：`manifest.json` + content-addressed skill zip blobs + 本机 `sync_state` + Device Flow 授权 + 双向同步 + A-lite 删除传播。

**Architecture:** Svelte 前端只负责展示、筛选、用户决策和调用 Tauri commands；本地文件读写、canonical zip、hash、GitHub API、三方比较、删除护栏、覆盖保护全部在 Rust。后端围绕 `SkillPacker`、`SyncStateStore`、`RemoteStore`、`GitHubVaultStore`、`LocalVaultStore` 和重写后的 `SyncEngine` 组织。

**Tech Stack:** Tauri 2, Rust 1.77+, Svelte 5, SvelteKit static SPA, TypeScript strict, Zod, i18next, Tailwind CSS v4, GitHub REST/Git Database APIs.

---

## Source Of Truth

- 设计文档：`docs/github-base-refactor-design.md`
- 删除补充设计：`C:\Users\11541\.claude\plans\docs-github-base-refactor-design-md-1-sk-federated-torvalds.md`
- 仓库规则：`AGENTS.md`

关键不变量：

- 不保留 Git CLI fallback，不再要求用户安装 Git、配置 SSH 或填写 Git remote URL。
- 远端 vault 只放 `manifest.json` 和 `blobs/sha256/<hash>.skill.zip`，不再展开 `skills/<host>/<name>`。
- hash 事实来源是 canonical zip bytes，manifest hash、本地 base 比较、blob 校验必须使用同一 hash。
- V1 做基于 base 的 advisory 删除传播，不做 tombstone。
- 删除永不静默；删除项默认不勾选，必须用户显式确认。
- root 缺失、root 不可读、扫描错误或空扫描不得推断删除。
- 删本地必须移入 trash；删云端只移除 manifest 条目，blob 不 GC。
- 下载-only、删本地-only、both_deleted 都是 0 commit；上传、删云端、保留本地重传合并为一次最多 1 个 commit。
- 删除成功后从 `sync_state.skills` 移除 base，重新导入即重新作为 `local_update` 上传。
- 第一阶段不做 Backups 页面、不做 restore flow、不做文件级 diff、不做 per-skill `.skill-syncignore`。
- GitHub token 不写入 YAML/JSON config、日志或长期前端状态。

最终验证命令：

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm run typecheck
npm run format:check
npm run lint
npm run build
```

UI、布局、颜色或文案变化后额外运行：

```bash
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
```

## Target File Map

Create:

- `src-tauri/src/ignore.rs`：内置 ignore + 用户全局 ignore。
- `src-tauri/src/pack.rs`：canonical zip、hash、size limit、zip-slip safe unpack、临时目录清理。
- `src-tauri/src/vault_manifest.rs`：云端 `manifest.json` DTO 与校验。
- `src-tauri/src/sync_state.rs`：本机 `base_hash` / `remote_commit` 状态。
- `src-tauri/src/remote_store.rs`：`RemoteStore` trait 与远端 DTO。
- `src-tauri/src/local_vault_store.rs`：测试/开发用本地 vault。
- `src-tauri/src/github_auth.rs`：GitHub Device Flow 与 token 存储边界。
- `src-tauri/src/github_store.rs`：GitHub manifest/blob/commit 适配器。
- `src-tauri/tests/github_vault_sync.rs`：基于 `LocalVaultStore` 的同步测试。
- `src/modules/sync/lib/syncStatus.ts`：同步状态筛选和搜索 helper。
- `src/modules/sync/components/SyncSkillCard.svelte`：同步状态卡片。
- `src/modules/sync/components/ConflictDetailDialog.svelte`：冲突摘要和决策弹窗。

Modify:

- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/config.rs`
- `src-tauri/src/detect.rs`
- `src-tauri/src/errors.rs`
- `src-tauri/src/skill.rs`
- `src-tauri/src/sync_engine.rs`
- `src/modules/skills/schemas/skill.ts`
- `src/modules/sync/schemas/syncPlan.ts`
- `src/modules/sync/api/syncApi.ts`
- `src/modules/sync/state/syncDecisions.svelte.ts`
- `src/modules/sync/pages/SyncPreviewPage.svelte`
- `src/modules/settings/schemas/config.ts`
- `src/modules/settings/pages/SettingsPage.svelte`
- `src/modules/onboarding/schemas/onboarding.ts`
- `src/modules/onboarding/api/onboardingApi.ts`
- `src/modules/onboarding/pages/OnboardingPage.svelte`
- `src/app/router/routeConfig.ts`
- `src/shared/i18n/locales/zh-CN.json`
- `src/shared/i18n/locales/en-US.json`
- `src/shared/schemas/apiResponse.ts`

Delete:

- `src-tauri/src/git_store.rs`
- `src-tauri/src/backup.rs`
- `src-tauri/src/manifest.rs`
- `src/modules/backups/**`
- `src/modules/conflicts/**`
- `src/routes/app/backups/+page.svelte`
- `src/routes/app/conflicts/+page.svelte`
- `src/modules/onboarding/components/SshSetupDialog.svelte`
- `src/modules/onboarding/content/ssh-setup-prompt.md`

## Chunk 1: 后端基础契约

### Task 1: 添加依赖和模块壳

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

- [ ] **Step 1: 添加 Rust 依赖**

```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
zip = { version = "2", default-features = false, features = ["deflate"] }
uuid = { version = "1", features = ["v4", "serde"] }
base64 = "0.22"
keyring = "3"
```

- [ ] **Step 2: 注册新模块**

在 `src-tauri/src/lib.rs` 加：

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

旧 `backup`、`git_store`、`manifest` 暂时保留，等 Task 17 删除。

- [ ] **Step 3: 创建可编译模块文件**

每个新文件先放最小占位内容：

```rust
// GitHub vault implementation lands in later tasks.
```

- [ ] **Step 4: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-run
```

Expected: dependency resolution succeeds；若失败，只应是后续未实现引用。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src
git commit -m "build: add github vault backend modules"
```

### Task 2: 替换 AppConfig 为 GitHub vault 配置

**Files:**

- Modify: `src-tauri/src/config.rs`
- Modify: `src/modules/settings/schemas/config.ts`
- Modify: `src/modules/onboarding/schemas/onboarding.ts`
- Modify: `src/shared/i18n/locales/zh-CN.json`
- Modify: `src/shared/i18n/locales/en-US.json`

- [ ] **Step 1: 写配置测试**

```rust
#[test]
fn default_config_uses_github_vault_remote() {
    let cfg = AppConfig::default_config();
    assert_eq!(cfg.remote.provider, "github");
    assert_eq!(cfg.remote.branch, "main");
    assert_eq!(cfg.limits.max_skill_zip_bytes, 20 * 1024 * 1024);
    assert_eq!(cfg.limits.max_auto_delete, 5);
    assert!(cfg.ignore.iter().any(|p| p == "**/cache/**"));
}

#[test]
fn config_roundtrip_preserves_ignore_and_limits() {
    let cfg = AppConfig::default_config();
    let text = serde_yaml::to_string(&cfg).unwrap();
    let back: AppConfig = serde_yaml::from_str(&text).unwrap();
    assert_eq!(back.ignore, cfg.ignore);
    assert_eq!(back.limits.max_skill_zip_bytes, cfg.limits.max_skill_zip_bytes);
    assert_eq!(back.limits.max_auto_delete, cfg.limits.max_auto_delete);
}
```

- [ ] **Step 2: 实现 Rust DTO**

替换 `RepositoryConfig` 和 `DefaultsConfig.backup`：

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
    #[serde(default = "default_max_auto_delete")]
    pub max_auto_delete: usize,
}
```

`AppConfig` 保留：

```rust
pub remote: RemoteConfig,
pub limits: LimitsConfig,
pub hosts: HostsConfig,
pub custom_paths: Vec<String>,
pub ignore: Vec<String>,
```

- [ ] **Step 3: 扩展默认 ignore**

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

- [ ] **Step 4: 更新 TS schema**

```ts
export const remoteConfigSchema = z.object({
  provider: z.literal('github'),
  owner: z.string(),
  repo: z.string(),
  branch: z.string(),
})

export const limitsConfigSchema = z.object({
  max_skill_zip_bytes: z.number(),
  max_auto_delete: z.number(),
})
```

- [ ] **Step 5: 更新文案**

新增中英文 key：

```json
{
  "settings.github": "GitHub Vault",
  "settings.githubOwner": "Owner",
  "settings.githubRepo": "Repository",
  "settings.githubBranch": "Branch",
  "settings.maxSkillSize": "单个 skill 大小上限",
  "settings.maxAutoDelete": "删除熔断阈值"
}
```

- [ ] **Step 6: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml config
npm run typecheck
```

Expected: Rust config tests pass；TS 若因页面仍引用旧字段失败，在后续 UI 任务修复，不要重新引入旧配置。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/config.rs src/modules/settings/schemas/config.ts src/modules/onboarding/schemas/onboarding.ts src/shared/i18n/locales
git commit -m "refactor: define github vault config"
```

### Task 3: 定义 VaultManifest 与 SyncState

**Files:**

- Create/Modify: `src-tauri/src/vault_manifest.rs`
- Create/Modify: `src-tauri/src/sync_state.rs`
- Modify: `src-tauri/src/errors.rs`

- [ ] **Step 1: 写 manifest round-trip 测试**

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

- [ ] **Step 2: 实现 manifest DTO**

使用 `BTreeMap` 保证稳定序列化：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultManifest {
    pub schema: u32,
    pub updated_at: String,
    pub updated_by: String,
    pub skills: BTreeMap<String, VaultSkill>,
}
```

不要在 V1 实现 tombstone；未来依靠 `schema` 版本升级。

- [ ] **Step 3: 写 sync_state 测试**

```rust
#[test]
fn sync_state_roundtrip_preserves_base_hash() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = SyncState::empty(RemoteIdentity {
        provider: "github".into(),
        owner: "example".into(),
        repo: "agent-skills".into(),
        branch: "main".into(),
        commit_sha: "github-head-sha".into(),
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

- [ ] **Step 4: 实现 sync_state 存储**

保存到：

```text
<config-dir>/skill-sync/sync_state.json
```

提供：

```rust
impl SyncState {
    pub fn load() -> Result<Self>;
    pub fn save(&self) -> Result<()>;
    pub fn empty(remote: RemoteIdentity) -> Self;
    pub fn remove_skill(&mut self, skill_id: &str);
}
```

- [ ] **Step 5: 增加错误类型**

```rust
Vault(String)
RemoteChanged(String)
Auth(String)
Blocked(String)
```

- [ ] **Step 6: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml vault_manifest sync_state
```

Expected: manifest 与 sync_state tests pass。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/vault_manifest.rs src-tauri/src/sync_state.rs src-tauri/src/errors.rs
git commit -m "feat: add vault manifest and sync state"
```

## Chunk 2: Pack、扫描与本地 Vault

### Task 4: 实现 ignore 与 SkillPacker

**Files:**

- Create/Modify: `src-tauri/src/ignore.rs`
- Create/Modify: `src-tauri/src/pack.rs`
- Modify: `src-tauri/src/errors.rs`
- Test fixtures: `src-tauri/tests/fixtures/skills/`

- [ ] **Step 1: 写 ignore 测试**

```rust
#[test]
fn built_in_ignore_excludes_cache_temp_and_logs() {
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

- [ ] **Step 2: 实现 ignore API**

```rust
pub fn is_ignored(rel_path: &str, user_ignore: &[String]) -> bool;
pub fn glob_match(pattern: &str, text: &str) -> bool;
pub fn default_ignore() -> Vec<String>;
```

- [ ] **Step 3: 写 pack 测试**

```rust
#[test]
fn pack_hash_is_stable_for_same_content() { /* same files, different write order */ }

#[test]
fn ignored_cache_does_not_affect_hash_or_size() { /* mutate ignored cache */ }

#[test]
fn oversized_pack_is_blocked() { /* max bytes lower than zip size */ }

#[test]
fn unpack_rejects_zip_slip_paths() { /* zip entry ../evil */ }
```

- [ ] **Step 4: 实现 pack DTO 和临时目录**

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

pub struct PackTaskDir {
    pub path: PathBuf,
}
```

临时目录放在：

```text
std::env::temp_dir()/skill-sync/<uuid>/
```

- [ ] **Step 5: 实现 canonical zip 与安全解包**

规则：

- 相对路径统一 `/`。
- 应用 built-in ignore + Settings 全局 ignore。
- included files 按规范化路径排序。
- hash 为 `sha256:<hex>`。
- zip 写完后按 `max_skill_zip_bytes` 判断 blocked。
- 解包拒绝绝对路径和 `..` 路径。

- [ ] **Step 6: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml ignore pack
```

Expected: ignore、pack、zip-slip、oversized tests pass。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/ignore.rs src-tauri/src/pack.rs src-tauri/src/errors.rs src-tauri/tests
git commit -m "feat: pack skills as canonical zip"
```

### Task 5: 扩展 skill 身份与扫描可读性

**Files:**

- Modify: `src-tauri/src/skill.rs`
- Modify: `src-tauri/src/detect.rs`
- Modify: `src/modules/skills/schemas/skill.ts`

- [ ] **Step 1: 写 skill id 测试**

```rust
#[test]
fn normalizes_skill_id_with_platform_prefix() {
    assert_eq!(skill_id("codex", "Pony Tail"), "codex:pony-tail");
    assert_eq!(skill_id("claude-code", "review_bot"), "claude-code:review-bot");
}
```

- [ ] **Step 2: 实现 `skill_id`**

规则：lowercase、trim、空格和 `_` 转 `-`、只保留 ASCII 字母数字和 `-`、连续 `-` 折叠。

- [ ] **Step 3: 更新 Skill DTO**

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

如前端过渡需要，`host` 可短期作为 optional compatibility field；新逻辑使用 `platform`。

- [ ] **Step 4: 扫描结果增加 root 状态**

```rust
pub struct ScanRootStatus {
    pub platform: String,
    pub root_path: String,
    pub exists: bool,
    pub readable: bool,
    pub error: Option<String>,
}
```

root 不存在或不可读时记录状态，不把该 root 下已跟踪 skill 判为删除。

- [ ] **Step 5: 更新 TS skill schema**

```ts
platform: z.string(),
host: z.string().optional(),
zip_size: z.number().optional(),
```

- [ ] **Step 6: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml skill detect
npm run typecheck
```

Expected: identity 和 scan tests pass。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/skill.rs src-tauri/src/detect.rs src/modules/skills/schemas/skill.ts
git commit -m "refactor: normalize skill identity"
```

### Task 6: 实现 RemoteStore 与 LocalVaultStore

**Files:**

- Create/Modify: `src-tauri/src/remote_store.rs`
- Create/Modify: `src-tauri/src/local_vault_store.rs`
- Modify: `src-tauri/src/vault_manifest.rs`

- [ ] **Step 1: 定义 RemoteStore**

```rust
pub trait RemoteStore {
    fn fetch_manifest(&self) -> Result<RemoteSnapshot>;
    fn fetch_blob(&self, blob_path: &str, expected_hash: &str) -> Result<Vec<u8>>;
    fn commit_changes(&self, changes: RemoteChanges) -> Result<RemoteCommit>;
}
```

核心 DTO：

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

- [ ] **Step 2: 写 LocalVaultStore 测试**

```rust
#[test]
fn empty_local_vault_returns_empty_manifest() { ... }

#[test]
fn commit_changes_writes_blob_and_manifest_once() { ... }

#[test]
fn changed_base_commit_returns_remote_changed() { ... }
```

- [ ] **Step 3: 实现本地 vault 布局**

```text
manifest.json
blobs/sha256/<hash>.skill.zip
.local-vault-commit
```

`.local-vault-commit` 每次 commit 写新 UUID，模拟 GitHub branch HEAD。

- [ ] **Step 4: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml local_vault_store remote_store
```

Expected: local vault tests pass。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/remote_store.rs src-tauri/src/local_vault_store.rs src-tauri/src/vault_manifest.rs
git commit -m "feat: add local vault store"
```

## Chunk 3: SyncEngine 与删除语义

### Task 7: 定义 SyncPlan DTO 和决策模型

**Files:**

- Modify: `src-tauri/src/sync_engine.rs`
- Modify: `src/modules/sync/schemas/syncPlan.ts`
- Modify: `src/modules/sync/api/syncApi.ts`
- Modify: `src/modules/sync/state/syncDecisions.svelte.ts`

- [ ] **Step 1: 定义 Rust status 和 conflict reason**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    Synced,
    LocalUpdate,
    RemoteUpdate,
    LocalDeleted,
    RemoteDeleted,
    Conflict,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictReason {
    SameNameFirstSeen,
    BothChanged,
    LocalDeletedRemoteChanged,
    RemoteDeletedLocalChanged,
}
```

- [ ] **Step 2: 定义 plan entry 和 plan**

```rust
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
    pub delete_direction: Option<DeleteDirection>,
    pub blocked_reason: Option<String>,
    pub warnings: Vec<String>,
}

pub struct SyncPlan {
    pub entries: Vec<SyncSkillEntry>,
    pub uploads: Vec<String>,
    pub downloads: Vec<String>,
    pub delete_remote: Vec<String>,
    pub delete_local: Vec<String>,
    pub conflicts: Vec<Conflict>,
    pub blocked: Vec<BlockedSkill>,
    pub warnings: Vec<String>,
    pub delete_guard_tripped: bool,
    pub expected_remote_commit: String,
    pub will_create_commit: bool,
    pub commit_summary: CommitSummary,
}
```

- [ ] **Step 3: 定义前端决策值**

```ts
export const syncDecisionSchema = z.enum([
  'keep_local',
  'use_remote',
  'delete_remote',
  'restore_remote',
  'accept_delete',
  'skip',
])
```

- [ ] **Step 4: 更新 TS status schema**

```ts
z.enum([
  'synced',
  'local_update',
  'remote_update',
  'local_deleted',
  'remote_deleted',
  'conflict',
  'blocked',
  'unknown',
])
```

`syncPlanSchema` 必须包含 `delete_remote`、`delete_local`、`delete_guard_tripped`。

- [ ] **Step 5: 验证**

Run:

```bash
npm run typecheck
cargo test --manifest-path src-tauri/Cargo.toml --no-run
```

Expected: schema compiles；Rust 可在 Task 8 完成实现后全绿。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/sync_engine.rs src/modules/sync
git commit -m "refactor: define vault sync plan dto"
```

### Task 8: 重写 build_plan，落地 13 行真值表

**Files:**

- Modify: `src-tauri/src/sync_engine.rs`
- Create/Modify: `src-tauri/tests/github_vault_sync.rs`

- [ ] **Step 1: 写三方比较测试**

```rust
#[test]
fn base_empty_local_only_is_local_update() { ... }

#[test]
fn base_empty_remote_only_is_remote_update() { ... }

#[test]
fn base_empty_both_same_is_synced_and_records_base() { ... }

#[test]
fn base_empty_both_different_is_same_name_conflict() { ... }

#[test]
fn local_deleted_remote_base_is_local_deleted() { ... }

#[test]
fn local_deleted_remote_changed_is_conflict() { ... }

#[test]
fn remote_deleted_local_base_is_remote_deleted() { ... }

#[test]
fn remote_deleted_local_changed_is_conflict() { ... }

#[test]
fn both_deleted_cleans_base_without_action() { ... }
```

- [ ] **Step 2: 实现 13 行状态表**

| #   | base | local | remote         | status                                   | 动作                       | commit |
| --- | ---- | ----- | -------------- | ---------------------------------------- | -------------------------- | ------ |
| 1   | `∅`  | 有    | `∅`            | `local_update`                           | 上传                       | 1      |
| 2   | `∅`  | `∅`   | 有             | `remote_update`                          | 下载                       | 0      |
| 3   | `∅`  | 有    | 有，`l==r`     | `synced`                                 | 记 base                    | 0      |
| 4   | `∅`  | 有    | 有，`l!=r`     | `conflict: same_name_first_seen`         | 用户选择                   | 视选择 |
| 5   | 有   | 有    | 有，`l==r`     | `synced`                                 | 跳过                       | 0      |
| 6   | 有   | 有    | 有，`r==b`     | `local_update`                           | 上传                       | 1      |
| 7   | 有   | 有    | 有，`l==b`     | `remote_update`                          | 下载                       | 0      |
| 8   | 有   | 有    | 有，三者互不等 | `conflict: both_changed`                 | 用户选择                   | 视选择 |
| 9   | 有   | `∅`   | 有，`r==b`     | `local_deleted`                          | 删云端 manifest 条目       | 1      |
| 10  | 有   | `∅`   | 有，`r!=b`     | `conflict: local_deleted_remote_changed` | 删云端 / 拉回本地 / 跳过   | 视选择 |
| 11  | 有   | 有    | `∅`，`l==b`    | `remote_deleted`                         | 删本地，移入 trash         | 0      |
| 12  | 有   | 有    | `∅`，`l!=b`    | `conflict: remote_deleted_local_changed` | 保留本地 / 接受删除 / 跳过 | 视选择 |
| 13  | 有   | `∅`   | `∅`            | `both_deleted`                           | 清 base 条目               | 0      |

- [ ] **Step 3: 写删除护栏测试**

```rust
#[test]
fn unreadable_root_does_not_infer_local_deletes() { ... }

#[test]
fn empty_scan_does_not_delete_tracked_skills() { ... }

#[test]
fn delete_count_over_threshold_trips_guard() { ... }

#[test]
fn delete_guard_counts_delete_side_conflicts() { ... }
```

- [ ] **Step 4: 实现 `build_plan`**

```rust
pub fn build_plan<S: RemoteStore>(
    config: &AppConfig,
    state: &SyncState,
    store: &S,
) -> Result<SyncPlan>
```

流程：

1. `store.fetch_manifest()` 获取 `RemoteSnapshot`。
2. 扫描本地 roots，带回 `ScanRootStatus`。
3. 对每个本地 skill pack/hash/size。
4. 合并 `base/local/remote`，按 13 行表判定。
5. root 缺失/不可读/扫描错误时，对该 root 下 base 中存在的 skill 标 `unknown`，不推断删除。
6. 删除数超过 `max_auto_delete` 或超过 tracked skills 的 50% 时设置 `delete_guard_tripped`。
7. 预览结束清理临时 zip 目录。
8. 不写本地、不上传、不下载、不删除。

- [ ] **Step 5: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml github_vault_sync
```

Expected: 三方比较、删除护栏、blocked size tests pass。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/sync_engine.rs src-tauri/tests/github_vault_sync.rs
git commit -m "feat: build vault sync plan with deletes"
```

### Task 9: 实现 apply、批量上传、批量下载和删除执行

**Files:**

- Modify: `src-tauri/src/sync_engine.rs`
- Modify: `src-tauri/tests/github_vault_sync.rs`

- [ ] **Step 1: 写 apply 测试**

```rust
#[test]
fn applying_local_updates_writes_one_remote_commit() { ... }

#[test]
fn download_only_creates_zero_remote_commits() { ... }

#[test]
fn local_deleted_removes_manifest_entry_in_one_commit() { ... }

#[test]
fn remote_deleted_moves_local_skill_to_trash_with_zero_commit() { ... }

#[test]
fn both_deleted_removes_sync_state_base() { ... }

#[test]
fn batch_upload_and_delete_remote_share_one_commit() { ... }

#[test]
fn remote_changed_between_preview_and_apply_returns_remote_changed() { ... }
```

- [ ] **Step 2: 实现 `apply_plan`**

```rust
pub fn apply_plan<S: RemoteStore>(
    config: &AppConfig,
    decisions: &HashMap<String, SyncDecision>,
    store: &S,
) -> Result<ApplyResult>
```

`ApplyResult` 不再返回 backups：

```rust
pub struct ApplyResult {
    pub applied: Vec<String>,
    pub warnings: Vec<String>,
    pub remote_commit: Option<String>,
}
```

- [ ] **Step 3: 执行前重新生成计划**

apply 必须重新 fetch manifest、重新 scan/pack/hash、重新 build plan，然后合并用户 decisions。预览阶段 zip 不允许复用。

- [ ] **Step 4: 实现下载覆盖保护**

- 目标不存在则写入。
- 目标存在且当前 local hash == base hash 才允许覆盖。
- 目标存在但无 base，或 local hash != base，返回 conflict/blocked，不静默覆盖。

- [ ] **Step 5: 实现上传和删云端**

上传写入 blob 并更新 `next_manifest.skills`。删云端只从 `next_manifest.skills` 移除条目，不删除 blob，并与上传合并为一次 `commit_changes`。

- [ ] **Step 6: 实现删本地进 trash**

路径：

```text
<config-dir>/skill-sync/trash/<task-id>/<skill_id>/
```

Windows 下将 `:` 转为安全片段，例如 `codex__ponytail`。

- [ ] **Step 7: 更新 sync_state**

- 成功 upload/download 后写入新的 `base_hash`。
- `local_deleted` 删云端成功后移除 base。
- `remote_deleted` 删本地成功后移除 base。
- `both_deleted` 静默移除 base。
- skip 或 blocked 不改 base。

- [ ] **Step 8: 实现批量内部函数**

```rust
pub fn upload_skills<S: RemoteStore>(skill_ids: &[String], ... ) -> Result<ApplyResult>;
pub fn download_skills<S: RemoteStore>(targets: &[DownloadTarget], ... ) -> Result<ApplyResult>;
```

复用 plan/apply，不开平行同步流程。

- [ ] **Step 9: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml github_vault_sync
```

Expected: apply、删除、批量、RemoteChanged tests pass。

- [ ] **Step 10: Commit**

```bash
git add src-tauri/src/sync_engine.rs src-tauri/tests/github_vault_sync.rs
git commit -m "feat: apply vault sync plan with deletes"
```

## Chunk 4: GitHub 与 Tauri Commands

### Task 10: 实现 GitHub Device Flow

**Files:**

- Create/Modify: `src-tauri/src/github_auth.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写 DTO 测试**

覆盖 `device_code`、`user_code`、`verification_uri`、`expires_in`、`interval`，以及 polling 错误：`authorization_pending`、`slow_down`、`expired_token`、`access_denied`。

- [ ] **Step 2: 实现 commands**

```rust
#[tauri::command]
pub async fn start_github_device_flow() -> Result<DeviceFlowStart>;

#[tauri::command]
pub async fn poll_github_device_flow(device_code: String) -> Result<DeviceFlowPoll>;
```

- [ ] **Step 3: 处理 client id**

从环境变量读取：

```text
SKILL_SYNC_GITHUB_CLIENT_ID
```

缺失时返回可操作的 `Auth` 错误，不硬编码假 client id。

- [ ] **Step 4: 实现 token storage**

使用 `keyring`：

```rust
pub fn save_github_token(token: &str) -> Result<()>;
pub fn load_github_token() -> Result<Option<String>>;
pub fn clear_github_token() -> Result<()>;
```

禁止 log token。

- [ ] **Step 5: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml github_auth
```

Expected: DTO tests pass；keyring 如 CI 不可用，只 gate storage tests。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/github_auth.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add github device flow auth"
```

### Task 11: 实现 GitHubVaultStore

**Files:**

- Create/Modify: `src-tauri/src/github_store.rs`
- Modify: `src-tauri/src/remote_store.rs`
- Modify: `src-tauri/src/errors.rs`

- [ ] **Step 1: 定义 GitHub client**

```rust
pub struct GitHubVaultStore {
    client: reqwest::Client,
    token: String,
    owner: String,
    repo: String,
    branch: String,
    device_id: String,
}
```

headers：`Authorization`、`Accept: application/vnd.github+json`、`X-GitHub-Api-Version: 2022-11-28`、`User-Agent: skill-sync`。

- [ ] **Step 2: 实现 manifest fetch**

读取 branch HEAD 与 `manifest.json`。404 时返回 `VaultManifest::empty(&device_id)`。

- [ ] **Step 3: 实现 blob fetch**

读取 `blobs/sha256/<hash>.skill.zip`，校验 bytes hash == expected hash。

- [ ] **Step 4: 实现 commit_changes**

使用 Git Database APIs：get ref、get commit/tree、create blobs、create tree、create commit、更新 ref 前校验 HEAD 仍等于 `base_commit_sha`、update ref。HEAD 变化返回 `RemoteChanged`。

- [ ] **Step 5: 不打真实网络测试**

默认只测 DTO、error mapping、request building。真实 GitHub 测试只能手动或 gated：

```text
SKILL_SYNC_GITHUB_TEST_REPO
```

- [ ] **Step 6: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml github_store
```

Expected: 默认不访问网络。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/github_store.rs src-tauri/src/remote_store.rs src-tauri/src/errors.rs
git commit -m "feat: add github vault store"
```

### Task 12: 重接 Tauri commands

**Files:**

- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/config.rs`

- [ ] **Step 1: 删除旧 commands**

删除：

```text
check_git
check_remote
prepare_repo
list_backups
restore_backup
```

- [ ] **Step 2: 暴露新 commands**

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

- [ ] **Step 3: 更新 AppState**

移除 `git_available` / `git_version`，新增：

```rust
pub github_authorized: bool,
pub github_user: Option<String>,
```

- [ ] **Step 4: 验证**

Run:

```bash
rg -n "check_git|check_remote|prepare_repo|list_backups|restore_backup" src-tauri/src
cargo test --manifest-path src-tauri/Cargo.toml commands
```

Expected: `rg` 无产品代码命中；commands compile。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/src/config.rs
git commit -m "refactor: route commands through github vault"
```

## Chunk 5: 前端契约与页面

### Task 13: 更新前端 API schemas

**Files:**

- Modify: `src/shared/schemas/apiResponse.ts`
- Modify: `src/modules/settings/schemas/config.ts`
- Modify: `src/modules/onboarding/schemas/onboarding.ts`
- Modify: `src/modules/sync/schemas/syncPlan.ts`
- Modify: `src/modules/sync/api/syncApi.ts`
- Modify: `src/modules/settings/api/configApi.ts`
- Modify: `src/modules/onboarding/api/onboardingApi.ts`

- [ ] **Step 1: 更新 AppState schema**

移除 `git_available` / `git_version`，新增：

```ts
github_authorized: z.boolean(),
github_user: z.string().nullable(),
```

- [ ] **Step 2: 添加 Device Flow schema**

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

- [ ] **Step 3: 添加 API 函数**

```ts
export const startGithubDeviceFlow = async (): Promise<DeviceFlowStart>;
export const pollGithubDeviceFlow = async (deviceCode: string): Promise<DeviceFlowPoll>;
export const checkGithubVault = async (config: AppConfig): Promise<GithubVaultCheck>;
export const uploadSkills = async (skillIds: string[]): Promise<ApplyResult>;
export const downloadSkills = async (targets: DownloadTarget[]): Promise<ApplyResult>;
```

- [ ] **Step 4: 验证**

Run:

```bash
npm run typecheck
```

Expected: schemas compile；页面错误在后续任务修复。

- [ ] **Step 5: Commit**

```bash
git add src/shared/schemas src/modules/settings src/modules/onboarding src/modules/sync
git commit -m "feat: add github vault frontend contracts"
```

### Task 14: 移除独立 Backups / Conflicts 路由

**Files:**

- Modify: `src/app/router/routeConfig.ts`
- Delete: `src/routes/app/backups/+page.svelte`
- Delete: `src/routes/app/conflicts/+page.svelte`
- Delete: `src/modules/backups/**`
- Delete: `src/modules/conflicts/**`
- Modify: `src/shared/i18n/locales/zh-CN.json`
- Modify: `src/shared/i18n/locales/en-US.json`

- [ ] **Step 1: 更新 route config**

删除 `/app/backups` 和 `/app/conflicts`，保留 `/app/sync`、`/app/settings`、`/app/onboarding`。

- [ ] **Step 2: 删除模块和路由文件**

使用 `apply_patch` 删除 backups/conflicts module folders 与 route files。

- [ ] **Step 3: 清理 locale route keys**

删除或停止使用：

```json
"routes.backups"
"routes.conflicts"
"backups"
```

冲突文案如仍被 Sync 弹窗使用，迁移到 `sync.conflictDetail.*`。

- [ ] **Step 4: 验证**

Run:

```bash
rg -n "modules/backups|modules/conflicts|routes.backups|routes.conflicts|/app/backups|/app/conflicts" src
npm run lint:i18n
npm run typecheck
```

Expected: `rg` no matches；i18n 与 typecheck pass。

- [ ] **Step 5: Commit**

```bash
git add src
git commit -m "refactor: remove standalone conflict and backup pages"
```

### Task 15: 构建 Sync 卡片、筛选、删除确认和冲突详情

**Files:**

- Create: `src/modules/sync/lib/syncStatus.ts`
- Create: `src/modules/sync/components/SyncSkillCard.svelte`
- Create: `src/modules/sync/components/ConflictDetailDialog.svelte`
- Modify: `src/modules/sync/pages/SyncPreviewPage.svelte`
- Modify: `src/modules/sync/state/syncDecisions.svelte.ts`
- Modify: `src/shared/i18n/locales/zh-CN.json`
- Modify: `src/shared/i18n/locales/en-US.json`

- [ ] **Step 1: 添加筛选 helper**

```ts
export const SYNC_STATUS_FILTERS = [
  'all',
  'synced',
  'local_update',
  'remote_update',
  'deleted',
  'conflict',
] as const
```

`deleted` 匹配 `local_deleted` 和 `remote_deleted`。

- [ ] **Step 2: 创建 SyncSkillCard**

Props:

```ts
entry: SyncSkillEntry
selected?: boolean
requiresConfirmation?: boolean
```

卡片显示 skill name、platform badge、status badge、删除方向 badge、local/remote/base hash 短值、路径、warnings、blocked reason。

- [ ] **Step 3: 创建 ConflictDetailDialog**

Props:

```ts
conflict: Conflict | null
decision: SyncDecision | ''
onDecision(choice: SyncDecision): void
```

不做文件级 diff。

- [ ] **Step 4: 冲突决策按 reason 分组**

普通冲突：

```text
keep_local -> 上传本地版本，产生 commit
use_remote -> 下载云端版本，0 commit
skip -> 不处理
```

本地删除、云端改动：

```text
delete_remote -> 移除云端条目，产生 commit
restore_remote -> 下载云端新版本，0 commit
skip -> 不处理
```

云端删除、本地改动：

```text
keep_local -> 重传本地版本，产生 commit
accept_delete -> 本地移入 trash，0 commit
skip -> 不处理
```

- [ ] **Step 5: 更新 Sync 页面**

添加搜索框、状态筛选、card grid、delete guard warning、删除项显式确认、commit summary。删除项不默认勾选。Apply 调用 `applySyncPlan(syncDecisions.decisions)`。

- [ ] **Step 6: 更新文案**

新增中英文 key：

```json
{
  "sync.filters.deleted": "删除",
  "sync.deleteGuard.title": "删除数量异常",
  "sync.deleteGuard.description": "本次计划包含较多删除，请确认 skill root 可读且删除符合预期。",
  "sync.deleteRemote": "删云端",
  "sync.deleteLocal": "删本地",
  "sync.confirmDelete": "确认删除",
  "sync.conflictDetail.localDeletedRemoteChanged": "本地已删除，但云端已修改",
  "sync.conflictDetail.remoteDeletedLocalChanged": "云端已删除，但本地已修改"
}
```

- [ ] **Step 7: 验证**

```bash
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
npm run typecheck
```

- [ ] **Step 8: Commit**

```bash
git add src/modules/sync src/shared/i18n/locales
git commit -m "feat: consolidate sync status ui"
```

### Task 16: 更新 Onboarding 与 Settings

**Files:**

- Modify: `src/modules/onboarding/pages/OnboardingPage.svelte`
- Modify: `src/modules/onboarding/api/onboardingApi.ts`
- Modify: `src/modules/onboarding/schemas/onboarding.ts`
- Delete: `src/modules/onboarding/components/SshSetupDialog.svelte`
- Delete: `src/modules/onboarding/content/ssh-setup-prompt.md`
- Modify: `src/modules/settings/pages/SettingsPage.svelte`
- Modify: `src/modules/settings/schemas/config.ts`
- Modify: `src/shared/i18n/locales/zh-CN.json`
- Modify: `src/shared/i18n/locales/en-US.json`

- [ ] **Step 1: 删除 SSH/Git onboarding UI**

删除 `SshSetupDialog`、Git 检测、remote URL 检测、SSH 教程入口。

- [ ] **Step 2: 添加 Device Flow 状态**

```ts
let deviceCode = $state('')
let userCode = $state('')
let verificationUri = $state('')
let authStatus = $state<
  'idle' | 'pending' | 'authorized' | 'expired' | 'denied' | 'error'
>('idle')
```

- [ ] **Step 3: 添加连接、轮询和 repo 检测**

连接按钮调用 `startGithubDeviceFlow()`，展示 `userCode` 和打开 `verificationUri`。按 interval 轮询到 authorized/expired/denied。授权成功且 owner/repo/branch 完整后调用 `checkGithubVault(config)`。

- [ ] **Step 4: 更新 Settings**

删除 `config.defaults.backup` checkbox，新增：

```ts
config.remote.owner
config.remote.repo
config.remote.branch
config.limits.max_skill_zip_bytes
config.limits.max_auto_delete
```

保留 ignore textarea，文案说明它影响 zip/hash/size/status。

- [ ] **Step 5: 验证**

```bash
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
npm run typecheck
```

- [ ] **Step 6: Commit**

```bash
git add src/modules/onboarding src/modules/settings src/shared/i18n/locales
git commit -m "feat: connect github vault settings"
```

## Chunk 6: 旧模型删除与最终验证

### Task 17: 删除旧 Git working tree 与备份后端

**Files:**

- Delete: `src-tauri/src/git_store.rs`
- Delete: `src-tauri/src/backup.rs`
- Delete: `src-tauri/src/manifest.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/detect.rs`
- Modify: `src-tauri/src/sync_engine.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: 删除 module declarations**

```rust
mod backup;
mod git_store;
mod manifest;
```

- [ ] **Step 2: 替换旧引用**

```text
crate::manifest::hash_dir -> crate::pack / packed local hash
crate::git_store::GitStore -> RemoteStore / GitHubVaultStore
crate::backup::* -> trash move logic in sync_engine
```

- [ ] **Step 3: 删除旧文件**

用 `apply_patch` 删除 legacy files。

- [ ] **Step 4: 验证无残留**

Run:

```bash
rg -n "git_store|GitStore|backup::|list_backups|restore_backup|manifest::hash_dir|check_git|check_remote|prepare_repo|skill-sync.lock.json" src-tauri src
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: `rg` 无产品代码命中；Rust tests pass。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src
git commit -m "refactor: remove git cli sync backend"
```

### Task 18: 更新 README / docs 并全量验证

**Files:**

- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `docs/development-plan.md`
- Modify: `docs/github-base-refactor-design.md` only if implementation details changed during execution.

- [ ] **Step 1: 搜索旧产品描述**

Run:

```bash
rg -n "Git CLI|git_store|GitStore|check_git|check_remote|prepare_repo|Backups|backup|restore|SSH|skill-sync.lock.json|skills/<host>/<name>" README.md README.zh-CN.md docs src src-tauri
```

Expected: 只有历史上下文或明确非 MVP 注记保留。

- [ ] **Step 2: 更新文档**

README 和开发文档说明 GitHub private repo vault、Device Flow、`manifest.json` + `blobs/sha256`、advisory 删除传播、本地 trash、blob 不 GC、不需要 Git/SSH。

- [ ] **Step 3: 全量验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm run typecheck
npm run format:check
npm run lint
npm run build
```

Expected: all pass。

- [ ] **Step 4: 手动烟测**

Run:

```bash
npm run tauri dev
```

检查：

- App opens to Sync。
- Nav 没有 Conflicts 或 Backups。
- Onboarding 展示 GitHub Device Flow。
- Settings 有 GitHub repo、size limit、delete guard、roots、ignore rules。
- Sync 页面有搜索、筛选、卡片、删除 badge、delete guard warning。
- 删除项不默认勾选，必须显式确认。
- 冲突详情能展示普通冲突和两类删改冲突。

- [ ] **Step 5: Commit**

```bash
git add README.md README.zh-CN.md docs src src-tauri
git commit -m "docs: update github vault behavior"
```

## Execution Notes For Agents

- 实施本计划前使用 `superpowers:executing-plans`；如果环境允许且用户明确授权 subagents，则使用 `superpowers:subagent-driven-development`。
- 每个任务按测试驱动执行：先写失败测试，再实现最小代码，再验证。
- 不要创建本地 Git clone/repo 作为同步实现。
- 不要保留 Git CLI fallback。
- 不要实现 Backups 页面或 restore flow。
- 不要实现文件级 conflict diff。
- 不要新增 per-skill `.skill-syncignore`。
- 不要把 GitHub token 写入 YAML、JSON config、日志或长期前端状态。
- 不要把 tombstone 当作 V1 功能实现。
- 删除不 apply 不生效。
- 空扫描禁止删除。
- 删本地进 trash，不硬删。
- 删云端只移除 manifest entry，blob 不 GC。
- 批量上传与批量删除远端条目一次最多 1 个 GitHub commit。
- 下载-only 和删本地-only 一律 0 个 GitHub commit。
- `RemoteChanged` 后必须重新 fetch manifest 并重新生成计划，不要覆盖远端 HEAD。
