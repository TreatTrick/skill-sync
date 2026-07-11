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
- Apply 必须回传预览的 `expected_remote_commit` 与 `plan_fingerprint`，并显式列出 `selected_action_ids`；旧计划的选择和冲突决策不得套用到新计划。
- `delete_guard_tripped` 时，后端必须校验同一计划指纹对应的 `delete_guard_ack`，不能只依赖前端提示。
- root 缺失、root 不可读、扫描错误或空扫描不得推断删除。
- 只管理三个固定用户级 namespace：`agents -> ~/.agents/skills`、`codex -> ~/.codex/skills`、`claude-code -> ~/.claude/skills`；用户不能配置、禁用或新增 root。
- 不扫描项目级 skill 目录；`~/.codex/skills/.system` 与不含合法 `SKILL.md` 的直接子目录不参与同步。
- 同 namespace ID/folder collision 必须 blocked，不得保留扫描顺序中的第一个。
- 删本地必须移入 trash；删云端只移除 manifest 条目，blob 不 GC。
- 下载-only、删本地-only、both_deleted 都是 0 commit；上传、删云端、保留本地重传合并为一次最多 1 个 commit。
- 预览只生成 `base_adoptions` / `base_removals`，不得写 sync_state；Apply 即使没有选中动作，也必须能以 0 GitHub commit 落盘这些本机状态协调。
- 删除成功后从 `sync_state.skills` 移除 base，重新导入即重新作为 `local_update` 上传。
- 第一阶段不做 Backups 页面、不做 restore flow、不做文件级 diff、不做 per-skill `.skill-syncignore`。
- GitHub token 不写入 YAML/JSON config、日志或长期前端状态。
- RemoteStore、SyncEngine 远端调用链和同步相关 Tauri commands 统一 async；阻塞文件/CPU 工作只通过 `tauri::async_runtime::spawn_blocking` 执行，不使用 reqwest blocking client、自建 runtime 或 block_on。

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
async-trait = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "sync"] }
zip = { version = "2", default-features = false, features = ["deflate"] }
uuid = { version = "1", features = ["v4", "serde"] }
base64 = "0.22"
keyring = "3"
unicode-normalization = "0.1"

[dev-dependencies]
wiremock = "0.6"
```

`wiremock` 加入现有 `[dev-dependencies]`（与 `tempfile` 同一 section），不要创建重复的 Cargo table。

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
- Create: `src-tauri/tests/fixtures/config-v1.yaml`

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

#[test]
fn legacy_hosts_and_custom_paths_are_ignored_on_migration() {
    let legacy = include_str!("../tests/fixtures/config-v1.yaml");
    let cfg = AppConfig::from_yaml_with_migration(legacy).unwrap();
    let serialized = serde_yaml::to_string(&cfg).unwrap();
    assert!(!serialized.contains("hosts:"));
    assert!(!serialized.contains("custom_paths:"));
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

`AppConfig` 只保留：

```rust
pub remote: RemoteConfig,
pub limits: LimitsConfig,
pub ignore: Vec<String>,
```

删除 `HostsConfig`、`HostConfig`、`hosts`、`custom_paths`、`enabled_hosts()` 和默认可编辑路径。配置版本递增；加载旧 YAML 时只迁移 remote/ignore/limits 所需字段，旧 root 字段忽略且保存后消失。固定 root registry 属于 `detect.rs` 的产品规则，不序列化到 AppConfig。

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

- [ ] **Step 4: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml config
```

Expected: Rust config 与 migration tests pass。前端 schema 和所有消费者在 Task 16 同一提交中原子迁移，避免中间提交留下无法通过 typecheck 的状态。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/config.rs src-tauri/tests/fixtures/config-v1.yaml
git commit -m "refactor: define github vault config"
```

### Task 3: 定义 VaultManifest 与 SyncState

**Files:**

- Create/Modify: `src-tauri/src/vault_manifest.rs`
- Create/Modify: `src-tauri/src/sync_state.rs`
- Modify: `src-tauri/src/skill.rs`
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
            namespace: SkillNamespace::Codex,
            folder_name: "ponytail".into(),
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

#[test]
fn manifest_rejects_key_id_namespace_and_unsafe_folder_mismatches() {
    for invalid in manifest_validation_fixtures() {
        assert!(VaultManifest::parse_validated(&invalid).is_err());
    }
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

先在 `skill.rs` 定义共享 `SkillNamespace` enum（serde 值 `agents`、`codex`、`claude-code`），供 manifest、sync_state、scanner 和 SyncPlan 复用。`VaultSkill` 使用单个 `namespace` 和 `folder_name`，删除旧 `platform/platforms`。所有远端 JSON 必须通过唯一入口 `VaultManifest::parse_validated(bytes)` 解析；不要让 adapter 直接调用可绕过校验的 `serde_json::from_slice::<VaultManifest>`。该入口验证 map key == id、id 为规范化的 `<namespace>:<name>`、id namespace == entry namespace，并拒绝不安全 folder_name（空、`.`、`..`、绝对路径或含 `/`、`\`）。负例逐项覆盖上述约束。

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
            namespace: SkillNamespace::Codex,
            relative_dir: "ponytail".into(),
        },
    );
    state.save_to(dir.path()).unwrap();
    let back = SyncState::load_from(dir.path()).unwrap();
    assert_eq!(back.skills["codex:ponytail"].base_hash, "sha256:abc");
    assert_eq!(back.remote, state.remote);
    assert_eq!(back.skills["codex:ponytail"].namespace, SkillNamespace::Codex);
    assert_eq!(back.skills["codex:ponytail"].relative_dir, "ponytail");
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
    pub fn load_from(config_dir: &Path) -> Result<Self>;
    pub fn save_to(&self, config_dir: &Path) -> Result<()>;
    pub fn load() -> Result<Self>;
    pub fn save(&self) -> Result<()>;
    pub fn empty(remote: RemoteIdentity) -> Self;
    pub fn remove_skill(&mut self, skill_id: &str);
}
```

`load_from/save_to` 是可注入 config directory 的核心实现，测试和 command 都复用它；`load/save` 只负责解析系统 config directory 后调用核心实现，不复制读写逻辑。保存使用同目录临时文件 + rename，确保 remote identity、`namespace` 和 `relative_dir` 在一次原子写入中持久化。

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
git add src-tauri/src/vault_manifest.rs src-tauri/src/sync_state.rs src-tauri/src/skill.rs src-tauri/src/errors.rs
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
fn pack_bytes_are_stable_across_write_order_mtime_and_permissions() {
    let a = skill_fixture_with_files([("SKILL.md", "name: demo"), ("scripts/run.sh", "echo ok")]);
    let b = skill_fixture_with_files([("scripts/run.sh", "echo ok"), ("SKILL.md", "name: demo")]);
    set_distinct_mtime_and_permissions(a.path(), b.path()).unwrap();
    let first = pack_fixture(a.path(), 1024 * 1024).unwrap();
    let second = pack_fixture(b.path(), 1024 * 1024).unwrap();
    assert_eq!(fs::read(first.zip_path()).unwrap(), fs::read(second.zip_path()).unwrap());
    assert_eq!(first.hash(), second.hash());
}

#[test]
fn ignored_cache_does_not_affect_hash_or_size() {
    let skill = skill_fixture_with_files([("SKILL.md", "name: demo"), ("cache/data", "one")]);
    let first = pack_fixture(skill.path(), 1024 * 1024).unwrap();
    fs::write(skill.path().join("cache/data"), "different and larger").unwrap();
    let second = pack_fixture(skill.path(), 1024 * 1024).unwrap();
    assert_eq!((first.hash(), first.zip_size()), (second.hash(), second.zip_size()));
}

#[test]
fn oversized_pack_is_blocked() {
    let skill = skill_fixture_with_files([("SKILL.md", "name: demo"), ("payload.bin", "not-empty")]);
    assert!(matches!(pack_fixture_outcome(skill.path(), 1).unwrap(), PackOutcome::Blocked(_)));
}

#[test]
fn unpack_rejects_zip_slip_paths() {
    let zip = zip_fixture([("../evil", b"bad".as_slice())]);
    let target = tempfile::tempdir().unwrap();
    assert!(matches!(unpack_skill(zip.path(), target.path()), Err(AppError::Blocked(_))));
    assert!(!target.path().join("../evil").exists());
}

