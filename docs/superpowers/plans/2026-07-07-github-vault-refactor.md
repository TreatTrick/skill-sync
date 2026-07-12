# GitHub Vault 重构实施计划

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将当前 Git CLI / 本地中转仓库同步模型替换为 GitHub App Device Flow 授权的单仓库 GitHub API vault：`manifest.json` + content-addressed skill zip blobs + 本机 `sync_state` + 双向同步 + A-lite 删除传播。

**Architecture:** Svelte 前端只负责认证门禁、渐进式 Onboarding、展示、筛选、用户决策和调用 Tauri commands；未完成 Ready Vault binding 时只渲染 Onboarding，完成后工作区导航只包含 Sync / Settings。本地文件读写、canonical zip、hash、GitHub API、三方比较、删除护栏、覆盖保护全部在 Rust。V1 唯一产品远端使用 GitHub App user token；GitHub 授权/credential 轮换、repository discovery/初始化和 steady-state store 分模块实现，空仓库初始化不混入 `RemoteStore` 的常规 commit 语义。

**Tech Stack:** Tauri 2, Rust 1.77+, Svelte 5, SvelteKit static SPA, TypeScript strict, Zod, i18next, Tailwind CSS v4, GitHub App Device Flow, GitHub REST Contents/Git Database APIs.

---

## Source Of Truth

- 设计文档：`docs/github-base-refactor-design.md`
- 删除补充设计：`C:\Users\11541\.claude\plans\docs-github-base-refactor-design-md-1-sk-federated-torvalds.md`
- 仓库规则：`AGENTS.md`

关键不变量：

- 不保留 Git CLI fallback，不再要求用户安装 Git、配置 SSH 或填写 Git remote URL。
- 远端 vault 只放 `manifest.json` 和 `blobs/sha256/<hash>.skill.zip`，不再展开 `skills/<host>/<name>`。
- hash 事实来源是 canonical zip bytes，manifest hash、本地 base 比较、blob 校验必须使用同一 hash。
- canonical zip 固定 UTF-8 NFC path、排序、ZIP epoch、Unix creator system、regular-file external attributes + 0644、Deflate level 6、无目录项/comment/extra；pack/unpack 拒绝链接/reparse、特殊 entry 和 portable path collision。
- 上传与下载共同执行 compressed bytes、file count、single unpacked bytes、total unpacked bytes 四项 limit；解包只进 temp，headers 与实际流式 bytes 都校验。
- 下载在目标 namespace root 内同盘 staging，校验 `SKILL.md`/identity 后才 rename；本地目录替换、trash move 与 sync_state 保存由可恢复 apply journal 串联。
- manifest parser 严格校验 schema、重复 map key、key/id/namespace、64 位 lowercase hash、hash-derived blob path、size 和 folder/collision；单项失败即整个 manifest invalid。
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
- V1 只支持 GitHub App Device Flow；不实现 OAuth App、PAT、Git URL、HTTPS、SSH 或 GitHub Enterprise fallback。
- GitHub App 只申请 Contents read/write + Metadata read-only；Device Flow 不发送 OAuth `repo` scope。
- access/refresh token 与过期时间作为一个 versioned credential 存入 keyring；刷新不使用 client secret，轮换持久化失败要求重新授权。
- client id/app slug 是构建时公开配置；release 缺失即失败，private key/client secret 永不进入桌面包。
- 授权后穷尽分页检查所有 user installations/repositories；V1 总计非唯一 repo 或 `repository_selection=all` 时 blocked。
- AppConfig 与 SyncState 均绑定 `installation_id + repository_id + branch`；远端 identity 变化不能复用旧 base。
- repo/branch/manifest 状态精确区分；invalid manifest 不覆盖，rate limit 不伪装成权限或不存在。
- empty repo / missing manifest 只在 Onboarding 显式确认后由 Contents API 初始化；有 HEAD 后稳态更新使用 Git Database API。
- Device Flow authorized 只是 Onboarding 的一个步骤；App installation、唯一 repo、branch、Ready manifest 和本机 bind 全部完成前不得显示业务导航。绑定完成后 authenticated navigation 严格只有 Sync / Settings，Onboarding 只作为隐藏引导路由。
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
- `src-tauri/src/pack.rs`：canonical zip、portable path/link 检测、四项资源 limit、安全流式 unpack、临时目录清理。
- `src-tauri/src/portable_path.rs`：共享 UTF-8/NFC/component 校验与跨平台 collision key。
- `src-tauri/src/local_apply.rs`：同盘 staging、rollback/trash、apply journal、目录提交与幂等恢复。
- `src-tauri/src/vault_manifest.rs`：云端 `manifest.json` DTO 与校验。
- `src-tauri/src/sync_state.rs`：本机 `base_hash` / `remote_commit` 状态。
- `src-tauri/src/remote_store.rs`：`RemoteStore` trait 与远端 DTO。
- `src-tauri/src/local_vault_store.rs`：测试/开发用本地 vault。
- `src-tauri/src/github_auth.rs`：GitHub App Device Flow 与 refresh HTTP client。
- `src-tauri/src/github_app_config.rs`：编译时 GitHub App client id / slug。
- `src-tauri/src/github_credentials.rs`：keyring credential、过期判断和单飞刷新。
- `src-tauri/src/github_repository.rs`：installation/repository 枚举、状态探测和显式初始化。
- `src-tauri/src/github_store.rs`：GitHub manifest/blob/commit 适配器。
- `src-tauri/src/vault_binding.rs`：ready vault 的 AppConfig/SyncState journaled bind/rebind transaction。
- `src-tauri/src/sync_engine/vault.rs`：新 vault DTO、plan/apply 实现；Task 13 后由 sync_engine boundary re-export。
- `src-tauri/tests/github_vault_sync.rs`：基于 `LocalVaultStore` 的同步测试。
- `src/modules/sync/lib/syncStatus.ts`：同步状态筛选和搜索 helper。
- `src/modules/sync/components/SyncSkillCard.svelte`：同步状态卡片。
- `src/modules/sync/components/ConflictDetailDialog.svelte`：冲突摘要和决策弹窗。

Modify:

- `src-tauri/Cargo.toml`
- `src-tauri/build.rs`
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
- `src/routes/+page.svelte`
- `src/routes/app/+page.svelte`
- `src/routes/app/+layout.svelte`
- `src/shared/i18n/locales/zh-CN.json`
- `src/shared/i18n/locales/en-US.json`
- `src/shared/schemas/apiResponse.ts`
- `.github/workflows/release.yml`

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
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/ignore.rs`
- Create: `src-tauri/src/pack.rs`
- Create: `src-tauri/src/portable_path.rs`
- Create: `src-tauri/src/local_apply.rs`
- Create: `src-tauri/src/vault_manifest.rs`
- Create: `src-tauri/src/sync_state.rs`
- Create: `src-tauri/src/remote_store.rs`
- Create: `src-tauri/src/local_vault_store.rs`
- Create: `src-tauri/src/github_app_config.rs`
- Create: `src-tauri/src/github_auth.rs`
- Create: `src-tauri/src/github_credentials.rs`
- Create: `src-tauri/src/github_repository.rs`
- Create: `src-tauri/src/github_store.rs`

- [x] **Step 1: 添加 Rust 依赖**

```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
async-trait = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "sync"] }
zip = { version = "2", default-features = false, features = ["deflate"] }
uuid = { version = "1", features = ["v4", "serde"] }
base64 = "0.22"
keyring = "3"
secrecy = "0.10"
unicode-normalization = "0.1"

[dev-dependencies]
wiremock = "0.6"
```

`wiremock` 加入现有 `[dev-dependencies]`（与 `tempfile` 同一 section），不要创建重复的 Cargo table。

提交 dependency resolution 后的 `src-tauri/Cargo.lock`。canonical ZIP 的 golden bytes 测试必须在 zip/Deflate 依赖升级时继续通过；若升级改变 bytes，不能在 schema 1 下直接更新 golden，必须把它作为 canonical format/schema 迁移处理。

- [x] **Step 2: 注册新模块**

在 `src-tauri/src/lib.rs` 加：

```rust
mod github_auth;
mod github_app_config;
mod github_credentials;
mod github_repository;
mod github_store;
mod ignore;
mod local_vault_store;
mod local_apply;
mod pack;
mod portable_path;
mod remote_store;
mod sync_state;
mod vault_manifest;
```

旧 `backup`、`git_store`、`manifest` 暂时保留，等 Task 18 删除。

- [x] **Step 3: 创建可编译模块文件**

每个新文件先放最小占位内容：

```rust
// GitHub vault implementation lands in later tasks.
```

- [x] **Step 4: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-run
```

Expected: dependency resolution succeeds；若失败，只应是后续未实现引用。

- [x] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src
git commit -m "build: add github vault backend modules"
```

### Task 2: 定义 GitHub vault 配置 DTO

**Files:**

- Modify: `src-tauri/src/config.rs`

- [x] **Step 1: 写配置测试**

```rust
#[test]
fn github_remote_config_roundtrip_preserves_stable_identity() {
    let remote = RemoteConfig {
        installation_id: 123456,
        repository_id: 987654,
        owner: "example".into(),
        repo: "agent-skills".into(),
        branch: "main".into(),
    };
    let back: RemoteConfig = serde_yaml::from_str(&serde_yaml::to_string(&remote).unwrap()).unwrap();
    assert_eq!(back, remote);
}

#[test]
fn default_limits_include_pack_and_unpack_budgets() {
    let limits = LimitsConfig::default();
    assert_eq!(limits.max_skill_zip_bytes, 20 * 1024 * 1024);
    assert_eq!(limits.max_skill_files, 2_000);
    assert_eq!(limits.max_single_file_unpacked_bytes, 50 * 1024 * 1024);
    assert_eq!(limits.max_skill_unpacked_bytes, 100 * 1024 * 1024);
    assert!(limits.validate().is_ok());
}
```

- [x] **Step 2: 实现 Rust DTO**

替换 `RepositoryConfig` 和 `DefaultsConfig.backup`：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteConfig {
    pub installation_id: u64,
    pub repository_id: u64,
    pub owner: String,
    pub repo: String,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsConfig {
    #[serde(default = "default_max_skill_zip_bytes")]
    pub max_skill_zip_bytes: u64,
    #[serde(default = "default_max_skill_files")]
    pub max_skill_files: usize,
    #[serde(default = "default_max_single_file_unpacked_bytes")]
    pub max_single_file_unpacked_bytes: u64,
    #[serde(default = "default_max_skill_unpacked_bytes")]
    pub max_skill_unpacked_bytes: u64,
    #[serde(default = "default_max_auto_delete")]
    pub max_auto_delete: usize,
}
```

`LimitsConfig::validate` 拒绝 0，并要求 `max_single_file_unpacked_bytes <= max_skill_unpacked_bytes`。这四个 pack/unpack limit 必须作为一个对象传给 packer/unpacker；不得在下载路径使用硬编码的另一组值。

本任务只新增 `RemoteConfig` / `LimitsConfig` 与默认值 helper，不立刻替换现有 `AppConfig` 字段；旧 commands/detect/sync_engine 仍需编译。Task 13 在重接所有 Rust consumers 的同一提交中把 `AppConfig` 原子迁移为 `remote: Option<RemoteConfig> + limits + ignore`，并删除旧 repository/hosts/custom_paths/defaults。新 GitHub 模块从本任务起只使用新 DTO，不读取旧字段。

- [x] **Step 3: 扩展默认 ignore**

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

- [x] **Step 4: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml config
```

Expected: 新 DTO tests pass，现有 crate consumers 仍编译。Rust AppConfig 原子迁移在 Task 13，前端 schema 和消费者在 Task 17 迁移。

- [x] **Step 5: Commit**

```bash
git add src-tauri/src/config.rs
git commit -m "feat: define github vault config dto"
```

### Task 3: 定义 VaultManifest 与 SyncState

**Files:**

- Create/Modify: `src-tauri/src/vault_manifest.rs`
- Create/Modify: `src-tauri/src/sync_state.rs`
- Create/Modify: `src-tauri/src/portable_path.rs`
- Modify: `src-tauri/src/skill.rs`
- Modify: `src-tauri/src/errors.rs`

- [x] **Step 1: 写 manifest round-trip 测试**

```rust
const HASH_A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BLOB_A: &str = "blobs/sha256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.skill.zip";

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
            hash: HASH_A.into(),
            blob: BLOB_A.into(),
            size: 123,
            updated_at: "2026-07-07T13:00:00Z".into(),
            updated_by: "device-a".into(),
        },
    );
    let text = serde_json::to_string(&manifest).unwrap();
    let back: VaultManifest = serde_json::from_str(&text).unwrap();
    assert_eq!(back.skills["codex:ponytail"].hash, HASH_A);
}

#[test]
fn manifest_rejects_key_id_namespace_and_unsafe_folder_mismatches() {
    for invalid in manifest_validation_fixtures() {
        assert!(VaultManifest::parse_validated(&invalid).is_err());
    }
}

#[test]
fn manifest_rejects_schema_duplicate_keys_hash_blob_and_size_violations() {
    for invalid in [
        unsupported_schema_fixture(),
        duplicate_skill_key_fixture(),
        invalid_hash_fixture("sha256:ABC"),
        invalid_hash_fixture("sha256:abc"),
        mismatched_blob_fixture("blobs/sha256/other.skill.zip"),
        invalid_size_fixture(0),
    ] {
        assert!(VaultManifest::parse_validated(&invalid).is_err());
    }
}
```

- [x] **Step 2: 实现 manifest DTO**

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

先在 `skill.rs` 定义共享 `SkillNamespace` enum（serde 值 `agents`、`codex`、`claude-code`），供 manifest、sync_state、scanner 和 SyncPlan 复用。`VaultSkill` 使用单个 `namespace` 和 `folder_name`，删除旧 `platform/platforms`。所有远端 JSON 必须通过唯一入口 `VaultManifest::parse_validated(bytes)` 解析；不要让 adapter 直接调用可绕过校验的 `serde_json::from_slice::<VaultManifest>`。

validated parser 使用自定义 serde Visitor 拒绝重复 skill map key，并整体验证：schema 精确为 1；map key == entry id；id 是 canonical `<namespace>:<normalized-name>` 且 namespace 一致；hash 匹配 `sha256:[0-9a-f]{64}`；blob 严格等于从 hash 推导的 `blobs/sha256/<hex>.skill.zip`；size > 0；folder_name 通过 `portable_path::validate_component`；manifest 内 id/folded folder collision 全部 invalid。任一 entry 失败都返回整个 `InvalidManifest`。manifest size 超过本机 max_skill_zip_bytes 时由 SyncPlan 将该 entry blocked，不把共享 manifest 判 invalid。

- [x] **Step 3: 写 sync_state 测试**

```rust
#[test]
fn sync_state_roundtrip_preserves_base_hash() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = SyncState::empty(RemoteIdentity {
        provider: "github".into(),
        installation_id: 123456,
        repository_id: 987654,
        owner: "example".into(),
        repo: "agent-skills".into(),
        branch: "main".into(),
        commit_sha: "github-head-sha".into(),
    });
    state.skills.insert(
        "codex:ponytail".into(),
        SkillSyncState {
            base_hash: HASH_A.into(),
            last_remote_hash: HASH_A.into(),
            last_synced_at: "2026-07-07T13:00:00Z".into(),
            namespace: SkillNamespace::Codex,
            relative_dir: "ponytail".into(),
        },
    );
    state.save_to(dir.path()).unwrap();
    let back = SyncState::load_from(dir.path()).unwrap();
    assert_eq!(back.skills["codex:ponytail"].base_hash, HASH_A);
    assert_eq!(back.remote, state.remote);
    assert_eq!(back.skills["codex:ponytail"].namespace, SkillNamespace::Codex);
    assert_eq!(back.skills["codex:ponytail"].relative_dir, "ponytail");
}

