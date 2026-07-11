# GitHub Vault 重构设计方案

## 结论

不建议从零重写整个项目。更合适的路线是：保留当前 Tauri 2 + Svelte 5 / SvelteKit 应用壳、页面结构、i18n、Zod schema、Tauri command 边界和本地扫描能力，局部重写后端同步核心和远端存储模型。

原因很简单：当前项目不是空壳，前端模块化、UI 组件、路由、设置页、同步预览页、Rust command、`SKILL.md` 解析、目录扫描、hash 等已经有可复用基础。真正需要推倒的是“把 skill 文件夹直接平铺进一个本地 Git 仓库，再用 Git CLI push/pull”的模型。聊天中收敛出的方案是 GitHub repo 作为远端 vault：用 `manifest.json` 描述云端索引，用 `blobs/sha256/*.skill.zip` 存内容包，用本地 `base_hash` 做三方比较。这是同步核心的重构，不是 UI 项目的重开。

推荐目标：

```text
当前项目继续演进
+ Rust 同步核心局部重写
+ GitHub vault 远端适配器
+ manifest + content-addressed zip blob
+ 本地 sync_state 做 base/hash/remote_commit 记录
+ 双向同步、批量上传、批量下载、冲突决策
+ 基于 base 的 advisory 删除传播（含护栏，不做 tombstone）
```

如果从零开始，只会重新付一次 Svelte/Tauri/i18n/路由/设置/扫描的成本，最后仍然要实现同一套复杂同步语义。除非产品方向改成 CLI-only、SaaS 后端或完全不同的 UI，否则没有必要重写整个项目。

## 聊天方案的核心判断

本项目不应该自己承担云端存储账单。开源 MVP 应该让用户自带远端，默认使用 GitHub private repo。GitHub 不直接存“展开后的 skills 目录树”，而是存一个稳定的 vault 格式。

云端 vault 结构：

```text
manifest.json
blobs/
  sha256/
    <hash>.skill.zip
```

同步判断不依赖文件修改时间，也不依赖 GitHub commit 时间。跨电脑时钟、复制文件、checkout 都会让时间不可靠。正确模型是三方比较：

```text
base   = 上次成功同步时该 skill 的内容 hash
local  = 当前本地 skill 的内容 hash
remote = 当前 GitHub manifest 中该 skill 的内容 hash
```

规则（`b`=base、`l`=local、`r`=remote 的内容 hash，`∅`=该侧缺失；穷举所有有意义组合）：

| #   | base | local | remote         | 状态                         | 动作                         | commit |
| --- | ---- | ----- | -------------- | ---------------------------- | ---------------------------- | ------ |
| 1   | `∅`  | 有    | `∅`            | 本地新增（local_update）     | 上传                         | 1      |
| 2   | `∅`  | `∅`   | 有             | 云端新增（remote_update）    | 下载                         | 0      |
| 3   | `∅`  | 有    | 有，`l==r`     | 已同步（待记 base）          | Apply 时写本机 base          | 0      |
| 4   | `∅`  | 有    | 有，`l!=r`     | 冲突：首次同名               | 用户选择                     | 视选择 |
| 5   | 有   | 有    | 有，`l==r`     | 已同步                       | 若 `b!=l`，Apply 时推进 base | 0      |
| 6   | 有   | 有    | 有，`r==b`     | 本地更新                     | 上传                         | 1      |
| 7   | 有   | 有    | 有，`l==b`     | 云端更新                     | 下载                         | 0      |
| 8   | 有   | 有    | 有，三者互不等 | 冲突：双方都改               | 用户选择                     | 视选择 |
| 9   | 有   | `∅`   | 有，`r==b`     | 本地已删除（local_deleted）  | 删云端（manifest 移除条目）  | 1      |
| 10  | 有   | `∅`   | 有，`r!=b`     | 冲突：本地删、云端改         | 删云端 / 拉回本地 / 跳过     | 视选择 |
| 11  | 有   | 有    | `∅`，`l==b`    | 云端已删除（remote_deleted） | 删本地（移入 trash）         | 0      |
| 12  | 有   | 有    | `∅`，`l!=b`    | 冲突：云端删、本地改         | 保留本地 / 接受删除 / 跳过   | 视选择 |
| 13  | 有   | `∅`   | `∅`            | 双方已删除（both_deleted）   | 清 base 条目（静默）         | 0      |

第 9–13 行为删除相关新增行。新增状态 `local_deleted` / `remote_deleted`；`both_deleted` 作静默 base 清理。新增冲突原因 `local_deleted_remote_changed`、`remote_deleted_local_changed`，与既有 `both_changed`、`same_name_first_seen` 并列。

第 3 行和第 5 行还承担本机基线协调：第 3 行必须产生 `base_adoption(skill_id, l)`；第 5 行仅在 `b != l` 时产生 `base_adoption(skill_id, l)`。第 13 行产生 `base_removal(skill_id)`。预览只把这些协调项放入 `SyncPlan`，不写 `sync_state`；用户执行 Apply 后才落盘，因此即使计划没有上传、下载或删除动作，也必须允许一次 0 GitHub commit 的“仅保存同步基线”操作。

删除传播基于 base 三方比较，V1 即做，但只做 advisory 语义，不做 tombstone：

- 全新电脑 `base` 为空，云端 skill 一律判为下载，不会被误判为“用户删除了所有 skill”。删除检测需要正向证据（base 有、某侧缺失），全新机器没有此证据。
- 真正的风险不是“新电脑”，而是“空扫描”（本地 root 临时不可读导致扫描为空）。这靠“删除安全护栏”解决，而非 tombstone。
- 删除后该 skill 从本地 `sync_state` 移除（base 清空），重新导入即重新上传，天然实现“反悔/同步回来”，无需专门机制。
- tombstone（权威/粘性删除）延后为未来增强，见文末“删除语义”。

## 当前项目现状

### 可以保留

- `src/routes`、`src/app`、`src/modules`、`src/shared` 的 Svelte 架构。
- `src/modules/sync/pages/SyncPreviewPage.svelte` 的同步预览 UI 骨架。
- `src/modules/sync/state/syncDecisions.svelte.ts` 的冲突决策状态。
- `src/shared/i18n/locales` 的文案管理方式。
- `src/shared/lib/tauri.ts` 的 Tauri invoke 封装。
- `src/modules/*/schemas` 的 Zod 响应校验模式。
- `src-tauri/src/skill.rs` 的 `SKILL.md` front matter 解析思路。
- `src-tauri/src/detect.rs` 的本地 root 扫描思路。
- `src-tauri/src/commands.rs` 的 command 边界。
- 现有 lint、responsive、colors、i18n 检查。

### 需要重构