#[test]
fn pack_warns_about_probable_secrets_without_blocking() {
    let skill = skill_fixture_with_files([
        ("SKILL.md", "name: demo"),
        ("id.pem", "-----BEGIN PRIVATE KEY-----\nredacted\n-----END PRIVATE KEY-----"),
        ("token.txt", "github_pat_REDACTED_TEST_VALUE"),
    ]);
    let packed = pack_fixture(skill.path(), 1024 * 1024).unwrap();
    assert_eq!(packed.warnings().len(), 2);
    assert!(packed.warnings().iter().all(|w| !format!("{w:?}").contains("REDACTED_TEST_VALUE")));
}

#[test]
fn pack_task_dir_is_removed_on_success_error_and_early_return() {
    for exit in [PackTestExit::Success, PackTestExit::Error, PackTestExit::EarlyReturn] {
        let path = run_pack_scope(exit);
        assert!(!path.exists(), "task dir leaked for {exit:?}");
    }
}
```

测试模块实现 `skill_fixture_with_files`、`set_distinct_mtime_and_permissions`、`zip_fixture` 和 `pack_fixture`，但断言必须保持如上，不得用“函数被调用”替代 bytes/hash、warning redaction 和目录不存在的结果验证。

- [ ] **Step 4: 实现 pack DTO 和临时目录**

```rust
pub struct PackedSkill {
    pub skill_id: String,
    pub hash: String,
    pub zip_path: PathBuf,
    pub zip_size: u64,
    pub ignored_files: usize,
    pub ignored_bytes: u64,
    pub warnings: Vec<PackWarning>,
}

pub struct PackWarning {
    pub relative_path: String,
    pub kind: SecretWarningKind,
}

pub enum PackOutcome {
    Packed(PackedSkill),
    Blocked(PackBlocked),
}

pub struct PackTaskDir {
    pub path: PathBuf,
}

pub struct PackBatch {
    task_dir: PackTaskDir,
    pub outcomes: Vec<PackOutcome>,
}

impl SkillPacker {
    pub fn pack_batch(inputs: &[SkillPackInput], options: &PackOptions) -> Result<PackBatch>;
}
```

临时目录放在：

```text
std::env::temp_dir()/skill-sync/<uuid>/
```

`PackTaskDir` 拥有目录并实现 `Drop` 递归清理；成功路径、错误路径和提前返回都依赖同一 RAII 语义。`SkillPacker::pack_batch` 返回同时拥有 outcomes 与 task dir 的 `PackBatch`，因此返回时 zip 仍存在。`PreparedSyncPlan` 必须持有整个 `PackBatch`；不能只复制 `zip_path`，也不能从 batch 提前取出 task dir。preview 返回 DTO 或 apply 结束时 batch drop，统一清理目录。

- [ ] **Step 5: 实现 canonical zip 与安全解包**

规则：

- 相对路径统一 `/`。
- 应用 built-in ignore + Settings 全局 ignore。
- included files 按规范化路径排序。
- 只写文件 entry，不写显式目录 entry；entry timestamp 固定为 ZIP 可表示的常量，Unix mode 固定为普通文件 `0o644`，清除平台 extra attributes/comment。
- 压缩算法和 level 固定，CRC 与 size 由同一 writer 计算；本机 mtime、ctime、owner、ACL 和原始权限不得进入 zip bytes。
- hash 为 `sha256:<hex>`。
- zip 写完后按 `max_skill_zip_bytes` 判断 blocked。
- 解包拒绝绝对路径和 `..` 路径。
- 对 included text files 扫描私钥 header、PEM block 和常见 token 格式，产生带相对路径与类型的结构化 `PackWarning`；warning 进入 SyncPlan 展示但不阻止上传，也不包含命中的 secret 原文。

- [ ] **Step 6: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml ignore pack
```

Expected: ignore、canonical bytes、secret warning、RAII cleanup、zip-slip、oversized tests pass。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/ignore.rs src-tauri/src/pack.rs src-tauri/src/errors.rs src-tauri/tests
git commit -m "feat: pack skills as canonical zip"
```

### Task 5: 实现三个固定 namespace 与扫描边界

**Files:**

- Modify: `src-tauri/src/skill.rs`
- Modify: `src-tauri/src/detect.rs`
- Modify: `src/modules/skills/schemas/skill.ts`

- [ ] **Step 1: 写 skill id 测试**

```rust
#[test]
fn normalizes_skill_id_with_namespace_prefix() {
    assert_eq!(skill_id(SkillNamespace::Agents, "Pony Tail"), "agents:pony-tail");
    assert_eq!(skill_id(SkillNamespace::Codex, "Pony Tail"), "codex:pony-tail");
    assert_eq!(skill_id(SkillNamespace::ClaudeCode, "review_bot"), "claude-code:review-bot");
}

#[test]
fn identity_uses_non_empty_frontmatter_name_then_folder_fallback() {
    assert_eq!(skill_id_from_metadata(SkillNamespace::Agents, Some("Pony Tail"), "folder").unwrap(), "agents:pony-tail");
    assert_eq!(skill_id_from_metadata(SkillNamespace::Agents, None, "Folder Name").unwrap(), "agents:folder-name");
    assert_eq!(skill_id_from_metadata(SkillNamespace::Agents, Some("   "), "Folder Name").unwrap(), "agents:folder-name");
    assert!(skill_id_from_metadata(SkillNamespace::Agents, Some("!!!"), "___").is_err());
}

#[test]
fn fixed_roots_resolve_from_home_and_cannot_be_configured() {
    let roots = fixed_skill_roots(Path::new("/home/test"));
    assert_eq!(root_pairs(roots), [
        (SkillNamespace::Agents, ".agents/skills"),
        (SkillNamespace::Codex, ".codex/skills"),
        (SkillNamespace::ClaudeCode, ".claude/skills"),
    ]);
}

#[test]
fn scanner_excludes_project_roots_codex_system_and_non_skills() {
    let result = scan_fixed_roots(fixed_root_fixture()).unwrap();
    assert_eq!(result.skill_ids(), ["agents:valid", "codex:valid", "claude-code:valid"]);
    assert!(!result.paths().any(|p| p.contains("project") || p.contains(".system") || p.contains("aggregate")));
}

#[test]
fn duplicate_normalized_id_or_case_folded_folder_is_blocked() {
    let result = scan_fixed_roots(collision_fixture()).unwrap();
    let id_collision = result.collision(ScanCollisionKind::NormalizedId).unwrap();
    assert_eq!(id_collision.skill_ids, ["agents:pony-tail", "agents:pony-tail"]);
    assert_eq!(id_collision.paths.len(), 2);
    let folder_collision = result.collision(ScanCollisionKind::FoldedFolderName).unwrap();
    assert_eq!(folder_collision.collision_key, "pónytail");
    assert_eq!(folder_collision.skill_ids.len(), 2);
    assert_eq!(folder_collision.paths.len(), 2);
}

#[test]
fn same_name_in_different_namespaces_is_not_a_collision() {
    let result = scan_fixed_roots(cross_namespace_fixture()).unwrap();
    assert!(result.collisions.is_empty());
    assert_eq!(result.skill_ids(), ["agents:shared", "codex:shared", "claude-code:shared"]);
}
```

- [ ] **Step 2: 实现 `skill_id`**

复用 Task 3 的 `SkillNamespace`。identity source 先取 trim 后非空的 frontmatter `name`，否则回退到 `folder_name`；两者都缺失或规范化后为空则该目录 blocked。ID 规则：`<namespace>:<normalized-name>`；name lowercase、trim、空格和 `_` 转 `-`、只保留 ASCII 字母数字和 `-`、连续 `-` 折叠。测试必须覆盖 name 缺失、空白、回退 folder name 和规范化为空。

固定 root registry：

```rust
pub struct FixedSkillRoot {
    pub namespace: SkillNamespace,
    pub path: PathBuf,
}