#[test]
fn ordinary_load_blocks_identity_mismatch_without_archiving() {
    let dir = tempfile::tempdir().unwrap();
    save_tracked_state(dir.path(), remote_identity(1, 10, "main"));
    for remote in [
        remote_identity(2, 10, "main"),
        remote_identity(1, 20, "main"),
        remote_identity(1, 10, "other"),
    ] {
        assert!(SyncState::load_and_validate(dir.path(), &remote).is_err());
        assert!(history_files(dir.path()).is_empty());
    }
}

#[test]
fn explicit_rebind_archives_old_base_and_starts_empty() {
    let dir = tempfile::tempdir().unwrap();
    save_tracked_state(dir.path(), remote_identity(1, 10, "main"));
    let state = SyncState::rebind_remote(dir.path(), remote_identity(2, 20, "main")).unwrap();
    assert!(state.skills.is_empty());
    assert_eq!(state.remote.repository_id, 20);
    assert_eq!(history_files(dir.path()).len(), 1);
}
```

- [x] **Step 4: 实现 sync_state 存储**

保存到：

```text
<config-dir>/skill-sync/sync_state.json
```

提供：

```rust
impl SyncState {
    pub fn load_from(config_dir: &Path) -> Result<Self>;
    pub fn load_and_validate(config_dir: &Path, remote: &RemoteIdentity) -> Result<Self>;
    pub fn rebind_remote(config_dir: &Path, remote: RemoteIdentity) -> Result<Self>;
    pub fn save_to(&self, config_dir: &Path) -> Result<()>;
    pub fn load() -> Result<Self>;
    pub fn save(&self) -> Result<()>;
    pub fn empty(remote: RemoteIdentity) -> Self;
    pub fn remove_skill(&mut self, skill_id: &str);
}
```

`RemoteIdentity` 包含 provider 常量、`installation_id`、`repository_id`、owner/repo、branch 和 commit SHA。`load_and_validate` 是普通 sync 的只读入口：installation/repository/branch 任一不一致即 blocked，不移动/保存文件。`rebind_remote` 只能由 Onboarding 显式切换调用，把旧 state 原子移动到 `<config-dir>/skill-sync/history/<repository_id>-<timestamp>.json`，再创建空 base。owner/repo rename但 repository id 不变时由显式 config/state 更新事务修改展示字段。`load_from/save_to` 是可注入 config directory 的核心实现；保存使用同目录 temp + file fsync + atomic replace/rename + parent fsync（或平台等价 durable replace）。

- [x] **Step 5: 增加错误类型**

```rust
Vault(String)
RemoteChanged(String)
RemoteOutcomeUnknown { base_commit_sha: String, candidate_commit_sha: String }
Auth(String)
Blocked(String)
RecoveryPending(String)
```

- [x] **Step 6: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml vault_manifest
cargo test --manifest-path src-tauri/Cargo.toml sync_state
```

Expected: manifest 与 sync_state tests pass。

- [x] **Step 7: Commit**

```bash
git add src-tauri/src/vault_manifest.rs src-tauri/src/sync_state.rs src-tauri/src/portable_path.rs src-tauri/src/skill.rs src-tauri/src/errors.rs
git commit -m "feat: add vault manifest and sync state"
```

## Chunk 2: Pack、扫描与本地 Vault

### Task 4: 实现 ignore 与 SkillPacker

**Files:**

- Create/Modify: `src-tauri/src/ignore.rs`
- Create/Modify: `src-tauri/src/pack.rs`
- Modify: `src-tauri/src/portable_path.rs`
- Modify: `src-tauri/src/errors.rs`
- Test fixtures: `src-tauri/tests/fixtures/skills/`
- Create: `src-tauri/tests/fixtures/canonical/demo.skill.zip`

- [x] **Step 1: 写 ignore 测试**

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

- [x] **Step 2: 实现 ignore API**

```rust
pub fn is_ignored(rel_path: &str, user_ignore: &[String]) -> bool;
pub fn glob_match(pattern: &str, text: &str) -> bool;
pub fn default_ignore() -> Vec<String>;
```

- [x] **Step 3: 写 pack 测试**

