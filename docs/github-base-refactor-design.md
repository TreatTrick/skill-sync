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

规则：

| 条件                                     | 结果                       |
| ---------------------------------------- | -------------------------- |
| `local == remote`                        | 已同步，跳过               |
| `local == base && remote != base`        | 云端更新，本地下载覆盖     |
| `remote == base && local != base`        | 本地更新，上传覆盖云端     |
| `local != base && remote != base` 且不同 | 双方都改，进入冲突         |
| `base` 不存在且本地/云端同名不同内容     | 首次遇到同名冲突，用户选择 |
| 本地有、云端无                           | 上传                       |
| 云端有、本地无                           | 下载                       |

删除同步 V1 不自动做。新电脑本地空目录不能被误判为“用户删除了所有 skill”。删除需要 tombstone 单独设计。

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
- `src-tauri/src/config.rs`：当前 repository 配置是 `local_path/remote/branch`，需要改成 provider-aware 配置，例如 GitHub owner/repo/branch 和本地 managed state。
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
    "codex:ponytail": {
      "id": "codex:ponytail",
      "name": "ponytail",
      "description": "Minimal implementation guidance",
      "platform": "codex",
      "platforms": ["codex"],
      "hash": "sha256:abc123",
      "blob": "blobs/sha256/abc123.skill.zip",
      "size": 12345,
      "updated_at": "2026-07-07T13:00:00Z",
      "updated_by": "device-laptop"
    }
  }
}
```

V1 的同步 key 建议使用平台限定 ID：

```text
skill_id = <platform>:<normalized(frontmatter.name || folder_name)>
```

例如：

```text
codex:ponytail
claude-code:ponytail
```

这样不会意外把 Codex 与 Claude Code 中同名但语义不同的 skill 合并。`platforms` 字段保留未来扩展空间，后续可以把同一个包安装到多个平台 root。

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
      "last_synced_at": "2026-07-07T13:00:00Z"
    }
  }
}
```