- `src-tauri/src/sync_engine.rs`：当前围绕 `skills/<host>/<name>` 展开目录做比较，需要改成基于 remote manifest、local state 和 canonical zip hash 的三方比较。
- `src-tauri/src/manifest.rs`：当前只做目录 hash，需要演进为 canonical package/hash 与 vault manifest 模型。
- `src-tauri/src/git_store.rs`：当前是本地 Git working tree + CLI 操作。目标方案应抽象为 `RemoteStore`，默认实现 GitHub repo vault，并完全删除 Git CLI 同步路径。
- `src-tauri/src/config.rs`：当前 repository 配置是 `local_path/remote/branch`，且 `hosts.paths/custom_paths` 允许任意扫描目录；目标配置改成 GitHub owner/repo/branch、limits 和 ignore，删除可编辑 root 配置。三个 root 由 Rust 固定 registry 提供。
- 前端 `SyncPlan` schema：需要表达 upload/download/skip/conflict 之外的 commit 语义、远端版本信息、冲突原因、批量上传/下载动作。
- `src/modules/conflicts`：独立冲突页面需要删除，冲突处理合并进 Sync 页面。
- `src/modules/backups` 与 `src-tauri/src/backup.rs`：第一阶段不做完整备份/恢复，相关页面、路由和 command 从产品路径删除。
- Onboarding/Settings：需要从“配置 Git remote”改成“通过 GitHub Device Flow 连接 vault”，同时支持检测授权状态、repo 和 branch。

## 目标架构

```text
Svelte UI
  Sync / Settings / Onboarding
  |
  | Tauri commands
  v
Rust backend
  SkillScanner
  SkillPacker
  SyncStateStore
  SyncEngine
  RemoteStore trait
    GitHubVaultStore
    LocalVaultStore (tests/dev)
```

前端继续只做交互和展示。所有本地文件访问、打包、hash、GitHub API、覆盖保护和文件写入都在 Rust 侧完成。

## 云端数据模型

### manifest.json

建议 schema：

```json
{
  "schema": 1,
  "updated_at": "2026-07-07T13:00:00Z",
  "updated_by": "device-laptop",
  "skills": {
    "agents:ponytail": {
      "id": "agents:ponytail",
      "name": "ponytail",
      "folder_name": "ponytail",
      "description": "Minimal implementation guidance",
      "namespace": "agents",
      "hash": "sha256:abc123",
      "blob": "blobs/sha256/abc123.skill.zip",
      "size": 12345,
      "updated_at": "2026-07-07T13:00:00Z",
      "updated_by": "device-laptop"
    }
  }
}
```

V1 的同步 key 使用固定 root namespace 限定 ID：

```text
skill_id = <namespace>:<normalized(frontmatter.name || folder_name)>
```

V1 固定三个 namespace，不允许用户添加或修改 root 路径：

| namespace     | 固定用户级 root    | 语义                                   |
| ------------- | ------------------ | -------------------------------------- |
| `agents`      | `~/.agents/skills` | 多个工具可扫描的共享 Agent Skills 目录 |
| `codex`       | `~/.codex/skills`  | Codex 专用安装目录中的用户 skill       |
| `claude-code` | `~/.claude/skills` | Claude Code 专用用户 skill             |

`agents:ponytail`、`codex:ponytail`、`claude-code:ponytail` 是三个独立同步对象，不自动合并或复制。这里的 namespace 表示本机安装 root，不表示某个工具是否能够消费该 skill；例如 Codex、Copilot、Gemini 都可能读取 `agents` namespace。

`folder_name` 是下载目标的单个安全目录名，必须拒绝空值、`.`、`..`、路径分隔符和绝对路径。同一 namespace 内 normalized ID 或经 Unicode NFC + lowercase 折叠后的 folder name 发生碰撞时，整组标为 `blocked`，不能选择扫描顺序中的第一个。碰撞检测同时覆盖本地扫描结果、远端 manifest 内部，以及 local/remote 合并后会写向同一固定目标的条目；两个 remote-new skill 也不能解析到同一个目录。跨 namespace 的同名仍是独立对象。

V1 只管理上述三个用户级 root，不扫描仓库中的项目级 `.agents/skills`、`.claude/skills` 或其他动态路径。`~/.codex/skills/.system` 明确排除；其他直接子目录只有存在合法 `SKILL.md` 时才作为 skill，`.system` 或不含 `SKILL.md` 的运行时聚合目录不会递归扫描。

### 本地 sync_state

本地状态不应该继续放在远端 repo 里，也不应该随 GitHub 同步。它是“这台机器上次同步到哪个远端版本”的记录，应该放在 app data/config 目录。

```json
{
  "schema": 1,
  "remote": {
    "provider": "github",
    "owner": "example",
    "repo": "agent-skills",
    "branch": "main",
    "commit_sha": "github-head-sha"
  },
  "device_id": "device-laptop",
  "skills": {
    "codex:ponytail": {
      "base_hash": "sha256:abc123",
      "last_remote_hash": "sha256:abc123",
      "last_synced_at": "2026-07-07T13:00:00Z",
      "namespace": "codex",
      "relative_dir": "ponytail"
    }
  }
}
```

这个文件替代当前 `skill-sync.lock.json` 的职责。

`sync_state` 只能在 Apply 成功路径更新。预览发现 local 与 remote 已相同但 base 缺失或落后时，通过 `base_adoptions` 请求写入/推进 base；发现 `both_deleted` 时通过 `base_removals` 清理条目。两类操作都只修改本机状态，不产生 GitHub commit，也不能由 `get_sync_plan()` 隐式落盘。

固定 namespace 本身就是稳定的 `root_id`。`SkillSyncState.namespace + relative_dir` 记录该 skill 上次同步的准确本机位置；更新下载写回该位置，云端新增则使用 manifest 的 namespace/folder_name 解析唯一目标。状态与 skill_id namespace 不一致时标为 `blocked`，不能猜测或跨 root 移动。

## 打包与 hash

新增 `SkillPacker`，负责把 skill 目录打成规范 zip。

要求：

- 文件按规范化相对路径排序。
- 路径分隔符统一为 `/`。
- 排除内置默认 ignore、用户全局 ignore 规则和常见缓存目录。
- zip 内不得包含绝对路径、`..` 路径或平台相关临时文件。
- hash 对 canonical zip bytes 计算，格式为 `sha256:<hex>`。
- 上传前做大小限制与 secret warning。
- 下载解包前校验 hash，解包时防止 zip-slip 路径穿越。

### Ignore 规则

第一阶段做两层 ignore：

```text
1. 内置默认 ignore
2. Settings 全局用户 ignore
```

现有项目已经有 `AppConfig.ignore`、Settings textarea 和 Rust hash ignore 的基础，重构时继续复用这个配置入口，但需要扩展默认规则，并让 ignore 同时作用于 canonical zip、hash、zip size 和同步状态判断。

内置默认 ignore 建议：

```text
**/.git/**
**/node_modules/**
**/.env
**/.DS_Store
**/cache/**
**/.cache/**
**/tmp/**
**/temp/**
**/*.log
```

Settings 全局 ignore 继续使用“每行一个 glob”的 textarea，例如：

```text
**/skillA/cache/**
**/outputs/**
**/*.zip
```

ignore 语义：