```rust
#[test]
fn pack_bytes_are_stable_across_write_order_mtime_and_permissions() {
    let a = skill_fixture_with_files([("SKILL.md", "name: demo"), ("scripts/run.sh", "echo ok")]);
    let b = skill_fixture_with_files([("scripts/run.sh", "echo ok"), ("SKILL.md", "name: demo")]);
    set_distinct_mtime_and_permissions(a.path(), b.path()).unwrap();
    let first = pack_fixture(a.path(), &test_limits()).unwrap();
    let second = pack_fixture(b.path(), &test_limits()).unwrap();
    assert_eq!(fs::read(first.zip_path()).unwrap(), fs::read(second.zip_path()).unwrap());
    assert_eq!(first.hash(), second.hash());
    assert_eq!(fs::read(first.zip_path()).unwrap(), include_bytes!("../tests/fixtures/canonical/demo.skill.zip"));
}

#[test]
fn canonical_zip_has_fixed_metadata_and_no_directory_entries() {
    let skill = skill_fixture_with_files([("SKILL.md", "name: demo"), ("nested/run.sh", "echo ok")]);
    let packed = pack_fixture(skill.path(), &test_limits()).unwrap();
    let entries = inspect_zip(packed.zip_path()).unwrap();
    assert_eq!(entries.names(), ["SKILL.md", "nested/run.sh"]);
    assert!(entries.iter().all(|e| e.timestamp == zip_epoch()));
    assert!(entries.iter().all(|e| e.creator_system == CreatorSystem::Unix));
    assert!(entries.iter().all(|e| e.file_type == ZipFileType::Regular));
    assert!(entries.iter().all(|e| e.unix_mode == 0o644));
    assert!(entries.iter().all(|e| e.method == CompressionMethod::Deflated));
    assert!(entries.iter().all(|e| e.comment.is_empty() && e.extra.is_empty()));
}

#[test]
fn ignored_cache_does_not_affect_hash_or_size() {
    let skill = skill_fixture_with_files([("SKILL.md", "name: demo"), ("cache/data", "one")]);
    let first = pack_fixture(skill.path(), &test_limits()).unwrap();
    fs::write(skill.path().join("cache/data"), "different and larger").unwrap();
    let second = pack_fixture(skill.path(), &test_limits()).unwrap();
    assert_eq!((first.hash(), first.zip_size()), (second.hash(), second.zip_size()));
}

#[test]
fn oversized_pack_is_blocked() {
    let skill = skill_fixture_with_files([("SKILL.md", "name: demo"), ("payload.bin", "not-empty")]);
    assert!(matches!(pack_fixture_outcome(skill.path(), &limits_with_zip_bytes(1)).unwrap(), PackOutcome::Blocked(_)));
}

#[test]
fn portable_path_validator_rejects_exact_case_and_unicode_collisions() {
    for paths in [
        vec!["same", "same"],
        vec!["Docs/a", "docs/A"],
        unicode_nfc_collision_paths(),
    ] {
        assert!(validate_portable_paths(paths).is_err());
    }
}

#[cfg(unix)]
#[test]
fn pack_rejects_symlink_without_reading_target() {
    let skill = symlink_escape_fixture("outside-secret");
    assert!(matches!(pack_fixture_outcome(skill.path(), &test_limits()).unwrap(), PackOutcome::Blocked(_)));
    assert!(!pack_debug_output().contains("outside-secret"));
}

#[cfg(windows)]
#[test]
fn pack_rejects_junction_and_other_reparse_points() {
    let skill = junction_escape_fixture();
    assert!(matches!(pack_fixture_outcome(skill.path(), &test_limits()).unwrap(), PackOutcome::Blocked(_)));
}

#[test]
fn unpack_rejects_zip_slip_paths() {
    let zip = zip_fixture([("../evil", b"bad".as_slice())]);
    let target = tempfile::tempdir().unwrap();
    assert!(matches!(unpack_skill(zip.path(), target.path(), &test_limits()), Err(AppError::Blocked(_))));
    assert!(!target.path().join("../evil").exists());
}

#[test]
fn unpack_rejects_symlink_special_directory_and_case_collision_entries() {
    for zip in [duplicate_entry_zip_fixture(), symlink_zip_fixture(), device_zip_fixture(), explicit_directory_fixture(), case_collision_zip_fixture()] {
        let target = tempfile::tempdir().unwrap();
        assert!(unpack_skill(zip.path(), target.path(), &test_limits()).is_err());
        assert!(target.path().read_dir().unwrap().next().is_none());
    }
}

#[test]
fn unpack_enforces_file_single_and_total_actual_byte_limits() {
    for zip in [too_many_entries_fixture(), oversized_single_file_fixture(), oversized_total_fixture(), lying_header_fixture()] {
        let target = tempfile::tempdir().unwrap();
        assert!(unpack_skill(zip.path(), target.path(), &tiny_unpack_limits()).is_err());
        assert!(target.path().read_dir().unwrap().next().is_none());
    }
}

#[test]
fn pack_warns_about_probable_secrets_without_blocking() {
    let skill = skill_fixture_with_files([
        ("SKILL.md", "name: demo"),
        ("id.pem", "-----BEGIN PRIVATE KEY-----\nredacted\n-----END PRIVATE KEY-----"),
        ("token.txt", "github_pat_REDACTED_TEST_VALUE"),
    ]);
    let packed = pack_fixture(skill.path(), &test_limits()).unwrap();
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

测试模块实现上述 filesystem/ZIP fixtures、`validate_portable_paths`、`test_limits` 和 pack helpers。checked-in golden ZIP 先由独立 inspection test确认 timestamp/creator system/regular-file external attributes/mode/method/no-directory/no-extra，再作为 byte-for-byte fixture 提交；不得在测试运行时用被测 packer重写 golden。断言不得用“函数被调用”替代 bytes/hash、资源计数、warning redaction 和正式目标未变化。

- [x] **Step 4: 实现 pack DTO 和临时目录**

```rust
pub struct PackedSkill {
    pub skill_id: String,
    pub hash: String,
    pub zip_path: PathBuf,
    pub zip_size: u64,
    pub file_count: usize,
    pub unpacked_size: u64,
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

pub fn unpack_skill(zip: &Path, task_target: &Path, limits: &LimitsConfig) -> Result<UnpackReport>;
```

临时目录放在：

```text
std::env::temp_dir()/skill-sync/<uuid>/
```

`PackTaskDir` 拥有目录并实现 `Drop` 递归清理；成功路径、错误路径和提前返回都依赖同一 RAII 语义。`SkillPacker::pack_batch` 返回同时拥有 outcomes 与 task dir 的 `PackBatch`，因此返回时 zip 仍存在。`PreparedSyncPlan` 必须持有整个 `PackBatch`；不能只复制 `zip_path`，也不能从 batch 提前取出 task dir。preview 返回 DTO 或 apply 结束时 batch drop，统一清理目录。

- [x] **Step 5: 实现 canonical zip 与安全解包**

规则：

- 相对路径必须是 UTF-8，经 Unicode NFC 和 `/` 规范化后按 UTF-8 bytes 排序；应用 built-in + Settings ignore。
- `validate_portable_component` 拒绝空/`.`/`..`、绝对路径、NUL、分隔符、Windows 保留字符/设备名和末尾空格/`.`。为每个完整路径计算 NFC+lowercase collision key，精确重复或 folded collision blocked。
- 遍历使用 `symlink_metadata` 且打开前复核类型；Unix symlink blocked。Windows 检查 `FILE_ATTRIBUTE_REPARSE_POINT`，junction 和所有 reparse point blocked。只允许 regular file。
- 只写 regular file entry，不写目录 entry。entry timestamp 精确为 `1980-01-01 00:00:00`；creator system 固定为 Unix，external attributes 精确编码 regular file + `0o644`，其余 DOS/平台位清零；comment/extra fields 为空。compression method 固定为 `Deflated`，level 常量为 `6`。mtime/ctime/owner/ACL/原权限不得进入 bytes。
- pack 流式统计 file_count、每个文件实际 bytes、累计 unpacked bytes；分别校验 `max_skill_files`、`max_single_file_unpacked_bytes`、`max_skill_unpacked_bytes`，zip 完成后再校验 `max_skill_zip_bytes`。hash 为 `sha256:<64 lowercase hex>`。
- unpack 先要求 compressed bytes <= max_skill_zip_bytes；扫描 central directory 拒绝过多 entry、声明的单文件/累计 uncompressed 超限、重复/folded collision、绝对/`..`/非 portable path、显式 directory、symlink/device/FIFO/其他特殊 mode，以及非 canonical timestamp/mode/method/comment/extra。
- 实际解包通过限量 reader 流式计数，每文件和累计实际 bytes 再校验，不能信任 header。所有 parent 只在 task temp 内按需创建；任一失败清空 temp，正式 target 不变。完整成功后才由 apply 的覆盖保护原子替换。
- 对 included text files 扫描私钥 header、PEM 和常见 token，产生不含 secret 原文的结构化 warning。

- [x] **Step 6: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml ignore
cargo test --manifest-path src-tauri/Cargo.toml pack
```

Expected: ignore、golden canonical bytes/metadata、portable collision、symlink/reparse、special ZIP entry、zip-slip、四项资源 limit、header/actual bytes 双重计数、secret warning 和 RAII cleanup tests pass。

- [x] **Step 7: Commit**

```bash
git add src-tauri/src/ignore.rs src-tauri/src/pack.rs src-tauri/src/portable_path.rs src-tauri/src/errors.rs src-tauri/tests
git commit -m "feat: pack skills as canonical zip"
```

### Task 5: 实现三个固定 namespace 与扫描边界

**Files:**

- Modify: `src-tauri/src/skill.rs`
- Modify: `src-tauri/src/detect.rs`
- Modify: `src-tauri/src/sync_engine.rs`（仅补全 legacy Skill constructor）
- Modify: `src/modules/skills/schemas/skill.ts`

- [x] **Step 1: 写 skill id 测试**

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
fn scanner_excludes_local_apply_reserved_directories() {
    let result = scan_fixed_roots(local_apply_artifact_fixture()).unwrap();
    assert!(!result.paths().any(|p| p.contains(".skill-sync-staging") || p.contains(".skill-sync-rollback") || p.contains(".skill-sync-trash")));
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

- [x] **Step 2: 实现 `skill_id`**

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

- [x] **Step 3: 更新 Skill DTO**

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
    // Transitional fields used only by legacy consumers until Task 13.
    pub host: String,
    pub repo_path: String,
    pub enabled: bool,
}
```

本任务新增 namespace/folder_name/relative_dir，所有新 scanner/engine 逻辑只使用这些字段。为保证旧 commands/页面在后续原子切换前仍可编译，Rust `host: String`、`repo_path: String`、`enabled: bool` 暂按原类型保留，但不得参与新 identity、扫描或计划；同步更新 detect.rs 和 legacy sync adapter 的全部 `Skill { ... }` literals，给新字段提供由 fixed registry 得出的值。Task 13 重接 Rust consumers 时删除旧字段。`relative_dir` 在 V1 等于经验证的单段 `folder_name`。

- [x] **Step 4: 扫描结果增加 root 状态**

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

root 不存在或不可读时记录状态，不把该 namespace 下已跟踪 skill 判为删除。扫描只看固定 root 的直接子目录；codex root 显式跳过 `.system`，所有 root 显式跳过 `.skill-sync-staging`、`.skill-sync-rollback`、`.skill-sync-trash` 三个事务保留目录，其他目录没有合法 `SKILL.md` 就跳过且不递归。

同 namespace 出现相同 normalized id，或 folder_name 经 Unicode NFC + lowercase 折叠后相同，返回结构化 `ScanCollision` 并把涉及项交给 SyncPlan 标为 blocked；不得保留第一个。三个 namespace 之间同名不构成 collision。

- [x] **Step 5: 更新 TS skill schema**

```ts
namespace: z.enum(['agents', 'codex', 'claude-code']),
folder_name: z.string(),
relative_dir: z.string(),
zip_size: z.number().optional(),
host: z.string(), // transitional; removed with page migration in Task 16
repo_path: z.string(), // transitional
enabled: z.boolean(), // transitional
```

`host` 在过渡期保持 required `z.string()`（不是 optional），使现有 `hostLabel(skill.host)` 继续 typecheck；Task 16 与页面消费者同一提交删除 host/repo_path/enabled。

`scanResultSchema` 增加 `roots: ScanRootStatus[]` 与 `collisions: ScanCollision[]`；root status 的 namespace 使用同一 enum，并包含 resolved `root_path`、`exists`、`readable`、`scan_complete`、`error`。TS `ScanCollision` 完整镜像 `namespace/collision_key/kind/skill_ids/paths`，其中 kind 为 `normalized_id | folded_folder_name`。

scanner 只负责本地碰撞。远端 manifest 和 local/remote 合并后的目标碰撞由 Task 8 在生成动作前再次检测，避免两个 remote-new 条目落到同一目录。

- [x] **Step 6: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml skill
cargo test --manifest-path src-tauri/Cargo.toml detect
npm run typecheck
```

Expected: identity 和 scan tests pass。

- [x] **Step 7: Commit**

```bash
git add src-tauri/src/skill.rs src-tauri/src/detect.rs src-tauri/src/sync_engine.rs src/modules/skills/schemas/skill.ts
git commit -m "refactor: normalize skill identity"
```

### Task 6: 实现 RemoteStore 与 LocalVaultStore

**Files:**

- Create/Modify: `src-tauri/src/remote_store.rs`
- Create/Modify: `src-tauri/src/local_vault_store.rs`
- Modify: `src-tauri/src/vault_manifest.rs`

- [x] **Step 1: 定义 RemoteStore**

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

`BlobWrite.expected_hash` 必须匹配 `sha256:[0-9a-f]{64}`，path 必须严格等于从其 hex 推导的 `blobs/sha256/<hex>.skill.zip`，bytes SHA-256 必须等于 expected_hash；无效 path/hash 返回 `AppError::Vault`，不允许部分提交。`RemoteCommit.commit_sha` 是提交后 snapshot 的版本标识。

- [x] **Step 2: 写 LocalVaultStore 测试**

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

- [x] **Step 3: 实现本地 vault 布局**

```text
manifest.json
blobs/sha256/<hash>.skill.zip
.local-vault-commit
```

`.local-vault-commit` 每次 commit 写新 UUID，模拟 GitHub branch HEAD。

`fetch_blob` 在返回前计算 bytes 的 SHA-256，并与 `expected_hash` 比较；不匹配返回 `AppError::Vault`。`commit_changes` 同样在任何写入前验证所有 `BlobWrite` 的 path/hash，确保失败时没有 manifest、blob 或 commit marker 副作用。远端 adapter 若已创建 candidate commit 但 update-ref 响应不明，必须重读 ref：HEAD 等于 candidate 返回成功，HEAD 等于 base 返回明确未发布错误，其他 HEAD 返回 `RemoteChanged`；无法重读时返回携带 base/candidate SHA 的 `RemoteOutcomeUnknown`，不得伪装成普通网络错误。

`LocalVaultStore` 实现同一 async trait；其 `fs::read` / `fs::write` / rename 等同步文件操作必须放进 `tauri::async_runtime::spawn_blocking`，并把 join error 映射为 `AppError`。closure 只捕获 owned/clone 数据，不能跨 await 持有借用或锁。

为规范化 vault path 建立 process-wide path-keyed lock registry，返回共享的 `Arc<tokio::sync::RwLock<()>>`；两个独立 `LocalVaultStore::open` 指向同一路径时必须拿到同一个 Arc。`fetch_manifest` / `fetch_blob` 获取读锁，其中 manifest 与 `.local-vault-commit` 必须在同一个 blocking closure 内读取；`commit_changes` 获取写锁，并把 base commit 校验、blob/manifest 原子写入和 commit marker 更新作为一个完整 blocking transaction 执行。registry 清理过期 Weak entry，避免开发测试反复创建路径导致无界增长。

- [x] **Step 4: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml local_vault_store
cargo test --manifest-path src-tauri/Cargo.toml remote_store
rg -n "reqwest::blocking|block_on|Runtime::new" src-tauri/src
```

Expected: local vault async tests pass；`rg` 无命中。

- [x] **Step 5: Commit**

```bash
git add src-tauri/src/remote_store.rs src-tauri/src/local_vault_store.rs src-tauri/src/vault_manifest.rs
git commit -m "feat: add local vault store"
```

## Chunk 3: SyncEngine 与删除语义

### Task 7: 定义 SyncPlan DTO 和决策模型

**Files:**

- Create: `src-tauri/src/sync_engine/vault.rs`
- Modify: `src-tauri/src/sync_engine.rs`（只添加 `pub mod vault;`）

- [x] **Step 1: 定义 Rust status 和 conflict reason**

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

- [x] **Step 2: 定义 plan entry 和 plan**

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

- [x] **Step 3: 定义 Apply 请求和响应**

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryPhase {
    RemoteOutcomeUnknown,
    LocalReplaceFailed,
    TrashMoveFailed,
    StateSaveFailed,
}

pub struct RecoveryInfo {
    pub task_id: String,
    pub phase: RecoveryPhase,
    pub remote_commit: Option<String>,
    pub completed_action_ids: Vec<String>,
    pub pending_action_ids: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ApplySyncResponse {
    Applied { result: ApplyResult },
    PlanChanged {
        reason: PlanChangeReason,
        latest_plan: SyncPlan,
    },
    RecoveryRequired { recovery: RecoveryInfo },
}
```

`selected_action_ids` 只控制普通可执行项；冲突仍由 `decisions` 控制。普通删除 action 未被选择时必须跳过。`delete_guard_tripped` 且本次普通动作或冲突 decision 包含删除时，必须要求 `delete_guard_ack == true`。非法 action id、非法 decision 或缺少删除确认使用现有 `AppError::Blocked` 返回。`RecoveryRequired` 只用于已经存在持久化副作用或远端结果无法证明的事务；它不是可由 mutation 自动重试的普通错误。

decision 白名单：

| ConflictReason                 | 允许的 decision                           |
| ------------------------------ | ----------------------------------------- |
| `same_name_first_seen`         | `keep_local`, `use_remote`, `skip`        |
| `both_changed`                 | `keep_local`, `use_remote`, `skip`        |
| `local_deleted_remote_changed` | `delete_remote`, `restore_remote`, `skip` |
| `remote_deleted_local_changed` | `keep_local`, `accept_delete`, `skip`     |

未提供 decision 表示该冲突不执行并返回 warning；不存在于当前冲突集合的 skill_id 或不在白名单内的 decision 返回 `Blocked`，整次 Apply 不得部分执行。

- [x] **Step 4: 保留旧 command adapter 的编译边界**

Task 7-9 的全部新类型和函数放在独立 `crate::sync_engine::vault` 子模块，测试和新调用方使用限定名；旧 `sync_engine.rs` 的同名 `SyncPlan/build_plan/apply_plan` 保持不动，因此不存在符号冲突。Task 13 重接 commands 的同一提交中删除 legacy 实现并从 boundary module re-export vault API。前端 sync schema/decision/apply adapter 全部延后到 Task 16 原子迁移。

- [x] **Step 5: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-run
```

Expected: schema compiles；Rust 可在 Task 8 完成实现后全绿。

- [x] **Step 6: Commit**

```bash
git add src-tauri/src/sync_engine.rs src-tauri/src/sync_engine/vault.rs
git commit -m "refactor: define vault sync plan dto"
```

### Task 8: 重写 build_plan，落地 13 行真值表

**Files:**

- Modify: `src-tauri/src/sync_engine/vault.rs`

- [x] **Step 1: 写三方比较测试**

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

- [x] **Step 2: 实现 13 行状态表**

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

- [x] **Step 3: 写删除护栏测试**

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

#[test]
fn remote_entry_over_local_compressed_limit_is_blocked_before_blob_fetch() { ... }

#[test]
fn missing_remote_config_requires_onboarding() { ... }

#[test]
fn state_remote_identity_mismatch_is_blocked_before_fetch() { ... }
```

- [x] **Step 4: 实现 `build_plan`**

```rust
pub async fn build_plan<S: RemoteStore>(
    config: &AppConfig,
    state: &SyncState,
    store: &S,
) -> Result<SyncPlan>
```

流程：

1. 要求 `config.remote` 存在，并验证 state 的 installation_id/repository_id/branch 与 config 一致；不一致在任何 RemoteStore 调用前 blocked。然后 `store.fetch_manifest().await` 获取 `RemoteSnapshot`。
2. 通过 `tauri::async_runtime::spawn_blocking` 扫描三个固定 namespace roots，带回每个 namespace 唯一的 `ScanRootStatus` 与 collisions。
3. 通过 `spawn_blocking` 对每个本地 skill pack/hash/size；禁止直接在 async future 中执行目录遍历、zip 或 hash。
4. 合并 `base/local/remote`，按 13 行表判定。
5. 根据 base/state 的 namespace 定位唯一 root；该 namespace root 缺失、不可读或 `scan_complete=false` 时，只把该 namespace 下 base 中存在的 skill 标 `unknown`，不推断删除。其他 namespace 不受影响。
6. 依赖 `VaultManifest::parse_validated` 已拒绝远端 manifest 内 normalized ID 与 NFC+lowercase `folder_name` collision；合并后再校验所有会写入本地的目标 key `(namespace, folded_folder_name)`。本地 scanner 或 local/remote 跨集合 collision 涉及的全部 entry 都标为 blocked；禁止产生 upload/download/delete/adoption。跨 namespace 同名仍保持独立。远端 entry manifest size 超过本机 max_skill_zip_bytes 时同样 blocked 且不 fetch blob，但不把整个 manifest 判 invalid。
7. 删除数超过 `max_auto_delete` 或超过 tracked skills 的 50% 时设置 `delete_guard_tripped`。
8. 第 3 行生成 `BaseAdoption { skill_id, hash: local_hash }`；第 5 行仅在 `base_hash != local_hash` 时生成 adoption；第 13 行把 skill_id 加入 `base_removals`。三者都只进入计划，不在 preview 写 sync_state。
9. 为每个条目生成全计划唯一的 `action_id = <action_kind>:<skill_id>`；`action_kind` 只取 `upload`、`download`、`delete_remote`、`delete_local`、`conflict`、`synced`、`blocked`、`unknown`、`both_deleted`，重复 id 直接返回 `Blocked`。
10. 定义固定字段顺序的 `PlanFingerprintInput` / `PlanFingerprintEntry`，optional 字段缺失时保留显式 null，entries、base_adoptions、base_removals 分别按 skill/action id 字节序排序；使用 `serde_json::to_vec` 后计算 `sha256:<hex>`。输入字段严格为设计文档列出的 `expected_remote_commit`、`delete_guard_tripped`、本机状态协调与动作安全字段；展示顺序、文案、时间戳和纯展示 warning 不参与计算。
11. 预览结束清理临时 zip 目录。
12. 不写本地、不上传、不下载、不删除。

`will_create_commit` 只由实际远端变化决定，base adoption/removal 不得把它设为 true。`commit_summary.local_state_updates` 等于 adoption/removal 去重后的 skill 数量。

- [x] **Step 5: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml sync_engine::vault::tests
```

Expected: 三方比较、删除护栏、blocked size tests pass。

- [x] **Step 6: Commit**

```bash
git add src-tauri/src/config.rs src-tauri/src/sync_engine/vault.rs docs/superpowers/plans/2026-07-07-github-vault-refactor.md
git commit -m "feat: build vault sync plan with deletes"
```

### Task 9: 实现 apply、批量上传、批量下载和删除执行

**Files:**

- Create/Modify: `src-tauri/src/local_apply.rs`
- Modify: `src-tauri/src/sync_engine/vault.rs`
- Modify: `src-tauri/tests/github_vault_sync.rs`

- [x] **Step 1: 写 apply 测试**

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

#[test]
fn missing_remote_or_state_remote_mismatch_blocks_before_fetch_and_preserves_state() { ... }

#[test]
fn download_rejects_manifest_size_or_hash_mismatch_before_unpack() { ... }

#[test]
fn download_limit_or_noncanonical_archive_failure_leaves_target_unchanged() { ... }

#[test]
fn download_stage_is_on_target_filesystem_and_identity_is_revalidated() { ... }

#[test]
fn download_missing_skill_md_or_mismatched_identity_leaves_target_unchanged() { ... }

#[test]
fn replace_failure_restores_old_target_or_returns_recovery_required() { ... }

#[test]
fn trash_move_failure_returns_recovery_required_without_repeating_remote_commit() { ... }

#[test]
fn state_save_failure_resumes_without_repeating_local_or_remote_actions() { ... }

#[test]
fn remote_outcome_unknown_blocks_new_sync_until_reconciled() { ... }

#[test]
fn cleanup_failure_returns_applied_warning_and_is_cleaned_on_next_start() { ... }

#[test]
fn pending_recovery_is_idempotent_across_repeated_resume_calls() { ... }
```

adoption-only 与 removal-only 测试必须显式使用空 `selected_action_ids`、空 `decisions` 和 `delete_guard_ack=false`，并断言：返回 `Applied`、`applied == []`、`state_updated` 为正确且去重后的 skill ids、`remote_commit == None`、`commit_changes` 调用次数为 0、`state.remote.commit_sha` 更新为当前 snapshot commit，且 base 被正确写入/推进或移除。

失败路径 fixture 必须包含至少一个 pending adoption/removal：fingerprint/commit 不匹配返回 `PlanChanged`、非法 selection/decision 返回 `Blocked`、以及只调用 preview 未调用 Apply 三种情况下，均断言 sync_state bytes 未变化、`state_updated` 不返回成功结果、远端 commit 次数为 0。

- [x] **Step 2: 实现 `apply_plan`**

```rust
pub async fn apply_plan<S: RemoteStore>(
    config: &AppConfig,
    state: &mut SyncState,
    request: &ApplySyncRequest,
    store: &S,
) -> Result<ApplySyncResponse>
```

command 先只读 load/validate state，再 clone 成 working state 传入；engine 在任何持久化副作用前只修改 working state。PlanChanged/Blocked/预提交错误直接丢弃 clone，原 state bytes 不变；一旦 apply journal 已进入远端结果不明或本地提交阶段，后续失败返回 `RecoveryRequired` 并由 journal 中的 next state 恢复，不能把它降级成普通 error。batch upload/download 使用同一 transaction/state 规则。

`ApplyResult` 不再返回 backups：

```rust
pub struct ApplyResult {
    pub applied: Vec<String>,
    pub state_updated: Vec<String>,
    pub warnings: Vec<String>,
    pub remote_commit: Option<String>,
}
```

- [x] **Step 3: 执行前重新生成计划**

async apply 必须通过内部 async `prepare_plan` await manifest，并以 `spawn_blocking` 重新 scan/pack/hash、重建计划，返回同时持有 `SyncPlan`、本次 `PackedSkill` 和 RAII 临时目录的 `PreparedSyncPlan`。预览阶段 zip 不允许复用，指纹校验后也不得再次 pack；后续上传必须使用 `PreparedSyncPlan` 中参与校验的准确 bytes。在任何持久化副作用（本地 skill 写入、trash move、远端提交或 sync_state 更新）之前：

0. 要求 config.remote 存在，并验证传入 state 的 installation_id/repository_id/branch；缺失或 mismatch 在第一次 `RemoteStore` 调用前返回 OnboardingRequired/Blocked，remote fetch count 为 0，working/original state 和本地目录均不变。

1. 比较 `request.expected_remote_commit` 与新计划的 `expected_remote_commit`。
2. 比较 `request.plan_fingerprint` 与新计划的 `plan_fingerprint`。
3. 任一不一致时返回 `ApplySyncResponse::PlanChanged { reason, latest_plan }`，不合并旧 decisions，不产生任何持久化副作用，并清理本次临时 pack。expected remote commit 不同优先使用 `RemoteChanged` reason；commit 相同但 fingerprint 不同使用 `PlanChanged` reason。
4. 校验 `selected_action_ids` 都存在于新计划且仍可执行；只执行被选择的普通动作。不存在、重复或不可执行的 action id 返回 `Blocked`，不得部分执行。
5. 校验每个 conflict decision 仍被当前 `ConflictReason` 允许。
6. 如果 delete guard 已触发且本次选择包含普通删除或删除型 conflict decision，要求 `delete_guard_ack == true`。

预检通过后到 GitHub branch ref 更新前仍可能发生竞态。下载内容先解包到各目标 namespace root 内的同盘 staging，delete_local 只准备同 root trash target，不立即移动；全部 staged 内容和 next state 都验证完成后，原子写入 apply journal，再执行可能的远端提交。`commit_changes` 的 `RemoteChanged` 不得强推：重新生成 `latest_plan`，清理未提交的 staging/journal，不更新 sync_state，并返回 `ApplySyncResponse::PlanChanged { reason: RemoteChanged, latest_plan }`。update-ref 响应不明时 store 先重读 ref；仍无法证明 candidate 已发布或未发布时保留 journal/staging，返回 `RecoveryRequired(RemoteOutcomeUnknown)`。

- [x] **Step 4: 实现下载覆盖保护**

- 目标不存在则写入。
- 目标存在且当前 local hash == base hash 才允许覆盖。
- 目标存在但无 base，或 local hash != base，返回 conflict/blocked，不静默覆盖。
- 云端新增目标严格为 `fixed_root(entry.namespace)/validated(folder_name)`；更新目标严格为 `fixed_root(state.namespace)/state.relative_dir`。
- remote-new 的固定 root 不存在时，Apply 可以先创建 root；该状态在 preview 仍不得触发任何本地删除推断。
- manifest namespace、skill_id namespace、state namespace 任一不一致，或 folder_name/relative_dir 不是安全单段目录时返回 blocked；不得回退到“第一个 root”或跨 namespace 写入。
- fetch blob 后先断言 actual compressed length == manifest size 且 SHA-256 == manifest hash，再调用 Task 4 的 canonical/resource-validating `unpack_skill`。解包后根级 `SKILL.md` 必须是 regular file，metadata 必须可解析，`skill_id(entry.namespace, metadata.name)` 必须等于 manifest map key/entry id。任一 size/hash/noncanonical entry/unpack limit/SKILL.md/identity 校验失败都清理 stage，现有 target 和 sync_state bytes 不变。
- staging 固定为 `<namespace-root>/.skill-sync-staging/<task-id>/<folder_name>`；不能使用 `std::env::temp_dir()` 作为最终 rename 来源。scanner 必须忽略所有 `.skill-sync-*` 保留目录。

- [x] **Step 5: 实现本地 apply transaction 与恢复**

在 `local_apply.rs` 定义 `LocalApplyTransaction`、versioned `ApplyJournal`、`LocalAction` 和 `recover_pending`。单一 pending journal 位于 `<config-dir>/skill-sync/apply-transaction.json`，字段包含 task/phase、remote base/candidate/result SHA、next manifest hash、next state bytes/hash，以及每个 action 的 stage/target/rollback/trash、expected-before/after hash 和完成状态。journal path 恢复时必须重新约束在 config dir/fixed roots 内，不能信任文件中的任意绝对路径。

目录提交规则：目标不存在时 stage -> target；目标存在时 target -> `<root>/.skill-sync-rollback/<task-id>/<folder>`，再 stage -> target。第二次 rename 失败时立即恢复 rollback；远端已发布、无法完整恢复或已有其他本地动作完成时返回 `RecoveryRequired(LocalReplaceFailed)`。delete_local 使用 target -> `<root>/.skill-sync-trash/<task-id>/<folder>` 的同盘 rename。每完成一个动作就用同目录 temp + fsync + atomic replace + parent fsync（或平台等价 durable replace）推进 journal。root 由本事务首次创建时在 journal 记录 `created_root`，预提交清理只能删除仍为空且仍由本事务拥有的 root。

sync_state 使用同样的 durable replace 写入 journal 预先记录的 next bytes。state 成功前不得删除 rollback；state 成功后 cleanup 失败返回 `Applied` 并追加 `cleanup_pending` warning，由启动或下一次写锁幂等清理。启动/get_app_state 和 mutation command 在 write lock 内检查 pending journal；preview/scan 在 read lock 内发现 journal 时只返回 `recovery_pending`，不得锁升级或读取部分 state。`resume_sync_recovery(task_id)` 只按 journal expected hashes 继续未完成动作，不重新提交远端或重复已完成 move；证据不一致时 fail closed。

实现和测试必须逐行锁定以下结果，不能把已产生副作用的失败统一映射成普通 `AppError`：

| 故障点                                 | 返回                                     | 后续动作                             |
| -------------------------------------- | ---------------------------------------- | ------------------------------------ |
| stage/blob/hash/SKILL.md/identity 失败 | `Blocked`/validation error               | 清理 stage，重新 preview             |
| 远端明确未发布                         | retryable error                          | 本地/state 不变，重新 preview        |
| HEAD 已变且非 candidate                | `PlanChanged(RemoteChanged)`             | 安装 latest plan，用户重新确认       |
| 远端结果无法证明                       | `RecoveryRequired(RemoteOutcomeUnknown)` | 保留 journal/stage，只允许 resume    |
| replace 无法恢复                       | `RecoveryRequired(LocalReplaceFailed)`   | 依据 before/after hash 幂等恢复/继续 |
| trash rename 失败                      | `RecoveryRequired(TrashMoveFailed)`      | 只重试 pending move                  |
| state durable replace 失败             | `RecoveryRequired(StateSaveFailed)`      | 只重写 next state                    |
| state 成功、cleanup 失败               | `Applied` + `cleanup_pending` warning    | 后台/下次启动清理                    |

- [x] **Step 6: 实现上传和删云端**

上传写入 blob 并更新 `next_manifest.skills`。删云端只从 `next_manifest.skills` 移除条目，不删除 blob，并与上传合并为一次 `commit_changes`。

- [x] **Step 7: 实现删本地进 trash**

路径：

```text
<namespace-root>/.skill-sync-trash/<task-id>/<folder_name>/
```

trash 必须与 target 位于同一 namespace root/文件系统，使用 rename 而不是跨卷 copy+delete。journal 单独记录 skill id，不把含 `:` 的 id 用作目录名。

- [x] **Step 8: 更新 sync_state**

- 成功 upload/download 后写入新的 `base_hash`。
- 成功 upload/download 同时写入 `namespace` 与 `relative_dir`，使后续覆盖与删除护栏定位同一固定 root。
- 保存 state 时保留已验证的 installation_id/repository_id/branch，只更新 owner/repo rename 展示字段和 commit SHA；RemoteSnapshot 不得改变 stable IDs。
- 对 `base_adoptions` 写入 `base_hash` / `last_remote_hash = adoption.hash` 并更新 `last_synced_at`。
- 对 `base_removals` 移除对应 skill state；`both_deleted` 不再依赖存在其他动作才清理。
- `local_deleted` 删云端成功后移除 base。
- `remote_deleted` 删本地成功后移除 base。
- skip 或 blocked 不改 base。
- 计划只有 adoption/removal、`selected_action_ids` 和 decisions 都为空时，仍原子保存 sync_state，更新 state.remote.commit_sha，返回 `Applied`，且不调用 `commit_changes`。
- `ApplyResult.state_updated` 返回 adoption/removal 的去重 skill ids；这些 id 不混入实际上传、下载或删除动作的 `applied`。
- 保存执行 `write temp -> flush/fsync file -> atomic replace/rename -> fsync parent`；失败返回 `RecoveryRequired(StateSaveFailed)` 并保留 rollback/trash/journal，resume 只重写 next state，不重复远端 commit 或本地动作。

- [x] **Step 9: 实现批量内部函数**

```rust
pub async fn upload_skills<S: RemoteStore>(skill_ids: &[String], ... ) -> Result<ApplySyncResponse>;
pub async fn download_skills<S: RemoteStore>(skill_ids: &[String], ... ) -> Result<ApplySyncResponse>;
```

复用 async plan/apply 和同一 `LocalApplyTransaction`，不开平行同步流程。下载后的解包、目录替换、trash move、journal 与 sync_state 文件保存通过 `spawn_blocking` 执行；RemoteStore 操作直接 await。

- [x] **Step 10: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test github_vault_sync
```

Expected: apply、删除、批量、RemoteChanged、同盘 staging、identity revalidation、逐故障点 RecoveryRequired 和幂等 resume tests pass。

- [x] **Step 11: Commit**

```bash
git add src-tauri/src/local_apply.rs src-tauri/src/sync_engine/vault.rs src-tauri/tests/github_vault_sync.rs
git commit -m "feat: apply vault sync plan with deletes"
```

## Chunk 4: GitHub 与 Tauri Commands

### Task 10: 实现 GitHub App 构建配置、Device Flow 与 credential 轮换

**Files:**

- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/build.rs`
- Modify: `.github/workflows/release.yml`
- Create/Modify: `src-tauri/src/github_app_config.rs`
- Create/Modify: `src-tauri/src/github_auth.rs`
- Create/Modify: `src-tauri/src/github_credentials.rs`
- Modify: `src-tauri/src/errors.rs`

- [x] **Step 1: 写公开构建配置测试**

```rust
#[test]
fn embedded_config_requires_non_empty_client_id_and_slug() {
    assert!(GithubAppPublicConfig::new("", "skill-sync").is_err());
    assert!(GithubAppPublicConfig::new("Iv1.test", "").is_err());
    assert!(GithubAppPublicConfig::new("Iv1.test", "skill-sync").is_ok());
}

#[test]
fn public_config_contains_no_private_key_or_client_secret_fields() {
    let fields = GithubAppPublicConfig::public_field_names();
    assert_eq!(fields, ["client_id", "slug"]);
}
```

`GithubAppPublicConfig { client_id, slug }` 只接受编译时值，另提供 test constructor。生产代码不得调用 `std::env::var`。`build.rs` 读取 `SKILL_SYNC_GITHUB_APP_CLIENT_ID` / `SKILL_SYNC_GITHUB_APP_SLUG` 并以 `cargo:rustc-env` 注入；普通 debug/test 构建可返回明确 `GithubAppNotConfigured`，release workflow 必须在 `tauri-action` 前检查 GitHub repository variables 非空并传入 build env，缺失直接失败。

```rust
fn main() {
    for key in ["SKILL_SYNC_GITHUB_APP_CLIENT_ID", "SKILL_SYNC_GITHUB_APP_SLUG"] {
        println!("cargo:rerun-if-env-changed={key}");
        let value = std::env::var(key).unwrap_or_default();
        if std::env::var("PROFILE").as_deref() == Ok("release") && value.trim().is_empty() {
            panic!("missing required release build variable: {key}");
        }
        println!("cargo:rustc-env={key}={value}");
    }
    tauri_build::build();
}
```

release workflow 在 validate step 与 `tauri-action` env 都传 `${{ vars.SKILL_SYNC_GITHUB_APP_CLIENT_ID }}` / `${{ vars.SKILL_SYNC_GITHUB_APP_SLUG }}`。它们是公开 repository variables，不是 GitHub secrets；现有 `GITHUB_TOKEN` 仍只用于发布 release artifact。

- [x] **Step 2: 写 Device Flow 请求与状态测试**

wiremock 覆盖：start 只发送 `client_id`，明确断言没有 `scope`；poll 发送 client id、device code 和 device grant type；pending 保持 interval，slow_down 增加 5 秒，expired/denied 终止；成功响应解析 `access_token/refresh_token/expires_in/refresh_token_expires_in`，但公共 `DeviceFlowPoll` JSON 不含任何 token。

- [x] **Step 3: 实现 Device Flow client**

```rust
pub struct GithubAuthClient {
    client: reqwest::Client,
    public_config: GithubAppPublicConfig,
    web_base_url: reqwest::Url,
    api_base_url: reqwest::Url,
}

pub async fn start(&self) -> Result<DeviceFlowStart>;
pub async fn poll(&self, device_code: &str, interval: u64) -> Result<InternalPollResult>;
pub async fn refresh(&self, current: &GithubCredential) -> Result<GithubCredential>;
```

生产 web/API base 固定 `https://github.com/` 与 `https://api.github.com/`，测试注入 wiremock。首次 token 成功后用该 token调用 `/user` 取得 login，再构造完整 credential；无法取得 login 不保存半成品。refresh 只发送 client id、`grant_type=refresh_token` 和 current refresh token，并沿用已验证 login；Device Flow token 不需要也不得发送 client secret。所有 token 请求使用 form encoding 与 `Accept: application/json`。内部 token DTO 使用 `secrecy::SecretString`，禁止 `Debug` 输出明文。

- [x] **Step 4: 写 credential store 与刷新测试**

覆盖：授权成功把完整 credential 作为单个 versioned JSON value 保存；尚未临近过期直接复用；临近过期只刷新一次；20 个并发 caller 共享一次 refresh；轮换后 access/refresh token 和绝对过期时间一起更新；HTTP refresh 失败保留仍可用旧 credential；GitHub refresh 成功但 store overwrite 失败时 best-effort clear 并返回 `CredentialPersistenceFailed`；bad/expired refresh 或撤销授权清除并返回 `ReauthorizationRequired`；任何 error/serialized public DTO 不含 token。

- [x] **Step 5: 实现 keyring 与 `GithubCredentialManager`**

```rust
pub struct GithubCredential {
    pub schema: u32,
    pub generation: Uuid,
    pub access_token: SecretString,
    pub refresh_token: SecretString,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
    pub github_login: String,
    pub app_client_id: String,
}

#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn load(&self) -> Result<Option<GithubCredential>>;
    async fn replace(&self, credential: &GithubCredential) -> Result<()>;
    async fn clear(&self) -> Result<()>;
}
```

keyring adapter 的 load/replace/clear 各自在一个 `spawn_blocking` closure 中完成；replace 写单个 credential JSON，不能分字段写。`GithubCredentialManager` 持 `tokio::sync::Mutex<()>`，`valid_credential()` 在锁内重新 load、按 5 分钟 skew 判断刷新、调用 auth client 并原子 replace。每次成功 refresh 生成新 generation；credential 的 app client id 与当前 build 不同直接要求重新授权。

不要给 `GithubCredential`/`SecretString` 派生 Serialize。keyring adapter 内部定义不派生 Debug 的 private `StoredGithubCredential`（token 字段为 String）作为唯一 JSON 边界，只在 blocking closure 内用 `ExposeSecret` 转换；离开 closure 前清理临时明文对象。错误只报告字段名/存储操作，不附序列化 JSON。

同文件定义共享 `GithubAuthenticatedClient`。所有生产 GitHub API 请求都通过它发送，并由 `lib.rs` 作为单个 `Arc` managed state 注入 commands/services/stores：首次 401 调用 `force_refresh(rejected_generation)`；锁内若 keyring generation 已变化则复用新 token，否则无视 access expiry 强制 refresh；只重放原请求一次；第二次 401 best-effort clear 并返回 `ReauthorizationRequired`。GET 和幂等 status 请求可按该规则重放；Contents PUT/Git Database 写请求只有收到明确 401 且 GitHub 未执行请求时才重放一次，其他网络/409/422 不在 wrapper 内重试。

- [x] **Step 6: 验证**

```bash
cargo test --manifest-path src-tauri/Cargo.toml github_app_config
cargo test --manifest-path src-tauri/Cargo.toml github_auth
cargo test --manifest-path src-tauri/Cargo.toml github_credentials
rg -n "std::env::var\(|client_secret\s*:|private_key\s*:|scope\s*[:=].*repo" src-tauri/src/github_*.rs
```

Expected: 默认测试只访问 wiremock；禁止项无产品代码命中。OS keyring adapter 可 gated，但内存 store 的完整授权与轮换测试必须默认运行。

- [x] **Step 7: Commit**

```bash
git add src-tauri/build.rs .github/workflows/release.yml src-tauri/Cargo.toml src-tauri/src/github_app_config.rs src-tauri/src/github_auth.rs src-tauri/src/github_credentials.rs src-tauri/src/errors.rs
git commit -m "feat: add github app device auth"
```

### Task 11: 实现 GitHub App installation、repo 状态与显式初始化

**Files:**

- Create/Modify: `src-tauri/src/github_repository.rs`
- Modify: `src-tauri/src/vault_manifest.rs`
- Modify: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/errors.rs`

- [x] **Step 1: 写 installation/repository 分页与单仓库测试**

wiremock 覆盖：无 installation 返回 `app_not_installed` 和基于 app slug 的安装 URL；穷尽 `/user/installations` 与每个 installation repositories 的 Link pagination；`repository_selection=all` blocked；跨所有 installations 汇总 0、2 个 repository blocked；恰好 1 个返回稳定 `installation_id/repository_id/owner/repo`。任何中间 page 401/403/404/网络失败都返回 unavailable，不能用部分结果证明唯一 repo。

- [x] **Step 2: 定义 repo 状态 DTO**

```rust
#[serde(rename_all = "snake_case")]
pub enum GithubVaultStatus {
    AppNotInstalled,
    RepositoryForbidden,
    RepositoryMissing,
    RepositoryUnavailable,
    EmptyRepository,
    BranchMissing,
    MissingManifest,
    InvalidManifest,
    Ready,
}

pub struct GithubVaultCheck {
    pub status: GithubVaultStatus,
    pub installation_id: Option<u64>,
    pub repository_id: Option<u64>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub head_sha: Option<String>,
    pub manifest_sha: Option<String>,
    pub retry_after: Option<String>,
    pub message: Option<String>,
}

pub struct InitializeGithubVaultRequest {
    pub remote: RemoteConfig,
    pub expected_status: GithubVaultStatus,
    pub expected_head_sha: Option<String>,
    pub expected_manifest_sha: Option<String>,
}

#[serde(tag = "status", rename_all = "snake_case")]
pub enum GithubRepositoryDiscovery {
    AppNotInstalled { install_url: String },
    SingleRepository { repository: GithubRepositorySelection },
    SelectionAll,
    MultipleRepositories { count: usize },
    Unavailable { message: String },
}
```

`message` 只保存消毒后的 GitHub message/request id，不含 token。初始化 request 只允许 expected status 为 empty_repository 或 missing_manifest。

- [x] **Step 3: 写精确状态与错误映射测试**

覆盖：未过期 token 首次 401 也强制单飞 refresh 并只重放一次；并发 401 只轮换一个 generation；重放仍 401 清 credential；403 + `Retry-After`、rate-limit headers、或无相关 headers 但 GitHub error body/message/documentation_url 明确 secondary rate limit 时返回 `RateLimited` 和 retry_after；非限流 403 是 forbidden；private repo 404 根据完整 installation 列表证据区分 forbidden/missing，证据不足是 unavailable；repo identity 已验证后 branch 404 才是 branch_missing；branches 空列表是 empty_repository；branch 存在且精确 manifest path 404 是 missing_manifest；HTTP 200 但 JSON/schema/path/hash 非法是 invalid_manifest；合法 manifest 是 ready。不得用 repository `size == 0` 判空。

- [x] **Step 4: 实现 `GithubRepositoryService`**

```rust
pub async fn discover_single_repository(&self) -> Result<GithubRepositoryDiscovery>;
pub async fn list_branches(&self, remote: &RemoteConfig) -> Result<Vec<String>>;
pub async fn check_vault(&self, remote: &RemoteConfig) -> Result<GithubVaultCheck>;
pub async fn validate_for_side_effect(&self, remote: &RemoteConfig) -> Result<GithubRepositoryContext>;
pub async fn initialize_vault(&self, request: InitializeGithubVaultRequest) -> Result<GithubVaultCheck>;
```

service 持 Tauri managed 的同一个 `Arc<GithubAuthenticatedClient>`，所有 GitHub 请求都通过其 generation/401 强制刷新路径发送，不直接取或缓存 token。`validate_for_side_effect` 每次穷尽 installation/repository 列表，要求总计唯一 repository 且 id 与 config 一致；owner/repo rename 可返回更新后的 display fields，repository id/installation id/branch 不一致 blocked。

- [x] **Step 5: 写初始化与幂等测试**

覆盖：empty repository 经显式 request 用 Contents API PUT canonical empty `manifest.json` 创建配置 branch 的首 commit；missing_manifest 在已有 branch 创建 1 commit；ready 返回现有 check 且 0 PUT；非空 repo branch_missing blocked；invalid_manifest 永不 PUT；expected status/head/manifest sha 变化返回 `VaultStateChanged(latest_check)`；409/422/网络结果不明后先 recheck，schema 合法且 skills 为空的 manifest（即使另一设备的 updated_by/updated_at 不同）视为成功，非空或非法 manifest blocked；并发两个 initialize 最多产生一个有效初始化 commit。

- [x] **Step 6: 实现 Contents API 初始化**

初始化 body 使用 `VaultManifest::empty(device_id)` 的 canonical JSON bytes、base64 content、固定 commit message `Initialize Skill Sync vault` 和 request branch。empty repo 没有 HEAD 时不调用 Git Database API；missing_manifest 不传 file sha，避免覆盖竞态文件。响应成功后必须重新 `check_vault` 并只接受 Ready。RateLimited 按 retry_after 返回 UI，不自动循环写请求。

- [x] **Step 7: 验证**

```bash
cargo test --manifest-path src-tauri/Cargo.toml github_repository
```

Expected: 分页、单 repo、状态分类、invalid manifest、rate limit 和初始化幂等测试全部通过；无真实网络。

另提供默认跳过的 `github_empty_repo_initialization` 集成测试，只在设置 `SKILL_SYNC_GITHUB_INTEGRATION=1`、系统 keyring 已有测试账号的 GitHub App credential、且配置 `SKILL_SYNC_GITHUB_EMPTY_TEST_REPO` 时运行；token 不通过环境变量传入。目标必须是已安装 App、无 branch/HEAD 的 disposable private repo；测试断言 Contents API 创建首个配置 branch + manifest commit、随后 check 为 ready、第二次 initialize 0 commit。测试不自动创建或删除 repository，避免默认 CI 产生外部副作用；正式 release checklist 必须人工执行一次。

- [x] **Step 8: Commit**

```bash
git add src-tauri/src/github_repository.rs src-tauri/src/vault_manifest.rs src-tauri/src/config.rs src-tauri/src/errors.rs
git commit -m "feat: discover and initialize github app vault"
```

### Task 12: 实现 GitHubVaultStore

**Files:**

- Create/Modify: `src-tauri/src/github_store.rs`
- Modify: `src-tauri/src/remote_store.rs`
- Modify: `src-tauri/src/github_repository.rs`
- Modify: `src-tauri/src/errors.rs`

- [x] **Step 1: 定义 GitHub client**

```rust
pub struct GitHubVaultStore {
    api: Arc<GithubAuthenticatedClient>,
    repository: GithubRepositoryContext,
    device_id: String,
}
```

headers：`Authorization`、`Accept: application/vnd.github+json`、`X-GitHub-Api-Version: 2022-11-28`、`User-Agent: skill-sync`。

使用 `#[async_trait] impl RemoteStore for GitHubVaultStore`，三个方法只通过共享 authenticated client 发送请求。Cargo 不启用 reqwest `blocking` feature；实现中禁止 `block_on` 或创建嵌套 Tokio runtime。

store 只能由 Task 11 的 `validate_for_side_effect` 结果与 Tauri managed 的同一个 `Arc<GithubAuthenticatedClient>` 构造；不得接收或缓存 `SecretString token`。context 包含 installation/repository id、owner/repo、branch 和已验证 HEAD。生产/test base URL 只在 authenticated client 构造；store 只构造相对 endpoint。

- [x] **Step 2: 实现 manifest fetch**

读取 branch HEAD 与 `manifest.json`，并强制 `VaultManifest::parse_validated`。本 task 只处理 Ready vault；manifest 404、invalid manifest、repo/branch 变化均返回 `VaultStateChanged`/`RemoteChanged`，绝不能构造 empty manifest。empty/missing vault 必须先走 Task 11 的显式初始化 command。

- [x] **Step 3: 实现 blob fetch**

读取 `blobs/sha256/<hash>.skill.zip`，校验 bytes hash == expected hash。

- [x] **Step 4: 实现 commit_changes**

使用 Git Database APIs：get ref、get commit/tree、create blobs、create tree、create candidate commit、更新 ref 前校验 HEAD 仍等于 `base_commit_sha`，并以 `force=false` update ref。预检后的并发更新可能发生在最后一次 HEAD 读取与 update ref 之间，因此仅 update-ref 操作返回的 non-fast-forward、409 或 422 映射为 `RemoteChanged`；其他 API 的 409/422 仍按正常 Vault validation error 处理。绝不重试为 force update。

update-ref 返回网络超时/连接中断等不确定结果时，不能直接重发写请求。先 GET branch ref：HEAD == candidate commit 视为本次成功；HEAD == base 表示未发布，返回明确 retryable error；HEAD 为其他值映射 `RemoteChanged`；ref 也无法读取时返回 `RemoteOutcomeUnknown { base_commit_sha, candidate_commit_sha }`。candidate SHA 和 next manifest hash 必须进入消毒后的错误/恢复数据，但不得包含 token 或 blob bytes。

- [x] **Step 5: 不打真实网络测试**

默认只测 DTO、error mapping、request building。真实 GitHub 测试只能手动或 gated：

使用 `wiremock` 的 Tokio mock server 测试 request/response path。默认测试必须包含：ready manifest/blob 请求；manifest 404/invalid 返回 state changed 且不写入；rate-limit 403 保留 retry_after；fetch manifest、fetch blob、commit path 各自首次 401 后使用新 generation 重放成功；重放仍 401 清 credential；update-ref request 的 `force=false`；仅 update-ref 模拟 non-fast-forward / 409 / 422 时映射为 `AppError::RemoteChanged`；update-ref 响应丢失后分别重读到 candidate/base/other/unavailable 的四种结果。其他 endpoint 的 409/422 仍映射为 Vault error。默认测试不访问真实 GitHub。

```text
SKILL_SYNC_GITHUB_TEST_REPO
```

- [x] **Step 6: 验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml github_store
```

Expected: 默认不访问网络。

- [x] **Step 7: Commit**

```bash
git add src-tauri/src/github_store.rs src-tauri/src/github_repository.rs src-tauri/src/remote_store.rs src-tauri/src/errors.rs
git commit -m "feat: add github vault store"
```

### Task 13: 重接 Tauri commands

**Files:**

- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/detect.rs`
- Modify: `src-tauri/src/sync_engine.rs`
- Modify: `src-tauri/src/skill.rs`
- Create: `src-tauri/src/vault_binding.rs`
- Create: `src-tauri/tests/fixtures/config-v1.yaml`

- [x] **Step 1: 原子迁移 AppConfig 和所有 Rust consumers**

先写 `default_config_has_no_unvalidated_remote`、`save_config_cannot_change_remote_identity` 等当前格式测试。将 AppConfig 固定为 `remote: Option<RemoteConfig> + limits + ignore`；demo 不迁移旧 YAML，旧版本和未知字段直接拒绝。所有 AppConfig load 先调用 `VaultBindingStore::recover_if_needed`。同步修改 commands/detect/sync engine 的所有字段引用，删除 Task 5 暂留的 Rust host/platform/enabled/repo_path 字段和 Task 7-9 的旧 engine adapter；扫描只用 fixed registry，未绑定 remote 返回 OnboardingRequired。

- [x] **Step 2: 删除旧 commands**

删除：

```text
check_git
check_remote
prepare_repo
list_backups
restore_backup
```

- [x] **Step 3: 暴露新 commands**

```rust
async get_app_state()
async save_config(config)
async scan_skills()
async get_sync_plan()
async apply_sync_plan(request: ApplySyncRequest) -> Result<ApplySyncResponse>
async resume_sync_recovery(task_id: String) -> Result<ApplySyncResponse>
open_path(path)
async start_github_device_flow()
async poll_github_device_flow(device_code, interval)
async get_github_app_info()
async list_github_installations()
async list_installation_repositories(installation_id)
async discover_single_github_repository()
async list_github_repository_branches(config)
async check_github_vault(config)
async initialize_github_vault(request: InitializeGithubVaultRequest)
async bind_github_vault(request: BindGithubVaultRequest)
async disconnect_github(expected_repository_id)
async list_remote_skills()
async upload_skills(skill_ids)
async download_skills(skill_ids)
```

`get_github_app_info` 只返回编译时 app slug、install URL 和 `configured`，不返回 client secret/private key。installation/repository/check/init 响应使用 Task 11 DTO；`GithubVaultCheck.status` 必须保留 app_not_installed/repository_forbidden/repository_missing/repository_unavailable/empty_repository/branch_missing/missing_manifest/invalid_manifest/ready，rate limit 作为带 retry_after 的 tagged error 返回。

Tauri command errors 统一序列化为 `AppErrorPayload { kind, message, retry_after?, latest_check? }`。只有 `RateLimited` 填 retry_after，只有 `VaultStateChanged` 填 latest_check；其他错误两字段省略。payload 的 message/details 必须经过 token redaction。

在 `commands.rs` 定义并由 `lib.rs` 注册 Tauri managed state：

```rust
#[derive(Default)]
pub struct SyncOperationGate {
    inner: tokio::sync::RwLock<()>,
}
```

`lib.rs` 还只构造一次 `Arc<GithubCredentialManager>` 和 `Arc<GithubAuthenticatedClient>`；auth、repository service、store 与所有 commands clone 同一 Arc。禁止每个 command 新建 manager/mutex，否则并发 401 会重复使用已轮换 refresh token。

- `get_app_state`、`scan_skills`、`get_sync_plan` 在读取 config/state 前获取 read guard，并持有到扫描/预览返回。
- `save_config`、`apply_sync_plan`、`upload_skills`、`download_skills` 在读取任何 config/state 前获取 write guard，并持有到所有本地目录变更和最终 sync_state/config 保存结束。
- app startup/`get_app_state` 和所有 mutation command 在 write guard 内检查 apply journal；仅 cleanup-complete 等不需要网络或新业务决策的阶段可自动恢复。`scan_skills`/`get_sync_plan` 在既有 read guard 内发现 journal 时返回 `recovery_pending`，不得释放后升级锁再继续原请求。`resume_sync_recovery` 始终获取 write guard，且 task id 必须等于 journal task id。
- engine 与 adapter 不再次获取 application gate；LocalVaultStore 自己的 path lock 是另一层 adapter transaction lock。
- remote-only auth polling、installation discovery 不获取 application gate；`check_github_vault` 获取 read guard。`initialize_github_vault`、保存新 remote identity和 disconnect 获取 write guard，持有到 config/sync_state 原子更新结束。

`apply_sync_plan` command 必须是 async，接收并完整转发单个 `request: ApplySyncRequest` 参数：通过 `spawn_blocking` 加载 config / `load_and_validate` sync_state（只读、mismatch 不归档），通过共享 authenticated client 与 `GithubRepositoryService.validate_for_side_effect` 复核单 repo 与稳定 identity，再构造 `GitHubVaultStore`、clone working state、await engine apply。Applied 表示 state 已 durable 保存；PlanChanged/预提交 error 丢弃 working clone；RecoveryRequired 保留 apply journal 管理的 next state 和 staged/rollback/trash，不得由 command 再保存或清理。不得绕过 installation validation。其他同步 commands 同样复用该构造路径。

Device Flow poll 的 authorized 分支必须先保存完整 credential 再返回公共状态；public response 不含 token。`initialize_github_vault` 只改变远端并返回 Ready check，不写本机 config/state。Ready 后调用 `bind_github_vault`：首次绑定创建空 state；已有不同 remote 时必须明确 confirm_rebind，普通 check/sync mismatch 不得隐式 rebind。

```rust
pub struct BindGithubVaultRequest {
    pub remote: RemoteConfig,
    pub expected_head_sha: String,
    pub expected_manifest_sha: String,
    pub expected_previous_binding: Option<RemoteBindingKey>,
    pub confirm_rebind: bool,
}

pub struct RemoteBindingKey {
    pub installation_id: u64,
    pub repository_id: u64,
    pub branch: String,
}
```

bind 在 write gate 内先比较 `expected_previous_binding` 与锁内当前 installation/repository/branch，再重新 validate single repo + Ready HEAD/manifest；同 repository 但 branch 已变化也返回 stale binding。随后由 `VaultBindingStore` 写 `<config-dir>/skill-sync/bind-transaction.json` journal：新 config/state temp 和 journal 均 fsync 后再 replace 两个目标，最后删除 journal。每次 AppConfig load 前先 recover journal，根据 previous/new hashes 完成或回滚，不能让新 config + 旧 base 进入 sync。首次绑定、same identity rename、confirmed rebind、stale previous branch，以及 temp/journal/config replace/state replace/clear 各故障点都写测试。`invalid_manifest`、branch_missing、rate limit 或 stale expected state 不 bind。disconnect 校验 expected repository id，best-effort 清 keyring，使用同类 journal 归档 state并清 remote config；不调用 GitHub uninstall/delete API。

`get_app_state` 与 `save_config` 也改为 async command，并分别用 `spawn_blocking` 包住完整的 AppConfig load/save。SyncState load/save 必须各自在一个 blocking closure 中完成；写入使用临时文件 + rename 的原子流程。所有 spawn join errors 映射为 `AppError`。

`save_config` 只允许修改 limits/ignore；request 中 remote identity 必须与当前 config 完全相同，否则 `Blocked`。只有成功的 Onboarding initialize/ready 绑定流程与 disconnect command 可以设置或清除 remote，防止前端手工伪造 installation/repository id。

`scan_skills` 不再读取 AppConfig roots，直接调用固定 registry，返回 `skills + roots + collisions + warnings`。Task 17 的 Settings 复用该响应显示只读路径状态。

- [x] **Step 4: 更新 AppState**

移除 `git_available` / `git_version`，新增：

```rust
pub github_authorized: bool,
pub github_user: Option<String>,
pub github_app_slug: Option<String>,
pub credential_status: CredentialStatus,
pub installation_id: Option<u64>,
pub repository_id: Option<u64>,
pub remote_owner: Option<String>,
pub remote_repo: Option<String>,
pub remote_branch: Option<String>,
pub vault_status: Option<GithubVaultStatus>,
pub device_name: String,
pub remote_commit: Option<String>,
pub pending_recovery: Option<RecoveryInfo>,
```

`pending_recovery` 只暴露 task/phase/remote commit/完成与待完成 action ids/消毒后的 message，不暴露 stage、rollback、trash 或 journal 绝对路径。

- [x] **Step 5: 验证**

Run:

```bash
rg -n "check_git\(|check_remote\(|prepare_repo\(|list_backups\(|restore_backup\(" src-tauri/src
cargo test --manifest-path src-tauri/Cargo.toml commands
```

Expected: `rg` 无产品代码命中；commands compile。

增加 `#[tokio::test]` command 边界测试：完整 request 字段不会丢失，`applied` / `plan_changed` / `recovery_required` tagged response 可序列化，提交期 `RemoteChanged` 会返回带 `latest_plan` 的响应而不是普通 command rejection；原请求不会自动重试；pending recovery 阻止新 preview/apply；resume task id 不匹配时 fail closed，匹配时不重复 remote commit。编译检查还必须证明 async command future 满足 Tauri 的 Send 要求。

增加 GitHub App command 测试：Device Flow public response 和 AppState serialization 不含 access/refresh token；installation discovery 不持 application gate；check/init 的 status DTO 不丢字段；initialize 即使 ready 也不写本机 binding；只有 bind 对 Ready check 写完整 config/state；普通 identity mismatch 保持 bytes 不变；confirmed rebind 才归档。expected previous installation/repository/branch 任一 stale（特别是同 repo branch 被另一窗口切换）都不得覆盖。对 bind journal 的 temp write、journal fsync、config replace、state replace、journal clear 逐点注入失败并模拟重启 recovery，结果只能是完整 previous 或完整 new binding。invalid/stale/rate-limited bind 保持 bytes 不变；sync/list/upload/download 每次构造 store 前 validate；disconnect 不发远端删除/uninstall 请求。

增加 operation gate 并发测试：两个 apply/upload/download 不会同时进入 engine；download-only 与 state-only Apply 同样串行；活动写操作期间启动的 scan/get_sync_plan 必须等待，写操作完成后读取最新 sync_state；测试使用 barrier/notify 控制交错，不能依赖 sleep。

Send 编译断言使用最小 helper：

```rust
fn assert_send<T: Send>(_: T) {}

assert_send(get_sync_plan_impl(...));
assert_send(apply_sync_plan_impl(...));
```

- [x] **Step 6: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/src/config.rs src-tauri/src/detect.rs src-tauri/src/sync_engine.rs src-tauri/src/skill.rs src-tauri/src/vault_binding.rs src-tauri/tests/fixtures/config-v1.yaml
git commit -m "refactor: route commands through github vault"
```

## Chunk 5: 前端契约与页面

### Task 14: 更新前端 API schemas

**Files:**

- Modify: `src/shared/schemas/apiResponse.ts`
- Modify: `src/shared/lib/tauri.ts`
- Modify: `src/modules/settings/schemas/config.ts`
- Modify: `src/modules/onboarding/schemas/onboarding.ts`
- Modify: `src/modules/settings/api/configApi.ts`
- Modify: `src/modules/onboarding/api/onboardingApi.ts`

- [x] **Step 1: 更新 AppState schema**

移除 `git_available` / `git_version`，新增：

```ts
export const recoveryInfoSchema = z.object({
  task_id: z.string(),
  phase: z.enum([
    'remote_outcome_unknown',
    'local_replace_failed',
    'trash_move_failed',
    'state_save_failed',
  ]),
  remote_commit: z.string().nullable(),
  completed_action_ids: z.array(z.string()),
  pending_action_ids: z.array(z.string()),
  message: z.string(),
})

github_authorized: z.boolean(),
github_user: z.string().nullable(),
github_app_slug: z.string().nullable(),
credential_status: z.enum(['disconnected', 'valid', 'refreshing', 'reauthorization_required']),
installation_id: z.number().int().nullable(),
repository_id: z.number().int().nullable(),
remote_owner: z.string().nullable(),
remote_repo: z.string().nullable(),
remote_branch: z.string().nullable(),
vault_status: githubVaultStatusSchema.nullable(),
device_name: z.string(),
remote_commit: z.string().nullable(),
pending_recovery: recoveryInfoSchema.nullable(),
```

`recoveryInfoSchema` 与 Task 7 Rust DTO 一致，放在共享 schema 中供 AppState 和 Task 16 的 Apply response 复用；不得为两个入口定义会漂移的重复 schema。

保留现有 `AppState.configured`，但把语义收紧为“本机已经通过 `bind_github_vault` 建立完整 Ready Vault binding”。它与 `GithubAppInfo.configured` 不同：后者只表示 release build 注入了公开 client id/app slug。前端路由门禁不得用 `GithubAppInfo.configured` 或单独的 `github_authorized` 提前解锁工作区。

- [x] **Step 2: 添加 Device Flow schema**

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

- [x] **Step 3: 添加其余 API 函数**

```ts
export const startGithubDeviceFlow = async (): Promise<DeviceFlowStart>;
export const pollGithubDeviceFlow = async (deviceCode: string, interval: number): Promise<DeviceFlowPoll>;
export const getGithubAppInfo = async (): Promise<GithubAppInfo>;
export const listGithubInstallations = async (): Promise<GithubInstallation[]>;
export const listInstallationRepositories = async (installationId: number): Promise<GithubRepository[]>;
export const discoverSingleGithubRepository = async (): Promise<GithubRepositoryDiscovery>;
export const listGithubRepositoryBranches = async (remote: RemoteConfig): Promise<string[]>;
export const checkGithubVault = async (remote: RemoteConfig): Promise<GithubVaultCheck>;
export const initializeGithubVault = async (request: InitializeGithubVaultRequest): Promise<GithubVaultCheck>;
export const bindGithubVault = async (request: BindGithubVaultRequest): Promise<AppState>;
export const disconnectGithub = async (expectedRepositoryId: number): Promise<void>;
```

定义并导出对应契约：

```ts
export const githubVaultStatusSchema = z.enum([
  'app_not_installed',
  'repository_forbidden',
  'repository_missing',
  'repository_unavailable',
  'empty_repository',
  'branch_missing',
  'missing_manifest',
  'invalid_manifest',
  'ready',
])

export const githubVaultCheckSchema = z.object({
  status: githubVaultStatusSchema,
  installation_id: z.number().int().nullable(),
  repository_id: z.number().int().nullable(),
  owner: z.string().nullable(),
  repo: z.string().nullable(),
  branch: z.string().nullable(),
  head_sha: z.string().nullable(),
  manifest_sha: z.string().nullable(),
  retry_after: z.string().nullable(),
  message: z.string().nullable(),
})
```

`githubAppInfoSchema` 只含 slug/install_url/configured。installation schema 包含 id/account_login/repository_selection，repository schema 包含 installation_id/repository_id/owner/repo/default_branch/private。discovery schema 必须表达 app_not_installed（带 install URL）、single_repository（带唯一 repo）、selection_all、multiple_repositories、unavailable；列表和 discovery 都不能包含 token。`initializeGithubVaultRequestSchema` 完整镜像 Task 11 expected status/head/manifest fields。

`bindGithubVaultRequestSchema` 包含完整 remote、expected_head_sha、expected_manifest_sha、nullable `expected_previous_binding { installation_id, repository_id, branch }` 和 confirm_rebind；ready check 不能由前端删减字段。bind 返回更新后的 AppState，页面只在成功后进入 Sync。

API adapter 对 `RateLimited` 保留 retry_after，对 reauthorization_required 进入重新授权状态。Onboarding 只有 empty_repository/missing_manifest 显示初始化，invalid_manifest 显示只读错误，branch_missing 要求选择现有 branch，ready 才进入同步。

扩展共享错误边界：

```ts
export const appErrorSchema = z.object({
  kind: z.string(),
  message: z.string(),
  retry_after: z.string().nullable().optional(),
  latest_check: z.unknown().optional(),
})
```

`SkillSyncError` 保留 `retryAfter` 和 `latestCheck: unknown`；`invokeCmd` 必须先用 `appErrorSchema.safeParse(raw)`，不能用 type assertion 丢字段。Onboarding 捕获 `vault_state_changed` 时再用 `githubVaultCheckSchema.parse(error.latestCheck)`，捕获 `rate_limited` 时使用 retryAfter。

- [x] **Step 4: 验证**

Run:

```bash
npm run typecheck
npm run lint
npm run build
```

Expected: schemas compile，现有页面仍通过 typecheck；不要在本任务提前改变 ApplyResult schema 或 `applySyncPlan` 的调用签名。

- [x] **Step 5: Commit**

```bash
git add src/shared/schemas src/modules/settings src/modules/onboarding src/modules/sync
git commit -m "feat: add github vault frontend contracts"
```

### Task 15: 移除独立 Backups / Conflicts 路由

**Files:**

- Modify: `src/app/router/routeConfig.ts`
- Delete: `src/routes/app/backups/+page.svelte`
- Delete: `src/routes/app/conflicts/+page.svelte`
- Delete: `src/modules/backups/**`
- Delete: `src/modules/conflicts/**`
- Modify: `src/shared/i18n/locales/zh-CN.json`
- Modify: `src/shared/i18n/locales/en-US.json`

- [x] **Step 1: 更新 route config**

删除 `/app/backups` 和 `/app/conflicts`。`appRoutes` authenticated navigation 只保留 `/app/sync` 与 `/app/settings`；`/app/onboarding` 的 Svelte route 继续存在，但不进入 `appRoutes`、侧边栏或任何工作区 Tab。`DEFAULT_ROUTE_PATH` 仍为 `/app/sync`，未绑定状态的条件跳转由 Task 17 的根路由和 `/app` layout gate 处理。

- [x] **Step 2: 删除模块和路由文件**

使用 `apply_patch` 删除 backups/conflicts module folders 与 route files。

- [x] **Step 3: 清理 locale route keys**

删除或停止使用：

```json
"routes.backups"
"routes.conflicts"
"backups"
```

冲突文案如仍被 Sync 弹窗使用，迁移到 `sync.conflictDetail.*`。

- [x] **Step 4: 验证**

Run:

```bash
rg -n "modules/backups|modules/conflicts|routes.backups|routes.conflicts|/app/backups|/app/conflicts" src
npm run lint:i18n
npm run typecheck
npm run lint
npm run build
```

Expected: `rg` no matches；i18n 与 typecheck pass；`appRoutes` 只包含 Sync / Settings，Onboarding route 文件仍存在但不是导航项。

- [x] **Step 5: Commit**

```bash
git add src
git commit -m "refactor: remove standalone conflict and backup pages"
```

### Task 16: 构建 Sync 卡片、筛选、删除确认和冲突详情

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

- [x] **Step 1: 添加筛选 helper**

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

- [x] **Step 2: 创建 SyncSkillCard**

Props:

```ts
entry: SyncSkillEntry
selected?: boolean
requiresConfirmation?: boolean
```

卡片显示 skill name、namespace badge（agents / codex / claude-code）、status badge、删除方向 badge、local/remote/base hash 短值、固定目标路径、warnings、blocked reason。

同一步原子更新 `SyncSkillEntry` Zod schema：`namespace` 必须为 `agents | codex | claude-code`，`folder_name` 为 string，`relative_dir` 为 nullable string；删除旧 `platform/platforms`。为三个 namespace、nullable relative_dir 与 `both_deleted` 添加解析 fixture，确保前端契约与 Task 7 Rust DTO 一致。

`syncStatusSchema` 原子替换为 synced/local_update/remote_update/local_deleted/remote_deleted/both_deleted/conflict/blocked/unknown。`syncPlanSchema` 完整包含 entries/action_id、uploads/downloads/delete_remote/delete_local/conflicts/blocked/warnings、delete_guard_tripped、expected_remote_commit、plan_fingerprint、base_adoptions/base_removals、will_create_commit 和 commit_summary.local_state_updates；同时删除 Task 5 暂留的 host 和其他旧 Git/platform 字段。

- [x] **Step 3: 创建 ConflictDetailDialog**

Props:

```ts
conflict: Conflict | null
decision: SyncDecision | ''
onDecision(choice: SyncDecision): void
```

不做文件级 diff。

- [x] **Step 4: 冲突决策按 reason 分组**

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

- [x] **Step 5: 更新 Sync 页面**

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
  z.object({
    status: z.literal('recovery_required'),
    recovery: recoveryInfoSchema,
  }),
])
```

同一任务内再添加返回 `applySyncResponseSchema` 的 `uploadSkills(skillIds)`、`downloadSkills(skillIds)` 与 `resumeSyncRecovery(taskId)` API。下载 API 不接收本地路径或 namespace override，目标完全由远端 entry namespace/folder_name 与固定 registry 决定。resume 只接收后端返回的 task id，不重发旧 Apply request。

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

收到 `recovery_required` 时，清空选择/decisions/ack，冻结 Recheck/Apply/批量上传下载，展示 phase、已完成/待完成数量、nullable remote commit 和“继续恢复”按钮。按钮调用 `resumeSyncRecovery(task_id)`；该 mutation 同样 `retry: false`，失败后保留恢复面板，不退回普通同步。不得把内部 stage/rollback 绝对路径放进前端 payload。

任何手动 recheck 或其他 query 更新导致展示中的 `plan_fingerprint` 变化时，同样清空上述三类状态。非删除动作只允许在首次成功加载或用户明确点击 Recheck 后按新 fingerprint 初始化默认选择；`plan_changed` 分支安装 `latest_plan` 时保持选择为空，不能立即重新默认选中。

- [x] **Step 6: 更新文案**

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
  "sync.recoveryRequired": "上次同步需要恢复后才能继续",
  "sync.resumeRecovery": "继续恢复",
  "sync.recoveryPending": "正在恢复本地写入",
  "sync.cleanupPending": "同步已完成，临时文件将在稍后清理",
  "sync.localBaseOnly": "仅保存本机同步基线，不会产生 GitHub commit",
  "sync.localBaseUpdated": "已更新 {{count}} 个本机同步基线",
  "sync.conflictDetail.localDeletedRemoteChanged": "本地已删除，但云端已修改",
  "sync.conflictDetail.remoteDeletedLocalChanged": "云端已删除，但本地已修改"
}
```

- [x] **Step 7: 自动验证**

```bash
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
npm run lint
npm run typecheck
npm run build
```

手动验证：删除 action 首次与计划变化后均不选中；Apply 只调用一次且 request 五个字段完整；`applied` 正确读取嵌套 result；`plan_changed` 使用 latest_plan 替换缓存并清空选择、决策和 ack，不触发自动 retry/refetch；`recovery_required` 冻结普通同步且只按 task id resume，不自动重发 Apply；应用以 `AppState.pending_recovery` 启动时直接显示同一恢复面板；分别用 adoption-only 与 removal-only 计划确认 Apply 仍可用、普通选择和 decisions 保持为空、显示 0 commit，并使用 `state_updated` 展示结果。

自动验证已通过；完整交互验收仍需要可运行的 Tauri 窗口和本机同步 fixture。

- [ ] **手动验收**：需要可运行的 Tauri 窗口和本机同步 fixture。

- [x] **Step 8: Commit**

```bash
git add src/modules/sync src/shared/i18n/locales
git commit -m "feat: consolidate sync status ui"
```

### Task 17: 更新 Onboarding 与 Settings

**Files:**

- Modify: `src/modules/onboarding/pages/OnboardingPage.svelte`
- Modify: `src/modules/onboarding/api/onboardingApi.ts`
- Modify: `src/modules/onboarding/schemas/onboarding.ts`
- Delete: `src/modules/onboarding/components/SshSetupDialog.svelte`
- Delete: `src/modules/onboarding/content/ssh-setup-prompt.md`
- Modify: `src/modules/settings/pages/SettingsPage.svelte`
- Modify: `src/modules/settings/schemas/config.ts`
- Modify: `src/app/router/routeConfig.ts`
- Modify: `src/routes/+page.svelte`
- Modify: `src/routes/app/+page.svelte`
- Modify: `src/routes/app/+layout.svelte`
- Modify: `src/shared/i18n/locales/zh-CN.json`
- Modify: `src/shared/i18n/locales/en-US.json`

- [x] **Step 1: 删除 SSH/Git onboarding UI**

删除 `SshSetupDialog`、Git 检测、remote URL 检测、SSH 教程入口。

- [x] **Step 2: 添加 Device Flow 状态**

```ts
type OnboardingStage =
  | 'app_not_configured'
  | 'authorize'
  | 'device_pending'
  | 'install_app'
  | 'repository_scope_blocked'
  | 'select_branch'
  | 'checking_vault'
  | 'confirm_initialize'
  | 'invalid_manifest'
  | 'rate_limited'
  | 'ready'
```

Device Flow 局部状态保存 device code、user code、verification URI、expires_at 和当前 interval。离开页面/组件销毁时停止 poll timer；重新进入根据 expires_at 恢复或重新申请 code。前端状态和 query cache 永不保存 access/refresh token。

页面使用单列、单当前步骤的渐进式向导。为 stage 映射稳定的 step number/title，显示“第 N/5 步”和原生或等价的可访问进度；每次只渲染当前步骤需要的说明与操作，已完成步骤只显示简短完成状态，不能把 Device Flow、installation、repo、branch 和初始化控件一次堆在同一页面。`authorized` 只推进到 installation discovery，不设置 workspace ready。

- [x] **Step 3: 添加连接、轮询和 repo 检测**

实现以下单一路径，不提供 OAuth/PAT/Git/SSH 切换：

1. `getGithubAppInfo()` 缺少编译配置时显示不可继续的发布配置错误。
2. `startGithubDeviceFlow()` 后展示 code/open URL，按 interval poll；slow_down 采用响应新 interval，authorized 后进入 installation discovery。
3. `discoverSingleGithubRepository()` 无 installation 时显示 app slug/install URL 和“安装后重新检查”；selection_all 或 multiple_repositories 时显示调整 GitHub App 为唯一 repo 的链接与原因，不能手填 owner/repo 绕过。
4. single_repository 后用 stable installation/repository id 构造临时 remote，列出已有 branches。empty repo 使用 repository default branch/name fallback `main` 作为待初始化 branch；非空 repo 只能选择 API 返回的 branch。
5. `checkGithubVault()`：ready 才完成；empty_repository/missing_manifest 展示“将创建 GitHub commit”的显式确认；branch_missing 回到 branch 选择；invalid_manifest 只显示校验错误和打开仓库按钮；rate_limited 显示 retry_after；unavailable/forbidden/missing 使用不同文案。
6. 用户确认后调用 `initializeGithubVault(expected status/head/manifest sha)`。VaultStateChanged 用 latest check 替换页面状态并要求重新确认，不自动重试 PUT；initialize 返回 ready 仍不写本机 binding。
7. 对原本 ready 或初始化后 ready 的 check 调用 `bindGithubVault`。首次绑定 confirm_rebind=false；若 AppState 已绑定不同 stable identity，先展示会归档旧 base 的切换确认，并携带 AppState 当前 installation id + repository id + branch 组成的 expected previous binding。bind 成功返回 `configured=true` 的新 AppState 后才进入 Sync 并显示工作区导航；同 repo branch 被另一窗口切换等 stale binding 必须刷新状态并重新确认，失败/恢复中保持无业务导航的 Onboarding。

在 Step 6 手动验收中逐项覆盖以上状态迁移、单当前步骤和进度、poll timer 清理、install 后 recheck、单 repo blocked、初始化确认、invalid manifest 不出现初始化按钮、rate limit、stale initialize 不自动重试，以及浏览器 devtools/query state 中没有 token；当前仓库没有前端 test runner，本任务不额外引入一套测试框架。

- [x] **Step 4: 实现认证门禁与双 Tab 工作区**

`src/routes/+page.svelte`、`src/routes/app/+page.svelte` 和 `src/routes/app/+layout.svelte` 必须先等待 `getAppState()`，加载期间只显示 spinner，不渲染后再撤掉业务侧边栏。定义单一前端门禁语义：

```ts
const workspaceReady =
  appState.configured &&
  appState.github_authorized &&
  ['valid', 'refreshing'].includes(appState.credential_status)
```

`vault_status` 的临时网络/限流结果不用于销毁已有本机 binding 或把已绑定用户踢出工作区；Sync 页面继续显示精确远端错误。`pending_recovery` 也保持在工作区并由 Sync 恢复面板处理。

- `/` 与 `/app`：等待 AppState 后，workspaceReady 跳 `/app/sync`，否则 replace 跳 `/app/onboarding`。
- `/app/sync`、`/app/settings` 及其他业务 route：workspaceReady=false 时 replace 跳 `/app/onboarding`，且跳转完成前不渲染 `AppLayout`。
- `/app/onboarding`：使用无 sidebar/breadcrumb 的最小引导壳；它不读取 `appRoutes`，页面中也不显示 Sync / Settings Tab。workspaceReady=true 时普通访问该路径 replace 回 `/app/sync`；Settings 的“重新配置 Vault”使用 `/app/onboarding?mode=reconfigure` 显式进入，并可取消返回 Sync。query 只决定前端展示，bind command 仍按 expected previous binding 和 confirm_rebind 做后端校验。
- bind 成功：直接使用 command 返回的 AppState 更新 `['app-state']` cache，确认 `configured=true` 后 replace 跳 `/app/sync`；不得先跳转再等待后台 refetch。
- reauthorization_required 或 disconnect 成功：更新/失效 AppState cache 并 replace 跳 `/app/onboarding`，旧工作区导航立即不可操作。

`src/app/router/routeConfig.ts` 的 `appRoutes` 只包含 Sync / Settings；不要给 Onboarding 添加 `showInNavigation` 一类默认 true 的可选配置，也不要保留第三个 Tab 再用 CSS 隐藏。route gate 只负责 UX，Rust command 的 credential/binding/remote identity 校验保持不变。

- [x] **Step 5: 更新 Settings**

删除 `config.defaults.backup` checkbox，新增：

```ts
config.remote.owner
config.remote.repo
config.remote.branch
config.remote.installation_id
config.remote.repository_id
config.limits.max_skill_zip_bytes
config.limits.max_skill_files
config.limits.max_single_file_unpacked_bytes
config.limits.max_skill_unpacked_bytes
config.limits.max_auto_delete
```

同一任务原子迁移 `appConfigSchema` 与 Onboarding schema：`remote` 为 nullable，完整值必须包含 installation/repository id、owner/repo/branch；limits schema 必须包含 max_skill_zip_bytes、max_skill_files、max_single_file_unpacked_bytes、max_skill_unpacked_bytes、max_auto_delete，并 refine single <= total；从 AppConfig 删除 provider selector、hosts、custom_paths 和旧 defaults，同步更新所有消费者。root 状态来自 scan/AppState 响应，不属于可保存配置。

保留 ignore textarea，文案说明它影响 zip/hash/size/status。四个 pack/unpack limit 使用带单位和最小值校验的数字输入；single unpacked 不得大于 total unpacked。说明 compressed limit 控制上传 blob，file/single/total limits 同时保护本地 pack 和远端解包。

Settings 只读显示 GitHub App slug、credential status、GitHub login、installation/repository id、owner/repo/branch、`AppState.device_name` 与 nullable remote commit；这些 identity/runtime 字段不能直接编辑。提供“重新配置 vault”和“断开 GitHub”命令：重新配置导航到 `/app/onboarding?mode=reconfigure` 并进入无业务导航的 Onboarding，但在新 bind transaction 成功前保留当前 binding，并提供取消返回 Sync；断开前显示会清除本机授权与归档本机 base、不会卸载 GitHub App 或删除远端内容，确认后携带 expected repository id，成功后回到 Onboarding。

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
  "settings.maxSkillFiles": "单个 Skill 最大文件数",
  "settings.maxSingleFileUnpacked": "单文件解压上限",
  "settings.maxSkillUnpacked": "单个 Skill 累计解压上限",
  "github.installApp": "安装 GitHub App",
  "github.singleRepoRequired": "GitHub App 必须只授权一个 vault 仓库",
  "github.initializeVault": "初始化 Vault",
  "github.initializeCreatesCommit": "这会在目标仓库创建 manifest.json commit",
  "github.invalidManifest": "manifest.json 存在但格式无效，应用不会覆盖它",
  "github.reauthorizationRequired": "GitHub 授权已失效，请重新连接",
  "onboarding.progress": "第 {{current}}/{{total}} 步",
  "onboarding.cancelReconfigure": "取消重新配置",
  "onboarding.bindingRequired": "完成 Vault 连接后才能进入同步与设置",
  "settings.rootNotFound": "目录尚未创建",
  "settings.rootUnreadable": "目录不可读"
}
```

- [x] **Step 6: 自动验证**

```bash
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
npm run typecheck
npm run lint
npm run build
```

手动验证：首次未授权打开 `/`、`/app`、`/app/sync` 和 `/app/settings` 都只出现无业务导航的 Onboarding，加载期间不闪现 sidebar；向导一次只显示当前步骤与明确进度；Device Flow authorized 后仍停留在 installation/repo/branch/init/bind 引导，不能提前看到业务 Tab。bind 成功后默认进入 Sync，导航严格只有 Sync / Settings；已绑定用户普通访问 `/app/onboarding` 会回到 Sync。GitHub App Device Flow -> install -> 唯一 repo -> branch -> empty/missing manifest 确认初始化 -> ready -> bind 全链路可完成；OAuth/PAT/Git/SSH 控件不存在；多 repo、invalid manifest 和 stale init 均不能越过。Settings 的重新配置通过 `?mode=reconfigure` 进入无导航向导且可取消保留旧 binding；disconnect/reauthorization_required 回到向导并隐藏旧工作区。Settings 不显示 token、不允许编辑 stable IDs。三个 namespace 均显示唯一固定路径；修改旧 hosts/custom_paths 不改变扫描目标；缺失 root 显示 unknown 而不产生删除。

自动验证已通过；完整链路验收需要 GitHub App credential、installation 和 disposable private repository。

- [ ] **手动验收**：需要 GitHub App credential、installation 和 disposable private repository。

- [x] **Step 7: Commit**

```bash
git add src/modules/onboarding src/modules/settings src/app/router/routeConfig.ts src/routes src/shared/i18n/locales
git commit -m "feat: connect github vault settings"
```

## Chunk 6: 旧模型删除与最终验证

### Task 18: 删除旧 Git working tree 与备份后端

**Files:**

- Delete: `src-tauri/src/git_store.rs`
- Delete: `src-tauri/src/backup.rs`
- Delete: `src-tauri/src/manifest.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/detect.rs`
- Modify: `src-tauri/src/sync_engine.rs`
- Modify: `src-tauri/src/commands.rs`

- [x] **Step 1: 删除 module declarations**

```rust
mod backup;
mod git_store;
mod manifest;
```

- [x] **Step 2: 替换旧引用**

```text
crate::manifest::hash_dir -> crate::pack / packed local hash
crate::git_store::GitStore -> RemoteStore / GitHubVaultStore
crate::backup::* -> trash move logic in sync_engine
```

- [x] **Step 3: 删除旧文件**

用 `apply_patch` 删除 legacy files。

- [x] **Step 4: 验证无残留**

Run:

```bash
rg -n "git_store|GitStore|backup::|list_backups|restore_backup|manifest::hash_dir|check_git|check_remote|prepare_repo|skill-sync.lock.json" src-tauri src
rg -n "HostsConfig|HostConfig|custom_paths|enabled_hosts|repo_path|\bplatforms?\b|\bhosts?\b" src-tauri/src src src/shared src/routes
cargo test --manifest-path src-tauri/Cargo.toml
npm run lint
npm run build
```

Expected: 两个 `rg` 均无产品代码命中；Rust tests pass。

- [x] **Step 5: Commit**

```bash
git add src-tauri/src
git commit -m "refactor: remove git cli sync backend"
```

### Task 19: 更新 README / docs 并全量验证

**Files:**

- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `docs/development-plan.md`
- Create: `docs/github-app-setup.md`
- Delete: `docs/ssh-setup.md`
- Modify: `docs/github-base-refactor-design.md` only if implementation details changed during execution.

- [x] **Step 1: 搜索旧产品描述**

Run:

```bash
rg -n "Git CLI|git_store|GitStore|check_git|check_remote|prepare_repo|Backups|backup|restore|SSH|skill-sync.lock.json|skills/<host>/<name>|HostsConfig|HostConfig|custom_paths|enabled_hosts|repo_path|\bplatforms?\b|\bhosts?\b|OAuth App|PAT|SKILL_SYNC_GITHUB_CLIENT_ID" README.md README.zh-CN.md docs src src-tauri
```

Expected: 只有历史上下文或明确非 MVP 注记保留。

- [x] **Step 2: 更新文档**

README 和开发文档说明 V1 唯一 provider 是 GitHub App Device Flow，private repo vault 使用 `manifest.json` + `blobs/sha256`，不需要 OAuth App/PAT/Git/SSH/SaaS backend。说明 installation 必须只授权一个 vault repo、token 只进 keyring且自动刷新、空 repo/缺 manifest 会在显式确认后创建初始化 commit，以及三个固定 namespace/root、advisory 删除、本地 trash、blob 不 GC。用户流程必须说明：未完成 Ready Vault binding 时应用只有逐步 Onboarding；bind 成功后才进入只含 Sync / Settings 两个 Tab 的工作区。

删除仍以有效指南口吻推荐 SSH/HTTPS/PAT 的 `docs/ssh-setup.md`。`docs/github-app-setup.md` 给维护者完整 release checklist：注册可安装 GitHub App；开启 Device Flow 和 expiring user tokens；Repository permissions 仅 Contents read/write + Metadata read-only；无 account/org permissions、events/webhook；记录公开 client id/app slug；在 GitHub Actions repository variables 配置 `SKILL_SYNC_GITHUB_APP_CLIENT_ID` / `SKILL_SYNC_GITHUB_APP_SLUG`；在打包前实际打开并核对 `https://github.com/apps/<slug>/installations/new`；确认 release artifact 不含 private key/client secret；用 keyring 中的测试 App credential 和 disposable empty private repo 运行 `SKILL_SYNC_GITHUB_INTEGRATION=1 SKILL_SYNC_GITHUB_EMPTY_TEST_REPO=<owner/repo> cargo test --manifest-path src-tauri/Cargo.toml github_empty_repo_initialization -- --ignored --nocapture`。给普通用户说明授权、Only select repositories、唯一 vault repo、installation 调整与撤销方式。

- [x] **Step 3: 全量验证**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
rg -n "reqwest::blocking|block_on|Runtime::new" src-tauri/src
rg -n "std::env::var\(|SKILL_SYNC_GITHUB_CLIENT_ID|client_secret\s*:|private_key\s*:|scope\s*[:=].*repo" src-tauri/src src .github/workflows
npm run typecheck
npm run format:check
npm run lint
npm run build
```

Expected: tests/build checks all pass；禁止项 `rg` 无产品代码命中，release workflow 只注入公开 App client id/slug。

- [ ] **Step 4: 手动烟测**

Run:

```bash
npm run tauri dev
```

检查：

- 未授权、credential 需要重新授权或尚未完成 binding 时，App 只打开无业务导航的 Onboarding；直接访问 Sync/Settings 也会被门禁拦回且不闪现 sidebar。
- Onboarding 是单当前步骤的渐进式向导，显示明确进度；能从 Device Flow 走到 installation、唯一 repo、branch、显式初始化、ready 和 bind。
- Device Flow authorized、App installed 或 initialize ready 都不会提前显示业务导航；只有 bind 成功后默认进入 Sync。
- bind 成功后的 Nav 严格只有 Sync / Settings，没有 Onboarding、Conflicts 或 Backups。
- Settings 重新配置会进入无业务导航的 Onboarding 且取消时保留旧 binding；disconnect 或 reauthorization_required 会回到 Onboarding 并使旧工作区不可操作。
- empty repo 初始化只创建一次 manifest commit；missing manifest 明确确认；invalid manifest 不可覆盖；非空 repo branch missing 只能选择已有 branch。
- 多 repo installation 被 blocked；扩大 installation scope 后下一次 check/sync 在副作用前 blocked。
- token 临近过期可自动刷新，撤销授权后进入重新连接；前端和日志不出现 token。
- Settings 有只读 GitHub App/repository stable identity、size limit、delete guard、ignore rules，以及 agents/codex/claude-code 三个只读固定 root；没有 provider/root 编辑控件。
- Sync 页面有搜索、筛选、卡片、删除 badge、delete guard warning。
- 删除项不默认勾选，必须显式确认。
- 冲突详情能展示普通冲突和两类删改冲突。

- [x] **Step 5: Commit**

```bash
git add README.md README.zh-CN.md docs src src-tauri
git commit -m "docs: update github vault behavior"
```

## Execution Notes For Agents

- 实施本计划前使用 `superpowers:executing-plans`；如果环境允许且用户明确授权 subagents，则使用 `superpowers:subagent-driven-development`。
- 每个任务按测试驱动执行：先写失败测试，再实现最小代码，再验证。
- 不要创建本地 Git clone/repo 作为同步实现。
- 不要保留 Git CLI fallback。
- 不要增加 OAuth App、PAT、HTTPS、SSH、Git URL 或 GitHub Enterprise provider fallback。
- GitHub App Device Flow 不发送 OAuth scope；App private key/client secret 不得进入桌面端、源码或 release artifact。
- 公开 client id/app slug 只在构建时注入；正式 release 缺失必须失败，运行时不读环境变量。
- access/refresh token 与过期时间只作为单个 keyring credential 保存；刷新轮换写入失败必须重新授权，不能使用部分 credential。
- installation/repository 枚举必须穷尽分页并 fail closed；总计非唯一 repo 或 repository_selection=all 时禁止任何远端副作用。
- 前端工作区门禁只在完整 Ready binding 和有效/刷新中 credential 下开放；`GithubAppInfo.configured` 或 Device Flow authorized 不得单独解锁。Onboarding 不进入 authenticated navigation，bind 后导航严格只有 Sync / Settings。
- 普通 preview/apply 发现 AppConfig/SyncState 的 installation_id、repository_id、branch 不一致时只读 blocked；只有用户明确确认切换 vault 后才归档旧 base 并 rebind，不得按 owner/repo 名称猜测。
- 只有 empty_repository/missing_manifest 可经用户确认初始化；invalid_manifest、branch_missing、unavailable/rate_limited 不得 PUT manifest。
- 不要实现 Backups 页面或 restore flow。
- 不要实现文件级 conflict diff。
- 不要新增 per-skill `.skill-syncignore`。
- 不要依赖 zip crate/操作系统默认 metadata 或 compression level；zip/Deflate 依赖升级也必须保持 golden bytes 不变。canonical constants 或输出 bytes 改动属于格式变更，必须升级 manifest schema 并更新 golden fixture。
- 不要跟随 symlink/junction/reparse point，也不要安装包含特殊 entry、非 portable path、重复或大小写折叠碰撞的 archive。
- 不要只信 ZIP header 的 uncompressed size；file/single/total limits 必须按实际流式 bytes 二次校验，失败只清理 temp。
- 不要接受 manifest 任意 blob path；blob 必须由严格 64 位 lowercase SHA-256 推导，size/hash 必须与实际 bytes 一致。
- 下载解包后必须重新解析根级 `SKILL.md` 并验证 manifest skill identity；正式 target 只接受目标 root 内同盘 staging 的 rename。
- 不要把 GitHub token 写入 YAML、JSON config、日志或长期前端状态。
- 不要把 tombstone 当作 V1 功能实现。
- 删除不 apply 不生效。
- 空扫描禁止删除。
- 删本地进 trash，不硬删。
- 删云端只移除 manifest entry，blob 不 GC。
- 批量上传与批量删除远端条目一次最多 1 个 GitHub commit。
- 下载-only 和删本地-only 一律 0 个 GitHub commit。
- `RemoteChanged` 后必须重新 fetch manifest 并重新生成计划，不要覆盖远端 HEAD。
- update-ref 结果不明时先按 candidate/base/other HEAD 分类；无法证明时返回 RecoveryRequired 并保留 journal，不得盲目重发远端写请求。
- Apply 前必须校验 `expected_remote_commit + plan_fingerprint`；不匹配时返回 `latest_plan`，不得执行任何持久化的本地或远端副作用，并清理用于重建计划的临时 pack。
- 普通动作只执行 `selected_action_ids` 中的条目；普通删除未显式选择一律跳过。
- delete guard 已触发且本次包含删除时，必须校验 `delete_guard_ack`；收到 `PlanChanged` 后旧选择、旧 decisions 和旧 ack 全部作废。
- preview 不得写 base；第 3/5 行通过 `base_adoptions`、第 13 行通过 `base_removals` 延迟到 Apply。仅状态协调的 Apply 必须写 sync_state、返回 `state_updated`，并保持 0 GitHub commit。
- 远端成功后的 local replace/trash/state failure 必须返回 RecoveryRequired；resume 只继续 journal pending steps，不重复远端 commit 或已完成的本地 move。pending recovery 解决前禁止普通同步。
- sync_state 与 apply journal 必须使用同目录 temp + file fsync + atomic replace/rename + parent fsync；state 成功前不得清理 rollback，cleanup failure 只返回 Applied warning。
- RemoteStore、build/prepare/apply/batch 与同步相关 Tauri commands 从 Task 6 起保持 async；GitHub I/O 直接 await，扫描、pack、hash、解包、目录移动和 sync_state 文件 I/O 使用 `tauri::async_runtime::spawn_blocking`。
- 禁止 `reqwest::blocking`、`block_on`、`tokio::runtime::Runtime::new` 或任何嵌套 runtime；async 测试使用 `#[tokio::test]`。
- namespace 固定为 agents/codex/claude-code，root 路径由代码 registry 决定；不要恢复 hosts/custom_paths、可编辑路径、项目级 root 扫描或“第一个匹配 root”回退。
- codex namespace 扫描 `~/.codex/skills` 的直接子目录，但必须排除 `.system`，且不递归进入没有合法 `SKILL.md` 的运行时聚合目录。
- 三个 namespace scanner 都必须排除 `.skill-sync-staging`、`.skill-sync-rollback` 和 `.skill-sync-trash`。
- Tauri `SyncOperationGate` 覆盖完整本机同步事务：preview/scan 读锁，apply/upload/download/save_config 写锁；LocalVaultStore 另用按 path 共享的 RwLock 保证 snapshot/commit 原子可见。