这个文件替代当前 `skill-sync.lock.json` 的职责。

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
trait RemoteStore {
    fn fetch_manifest(&self) -> Result<RemoteSnapshot>;
    fn fetch_blob(&self, blob_path: &str, expected_hash: &str) -> Result<Vec<u8>>;
    fn commit_changes(&self, changes: RemoteChanges) -> Result<RemoteCommit>;
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

### GitHubVaultStore

GitHub 作为默认远端适配器。实现语义：

- 读取指定 branch 的 HEAD commit。
- 读取 `manifest.json`，不存在时视为空 vault。
- 上传缺失的 `blobs/sha256/<hash>.skill.zip`。
- 更新 `manifest.json`。
- 基于读取时的 `base_commit_sha` 创建一次 commit。
- 移动 branch ref 前做乐观锁校验。
- 如果远端 HEAD 已变化，返回 `RemoteChanged`，上层重新 fetch manifest 并重新生成 sync plan。

commit 规则：

| 操作                       | GitHub commit |
| -------------------------- | ------------- |
| 只是从云端下载到本地       | 0             |
| 没有变化                   | 0             |
| 本地新增/修改上传          | 1             |
| 批量 skill 上传            | 1             |
| 冲突选择保留本地           | 1             |
| 冲突选择使用云端           | 0             |
| manifest metadata 发生变化 | 1             |

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

## 同步流程

### 预览同步

```text
1. 读取 AppConfig 与 SyncState
2. RemoteStore.fetch_manifest()
3. 扫描本地 Codex / Claude Code skill roots
4. SkillPacker 为每个本地 skill 生成 hash 与 zip metadata
5. 以 skill_id 合并 local / remote / base
6. 生成 SyncPlan
7. 清理预览阶段临时 zip 目录
8. 返回 UI 展示，不写文件、不上传、不下载
```

### 扫描与 hash 性能策略

第一阶段采用简单可靠的策略：

```text
每次预览/执行同步前：
1. 全量扫描已配置的 Codex / Claude Code skill roots
2. 对每个有效本地 skill 生成 canonical zip
3. 计算 hash 和 zip size
4. 基于 local / remote / base 生成同步计划
```

暂不引入 fingerprint cache、增量扫描或文件监听。几百个 skill 在默认 `20 MiB` 单包上限下可以先接受这套朴素实现，换取实现简单和状态可靠。如果后续真实使用中发现预览明显变慢，再增加基于文件数量、总大小、最新 mtime、ignore 规则版本的 pack cache。

### 执行同步

```text
1. 重新 fetch remote manifest，拿到最新 commit_sha
2. 重新扫描本地、重新 pack/hash，并重建计划
3. 合并用户 conflict decisions
4. 对 download 动作：
   - 拉取 blob
   - 校验 hash
   - 检查本地目标路径是否可安全覆盖
   - 如果目标路径存在但不属于当前 base，标记为冲突或 blocked
   - 解包到目标 root
5. 对 upload 动作：
   - 收集本次执行阶段重新生成的 canonical zip blob
   - 更新 next_manifest
6. 如果需要改云端：
   - commit_changes(base_commit_sha, blobs, next_manifest)
   - 若 base_commit_sha 过期，重新生成计划并要求 UI 确认
7. 写入 sync_state：
   - 更新 remote commit_sha
   - 更新成功同步项的 base_hash
8. 清理执行阶段临时 zip 目录
9. 返回 applied/warnings/remote_commit
```

### 批量上传

批量上传不走“全部同步”的大按钮，但复用同一套 pack、hash、manifest、commit 逻辑。上传 1 个 skill 和上传多个 skill 使用同一个接口，区别只是传入的 `skill_ids` 数量。

```text
upload_skills(skill_ids)
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
download_skills(targets)
  fetch manifest
  对每个 target fetch blob
  verify hash
  ensure local target is safe to overwrite
  unpack to configured roots
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

## Tauri command 设计

保留现有 command 名称的同时新增 GitHub/vault 能力。

```text
get_app_state()
save_config(config)
scan_skills()
get_sync_plan()
apply_sync_plan(decisions)
open_path(path)

新增：
start_github_device_flow()
poll_github_device_flow(device_code)
check_github_vault(config)
list_remote_skills()
upload_skills(skill_ids)
download_skills(targets)
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
- ignore rules：复用现有 textarea，全局生效，每行一个 glob。
- Codex / Claude Code roots。

GitHub access token 不写入 YAML 配置。应进入系统密钥链或 Tauri 安全存储能力；如果短期做不到，至少要明确标记为临时开发实现，不能把 token 明文放进仓库或日志。

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
冲突
```

状态语义：

| 筛选项   | 含义                                               | 默认动作                 |
| -------- | -------------------------------------------------- | ------------------------ |
| 全部     | 展示所有已发现或远端存在的 skill                   | 无                       |
| 已同步   | `local_hash == remote_hash`                        | 跳过                     |
| 本地更新 | 本地新增，或 `remote_hash == base_hash` 后本地变更 | 上传到云端               |
| 云端更新 | 云端新增，或 `local_hash == base_hash` 后云端变更  | 下载到本地               |
| 冲突     | 本地和云端都相对 base 变化，且 hash 不同           | 用户选择保留哪一边或跳过 |

搜索框按 skill 名称过滤卡片；状态筛选与搜索可以叠加。

继续使用当前页面骨架，但 plan schema 需要扩展：

```text
uploads
downloads
updates
conflicts
skips
blocked
warnings
remote_changed
expected_remote_commit
will_create_commit
commit_summary
```

UI 上应该明确：

- 本次是否会产生 GitHub commit。
- 下载动作不会改云端。
- 冲突未决不会自动覆盖。
- 每个写入本地的目标路径。
- 超过大小限制的 skill 被标记为无法上传，并显示实际 zip 大小、上限和处理建议。
- 冲突详情第一阶段只展示摘要，不做文件级 diff。

冲突详情保留三项决策：

```text
保留本地 -> 上传本地版本，产生 commit
使用云端 -> 下载云端版本，不产生 commit
跳过 -> 不处理
```

“另存为副本”可以作为后续增强，不放入第一轮重构。

## Rust 模块改造建议

目标结构：

```text
src-tauri/src/
  commands.rs            # 调整 DTO 与新增 command
  config.rs              # provider-aware 配置
  detect.rs              # 保留，补充平台命名
  errors.rs              # 新增 RemoteChanged / Auth / Vault 错误类型
  skill.rs               # 保留 metadata parser，补充 normalized id
  pack.rs                # 新增 canonical zip / hash / unpack
  vault_manifest.rs      # 新增 manifest schema 与校验
  sync_state.rs          # 新增本地 base_hash/commit_sha 状态
  remote_store.rs        # 新增 trait 与 DTO
  github_store.rs        # 新增 GitHub vault adapter
  local_vault_store.rs   # 新增测试/开发 adapter
  sync_engine.rs         # 重写三方比较与 apply flow
```

删除 `git_store.rs` 及其调用方，不保留旧适配器。测试和离线调试只使用不依赖 Git 的 `LocalVaultStore`。

## 迁移步骤

### 阶段 1：先落数据契约

- 新增 `vault_manifest.rs`、`sync_state.rs`、`pack.rs` 的类型与测试。
- 前端新增/扩展 `syncPlan.ts` schema。
- 不改 UI 行为，只保证类型边界成型。

### 阶段 2：实现 canonical pack/hash

- 用测试 fixture 验证同内容不同文件遍历顺序 hash 一致。
- 验证 ignore、生效路径、zip-slip 防护、大小限制。
- 扩展内置默认 ignore，并保留 Settings 全局 ignore 配置入口。
- 实现 `max_skill_zip_bytes` 硬限制，超过上限的 skill 只进入 blocked plan，不上传。
- 第一阶段采用全量扫描和朴素 canonical zip/hash，不做 fingerprint cache。

### 阶段 3：实现 LocalVaultStore

- 用本地目录模拟 `manifest.json` 与 `blobs/sha256`。
- 重写 `sync_engine` 的三方比较，先不接 GitHub。
- 覆盖 upload/download/update/conflict/base missing/download-only 0 commit 等规则。

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
- 状态筛选只保留：全部、已同步、本地更新、云端更新、冲突。
- skills 卡片显示同步状态、方向、hash、路径和 warning。
- 冲突卡片点击打开详情弹窗或抽屉。
- 冲突详情只展示摘要与决策按钮，不做文件级 diff。

### 阶段 6：更新 Onboarding/Settings

- 主路径切到 GitHub vault 配置。
- Onboarding 用 Device Flow 作为第一阶段授权方式。
- 删除旧 Git remote 相关 UI。
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
- skill id normalize。
- canonical zip hash 稳定性。
- 全量扫描后能为所有有效本地 skill 生成 canonical hash 和 zip size。
- 预览和执行阶段的临时 zip 目录会被清理，执行阶段不复用预览阶段 zip。
- 内置默认 ignore 和 Settings 全局 ignore 都会影响 zip/hash/size/status。
- manifest round trip。
- sync_state round trip。
- 三方比较规则。
- canonical zip 超过 `max_skill_zip_bytes` 时进入 blocked，不上传 blob，不更新 manifest。
- download-only 不创建 remote commit。
- upload/update 只创建一次 remote commit。
- GitHub commit SHA 变化时返回 `RemoteChanged`。
- 解包防 zip-slip。
- 目标路径存在但无 base 或 hash 与 base 不一致时，不自动覆盖，进入冲突或 blocked。

前端验证：

- Zod schema 能解析新 `SyncPlan`。
- Sync 页面正确显示 commit 语义。
- Sync 页面搜索和状态筛选能叠加过滤 skill 卡片。
- 冲突详情摘要能写入 `syncDecisions` 的三项决策。
- 独立 `Conflicts` 路由、侧边栏入口和模块已移除。
- 独立 `Backups` 路由、侧边栏入口和模块已移除。
- Onboarding/Settings 无中文硬编码，文案进入 locale。

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

多台电脑同时同步时，必须用 commit SHA 乐观锁。发现远端 HEAD 变化后不要强推，不要覆盖，重新拉 manifest 再生成计划。

### 第一阶段不做备份恢复

第一阶段删除完整备份/恢复能力，风险控制依赖三方比较、冲突阻止和覆盖前目标路径检查。只要目标路径不是“本机 base 记录对应的未修改版本”，就不能被云端下载静默覆盖。后续如果用户反馈需要误操作恢复，再单独设计 Backups 页面、保留周期和清理策略。

### 扫描与打包性能

第一阶段每次预览/执行同步前全量扫描并重新 pack/hash。数百个小 skill 可以先接受；如果后续出现明显卡顿，再增加 fingerprint cache。缓存优化不进入第一阶段，避免引入“缓存是否可信”的额外状态复杂度。

### 大文件与仓库膨胀

GitHub repo 不适合大 blob。默认排除缓存、临时文件、日志和依赖目录，并设置单个 skill zip 硬限制。比如 `skills/skillA/cache/` 里有 100 MiB 缓存时，应通过默认 `**/cache/**` 或用户规则 `**/skillA/cache/**` 排除；被排除内容不进入 zip、不参与 hash、不触发本地更新。超过大小限制时在 Sync 页面标记为 blocked，拒绝上传，不写入 manifest；用户需要清理大文件或添加 ignore 规则后再重新同步。

### 同名 skill

V1 用 `platform:name` 避免跨平台误合并。同平台同名不同内容且没有 base 时，必须进入冲突。

### Secret 泄露

上传前做基础扫描：`.env`、私钥、pem、常见 token 格式。不要阻止所有情况，但要在预览中明确 warning。

### 删除语义

V1 不自动同步删除。后续如果要做删除，需要 tombstone：

```text
tombstones/<skill_id>.json
deleted_at
deleted_by
previous_hash
```

否则新电脑空目录会被误判。

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