- 被 ignore 的文件不进入 canonical zip。
- 被 ignore 的文件不参与 hash。
- 被 ignore 的文件不计入 `max_skill_zip_bytes`。
- 被 ignore 的文件变化不会让 skill 变成“本地更新”。
- Sync 页面可以展示被忽略文件数量/大小作为提示，但第一阶段不强制做详细列表。

第一阶段不做 per-skill `.skill-syncignore`。如果后续用户需要更细粒度控制，再单独设计 skill 内局部 ignore。

MVP 使用单个硬限制：按 canonical zip 后大小计算，单个 skill zip 超过 `max_skill_zip_bytes` 就拒绝上传。默认值建议先设为 `20 MiB`。

```text
canonical_zip_size <= max_skill_zip_bytes
  -> 可以进入上传计划

canonical_zip_size > max_skill_zip_bytes
  -> 标记为 blocked，Sync Preview 显示警告，不上传 blob，不更新 manifest
```

拒绝上传时，UI 应提示用户检查并清理大文件、依赖目录、缓存文件、截图或压缩包，也可以通过 ignore 规则排除不需要同步的内容。

### 临时 zip 产物

第一阶段不做本地 zip cache。`SkillPacker` 生成的 canonical zip 只作为本次预览或执行同步的临时产物，放在系统临时目录下：

```text
<system-temp>/skill-sync/<sync-task-id>/
  codex-ponytail.skill.zip
  claude-review.skill.zip
```

规则：

- 不放进项目目录。
- 不放进本地 Git repo。
- 不长期保存。
- 不跨多次同步复用。
- 执行同步结束后删除当前任务临时目录。
- 应用启动时可以顺手清理遗留的 `skill-sync` 临时目录。

预览阶段不依赖临时 zip 被后续 apply 复用。执行同步前会重新扫描、重新 pack、重新计算 hash 和 size，避免用户预览后修改文件但 apply 上传旧 zip。

当前 `hash_dir` 的稳定排序思想可以保留，但目标 hash 应该绑定 canonical zip，而不是直接绑定当前文件系统读取结果。这样 blob 内容、manifest hash 和本地比较使用同一个事实来源。

## RemoteStore 抽象

新增远端接口：

```rust
#[async_trait]
trait RemoteStore: Send + Sync {
    async fn fetch_manifest(&self) -> Result<RemoteSnapshot>;
    async fn fetch_blob(&self, blob_path: &str, expected_hash: &str) -> Result<Vec<u8>>;
    async fn commit_changes(&self, changes: RemoteChanges) -> Result<RemoteCommit>;
}
```

`RemoteSnapshot` 包含：

```text
manifest
commit_sha
branch
```

`RemoteChanges` 包含：

```text
base_commit_sha
new_or_existing_blobs
next_manifest
commit_message
```

### 异步执行边界

GitHub API 使用异步 `reqwest::Client`，因此 `RemoteStore`、`SyncEngine` 的远端调用链和相关 Tauri commands 必须从第一版起统一为 async，不能在 Task 11 接入 GitHub 时再把同步接口整体改写。

```text
async Tauri command
  -> async SyncEngine
     -> await RemoteStore (GitHubVaultStore / LocalVaultStore)
     -> spawn_blocking(scan / pack / hash / unzip / directory move / sync_state fs)
```

- 使用 `async-trait` 保证 `RemoteStore` future 为 `Send`，trait 本身要求 `Send + Sync`。
- 使用当前 Tauri async runtime；阻塞工作通过 `tauri::async_runtime::spawn_blocking` 提交，JoinError 统一映射为 `AppError`。
- 不允许在 command 或 adapter 内创建新的 Tokio runtime，不允许调用 `block_on`，也不启用 `reqwest::blocking`。
- `GitHubVaultStore` 直接 await reqwest；`LocalVaultStore` 保持同一异步 port，但其文件系统实现放入 `spawn_blocking`。
- `build_plan`、`prepare_plan`、`apply_plan`、`upload_skills`、`download_skills` 都定义为 async；canonical pack 等纯文件/CPU 阶段在 blocking closure 内完成，不能阻塞 Tauri 事件循环。
- AppConfig 与 SyncState 的 load/save、系统 keyring 的 load/save/clear 同样是阻塞 I/O：对外 command/API 为 async，内部用 `spawn_blocking` 包住一次完整读写事务并映射 join error。
- Tauri application state 管理一个 `SyncOperationGate(tokio::sync::RwLock<()>)`：`get_app_state`、`scan_skills`、`get_sync_plan` 获取读锁；`save_config`、`apply_sync_plan`、`upload_skills`、`download_skills` 获取写锁，并从 config/state 读取一直持有到本地变更和最终 state 保存完成。engine 内部不重复获取该锁，避免死锁。
- Cargo 显式加入 `async-trait` 与 Tokio（macros、rt-multi-thread）依赖，测试使用 `#[tokio::test]`，避免依赖未声明的传递 runtime。

### GitHubVaultStore

GitHub 作为默认远端适配器。实现语义：

- 读取指定 branch 的 HEAD commit。
- 读取 `manifest.json`，不存在时视为空 vault。
- 上传缺失的 `blobs/sha256/<hash>.skill.zip`。
- 更新 `manifest.json`。
- 基于读取时的 `base_commit_sha` 创建一次 commit。
- 移动 branch ref 前做乐观锁校验。
- 如果远端 HEAD 已变化，返回 `RemoteChanged`，上层重新 fetch manifest 并重新生成 sync plan。
- GitHub API base URL 由 store 构造函数注入：生产固定官方 endpoint，测试使用 `wiremock` 本地 async server；方法内部不硬编码 host，默认测试不访问真实网络。

commit 规则：

| 操作                           | GitHub commit |
| ------------------------------ | ------------- |
| 只是从云端下载到本地           | 0             |
| 没有变化                       | 0             |
| 只写本机 base adoption/removal | 0             |
| 本地新增/修改上传              | 1             |
| 批量 skill 上传                | 1             |
| 本地删除，删除云端条目         | 1             |
| 云端删除，删除本地             | 0             |
| 双方都删除，清理 base          | 0             |
| 冲突选择保留本地               | 1             |
| 冲突选择使用云端               | 0             |
| manifest metadata 发生变化     | 1             |

这比当前 `apply_plan` 更清晰：只有云端状态变化才产生 commit。

### GitHub 授权：Device Flow

第一阶段采用 GitHub OAuth Device Flow，而不是让用户手动创建 PAT，也不依赖 SSH key 或本机 `git config`。

Device Flow 适合桌面应用：

```text
1. 应用向 GitHub 申请 device/user code。
2. UI 展示 user code，并提供打开 GitHub 授权页面的按钮。
3. 用户在浏览器登录 GitHub，输入 code 并确认授权。
4. 应用轮询 GitHub token endpoint。
5. 授权成功后拿到 access token。
6. token 保存到系统密钥链或 Tauri 安全存储，不写入 YAML 配置、不写入日志。
7. 后续 GitHubVaultStore 使用该 token 调用 GitHub API。
```

Device Flow 的产品优势：