pub fn fixed_skill_roots(home: &Path) -> [FixedSkillRoot; 3];
```

映射严格为 `agents -> ~/.agents/skills`、`codex -> ~/.codex/skills`、`claude-code -> ~/.claude/skills`。函数不接收 AppConfig，不扫描项目级目录、环境自定义路径或旧 hosts/custom_paths。

- [ ] **Step 3: 更新 Skill DTO**

```rust
pub struct Skill {
    pub id: String,
    pub name: String,
    pub folder_name: String,
    pub description: String,
    pub namespace: SkillNamespace,
    pub relative_dir: String,
    pub source_path: String,
    pub hash: String,
    pub zip_size: u64,
    pub modified_at: String,
}
```

删除旧 `host/platform/enabled/repo_path` compatibility 字段，新逻辑只使用 namespace。`relative_dir` 在 V1 等于经验证的单段 `folder_name`，保留字段是为了 sync_state 明确记录上次安装位置。

- [ ] **Step 4: 扫描结果增加 root 状态**

```rust
pub struct ScanRootStatus {
    pub namespace: SkillNamespace,
    pub root_path: String,
    pub exists: bool,
    pub readable: bool,
    pub scan_complete: bool,
    pub error: Option<String>,
}

pub enum ScanCollisionKind {
    NormalizedId,
    FoldedFolderName,
}

pub struct ScanCollision {
    pub namespace: SkillNamespace,
    pub collision_key: String,
    pub kind: ScanCollisionKind,
    pub skill_ids: Vec<String>,
    pub paths: Vec<String>,
}
```

root 不存在或不可读时记录状态，不把该 namespace 下已跟踪 skill 判为删除。扫描只看固定 root 的直接子目录；codex root 显式跳过 `.system`，其他目录没有合法 `SKILL.md` 就跳过且不递归。

同 namespace 出现相同 normalized id，或 folder_name 经 Unicode NFC + lowercase 折叠后相同，返回结构化 `ScanCollision` 并把涉及项交给 SyncPlan 标为 blocked；不得保留第一个。三个 namespace 之间同名不构成 collision。

- [ ] **Step 5: 更新 TS skill schema**

```ts
namespace: z.enum(['agents', 'codex', 'claude-code']),
folder_name: z.string(),
relative_dir: z.string(),
zip_size: z.number().optional(),
```

`scanResultSchema` 增加 `roots: ScanRootStatus[]` 与 `collisions: ScanCollision[]`；root status 的 namespace 使用同一 enum，并包含 resolved `root_path`、`exists`、`readable`、`scan_complete`、`error`。TS `ScanCollision` 完整镜像 `namespace/collision_key/kind/skill_ids/paths`，其中 kind 为 `normalized_id | folded_folder_name`。

scanner 只负责本地碰撞。远端 manifest 和 local/remote 合并后的目标碰撞由 Task 8 在生成动作前再次检测，避免两个 remote-new 条目落到同一目录。

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
#[async_trait]
pub trait RemoteStore: Send + Sync {
    async fn fetch_manifest(&self) -> Result<RemoteSnapshot>;
    async fn fetch_blob(&self, blob_path: &str, expected_hash: &str) -> Result<Vec<u8>>;
    async fn commit_changes(&self, changes: RemoteChanges) -> Result<RemoteCommit>;
}
```

使用 `async_trait::async_trait`。禁止为 trait 实现创建 Tokio runtime 或调用 `block_on`；所有调用方从本任务起直接 `.await`。

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

pub struct BlobWrite {
    pub path: String,
    pub bytes: Vec<u8>,
    pub expected_hash: String,
}

pub struct RemoteCommit {
    pub commit_sha: String,
}

impl LocalVaultStore {
    pub fn open(vault_dir: PathBuf, device_id: String) -> Result<Self>;
}
```

`BlobWrite.path` 必须是经过验证的 `blobs/sha256/<hash>.skill.zip`，`expected_hash` 必须等于 bytes 的 canonical SHA-256；无效 path/hash 返回 `AppError::Vault`，不允许部分提交。`RemoteCommit.commit_sha` 是提交后 snapshot 的版本标识。

- [ ] **Step 2: 写 LocalVaultStore 测试**

```rust
#[tokio::test]
async fn empty_local_vault_returns_empty_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalVaultStore::open(dir.path().into(), "device-a".into()).unwrap();
    let snapshot = store.fetch_manifest().await.unwrap();
    assert!(snapshot.manifest.skills.is_empty());
    assert!(!snapshot.commit_sha.is_empty());
}

#[tokio::test]
async fn commit_changes_writes_blob_and_manifest_once() {
    let fixture = LocalVaultFixture::new();
    let before = fixture.store.fetch_manifest().await.unwrap();
    let changes = fixture.one_skill_changes(&before, b"canonical zip bytes");
    let committed = fixture.store.commit_changes(changes.clone()).await.unwrap();
    let after = fixture.store.fetch_manifest().await.unwrap();
    assert_eq!(after.commit_sha, committed.commit_sha);
    assert_ne!(after.commit_sha, before.commit_sha);
    assert_eq!(after.manifest, changes.next_manifest);
    assert_eq!(fixture.store.fetch_blob(&changes.blobs[0].path, &changes.blobs[0].expected_hash).await.unwrap(), b"canonical zip bytes");
}

#[tokio::test]
async fn changed_base_commit_returns_remote_changed() {
    let fixture = LocalVaultFixture::new();
    let stale = fixture.store.fetch_manifest().await.unwrap();
    fixture.commit_from(&stale, b"first").await.unwrap();
    let err = fixture.commit_from(&stale, b"stale").await.unwrap_err();
    assert!(matches!(err, AppError::RemoteChanged(_)));
}

#[tokio::test]
async fn concurrent_fetch_never_observes_mixed_manifest_and_commit() {
    let fixture = Arc::new(LocalVaultFixture::new());
    let writer = fixture.spawn_sequential_commits(100);
    let reader = fixture.spawn_snapshot_probe(500);
    writer.await.unwrap();
    let observed = reader.await.unwrap();
    assert!(observed.iter().all(|snapshot| fixture.manifest_matches_commit(snapshot)));
}

#[tokio::test]
async fn separate_store_instances_for_same_path_share_lock() {
    let fixture = LocalVaultFixture::new();
    let second = LocalVaultStore::open(fixture.path.clone(), "device-b".into()).unwrap();
    let base = fixture.store.fetch_manifest().await.unwrap();
    let (left, right) = tokio::join!(
        fixture.store.commit_changes(fixture.changes_from(&base, b"left")),
        second.commit_changes(fixture.changes_from(&base, b"right")),
    );
    assert_eq!([left.is_ok(), right.is_ok()].into_iter().filter(|ok| *ok).count(), 1);
    assert_eq!([left, right].into_iter().filter(|r| matches!(r, Err(AppError::RemoteChanged(_)))).count(), 1);
}

#[tokio::test]
async fn fetch_blob_rejects_bytes_that_do_not_match_expected_hash() {
    let fixture = LocalVaultFixture::with_raw_blob("blobs/sha256/bad.skill.zip", b"corrupt");
    let err = fixture.store.fetch_blob("blobs/sha256/bad.skill.zip", "sha256:expected").await.unwrap_err();
    assert!(matches!(err, AppError::Vault(_)));
}
```

`LocalVaultFixture` 在测试模块中保存 `path`、store 和 `commit_sha -> manifest hash` 对照表；`spawn_sequential_commits` 每次用上一次返回的 SHA 提交并记录 pair，`spawn_snapshot_probe` 只调用公共 `fetch_manifest`，`manifest_matches_commit` 根据该对照表验证 snapshot 的 manifest 与 commit 来自同一次事务。测试不得读取 store 内部锁来制造通过结果。

- [ ] **Step 3: 实现本地 vault 布局**

```text
manifest.json
blobs/sha256/<hash>.skill.zip
.local-vault-commit
```

`.local-vault-commit` 每次 commit 写新 UUID，模拟 GitHub branch HEAD。

`fetch_blob` 在返回前计算 bytes 的 SHA-256，并与 `expected_hash` 比较；不匹配返回 `AppError::Vault`。`commit_changes` 同样在任何写入前验证所有 `BlobWrite` 的 path/hash，确保失败时没有 manifest、blob 或 commit marker 副作用。

`LocalVaultStore` 实现同一 async trait；其 `fs::read` / `fs::write` / rename 等同步文件操作必须放进 `tauri::async_runtime::spawn_blocking`，并把 join error 映射为 `AppError`。closure 只捕获 owned/clone 数据，不能跨 await 持有借用或锁。

为规范化 vault path 建立 process-wide path-keyed lock registry，返回共享的 `Arc<tokio::sync::RwLock<()>>`；两个独立 `LocalVaultStore::open` 指向同一路径时必须拿到同一个 Arc。`fetch_manifest` / `fetch_blob` 获取读锁，其中 manifest 与 `.local-vault-commit` 必须在同一个 blocking closure 内读取；`commit_changes` 获取写锁，并把 base commit 校验、blob/manifest 原子写入和 commit marker 更新作为一个完整 blocking transaction 执行。registry 清理过期 Weak entry，避免开发测试反复创建路径导致无界增长。

- [ ] **Step 4: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml local_vault_store remote_store
rg -n "reqwest::blocking|block_on|Runtime::new" src-tauri/src
```