- 不要求用户安装 Git。
- 不要求用户配置 SSH。
- 不要求用户复制粘贴长期 PAT。
- 不需要注册 `skill-sync://` deep link。
- 授权失败、取消、过期都可以在 Onboarding 内展示明确状态。

授权范围第一阶段只申请访问目标同步仓库所需的内容读写能力。长期更安全的路线是 GitHub App：让用户只把应用安装到指定 vault 仓库，并授予 Contents read/write 权限；但 GitHub App 的实现和分发成本更高，不作为第一阶段目标。

### LocalVaultStore

建议保留一个本地目录版 vault 适配器，只用于测试、开发和离线调试。它不作为主产品路线，但能让 Rust 测试不依赖真实 GitHub 网络。

`LocalVaultStore` 对同一规范化 vault path 的所有实例共享一个 `Arc<tokio::sync::RwLock<()>>`。`fetch_manifest` 持读锁，并在同一个 `spawn_blocking` closure 内读取 manifest 与 `.local-vault-commit`；`fetch_blob` 同样持读锁；`commit_changes` 持写锁覆盖 base commit 校验、blob/manifest 原子写入和 commit marker 更新。锁由 path-keyed registry 复用，不能只存在于单个 store 实例，否则两个实例仍会产生混合快照。

## 同步流程

### 预览同步

```text
1. 读取 AppConfig 与 SyncState
2. RemoteStore.fetch_manifest().await
3. 扫描 `agents` / `codex` / `claude-code` 三个固定用户 root（记录每个 namespace 的可读性与扫描完整性）
4. SkillPacker 为每个本地 skill 生成 hash 与 zip metadata
5. 以 skill_id 合并 local / remote / base，按三方表判定状态（含本地删除 / 云端删除 / 双方删除）
6. 应用删除护栏：root 不可读则不推断删除；删除数超阈则置 delete_guard_tripped
7. 生成 base_adoptions（第 3 行、第 5 行 b!=l）和 base_removals（第 13 行）
8. 为每个计划项生成稳定的 action_id，并基于规范化计划内容生成 plan_fingerprint
9. 清理预览阶段临时 zip 目录
10. 返回 UI 展示（含 expected_remote_commit 和 plan_fingerprint），不写文件、不上传、不下载、不删除
```

### 预览版本与 Apply 请求

预览和执行之间，本地文件与远端 branch 都可能变化。Apply 不能只提交 conflict decisions，然后把旧决策套到重新生成的新计划上；它必须携带用户实际确认的计划版本与动作集合。

`SyncPlan` 中每个条目包含全计划唯一且稳定的 `action_id`，格式由后端统一生成：`<action_kind>:<skill_id>`。`action_kind` 只取 `upload`、`download`、`delete_remote`、`delete_local`、`conflict`、`synced`、`blocked`、`unknown`、`both_deleted`。`plan_fingerprint` 格式为 `sha256:<hex>`，是对以下规范化结构序列化为 UTF-8 JSON 后计算的 SHA-256：

```text
expected_remote_commit
delete_guard_tripped
按 skill_id 排序的 base_adoptions（skill_id / hash）
按 skill_id 排序的 base_removals
按 action_id 排序的计划条目：
  action_id / namespace / folder_name / relative_dir / status
  local_hash / remote_hash / base_hash / local_path / remote_blob
  delete_direction / conflict_reason / blocked_reason
```

规范化结构使用固定字段顺序，所有 optional 字段缺失时序列化为显式 `null`，条目按 `action_id` 字节序排序，并在生成 fingerprint 前拒绝重复 `action_id`。展示顺序、翻译文案、时间戳和纯展示 warning 不进入 fingerprint，避免无业务变化时产生不同指纹。会改变实际动作或安全判断的字段必须进入 fingerprint。

Apply 请求：

```text
ApplySyncRequest
  expected_remote_commit
  plan_fingerprint
  selected_action_ids
  decisions                 // skill_id -> SyncDecision，仅用于冲突
  delete_guard_ack
```

Apply 响应使用 tagged union：`Applied { result }` 或 `PlanChanged { reason, latest_plan }`。`reason` 只取 `remote_changed` 与 `plan_changed`：remote commit 不同优先判为 `remote_changed`；commit 相同但 fingerprint 不同判为 `plan_changed`。`Applied.result.state_updated` 返回本次完成 base adoption/removal 的 skill id；仅状态协调时 `applied` 为空、`state_updated` 非空、`remote_commit` 为空。

- `selected_action_ids` 是用户本次明确选择的普通动作集合。上传/下载可以由 UI 默认选中，删除动作永不默认选中。
- `base_adoptions` 与 `base_removals` 是后端根据三方表生成的本机状态协调，不属于 `selected_action_ids`。只要 Apply 请求通过版本与指纹校验，就自动执行这些协调项；它们不改变远端，也不触发删除确认。
- 冲突 decision 白名单按当前 `ConflictReason` 固定：`same_name_first_seen` / `both_changed` 允许 `keep_local | use_remote | skip`；`local_deleted_remote_changed` 允许 `delete_remote | restore_remote | skip`；`remote_deleted_local_changed` 允许 `keep_local | accept_delete | skip`。未提供 decision 表示该冲突不执行并返回 warning；不存在的 skill_id、重复 action id、不可执行 action id或 reason 不允许的 decision 返回 `Blocked`，整次 Apply 不得部分执行。
- `delete_guard_tripped == true` 且选择集合或冲突决策包含删除时，`delete_guard_ack` 必须为 true。
- 后端执行前重新生成计划，并先比较 `expected_remote_commit` 与 `plan_fingerprint`。任一不一致时，在任何持久化副作用（本地 skill 写入、trash move、远端变更或 sync_state 更新）之前返回 `PlanChanged { reason, latest_plan }`。重新扫描和 pack 产生的临时文件必须清理。
- UI 收到 `PlanChanged` 后替换为 `latest_plan`，清空旧选择、旧 conflict decisions 和删除熔断确认，要求用户重新确认；禁止自动重试 Apply。手动 recheck 或其他 query 更新导致展示中的 `plan_fingerprint` 变化时，也执行同样的清空逻辑。
- 预检通过后，GitHub 更新 branch ref 时仍使用 `base_commit_sha` 乐观锁，覆盖预检通过后到提交前的竞态窗口。

执行阶段内部使用 `PreparedSyncPlan` 同时持有 `SyncPlan`、本次重新生成且已参与 fingerprint 校验的 `PackedSkill` 以及任务临时目录。Apply 必须使用这些已校验 bytes，不能在指纹校验后再次 pack；`PreparedSyncPlan` 在成功、拒绝或错误退出时都通过 RAII 清理临时目录。计划只有 base adoption/removal 时，`selected_action_ids` 和 `decisions` 可以为空，Apply 仍返回 `Applied` 并写入本机状态。

### 扫描与 hash 性能策略

第一阶段采用简单可靠的策略：

```text
每次预览/执行同步前：
1. 全量扫描三个固定 namespace root
2. 对每个有效本地 skill 生成 canonical zip
3. 计算 hash 和 zip size
4. 基于 local / remote / base 生成同步计划
```

暂不引入 fingerprint cache、增量扫描或文件监听。几百个 skill 在默认 `20 MiB` 单包上限下可以先接受这套朴素实现，换取实现简单和状态可靠。如果后续真实使用中发现预览明显变慢，再增加基于文件数量、总大小、最新 mtime、ignore 规则版本的 pack cache。

### 执行同步

```text
1. 接收 ApplySyncRequest
2. await remote manifest；通过 spawn_blocking 重新扫描本地、重新 pack/hash，并重建计划（含删除判定与护栏复核）
3. 在任何持久化副作用前比较 expected_remote_commit 和 plan_fingerprint；不一致则返回 PlanChanged 和 latest_plan，并清理本次临时 pack
4. 校验 selected_action_ids、当前 conflict reason 对应的 decisions，以及 delete_guard_ack；非法请求返回 Blocked，且不部分执行
5. 使用 PreparedSyncPlan 中已校验的 pack bytes，并将 download 解包结果暂存到任务临时目录：
   - 拉取 blob
   - 校验 hash
   - 检查本地目标路径是否可安全覆盖
   - 如果目标路径存在但不属于当前 base，标记为冲突或 blocked
   - 解包到目标 root
6. 对选中的 upload 动作：
   - 收集本次执行阶段重新生成的 canonical zip blob
   - 更新 next_manifest
7. 对明确选中的 delete_remote 动作：从 next_manifest 移除该 skill 条目（blob 暂不 GC）
8. 对明确选中的 delete_local 动作：准备 trash move，但在需要远端 commit 时先不移动本地目录
9. 如果需要改云端，先提交云端变更：
   - await commit_changes(base_commit_sha, blobs, next_manifest)  // 上传与删云端条目合并为 1 个 commit
   - GitHub update ref 必须 force=false；non-fast-forward / 409 / 422 映射为 RemoteChanged
   - RemoteChanged 时重新生成 latest_plan，返回 PlanChanged(RemoteChanged)，不提交已暂存的本地写入
10. 远端提交成功或本次不需要远端提交后，再提交暂存的本地目录替换与 trash move
11. 写入 sync_state：
   - 更新 remote commit_sha
   - 更新成功同步项的 base_hash
   - 写入 base_adoptions：base_hash / last_remote_hash = adoption.hash，并更新 last_synced_at
   - 执行 base_removals：从 sync_state.skills 移除 both_deleted 项
   - 已执行的 local_deleted / remote_deleted 删除项同样从 sync_state 移除（base 清空）
   - 计划只有 base adoption/removal 时也必须原子保存 sync_state，且不调用 commit_changes
12. 清理执行阶段临时目录
13. 返回 ApplySyncResponse::Applied
```

### 批量上传

批量上传不走“全部同步”的大按钮，但复用同一套 pack、hash、manifest、commit 逻辑。上传 1 个 skill 和上传多个 skill 使用同一个接口，区别只是传入的 `skill_ids` 数量。

```text
async upload_skills(skill_ids)
  scan selected local skills
  pack/hash selected skills
  fetch manifest
  对每个 skill 做 base/local/remote 检查
  将可上传的 blobs 和 next_manifest 合并成 1 个 commit
  冲突或 blocked 项返回给 UI，不参与本次上传
  更新成功上传项的 sync_state
```

### 批量下载

```text
async download_skills(skill_ids)
  fetch manifest
  对每个 skill_id 读取远端 entry 并 fetch blob
  verify hash
  从 namespace/folder_name 解析唯一固定 target，并检查覆盖安全
  按 target namespace/folder_name 解包到唯一固定 root
  update sync_state base_hash for downloaded skills
  0 commit
```

### 覆盖保护与备份取舍

第一阶段不做完整备份/恢复能力：

```text
- 不做 Backups 页面
- 不做备份列表
- 不做一键恢复
- 不在每次下载覆盖前复制一份完整 skill
```

原因是 skill 不是大体量资产，三方比较已经能避免大部分误覆盖；备份列表、恢复目标、备份清理和 UI 状态会显著增加第一阶段复杂度。

第一阶段保留必要的覆盖保护：

- 冲突必须由用户选择，不自动覆盖。
- `local_hash == base_hash` 时，云端更新覆盖本地才视为安全。
- 目标路径存在但没有 base 记录，或 hash 与 base 不一致时，不直接覆盖，进入冲突或 blocked。
- “使用云端”这类覆盖动作必须在 Sync 页面明确展示目标路径。

备份/恢复可以作为后续增强，但不进入 GitHub vault 第一阶段。

### 删除安全护栏

删除是破坏性操作，且第一阶段不做 Backups，必须有护栏。真正要防的不是“新电脑空目录”（base 为空不会误判），而是“空扫描”——本地 root 临时不可读导致扫描为空，被误判成“删光了”。

- 删除永不静默：删除只作为 Sync 预览里的显式条目出现，不 apply 就不删；删除条目默认不勾选，需用户显式确认。
- 后端不信任前端默认值：普通删除的 `action_id` 必须出现在 `selected_action_ids`；删改冲突必须有当前 reason 允许的删除 decision，否则一律跳过。
- root 可读性闸门：把 `local == ∅` 判成删除前，根据 skill_id / sync_state namespace 定位唯一固定 root，并校验它存在、可读且扫描完整。root 缺失、不可读或扫描有错误时，该 namespace 下已跟踪 skill 标记为 `unknown`，不推断删除。其他 namespace 的健康状态不影响它。
- 固定 root 尚不存在时，remote-new 下载可以在 Apply 阶段创建该 root；但缺失 root 永远不能作为“本地删除”证据。
- 批量删除熔断：单个计划的删除数超过 `max_auto_delete`（默认 5）或超过被跟踪 skill 的 50% 时，置 `delete_guard_tripped`，UI 醒目警告、不预选删除、要求额外确认；后端要求同一 `plan_fingerprint` 下的 `delete_guard_ack == true`，不能只依赖 UI 弹窗。
- 删本地进 trash：删本地把目录移到 `<config-dir>/skill-sync/trash/<task-id>/<skill_id>/` 而不是硬删（删云端有 Git 历史兜底）。
- blob 不做 GC：删云端只移除 `manifest.json` 条目，orphan blob 保留（内容寻址、可 dedup、可恢复），GC 延后。

## Tauri command 设计

保留现有 command 名称的同时新增 GitHub/vault 能力。

```text
async get_app_state()
async save_config(config)
async scan_skills()
async get_sync_plan()
async apply_sync_plan(request: ApplySyncRequest) -> Result<ApplySyncResponse>
open_path(path)

新增：
async start_github_device_flow()
async poll_github_device_flow(device_code)
async check_github_vault(config)
async list_remote_skills()
async upload_skills(skill_ids)
async download_skills(skill_ids)
```

删除 `check_git()`、`check_remote()`、`prepare_repo()` 这类 Git CLI / Git remote command。GitHub vault 模式不要求用户安装 Git，也不保留旧 Git 同步路径。