Expected: local vault async tests pass；`rg` 无命中。

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
    BothDeleted,
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
    pub action_id: String,
    pub skill_id: String,
    pub name: String,
    pub namespace: SkillNamespace,
    pub folder_name: String,
    pub relative_dir: Option<String>,
    pub status: SyncStatus,
    pub local_hash: Option<String>,
    pub remote_hash: Option<String>,
    pub base_hash: Option<String>,
    pub local_path: Option<String>,
    pub remote_blob: Option<String>,
    pub conflict_reason: Option<ConflictReason>,
    pub delete_direction: Option<DeleteDirection>,
    pub blocked_reason: Option<String>,
    pub warnings: Vec<String>,
}

pub struct BaseAdoption {
    pub skill_id: String,
    pub hash: String,
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
    pub plan_fingerprint: String,
    pub base_adoptions: Vec<BaseAdoption>,
    pub base_removals: Vec<String>,
    pub will_create_commit: bool,
    pub commit_summary: CommitSummary,
}
```

`CommitSummary` 增加 `local_state_updates`，值为去重后的 adoption/removal 数量。`base_adoptions` 按 skill_id 唯一，`base_removals` 与 adoption 不得包含同一个 skill_id。

- [ ] **Step 3: 定义 Apply 请求和响应**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplySyncRequest {
    pub expected_remote_commit: String,
    pub plan_fingerprint: String,
    pub selected_action_ids: Vec<String>,
    pub decisions: HashMap<String, SyncDecision>,
    pub delete_guard_ack: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanChangeReason {
    RemoteChanged,
    PlanChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ApplySyncResponse {
    Applied { result: ApplyResult },
    PlanChanged {
        reason: PlanChangeReason,
        latest_plan: SyncPlan,
    },
}
```

`selected_action_ids` 只控制普通可执行项；冲突仍由 `decisions` 控制。普通删除 action 未被选择时必须跳过。`delete_guard_tripped` 且本次普通动作或冲突 decision 包含删除时，必须要求 `delete_guard_ack == true`。非法 action id、非法 decision 或缺少删除确认使用现有 `AppError::Blocked` 返回，`ApplySyncResponse` 只表达成功或计划已变化。

decision 白名单：

| ConflictReason                 | 允许的 decision                           |
| ------------------------------ | ----------------------------------------- |
| `same_name_first_seen`         | `keep_local`, `use_remote`, `skip`        |
| `both_changed`                 | `keep_local`, `use_remote`, `skip`        |
| `local_deleted_remote_changed` | `delete_remote`, `restore_remote`, `skip` |
| `remote_deleted_local_changed` | `keep_local`, `accept_delete`, `skip`     |

未提供 decision 表示该冲突不执行并返回 warning；不存在于当前冲突集合的 skill_id 或不在白名单内的 decision 返回 `Blocked`，整次 Apply 不得部分执行。

- [ ] **Step 4: 定义前端决策值**

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

- [ ] **Step 5: 更新 TS status 与 plan schema**

```ts
z.enum([
  'synced',
  'local_update',
  'remote_update',
  'local_deleted',
  'remote_deleted',
  'both_deleted',
  'conflict',
  'blocked',
  'unknown',
])
```

`syncPlanSchema` 必须包含 `entries[].action_id`、`entries[].namespace/folder_name/relative_dir`、`both_deleted`、`delete_remote`、`delete_local`、`delete_guard_tripped`、`expected_remote_commit`、`plan_fingerprint`、`base_adoptions`、`base_removals` 和 `commit_summary.local_state_updates`，并删除旧 `platform/platforms`。增加 Rust serde 与 Zod fixture，断言 `both_deleted` 和三个 namespace 均能 round-trip。前端 Apply request/response 与 ApplyResult schema 必须和 adapter、页面原子迁移，统一放到 Task 15，避免中间提交破坏现有页面类型。

- [ ] **Step 6: 验证**

Run:

```bash
npm run typecheck
cargo test --manifest-path src-tauri/Cargo.toml --no-run
```

Expected: schema compiles；Rust 可在 Task 8 完成实现后全绿。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/sync_engine.rs src/modules/sync
git commit -m "refactor: define vault sync plan dto"
```

### Task 8: 重写 build_plan，落地 13 行真值表

**Files:**

- Modify: `src-tauri/src/sync_engine.rs`
- Create/Modify: `src-tauri/tests/github_vault_sync.rs`

- [ ] **Step 1: 写三方比较测试**

本 Task 中所有调用 async `build_plan` / `RemoteStore` 的测试实际使用 `#[tokio::test] async fn`；下列简写中的同步 `#[test] fn` 仅表示断言主体。纯 `plan_fingerprint` helper 测试可以保留普通 `#[test]`。

```rust
#[test]
fn base_empty_local_only_is_local_update() { ... }

#[test]
fn base_empty_remote_only_is_remote_update() { ... }

#[test]
fn base_empty_both_same_emits_base_adoption_without_writing_state() { ... }

#[test]
fn both_same_new_hash_advances_existing_base() { ... }

#[test]
fn both_same_current_base_emits_no_adoption() { ... }

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
fn both_deleted_emits_base_removal_without_writing_state() { ... }

#[test]
fn plan_fingerprint_is_stable_when_entry_order_changes() { ... }

#[test]
fn plan_fingerprint_changes_when_action_input_changes() { ... }

#[test]
fn plan_fingerprint_uses_explicit_nulls_and_excludes_display_warnings() { ... }

#[test]
fn plan_fingerprint_matches_golden_json_and_digest() { ... }

#[test]
fn duplicate_action_id_is_blocked() { ... }
```

`plan_fingerprint_changes_when_action_input_changes` 使用表驱动 mutation，分别覆盖 remote commit、delete guard、action id、namespace、folder_name、relative_dir、status、local/remote/base hash、local path、remote blob、delete direction、conflict reason、blocked reason、base adoption 的 skill/hash 和 base removal membership。顺序稳定测试分别打乱 entries、base_adoptions、base_removals；golden vector 必须包含非空 adoption/removal，锁定跨重构的 canonical serialization。纯展示 warnings 不影响 fingerprint。

再增加不变量测试：adoptions 内 skill id 唯一、removals 内 skill id 唯一、两者无交集，`commit_summary.local_state_updates` 对协调 skill id 去重计数。

- [ ] **Step 2: 实现 13 行状态表**

| #   | base | local | remote         | status                                   | 动作                       | commit |
| --- | ---- | ----- | -------------- | ---------------------------------------- | -------------------------- | ------ |
| 1   | `∅`  | 有    | `∅`            | `local_update`                           | 上传                       | 1      |
| 2   | `∅`  | `∅`   | 有             | `remote_update`                          | 下载                       | 0      |
| 3   | `∅`  | 有    | 有，`l==r`     | `synced`                                 | 生成 base_adoption         | 0      |
| 4   | `∅`  | 有    | 有，`l!=r`     | `conflict: same_name_first_seen`         | 用户选择                   | 视选择 |
| 5   | 有   | 有    | 有，`l==r`     | `synced`                                 | `b!=l` 时生成 adoption     | 0      |
| 6   | 有   | 有    | 有，`r==b`     | `local_update`                           | 上传                       | 1      |
| 7   | 有   | 有    | 有，`l==b`     | `remote_update`                          | 下载                       | 0      |
| 8   | 有   | 有    | 有，三者互不等 | `conflict: both_changed`                 | 用户选择                   | 视选择 |
| 9   | 有   | `∅`   | 有，`r==b`     | `local_deleted`                          | 删云端 manifest 条目       | 1      |
| 10  | 有   | `∅`   | 有，`r!=b`     | `conflict: local_deleted_remote_changed` | 删云端 / 拉回本地 / 跳过   | 视选择 |
| 11  | 有   | 有    | `∅`，`l==b`    | `remote_deleted`                         | 删本地，移入 trash         | 0      |
| 12  | 有   | 有    | `∅`，`l!=b`    | `conflict: remote_deleted_local_changed` | 保留本地 / 接受删除 / 跳过 | 视选择 |
| 13  | 有   | `∅`   | `∅`            | `both_deleted`                           | 生成 base_removal          | 0      |

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