## 前端改造范围

### Onboarding

从“填写 Git remote URL”改成“连接 GitHub vault”：

- 通过 GitHub Device Flow 完成授权。
- 显示 GitHub user code、授权页面入口、轮询中、已授权、过期、取消、失败等状态。
- GitHub owner/repo/branch。
- 授权账号与 token 可用状态。
- 检测 repo 是否可访问。
- 检测 `manifest.json` 是否存在，不存在时提示创建空 vault。
- 删除 SSH/Git 教程入口，避免用户误以为同步依赖本机 Git 配置。

### Settings

新增：

- Remote provider：GitHub。
- GitHub repo 信息。
- 当前 branch。
- 当前 remote commit。
- 本机 device name。
- skill size limit：默认 `20 MiB`，按 canonical zip 后大小计算，超过则拒绝上传。
- delete guard：`max_auto_delete` 删除熔断阈值，默认 `5`，超过则要求额外确认。
- ignore rules：复用现有 textarea，全局生效，每行一个 glob。
- 三个固定 namespace root：`agents`、`codex`、`claude-code`。Settings 只读显示 namespace、解析后的绝对路径、存在/可读/扫描状态，不提供路径输入、启用开关或新增 root。

GitHub access token 不写入 YAML 配置。应进入系统密钥链或 Tauri 安全存储能力；如果短期做不到，至少要明确标记为临时开发实现，不能把 token 明文放进仓库或日志。

旧配置中的 `hosts`、`hosts.*.paths` 和 `custom_paths` 在配置版本迁移时删除/忽略，不映射成新的扫描目录。用户仍可以在固定 root 内自由新增、编辑、移动和删除 skill；只是不允许通过应用改变工具约定的 root 路径。

### Sync

Sync 页面统一管理本地、云端、同步计划与冲突状态。第一阶段删除独立 `Conflicts` 页面，不再在侧边栏提供单独冲突入口。

Sync 页面包含：

```text
+ 搜索
+ 状态筛选
+ skills 卡片列表
+ 冲突卡片点击打开详情
+ 详情里只展示摘要和决策按钮
- 独立 Conflicts 页面
- 文件级 diff
```

状态筛选只保留用户真正关心的同步结果维度：

```text
全部
已同步
本地更新
云端更新
删除
冲突
```

状态语义：

| 筛选项   | 含义                                                            | 默认动作                        |
| -------- | --------------------------------------------------------------- | ------------------------------- |
| 全部     | 展示所有已发现或远端存在的 skill                                | 无                              |
| 已同步   | `local_hash == remote_hash`                                     | 跳过                            |
| 本地更新 | 本地新增，或 `remote_hash == base_hash` 后本地变更              | 上传到云端                      |
| 云端更新 | 云端新增，或 `local_hash == base_hash` 后云端变更               | 下载到本地                      |
| 删除     | 一侧相对 base 消失：local_deleted 删云端，remote_deleted 删本地 | 卡片 badge 区分方向，需显式确认 |
| 冲突     | 本地和云端都相对 base 变化，且 hash 不同                        | 用户选择保留哪一边或跳过        |

搜索框按 skill 名称过滤卡片；状态筛选与搜索可以叠加。

继续使用当前页面骨架，但 plan schema 需要扩展：

```text
uploads
downloads
delete_remote
delete_local
conflicts
blocked
warnings
delete_guard_tripped
expected_remote_commit
plan_fingerprint
entries[].action_id
entries[].namespace
entries[].folder_name
entries[].relative_dir
base_adoptions
base_removals
will_create_commit
commit_summary
```

`entries` 是所有状态（包括 synced、both_deleted、blocked 和 unknown）的事实来源，因此不再保留含义重叠的 `updates` / `skips` 汇总字段。远端或本地计划变化通过 `ApplySyncResponse::PlanChanged` 表达，不在 `SyncPlan` 内保留易过期的 `remote_changed` 布尔值。

UI 上应该明确：

- 本次是否会产生 GitHub commit。
- 下载动作不会改云端。
- 冲突未决不会自动覆盖。
- 每个写入本地的目标路径。
- 每个删除动作的方向与目标（删云端条目 / 删本地路径，删本地进 trash），默认不勾选、需显式确认。
- delete_guard_tripped 时给出醒目警告，说明删除数量超阈的原因（可能是 root 不可读或批量误删）。
- Apply 必须提交当前 `expected_remote_commit`、`plan_fingerprint`、选中的 `action_id`、冲突 decisions 和删除熔断确认；收到 `PlanChanged` 后清空旧选择并展示后端返回的新计划，不自动重试。
- 只有 base adoption/removal 时仍显示可执行的 Apply，并明确这是“保存本机同步基线”、不会产生 GitHub commit；成功结果使用 `state_updated` 展示更新数量。
- 超过大小限制的 skill 被标记为无法上传，并显示实际 zip 大小、上限和处理建议。
- 冲突详情第一阶段只展示摘要，不做文件级 diff。

冲突详情保留三项决策：

```text
保留本地 -> 上传本地版本，产生 commit
使用云端 -> 下载云端版本，不产生 commit
跳过 -> 不处理
```

删改冲突（一侧删除、另一侧改动）的决策语义：

```text
本地删除、云端又改动：
  删云端   -> 移除云端条目，产生 commit
  拉回本地 -> 下载云端新版本，不产生 commit
  跳过     -> 不处理
云端删除、本地又改动：
  保留本地 -> 重新上传本地版本，产生 commit
  接受删除 -> 本地移入 trash，不产生 commit
  跳过     -> 不处理
```

“另存为副本”可以作为后续增强，不放入第一轮重构。

## Rust 模块改造建议

目标结构：

```text
src-tauri/src/
  commands.rs            # 调整 DTO 与新增 command
  config.rs              # provider-aware 配置（含 limits.max_auto_delete）
  detect.rs              # 保留，改为三个固定 namespace root registry、状态与碰撞检测
  errors.rs              # 新增 RemoteChanged / Auth / Vault 错误类型
  skill.rs               # 保留 metadata parser，补充 normalized id
  pack.rs                # 新增 canonical zip / hash / unpack
  vault_manifest.rs      # 新增 manifest schema 与校验
  sync_state.rs          # 新增本地 base_hash/commit_sha 状态
  remote_store.rs        # 新增 trait 与 DTO
  github_store.rs        # 新增 GitHub vault adapter
  local_vault_store.rs   # 新增测试/开发 adapter
  sync_engine.rs         # 重写三方比较与 apply flow（含删除传播与删除护栏）
```

删除 `git_store.rs` 及其调用方，不保留旧适配器。测试和离线调试只使用不依赖 Git 的 `LocalVaultStore`。

## 迁移步骤

### 阶段 1：先落数据契约

- 新增 `vault_manifest.rs`、`sync_state.rs`、`pack.rs` 的类型与测试。
- 前端新增/扩展 `syncPlan.ts` schema。
- 定义 `agents` / `codex` / `claude-code` 固定 namespace registry，manifest、state、plan 统一改用 namespace/folder_name。
- 不改 UI 行为，只保证类型边界成型。

### 阶段 2：实现 canonical pack/hash

- 用测试 fixture 验证同内容不同文件遍历顺序 hash 一致。
- 验证 ignore、生效路径、zip-slip 防护、大小限制。
- 扩展内置默认 ignore，并保留 Settings 全局 ignore 配置入口。
- 实现 `max_skill_zip_bytes` 硬限制，超过上限的 skill 只进入 blocked plan，不上传。
- 第一阶段采用全量扫描和朴素 canonical zip/hash，不做 fingerprint cache。

### 阶段 3：实现 LocalVaultStore

- 用本地目录模拟 `manifest.json` 与 `blobs/sha256`。
- 扫描只使用三个固定用户 root，删除旧 hosts/custom_paths 参与扫描的路径。
- 重写 `sync_engine` 的三方比较，先不接 GitHub。
- 覆盖 upload/download/update/conflict/base missing/download-only 0 commit 等规则。
- 覆盖删除规则：local_deleted 删云端、remote_deleted 删本地、删改冲突、双方删除清 base。
- 实现删除护栏：root 可读性闸门、`max_auto_delete` 熔断、删本地进 trash、blob 不 GC。

### 阶段 4：接入 GitHubVaultStore

- 实现 GitHub Device Flow 授权命令。
- 授权 token 保存到系统密钥链或安全存储。
- 增加 GitHub repo 检测。
- 增加 manifest fetch。
- 增加 blob 上传与 manifest commit。
- 增加 commit SHA 乐观锁。
- 远端变化时返回可恢复错误，让 UI 重新预览。

### 阶段 5：更新 Sync 页面

- 删除独立 `Conflicts` 路由、侧边栏入口和模块。
- 删除 `Backups` 路由、侧边栏入口和模块；第一阶段不做备份/恢复 UI。
- Sync 页面新增搜索框和状态筛选。
- 状态筛选只保留：全部、已同步、本地更新、云端更新、删除、冲突。
- skills 卡片显示同步状态、方向、hash、路径和 warning；删除卡片用 badge 区分删云端/删本地，需显式确认。
- delete_guard_tripped 时展示醒目警告且不预选删除。
- 冲突卡片点击打开详情弹窗或抽屉。
- 冲突详情只展示摘要与决策按钮，不做文件级 diff；删改冲突展示删除感知决策。

### 阶段 6：更新 Onboarding/Settings

- 主路径切到 GitHub vault 配置。
- Onboarding 用 Device Flow 作为第一阶段授权方式。
- 删除旧 Git remote 相关 UI。
- Settings 删除可编辑 roots，改成三个固定 namespace 的只读状态列表。
- i18n 文案同步更新。

### 阶段 7：完善批量上传/下载

- Skills 或 Remote 列表中支持选择一个或多个 skill 上传/下载。
- 复用同一套 conflict/base/hash/覆盖保护逻辑。
- 批量上传一次最多创建 1 个 GitHub commit；批量下载不创建 commit。
- 不新增平行同步流程。

### 阶段 8：删除旧 Git working tree 模型

- 删除 `git_store.rs`、Git CLI commands 和 `skills/<host>/<name>` 远端目录模型。
- 删除备份/恢复 commands 的第一阶段产品入口。
- 移除远端 `skill-sync.lock.json` 依赖。
- README 与开发文档更新为 GitHub vault。

## 测试策略

Rust 优先测试：

- `SKILL.md` metadata parse。
- namespace + skill id normalize；三个固定 root 分别生成 `agents:*`、`codex:*`、`claude-code:*`。
- root registry 路径不可由 AppConfig 改写；旧 hosts/custom_paths 配置迁移后不参与扫描。
- 只扫描固定用户 root 的直接子目录；项目级目录、`~/.codex/skills/.system` 和无合法 `SKILL.md` 的聚合目录不会进入计划。
- 同 namespace normalized ID 或大小写不敏感 folder name 碰撞时全部 blocked，不保留扫描到的第一个；测试同时覆盖纯本地、纯远端和 local/remote 合并后的目标碰撞。
- 云端新增按 namespace/folder_name 落到唯一 root；更新按 sync_state namespace/relative_dir 写回，非法或不一致目标 blocked。
- canonical zip hash 稳定性。
- 全量扫描后能为所有有效本地 skill 生成 canonical hash 和 zip size。
- 预览和执行阶段的临时 zip 目录会被清理，执行阶段不复用预览阶段 zip。
- 内置默认 ignore 和 Settings 全局 ignore 都会影响 zip/hash/size/status。
- manifest round trip。
- sync_state round trip。
- RemoteStore、LocalVaultStore 与 SyncEngine 测试运行在 Tokio test runtime；GitHub request path 使用异步 mock，不启用 reqwest blocking client。
- scan/pack/unpack/trash move 等阻塞工作只从 `spawn_blocking` 路径调用；相关 async command future 可满足 Tauri 的 Send 要求，且代码中不存在自建 runtime 或 `block_on`。
- LocalVaultStore 并发读写测试不能观察到新 manifest + 旧 commit（或相反）的混合 snapshot；两个指向同一路径的独立 store 实例必须共享同一把锁。
- command 并发测试证明 apply/upload/download 串行执行，preview/scan 会等待活动写操作结束，state-only 与 download-only 操作也不能互相覆盖 sync_state。
- 三方比较规则。
- base 为空且 local==remote 时，预览产生 base_adoption 但不写 sync_state；空选择 Apply 后写入 base，0 GitHub commit。
- base 与 local/remote 同一新 hash 不同时，Apply 将 base 推进到新 hash；base 已等于该 hash 时不产生重复 adoption。
- both_deleted 产生 base_removal；只有该协调项时 Apply 仍会移除本机 base，0 GitHub commit。
- base adoption/removal 参与 plan_fingerprint；PlanChanged、Blocked 或未 Apply 时均不得写 sync_state。
- adoption-only / removal-only Apply 使用空 selections 和 decisions，返回 `Applied`、`applied=[]`、去重后的 `state_updated`、`remote_commit=null`，并更新 state.remote.commit_sha；RemoteStore commit 调用次数为 0。
- 本地删除（base 有、local 缺、remote==base）判为 local_deleted，删云端产生 1 commit。
- 云端删除（base 有、remote 缺、local==base）判为 remote_deleted，删本地移入 trash，0 commit。
- 删改并发（本地删/云端改、云端删/本地改）进入冲突，不自动删除。
- 双方都删除清理 base 条目，删除后重新导入判为 local_new 并重新上传。
- root 不可读导致的空扫描不产生任何删除；删除数超阈时置 delete_guard_tripped。
- canonical zip 超过 `max_skill_zip_bytes` 时进入 blocked，不上传 blob，不更新 manifest。
- download-only 不创建 remote commit。
- upload/update 只创建一次 remote commit。
- 预览后、Apply 预检前 GitHub commit SHA 变化时返回 `PlanChanged(remote_changed, latest_plan)`；预检通过后的 update-ref 竞态先由 store 返回 `RemoteChanged`，再转换成带最新计划的 `PlanChanged` 响应。
- 相同计划内容无论条目生成顺序如何都产生相同 `plan_fingerprint`；本地 hash、远端 commit、冲突原因、动作方向或目标路径变化时 fingerprint 必须变化。
- Apply 的 expected_remote_commit 或 plan_fingerprint 过期时，在任何持久化副作用前返回包含 latest_plan 的 `PlanChanged`。
- 未出现在 selected_action_ids 的普通删除不得执行；delete guard 已触发但未确认时，任何删除不得执行。
- 旧计划的 conflict decision 不得应用到 reason 已变化的新冲突。
- 解包防 zip-slip。
- 目标路径存在但无 base 或 hash 与 base 不一致时，不自动覆盖，进入冲突或 blocked。