#[test]
fn unhealthy_namespace_only_blocks_deletes_for_that_namespace() { ... }

#[test]
fn scan_collision_blocks_all_involved_paths() { ... }

#[test]
fn remote_or_merged_target_collision_blocks_all_involved_entries() { ... }
```

- [ ] **Step 4: 实现 `build_plan`**

```rust
pub async fn build_plan<S: RemoteStore>(
    config: &AppConfig,
    state: &SyncState,
    store: &S,
) -> Result<SyncPlan>
```

流程：

1. `store.fetch_manifest().await` 获取 `RemoteSnapshot`。
2. 通过 `tauri::async_runtime::spawn_blocking` 扫描三个固定 namespace roots，带回每个 namespace 唯一的 `ScanRootStatus` 与 collisions。
3. 通过 `spawn_blocking` 对每个本地 skill pack/hash/size；禁止直接在 async future 中执行目录遍历、zip 或 hash。
4. 合并 `base/local/remote`，按 13 行表判定。
5. 根据 base/state 的 namespace 定位唯一 root；该 namespace root 缺失、不可读或 `scan_complete=false` 时，只把该 namespace 下 base 中存在的 skill 标 `unknown`，不推断删除。其他 namespace 不受影响。
6. 合并前校验远端 manifest 内 normalized ID 与 NFC+lowercase `folder_name`，合并后再校验所有会写入本地的目标 key `(namespace, folded_folder_name)`。本地 scanner、纯远端或 local/remote 跨集合 collision 涉及的全部 entry 都标为 blocked；禁止产生 upload/download/delete/adoption。跨 namespace 同名仍保持独立。
7. 删除数超过 `max_auto_delete` 或超过 tracked skills 的 50% 时设置 `delete_guard_tripped`。
8. 第 3 行生成 `BaseAdoption { skill_id, hash: local_hash }`；第 5 行仅在 `base_hash != local_hash` 时生成 adoption；第 13 行把 skill_id 加入 `base_removals`。三者都只进入计划，不在 preview 写 sync_state。
9. 为每个条目生成全计划唯一的 `action_id = <action_kind>:<skill_id>`；`action_kind` 只取 `upload`、`download`、`delete_remote`、`delete_local`、`conflict`、`synced`、`blocked`、`unknown`、`both_deleted`，重复 id 直接返回 `Blocked`。
10. 定义固定字段顺序的 `PlanFingerprintInput` / `PlanFingerprintEntry`，optional 字段缺失时保留显式 null，entries、base_adoptions、base_removals 分别按 skill/action id 字节序排序；使用 `serde_json::to_vec` 后计算 `sha256:<hex>`。输入字段严格为设计文档列出的 `expected_remote_commit`、`delete_guard_tripped`、本机状态协调与动作安全字段；展示顺序、文案、时间戳和纯展示 warning 不参与计算。
11. 预览结束清理临时 zip 目录。
12. 不写本地、不上传、不下载、不删除。

`will_create_commit` 只由实际远端变化决定，base adoption/removal 不得把它设为 true。`commit_summary.local_state_updates` 等于 adoption/removal 去重后的 skill 数量。

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

本 Task 的测试全部使用 `#[tokio::test] async fn` 并 await engine/store；不得在测试中用 `block_on` 包装同步 API。

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
fn adoption_only_apply_writes_base_with_zero_remote_commits() { ... }

#[test]
fn adoption_advances_old_base_to_converged_hash() { ... }

#[test]
fn current_base_produces_no_state_update() { ... }

#[test]
fn removal_only_apply_clears_base_with_zero_remote_commits() { ... }

#[test]
fn preview_plan_does_not_persist_adoption_or_removal() { ... }

#[test]
fn batch_upload_and_delete_remote_share_one_commit() { ... }

#[test]
fn remote_change_after_preview_returns_latest_plan_without_side_effect() { ... }

#[test]
fn stale_plan_returns_latest_plan_before_persistent_side_effect() { ... }

#[test]
fn remote_change_after_preflight_discards_staged_local_changes_and_returns_latest_plan() { ... }

#[test]
fn local_change_after_preview_invalidates_plan_fingerprint() { ... }

#[test]
fn unselected_delete_action_is_not_applied() { ... }

#[test]
fn unselected_upload_and_download_are_not_applied() { ... }

#[test]
fn duplicate_selected_action_id_blocks_whole_apply() { ... }

#[test]
fn delete_guard_requires_ack_for_selected_delete() { ... }

#[test]
fn stale_conflict_decision_is_not_applied_to_changed_reason() { ... }

#[test]
fn unknown_or_non_executable_action_id_blocks_whole_apply() { ... }

#[test]
fn missing_conflict_decision_skips_with_warning() { ... }

#[test]
fn extra_or_invalid_conflict_decision_blocks_whole_apply() { ... }

#[test]
fn delete_conflict_decision_requires_guard_ack() { ... }

#[test]
fn validation_error_cleans_temp_pack_and_has_zero_persistent_side_effects() { ... }

#[test]
fn remote_new_downloads_to_namespace_fixed_root() { ... }

#[test]
fn update_download_uses_state_namespace_and_relative_dir() { ... }