前端验证：

- Zod schema 能解析新 `SyncPlan`（含 delete_remote/delete_local/delete_guard_tripped）。
- Sync 页面正确显示 commit 语义。
- Sync 页面搜索和状态筛选能叠加过滤 skill 卡片。
- “删除”筛选能列出 local_deleted / remote_deleted，卡片 badge 区分删云端与删本地。
- 删改冲突详情能写入删除感知决策。
- 冲突详情摘要能写入 `syncDecisions` 的三项决策。
- Apply 请求包含当前计划版本和明确选择；收到 `PlanChanged` 后替换计划并清空选择、决策和删除确认。
- 只有 base adoption/removal 时 Apply 按钮仍可用，页面显示 0 GitHub commit，并用 `state_updated` 展示已保存的同步基线。
- 独立 `Conflicts` 路由、侧边栏入口和模块已移除。
- 独立 `Backups` 路由、侧边栏入口和模块已移除。
- Onboarding/Settings 无中文硬编码，文案进入 locale。
- Settings 的三个 root 路径只读展示且没有自定义、启用或新增 root 控件。

必要命令：

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm run typecheck
npm run lint
npm run build
```

UI、布局、颜色或文案变化后，还需要按仓库规则运行：

```bash
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
```

## 风险与处理

### GitHub Device Flow 体验

Device Flow 需要 GitHub OAuth app 支持，并要处理 user code 过期、用户取消、轮询间隔、授权失败和网络错误。第一阶段不要求 deep link 回跳应用，也不让用户手动创建 PAT。UI 需要把授权状态做成可恢复流程：过期后重新生成 code，失败后允许重试，已授权后再检测 repo/branch。

### GitHub API 并发

多台电脑同时同步时，必须同时使用预览版本校验和 commit SHA 乐观锁。Apply 先用 `expected_remote_commit + plan_fingerprint` 防止旧页面决策套到新计划；移动 branch ref 时再用 `base_commit_sha` 覆盖预检后的竞态窗口。任一校验失败都不要强推、不要覆盖，返回最新计划并要求用户重新确认。

### Async runtime 与阻塞 I/O

reqwest 网络请求运行在 Tauri/Tokio async runtime，目录扫描、zip、hash、解包和移动目录属于阻塞工作，必须进入 `spawn_blocking`。禁止在 async command 内直接执行大批量文件 I/O，也禁止为同步 API 创建嵌套 runtime；否则会造成 UI event loop 卡顿或 runtime panic。应用级 SyncOperationGate 负责本机操作串行化，RemoteStore 的 commit SHA 乐观锁负责跨设备远端并发，两者不能互相替代。

### 第一阶段不做备份恢复

第一阶段删除完整备份/恢复能力，风险控制依赖三方比较、冲突阻止和覆盖前目标路径检查。只要目标路径不是“本机 base 记录对应的未修改版本”，就不能被云端下载静默覆盖。后续如果用户反馈需要误操作恢复，再单独设计 Backups 页面、保留周期和清理策略。

### 扫描与打包性能

第一阶段每次预览/执行同步前全量扫描并重新 pack/hash。数百个小 skill 可以先接受；如果后续出现明显卡顿，再增加 fingerprint cache。缓存优化不进入第一阶段，避免引入“缓存是否可信”的额外状态复杂度。

### 大文件与仓库膨胀

GitHub repo 不适合大 blob。默认排除缓存、临时文件、日志和依赖目录，并设置单个 skill zip 硬限制。比如 `skills/skillA/cache/` 里有 100 MiB 缓存时，应通过默认 `**/cache/**` 或用户规则 `**/skillA/cache/**` 排除；被排除内容不进入 zip、不参与 hash、不触发本地更新。超过大小限制时在 Sync 页面标记为 blocked，拒绝上传，不写入 manifest；用户需要清理大文件或添加 ignore 规则后再重新同步。

### 同名 skill

V1 用 `namespace:name` 避免跨固定 root 误合并。同 namespace 内 normalized ID 或大小写不敏感 folder name 碰撞时标为 blocked；local/remote 首次同 ID 但内容不同仍进入 `same_name_first_seen` 冲突。

### Secret 泄露

上传前做基础扫描：`.env`、私钥、pem、常见 token 格式。不要阻止所有情况，但要在预览中明确 warning。

### 删除语义

V1 做基于 base 三方比较的 advisory 删除传播（见前面的三方规则表与“删除安全护栏”）。一侧相对 base 消失即传播到另一侧：本地删除则移除云端条目，云端删除则把本地移入 trash；删改并发进冲突。全新电脑 base 为空不会误判，空扫描由 root 可读性闸门与熔断护栏兜底。删除后从 sync_state 移除，重新导入即重新上传，天然支持反悔。

tombstone 留作未来的“权威/粘性删除”增强——只用于处理“某台从未同步、却独立持有该 skill 的机器会把它重新上传复活”这一跨设备边缘场景。它与“重新导入即找回”相冲突，且对个人/少设备工具非必需，因此不进入 V1：

```text
tombstones/<skill_id>.json
deleted_at
deleted_by
previous_hash
```

引入时保持 manifest schema 版本化，可预留空的 deleted 段以便非破坏性升级。

## 是否重写项目

不建议。

更精确的说法是：同步核心要“局部重写”，项目不要“从零重写”。当前项目已经完成了很多跟最终方案无冲突的基础工程：

- Svelte 5 / SvelteKit 架构已经符合目标技术栈。
- 页面和模块边界已经接近最终产品形态。
- i18n、颜色 token、响应式检查、lint 规则已经建立。
- Rust 侧已经有扫描、解析、command 的初始结构。
- 当前 UI 只需要换数据模型，不需要重新设计整套桌面应用。

从零开始看起来轻松，是因为它暂时没有历史包袱；但同步引擎、GitHub commit、冲突、路径、防泄露、i18n、设置、错误处理这些复杂度不会消失，只会在新项目里重新出现。

建议执行方式：

```text
保留当前 repo
新建一条 refactor 分支
先加新 vault 模块和测试
让旧 UI 逐步接新 schema
最后移除旧 Git working tree 同步模型
```

这条路既能控制风险，也不会浪费已经做好的工程。