#[test]
fn unsafe_folder_or_state_namespace_mismatch_is_blocked() { ... }
```

adoption-only 与 removal-only 测试必须显式使用空 `selected_action_ids`、空 `decisions` 和 `delete_guard_ack=false`，并断言：返回 `Applied`、`applied == []`、`state_updated` 为正确且去重后的 skill ids、`remote_commit == None`、`commit_changes` 调用次数为 0、`state.remote.commit_sha` 更新为当前 snapshot commit，且 base 被正确写入/推进或移除。

失败路径 fixture 必须包含至少一个 pending adoption/removal：fingerprint/commit 不匹配返回 `PlanChanged`、非法 selection/decision 返回 `Blocked`、以及只调用 preview 未调用 Apply 三种情况下，均断言 sync_state bytes 未变化、`state_updated` 不返回成功结果、远端 commit 次数为 0。

- [ ] **Step 2: 实现 `apply_plan`**

```rust
pub async fn apply_plan<S: RemoteStore>(
    config: &AppConfig,
    request: &ApplySyncRequest,
    store: &S,
) -> Result<ApplySyncResponse>
```

`ApplyResult` 不再返回 backups：

```rust
pub struct ApplyResult {
    pub applied: Vec<String>,
    pub state_updated: Vec<String>,
    pub warnings: Vec<String>,
    pub remote_commit: Option<String>,
}
```

- [ ] **Step 3: 执行前重新生成计划**

async apply 必须通过内部 async `prepare_plan` await manifest，并以 `spawn_blocking` 重新 scan/pack/hash、重建计划，返回同时持有 `SyncPlan`、本次 `PackedSkill` 和 RAII 临时目录的 `PreparedSyncPlan`。预览阶段 zip 不允许复用，指纹校验后也不得再次 pack；后续上传必须使用 `PreparedSyncPlan` 中参与校验的准确 bytes。在任何持久化副作用（本地 skill 写入、trash move、远端提交或 sync_state 更新）之前：

1. 比较 `request.expected_remote_commit` 与新计划的 `expected_remote_commit`。
2. 比较 `request.plan_fingerprint` 与新计划的 `plan_fingerprint`。
3. 任一不一致时返回 `ApplySyncResponse::PlanChanged { reason, latest_plan }`，不合并旧 decisions，不产生任何持久化副作用，并清理本次临时 pack。expected remote commit 不同优先使用 `RemoteChanged` reason；commit 相同但 fingerprint 不同使用 `PlanChanged` reason。
4. 校验 `selected_action_ids` 都存在于新计划且仍可执行；只执行被选择的普通动作。不存在、重复或不可执行的 action id 返回 `Blocked`，不得部分执行。
5. 校验每个 conflict decision 仍被当前 `ConflictReason` 允许。
6. 如果 delete guard 已触发且本次选择包含普通删除或删除型 conflict decision，要求 `delete_guard_ack == true`。

预检通过后到 GitHub branch ref 更新前仍可能发生竞态。下载内容先解包到任务临时目录，delete_local 只准备目标，不立即移动；如果本次包含远端变更，必须先完成 `commit_changes`，成功后才提交本地目录替换和 trash move。`commit_changes` 的 `RemoteChanged` 不得强推：重新生成 `latest_plan`，清理所有暂存内容，不更新 sync_state，并返回 `ApplySyncResponse::PlanChanged { reason: RemoteChanged, latest_plan }`。

- [ ] **Step 4: 实现下载覆盖保护**

- 目标不存在则写入。
- 目标存在且当前 local hash == base hash 才允许覆盖。
- 目标存在但无 base，或 local hash != base，返回 conflict/blocked，不静默覆盖。
- 云端新增目标严格为 `fixed_root(entry.namespace)/validated(folder_name)`；更新目标严格为 `fixed_root(state.namespace)/state.relative_dir`。
- remote-new 的固定 root 不存在时，Apply 可以先创建 root；该状态在 preview 仍不得触发任何本地删除推断。
- manifest namespace、skill_id namespace、state namespace 任一不一致，或 folder_name/relative_dir 不是安全单段目录时返回 blocked；不得回退到“第一个 root”或跨 namespace 写入。

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
- 成功 upload/download 同时写入 `namespace` 与 `relative_dir`，使后续覆盖与删除护栏定位同一固定 root。
- 对 `base_adoptions` 写入 `base_hash` / `last_remote_hash = adoption.hash` 并更新 `last_synced_at`。
- 对 `base_removals` 移除对应 skill state；`both_deleted` 不再依赖存在其他动作才清理。
- `local_deleted` 删云端成功后移除 base。
- `remote_deleted` 删本地成功后移除 base。
- skip 或 blocked 不改 base。
- 计划只有 adoption/removal、`selected_action_ids` 和 decisions 都为空时，仍原子保存 sync_state，更新 state.remote.commit_sha，返回 `Applied`，且不调用 `commit_changes`。
- `ApplyResult.state_updated` 返回 adoption/removal 的去重 skill ids；这些 id 不混入实际上传、下载或删除动作的 `applied`。

- [ ] **Step 8: 实现批量内部函数**

```rust
pub async fn upload_skills<S: RemoteStore>(skill_ids: &[String], ... ) -> Result<ApplyResult>;
pub async fn download_skills<S: RemoteStore>(skill_ids: &[String], ... ) -> Result<ApplyResult>;
```

复用 async plan/apply，不开平行同步流程。下载后的解包、目录替换、trash move 与 sync_state 文件保存通过 `spawn_blocking` 执行；RemoteStore 操作直接 await。

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

- [ ] **Step 1: 写 Device Flow 请求与状态测试**

使用 wiremock 覆盖：start request 的 client id/scope 与 `device_code`、`user_code`、`verification_uri`、`expires_in`、`interval` 解析；poll request 的 device grant type；`authorization_pending`、`slow_down`、`expired_token`、`access_denied` 映射；authorized 成功后 token 已写入 injected credential store 且响应/日志不含 token。`slow_down` 必须把下一次轮询间隔在当前值上增加 GitHub 要求的 5 秒，并通过 poll DTO 返回新 interval。

- [ ] **Step 2: 实现 commands**

```rust
#[tauri::command]
pub async fn start_github_device_flow() -> Result<DeviceFlowStart>;

#[tauri::command]
pub async fn poll_github_device_flow(device_code: String) -> Result<DeviceFlowPoll>;
```

`github_auth.rs` 定义可注入 `api_base_url` 和 `CredentialStore` 的 `GitHubAuthClient`。生产 endpoint 使用 GitHub 官方 Device Flow URL；测试只访问 wiremock。`start` POST `client_id` 与 scope，`poll` POST `client_id`、`device_code` 和 `urn:ietf:params:oauth:grant-type:device_code`。`DeviceFlowPoll` 只返回 status/message/next interval，不暴露 access token。

- [ ] **Step 3: 处理 client id**

从环境变量读取：

```text
SKILL_SYNC_GITHUB_CLIENT_ID
```

缺失时返回可操作的 `Auth` 错误，不硬编码假 client id。

- [ ] **Step 4: 实现 token storage**

使用 `keyring` 实现 `CredentialStore`，测试注入内存实现：

```rust
pub async fn save_github_token(token: String) -> Result<()>;
pub async fn load_github_token() -> Result<Option<String>>;
pub async fn clear_github_token() -> Result<()>;
```

keyring 是阻塞 OS credential-store I/O，三个 async 函数内部必须用 `tauri::async_runtime::spawn_blocking` 包住完整 keyring 调用并映射 join error；禁止在 async command 中直接调用 keyring，禁止 log token。

poll 收到 `access_token` 后必须先成功保存，再返回 `authorized`；保存失败则返回 `Auth` 错误，不能向前端返回 token 或假装授权完成。`authorization_pending` 保持原 interval，`slow_down` 返回增加后的 interval，expired/denied 终止轮询。

- [ ] **Step 5: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml github_auth
```

Expected: wiremock Device Flow 成功/等待/减速/终止测试 pass；keyring 如 CI 不可用，只 gate OS storage adapter tests，内存 credential store 的成功路径必须默认执行。

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
    api_base_url: reqwest::Url,
}
```

headers：`Authorization`、`Accept: application/vnd.github+json`、`X-GitHub-Api-Version: 2022-11-28`、`User-Agent: skill-sync`。

使用 `#[async_trait] impl RemoteStore for GitHubVaultStore`，三个方法直接 await `reqwest::Client`。Cargo 不启用 reqwest `blocking` feature；实现中禁止 `block_on` 或创建嵌套 Tokio runtime。

生产构造函数固定 `api_base_url = https://api.github.com/`；另提供 `#[cfg(test)] new_with_base_url(..., wiremock_server.uri())`。所有 endpoint 必须相对该 base URL 构建，不能在方法中散落硬编码 GitHub host。

- [ ] **Step 2: 实现 manifest fetch**

先读取 repository 与 branch ref/HEAD；这些请求的 401/403/404 都按认证、repo 或 branch 错误返回，不能伪装成空 vault。只有 branch 已确认存在后，读取该 commit 上 `manifest.json` 的 path 请求返回 404，才构造 `VaultManifest::empty(&device_id)`，并保留已读取的 HEAD SHA 作为 `RemoteSnapshot.commit_sha`。

- [ ] **Step 3: 实现 blob fetch**

读取 `blobs/sha256/<hash>.skill.zip`，校验 bytes hash == expected hash。

- [ ] **Step 4: 实现 commit_changes**

使用 Git Database APIs：get ref、get commit/tree、create blobs、create tree、create commit、更新 ref 前校验 HEAD 仍等于 `base_commit_sha`，并以 `force=false` update ref。预检后的并发更新可能发生在最后一次 HEAD 读取与 update ref 之间，因此仅 update-ref 操作返回的 non-fast-forward、409 或 422 映射为 `RemoteChanged`；其他 API 的 409/422 仍按正常 Vault validation error 处理。绝不重试为 force update。

- [ ] **Step 5: 不打真实网络测试**

默认只测 DTO、error mapping、request building。真实 GitHub 测试只能手动或 gated：

使用 `wiremock` 的 Tokio mock server 测试 request/response path。默认测试必须包含：branch HEAD 成功 + manifest path 404 返回 empty manifest；repo/ref/branch 404 返回明确错误；manifest/blob 请求；update-ref request 的 `force=false`；以及仅 update-ref 模拟 non-fast-forward / 409 / 422 时映射为 `AppError::RemoteChanged`。其他 endpoint 的 409/422 仍映射为 Vault error。默认测试不访问真实 GitHub。

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
async get_app_state()
async save_config(config)
async scan_skills()
async get_sync_plan()
async apply_sync_plan(request: ApplySyncRequest) -> Result<ApplySyncResponse>
open_path(path)
async start_github_device_flow()
async poll_github_device_flow(device_code)
async check_github_vault(config)
async list_remote_skills()
async upload_skills(skill_ids)
async download_skills(skill_ids)
```

`get_app_state` 响应在移除 Git 字段后必须包含 `github_authorized`、`github_user`、`device_name` 和来自 `sync_state.remote.commit_sha` 的 nullable `remote_commit`。`check_github_vault` 返回稳定 DTO：

```rust
pub struct GithubVaultCheck {
    pub repository_accessible: bool,
    pub branch_accessible: bool,
    pub manifest_exists: bool,
    pub remote_commit: Option<String>,
}
```

认证、repo 或 branch 不可访问返回明确错误；repo/branch 可访问但 `manifest.json` 不存在时返回 `manifest_exists=false`，供 Onboarding 提示创建空 vault，而不是把两类状态混为一谈。

在 `commands.rs` 定义并由 `lib.rs` 注册 Tauri managed state：

```rust
#[derive(Default)]
pub struct SyncOperationGate {
    inner: tokio::sync::RwLock<()>,
}
```

- `get_app_state`、`scan_skills`、`get_sync_plan` 在读取 config/state 前获取 read guard，并持有到扫描/预览返回。
- `save_config`、`apply_sync_plan`、`upload_skills`、`download_skills` 在读取任何 config/state 前获取 write guard，并持有到所有本地目录变更和最终 sync_state/config 保存结束。
- engine 与 adapter 不再次获取 application gate；LocalVaultStore 自己的 path lock 是另一层 adapter transaction lock。
- remote-only auth polling 不获取该 gate；需要读取当前配置的 vault check/list command 至少获取 read guard。

`apply_sync_plan` command 必须是 async，接收并完整转发单个 `request: ApplySyncRequest` 参数：通过 `spawn_blocking` 加载 config / sync_state，通过 async keyring API 读取 token，构造 `GitHubVaultStore`、await 新的 engine apply 路径并返回 `Result<ApplySyncResponse>`，不得只提取 decisions。command 层或 engine 层捕获提交期 `AppError::RemoteChanged` 后，重新生成并返回 `ApplySyncResponse::PlanChanged { reason: RemoteChanged, latest_plan }`；此路径不得写 sync_state。其他同步 commands 同样 await engine/store；扫描、pack、解包和目录移动不得直接阻塞 command future。

`get_app_state` 与 `save_config` 也改为 async command，并分别用 `spawn_blocking` 包住完整的 AppConfig load/save。SyncState load/save 必须各自在一个 blocking closure 中完成；写入使用临时文件 + rename 的原子流程。所有 spawn join errors 映射为 `AppError`。

`scan_skills` 不再读取 AppConfig roots，直接调用固定 registry，返回 `skills + roots + collisions + warnings`。Task 16 的 Settings 复用该响应显示只读路径状态。

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

增加 `#[tokio::test]` command 边界测试：完整 request 字段不会丢失，`applied` / `plan_changed` tagged response 可序列化，提交期 `RemoteChanged` 会返回带 `latest_plan` 的响应而不是普通 command rejection；原请求不会自动重试，暂存的本地变更会被丢弃。编译检查还必须证明 async command future 满足 Tauri 的 Send 要求。

增加 operation gate 并发测试：两个 apply/upload/download 不会同时进入 engine；download-only 与 state-only Apply 同样串行；活动写操作期间启动的 scan/get_sync_plan 必须等待，写操作完成后读取最新 sync_state；测试使用 barrier/notify 控制交错，不能依赖 sleep。

Send 编译断言使用最小 helper：

```rust
fn assert_send<T: Send>(_: T) {}

assert_send(get_sync_plan_impl(...));
assert_send(apply_sync_plan_impl(...));
```

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
- Modify: `src/modules/settings/api/configApi.ts`
- Modify: `src/modules/onboarding/api/onboardingApi.ts`

- [ ] **Step 1: 更新 AppState schema**

移除 `git_available` / `git_version`，新增：

```ts
github_authorized: z.boolean(),
github_user: z.string().nullable(),
device_name: z.string(),
remote_commit: z.string().nullable(),
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
  interval: z.number(),
})
```

`slow_down` 时页面用响应中的新 `interval` 安排下一次 poll，不能继续使用 start response 的旧间隔。

- [ ] **Step 3: 添加其余 API 函数**

```ts
export const startGithubDeviceFlow = async (): Promise<DeviceFlowStart>;
export const pollGithubDeviceFlow = async (deviceCode: string): Promise<DeviceFlowPoll>;
export const checkGithubVault = async (config: AppConfig): Promise<GithubVaultCheck>;
```

定义并导出对应契约：

```ts
export const githubVaultCheckSchema = z.object({
  repository_accessible: z.boolean(),
  branch_accessible: z.boolean(),
  manifest_exists: z.boolean(),
  remote_commit: z.string().nullable(),
})
```

`checkGithubVault` 必须用该 schema 解析 command 返回值。Onboarding 将 `manifest_exists=false` 映射为“创建空 vault”状态；command error 映射为认证/repo/branch 错误，不能显示成空 vault。

- [ ] **Step 4: 验证**

Run:

```bash
npm run typecheck
npm run lint
npm run build
```

Expected: schemas compile，现有页面仍通过 typecheck；不要在本任务提前改变 ApplyResult schema 或 `applySyncPlan` 的调用签名。

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
npm run lint
npm run build
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
- Modify: `src/modules/sync/api/syncApi.ts`
- Modify: `src/modules/sync/schemas/syncPlan.ts`
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

卡片显示 skill name、namespace badge（agents / codex / claude-code）、status badge、删除方向 badge、local/remote/base hash 短值、固定目标路径、warnings、blocked reason。

同一步原子更新 `SyncSkillEntry` Zod schema：`namespace` 必须为 `agents | codex | claude-code`，`folder_name` 为 string，`relative_dir` 为 nullable string；删除旧 `platform/platforms`。为三个 namespace、nullable relative_dir 与 `both_deleted` 添加解析 fixture，确保前端契约与 Task 7 Rust DTO 一致。

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

添加搜索框、状态筛选、card grid、delete guard warning、删除项显式确认、commit summary。页面使用本地 `$state` 保存 `selectedActionIds` 与 `deleteGuardAck`：普通上传/下载可以默认选中，删除 action 永不默认选中。

本任务原子迁移 Apply 的 schema、API adapter 和页面调用：

```ts
export const applyResultSchema = z.object({
  applied: z.array(z.string()),
  state_updated: z.array(z.string()),
  warnings: z.array(z.string()),
  remote_commit: z.string().nullable(),
})

export const applySyncRequestSchema = z.object({
  expected_remote_commit: z.string(),
  plan_fingerprint: z.string(),
  selected_action_ids: z.array(z.string()),
  decisions: z.record(z.string(), syncDecisionSchema),
  delete_guard_ack: z.boolean(),
})

export const applySyncResponseSchema = z.discriminatedUnion('status', [
  z.object({ status: z.literal('applied'), result: applyResultSchema }),
  z.object({
    status: z.literal('plan_changed'),
    reason: z.enum(['remote_changed', 'plan_changed']),
    latest_plan: syncPlanSchema,
  }),
])
```

同一任务内再添加使用新 `applyResultSchema` 的 `uploadSkills(skillIds)` 与 `downloadSkills(skillIds)` API。下载 API 不接收本地路径或 namespace override，目标完全由远端 entry namespace/folder_name 与固定 registry 决定。

在本任务中同步迁移 API adapter，保证 adapter 与页面始终一起通过 typecheck：

```ts
export const applySyncPlan = async (
  request: ApplySyncRequest,
): Promise<ApplySyncResponse> => {
  const parsedRequest = applySyncRequestSchema.parse(request)
  const raw = await invokeCmd<unknown>('apply_sync_plan', {
    request: parsedRequest,
  })
  return applySyncResponseSchema.parse(raw)
}
```

Apply 调用：

```ts
applySyncPlan({
  expected_remote_commit: plan.expected_remote_commit,
  plan_fingerprint: plan.plan_fingerprint,
  selected_action_ids: [...selectedActionIds],
  decisions: syncDecisions.decisions,
  delete_guard_ack: deleteGuardAck,
})
```

`applied` 分支从 `response.result.applied`、`response.result.warnings` 和 `response.result.remote_commit` 读取结果，不再读取 `backups`。

当 `base_adoptions` 或 `base_removals` 非空时，即使没有选中的普通动作和 conflict decision，Apply 按钮仍可用。UI 显示 `commit_summary.local_state_updates` 和“仅保存本机同步基线，不会产生 GitHub commit”；成功后使用 `response.result.state_updated` 展示基线更新数量。

收到 `plan_changed` 响应时，使用 `queryClient.setQueryData(['sync-plan'], response.latest_plan)` 直接替换当前计划，清空 `selectedActionIds`、`syncDecisions` 和 `deleteGuardAck`，显示计划已变化提示并要求用户重新确认；禁止自动重试 Apply，mutation 显式设置 `retry: false`。

任何手动 recheck 或其他 query 更新导致展示中的 `plan_fingerprint` 变化时，同样清空上述三类状态。非删除动作只允许在首次成功加载或用户明确点击 Recheck 后按新 fingerprint 初始化默认选择；`plan_changed` 分支安装 `latest_plan` 时保持选择为空，不能立即重新默认选中。

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
  "sync.planChanged": "同步计划已变化，请重新确认",
  "sync.localBaseOnly": "仅保存本机同步基线，不会产生 GitHub commit",
  "sync.localBaseUpdated": "已更新 {{count}} 个本机同步基线",
  "sync.conflictDetail.localDeletedRemoteChanged": "本地已删除，但云端已修改",
  "sync.conflictDetail.remoteDeletedLocalChanged": "云端已删除，但本地已修改"
}
```

- [ ] **Step 7: 验证**

```bash
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
npm run lint
npm run typecheck
npm run build
```

手动验证：删除 action 首次与计划变化后均不选中；Apply 只调用一次且 request 五个字段完整；`applied` 正确读取嵌套 result；`plan_changed` 使用 latest_plan 替换缓存并清空选择、决策和 ack，不触发自动 retry/refetch；分别用 adoption-only 与 removal-only 计划确认 Apply 仍可用、普通选择和 decisions 保持为空、显示 0 commit，并使用 `state_updated` 展示结果。

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

连接按钮调用 `startGithubDeviceFlow()`，展示 `userCode` 和打开 `verificationUri`。按 interval 轮询到 authorized/expired/denied；收到 slow_down 时采用响应的新 interval。授权成功且 owner/repo/branch 完整后调用 `checkGithubVault(config)`：command error 显示认证/repo/branch 问题，`manifest_exists=false` 显示创建空 vault 提示，true 才进入 ready。

- [ ] **Step 4: 更新 Settings**

删除 `config.defaults.backup` checkbox，新增：

```ts
config.remote.owner
config.remote.repo
config.remote.branch
config.limits.max_skill_zip_bytes
config.limits.max_auto_delete
```

同一任务原子迁移 `appConfigSchema` 与 Onboarding schema：定义 `remoteConfigSchema` 和 `limitsConfigSchema`，从 `appConfigSchema` 删除 `hosts`、`custom_paths` 和旧 `defaults`；同步更新所有 Settings/Onboarding 消费者。root 状态来自 scan/AppState 响应，不属于可保存配置。

保留 ignore textarea，文案说明它影响 zip/hash/size/status。

Settings 还只读显示 `AppState.device_name` 与 nullable `remote_commit`；没有 commit 时显示 locale 中的“尚未同步”，不得从可编辑表单伪造这两个运行时字段。

新增固定 root 只读列表，数据来自 `scanSkills().roots`：

```text
agents       ~/.agents/skills       resolved path + exists/readable/scan status
codex        ~/.codex/skills        resolved path + exists/readable/scan status
claude-code  ~/.claude/skills       resolved path + exists/readable/scan status
```

删除 hosts/custom_paths、root enable toggle、路径输入和目录选择按钮。用户可以打开/复制解析后的路径，但不能通过 Settings 修改、禁用或新增 root。新增 namespace/root 状态中英文 locale keys，并明确项目级目录不在 V1 管理范围。

```json
{
  "settings.skillRoots": "Skill 目录",
  "settings.skillRootsReadOnly": "目录由对应工具约定，只读展示",
  "settings.namespace.agents": "Agent Skills 共享目录",
  "settings.namespace.codex": "Codex Skills 目录",
  "settings.namespace.claudeCode": "Claude Code Skills 目录",
  "settings.deviceName": "本机设备名",
  "settings.remoteCommit": "当前远端版本",
  "settings.remoteCommitEmpty": "尚未同步",
  "settings.rootNotFound": "目录尚未创建",
  "settings.rootUnreadable": "目录不可读"
}
```

- [ ] **Step 5: 验证**

```bash
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
npm run typecheck
npm run lint
npm run build
```

手动验证：三个 namespace 均显示唯一固定路径；没有 root 输入、开关或新增按钮；修改旧 config YAML 的 hosts/custom_paths 不会改变页面或扫描目标；缺失 root 显示 unknown/待创建而不产生删除。

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
rg -n "HostsConfig|HostConfig|custom_paths|enabled_hosts|repo_path|\bplatforms?\b|\bhosts?\b" src-tauri/src src src/shared src/routes
cargo test --manifest-path src-tauri/Cargo.toml
npm run lint
npm run build
```

Expected: 两个 `rg` 均无产品代码命中；Rust tests pass。

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
rg -n "Git CLI|git_store|GitStore|check_git|check_remote|prepare_repo|Backups|backup|restore|SSH|skill-sync.lock.json|skills/<host>/<name>|HostsConfig|HostConfig|custom_paths|enabled_hosts|repo_path|\bplatforms?\b|\bhosts?\b" README.md README.zh-CN.md docs src src-tauri
```

Expected: 只有历史上下文或明确非 MVP 注记保留。

- [ ] **Step 2: 更新文档**

README 和开发文档说明 GitHub private repo vault、Device Flow、`manifest.json` + `blobs/sha256`、三个固定 namespace/root、advisory 删除传播、本地 trash、blob 不 GC、不需要 Git/SSH。明确 V1 不管理项目级 skill roots，也不允许自定义用户 root。

- [ ] **Step 3: 全量验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
rg -n "reqwest::blocking|block_on|Runtime::new" src-tauri/src
npm run typecheck
npm run format:check
npm run lint
npm run build
```

Expected: tests/build checks all pass；禁止项 `rg` 无命中。

- [ ] **Step 4: 手动烟测**

Run:

```bash
npm run tauri dev
```

检查：

- App opens to Sync。
- Nav 没有 Conflicts 或 Backups。
- Onboarding 展示 GitHub Device Flow。
- Settings 有 GitHub repo、size limit、delete guard、ignore rules，以及 agents/codex/claude-code 三个只读固定 root；没有 root 输入、开关或新增按钮。
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
- Apply 前必须校验 `expected_remote_commit + plan_fingerprint`；不匹配时返回 `latest_plan`，不得执行任何持久化的本地或远端副作用，并清理用于重建计划的临时 pack。
- 普通动作只执行 `selected_action_ids` 中的条目；普通删除未显式选择一律跳过。
- delete guard 已触发且本次包含删除时，必须校验 `delete_guard_ack`；收到 `PlanChanged` 后旧选择、旧 decisions 和旧 ack 全部作废。
- preview 不得写 base；第 3/5 行通过 `base_adoptions`、第 13 行通过 `base_removals` 延迟到 Apply。仅状态协调的 Apply 必须写 sync_state、返回 `state_updated`，并保持 0 GitHub commit。
- RemoteStore、build/prepare/apply/batch 与同步相关 Tauri commands 从 Task 6 起保持 async；GitHub I/O 直接 await，扫描、pack、hash、解包、目录移动和 sync_state 文件 I/O 使用 `tauri::async_runtime::spawn_blocking`。
- 禁止 `reqwest::blocking`、`block_on`、`tokio::runtime::Runtime::new` 或任何嵌套 runtime；async 测试使用 `#[tokio::test]`。
- namespace 固定为 agents/codex/claude-code，root 路径由代码 registry 决定；不要恢复 hosts/custom_paths、可编辑路径、项目级 root 扫描或“第一个匹配 root”回退。
- codex namespace 扫描 `~/.codex/skills` 的直接子目录，但必须排除 `.system`，且不递归进入没有合法 `SKILL.md` 的运行时聚合目录。
- Tauri `SyncOperationGate` 覆盖完整本机同步事务：preview/scan 读锁，apply/upload/download/save_config 写锁；LocalVaultStore 另用按 path 共享的 RwLock 保证 snapshot/commit 原子可见。
