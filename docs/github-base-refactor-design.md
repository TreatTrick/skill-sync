# GitHub Vault 重构设计方案

## 结论

不建议从零重写整个项目。更合适的路线是：保留当前 Tauri 2 + Svelte 5 / SvelteKit 应用壳、页面结构、i18n、Zod schema、Tauri command 边界和本地扫描能力，局部重写后端同步核心和远端存储模型。

原因很简单：当前项目不是空壳，前端模块化、UI 组件、路由、设置页、同步预览页、Rust command、`SKILL.md` 解析、目录扫描、hash 等已经有可复用基础。真正需要推倒的是“把 skill 文件夹直接平铺进一个本地 Git 仓库，再用 Git CLI push/pull”的模型。聊天中收敛出的方案是 GitHub repo 作为远端 vault：用 `manifest.json` 描述云端索引，用 `blobs/sha256/*.skill.zip` 存内容包，用本地 `base_hash` 做三方比较。这是同步核心的重构，不是 UI 项目的重开。

推荐目标：

```text
当前项目继续演进
+ Rust 同步核心局部重写
+ GitHub App Device Flow 单一授权路径
+ GitHub API vault 远端适配器
+ manifest + content-addressed zip blob
+ 本地 sync_state 做 base/hash/remote_commit 记录
+ 双向同步、批量上传、批量下载、冲突决策
+ 基于 base 的 advisory 删除传播（含护栏，不做 tombstone）
+ 未完成 Vault 绑定时只显示逐步引导，完成后工作区只显示 Sync / Settings
```

如果从零开始，只会重新付一次 Svelte/Tauri/i18n/路由/设置/扫描的成本，最后仍然要实现同一套复杂同步语义。除非产品方向改成 CLI-only、SaaS 后端或完全不同的 UI，否则没有必要重写整个项目。

## 聊天方案的核心判断

本项目不应该自己承担云端存储账单。开源 MVP 让用户自带 GitHub private repo，并通过 Skill Sync 的 GitHub App 授权。V1 不实现 OAuth App、PAT、Git URL、HTTPS 或 SSH provider；这些路径会扩大授权、配置和测试矩阵，而且无法提供 GitHub App installation 级别的权限边界。GitHub 不直接存“展开后的 skills 目录树”，而是存一个稳定的 vault 格式。

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
- `src-tauri/src/config.rs`：当前 repository 配置是 `local_path/remote/branch`，且 `hosts.paths/custom_paths` 允许任意扫描目录；目标配置改成 GitHub App installation/repository identity、owner/repo/branch、limits 和 ignore，删除可编辑 root 配置。三个 root 由 Rust 固定 registry 提供。
- 前端 `SyncPlan` schema：需要表达 upload/download/skip/conflict 之外的 commit 语义、远端版本信息、冲突原因、批量上传/下载动作。
- `src/modules/conflicts`：独立冲突页面需要删除，冲突处理合并进 Sync 页面。
- `src/modules/backups` 与 `src-tauri/src/backup.rs`：第一阶段不做完整备份/恢复，相关页面、路由和 command 从产品路径删除。
- 应用壳/Onboarding/Settings：需要从“始终展示全部业务导航并配置 Git remote”改成“未完成绑定时只显示逐步 GitHub Vault 引导，Ready bind 成功后才进入只含 Sync / Settings 的工作区”。

## 目标架构

```text
Svelte UI
  Connection gate
    Unbound / reauthorization required -> Onboarding only
    Ready binding -> Sync / Settings workspace
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
      "hash": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "blob": "blobs/sha256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.skill.zip",
      "size": 12345,
      "updated_at": "2026-07-07T13:00:00Z",
      "updated_by": "device-laptop"
    }
  }
}
```

Manifest 是不可信输入，所有来源（GitHub、LocalVaultStore、测试 fixture）必须经过同一个 validated parser。V1 校验规则：

- 顶层 `schema` 必须精确等于支持的版本 `1`；未知版本返回 `unsupported_schema`，不能按 V1 猜测解析。
- JSON parser 必须拒绝重复的 skill map key，不能接受 serde/map 默认的“后值覆盖前值”。
- `skills` 的 map key 必须逐字节等于 entry `id`；`id` 必须是 canonical `<namespace>:<normalized-name>`，且 id namespace 等于 entry namespace。
- `hash` 必须精确匹配 `sha256:[0-9a-f]{64}`，只接受小写十六进制。
- `blob` 不接受任意路径，必须由 hash 唯一推导：若 hash 为 `sha256:<hex>`，blob 必须严格等于 `blobs/sha256/<hex>.skill.zip`。
- `size` 必须大于 0；当前设备的 `max_skill_zip_bytes` 是本地 policy，超限 entry 在 plan 中 blocked，而不是把跨设备共享的 manifest 判为结构无效。下载后实际 blob bytes 长度必须等于 size，实际 SHA-256 必须等于 hash。
- `folder_name` 必须是可移植的安全单目录名；manifest 内 normalized id、folded folder target 的任何碰撞都使整份 manifest invalid。

任何一项失败都使整个 manifest 进入 `invalid_manifest`，不能跳过坏 entry 后继续，也不能用初始化流程覆盖。

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

`folder_name` 是下载目标的单个安全目录名，必须拒绝空值、`.`、`..`、路径分隔符和绝对路径。远端 manifest 内同一 namespace 的 normalized ID 或经 Unicode NFC + lowercase 折叠后的 folder name 碰撞由 validated parser 拒绝，使整份 manifest invalid；本地扫描结果或 local/remote 合并后才出现的目标碰撞，则把涉及条目整组标为 `blocked`，不能选择扫描顺序中的第一个。两个 remote-new skill 也不能解析到同一个目录。跨 namespace 的同名仍是独立对象。

V1 只管理上述三个用户级 root，不扫描仓库中的项目级 `.agents/skills`、`.claude/skills` 或其他动态路径。`~/.codex/skills/.system` 明确排除；其他直接子目录只有存在合法 `SKILL.md` 时才作为 skill，`.system` 或不含 `SKILL.md` 的运行时聚合目录不会递归扫描。

### 本地远端配置

`AppConfig.remote` 不保存 token 或 provider 选择；V1 provider 恒为 GitHub App。它保存用户确认后的稳定远端 identity：

```json
{
  "installation_id": 123456,
  "repository_id": 987654,
  "owner": "example",
  "repo": "agent-skills",
  "branch": "main"
}
```

`installation_id` 和 `repository_id` 是安全校验与 rename 跟踪的事实来源；owner/repo 是可读定位信息。加载旧配置时不迁移 OAuth token、Git URL、SSH、hosts 或 custom paths。没有完成 GitHub App installation/repository 校验前，不保存半成品 remote config。

### 本地 sync_state

本地状态不应该继续放在远端 repo 里，也不应该随 GitHub 同步。它是“这台机器上次同步到哪个远端版本”的记录，应该放在 app data/config 目录。

```json
{
  "schema": 1,
  "remote": {
    "provider": "github",
    "installation_id": 123456,
    "repository_id": 987654,
    "owner": "example",
    "repo": "agent-skills",
    "branch": "main",
    "commit_sha": "github-head-sha"
  },
  "device_id": "device-laptop",
  "skills": {
    "codex:ponytail": {
      "base_hash": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "last_remote_hash": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
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

`sync_state.remote.installation_id + repository_id + branch` 是本机 base 所属远端的稳定 identity。普通 preview/apply 只读加载 state 并校验它与 `AppConfig.remote` 及 GitHub 实际 repository id 一致；任一 ID 或 branch 变化都在网络/文件副作用前 blocked，不能由普通加载隐式归档或重置。只有用户在 Onboarding 明确确认切换 vault 时调用 rebind：将旧 sync state 归档到按旧 repository id 命名的本机历史文件并创建空 state，不能把旧 base 原子迁移到新 repo。同名仓库删除后重建会得到新 repository id，因此也按新 vault 处理。

## 打包与 hash

新增 `SkillPacker`，负责把 skill 目录打成规范 zip。

要求：

- 路径转换为 UTF-8、Unicode NFC、`/` 分隔的相对路径，并按规范化 UTF-8 bytes 排序；无法表示为 UTF-8 的路径 blocked。
- ZIP 只写 regular file entry，不写显式 directory entry；空目录不属于 skill 内容。
- 每个 entry 的 timestamp 固定为 ZIP epoch `1980-01-01 00:00:00`；creator system 固定为 Unix，external attributes 精确编码为 regular file + `0o644`，其余 DOS/平台属性位清零；comment 和 extra fields 为空。
- 压缩方法固定为 Deflate，level 固定为 `6`；不得因操作系统、zip crate 或压缩后端默认值改变压缩参数。依赖升级必须继续通过同一 golden bytes 测试，否则属于 canonical format 变更，必须升级 manifest schema。
- 排除内置默认 ignore、用户全局 ignore 规则和常见缓存目录。
- 每个 path component 都必须通过 portable path 校验：拒绝空值、`.`、`..`、绝对路径、NUL、路径分隔符、Windows 保留字符/设备名以及末尾空格或 `.`。
- 对所有相对路径计算 `Unicode NFC + lowercase + /` 的 portable collision key；精确重复或大小写折叠碰撞均 blocked，不能保留第一个。
- 源目录遍历使用 `symlink_metadata` 且读取前复核；拒绝 symlink。Windows 下任何 `FILE_ATTRIBUTE_REPARSE_POINT`（含 junction）都 blocked，不跟随到 root 外。
- hash 对上述 canonical zip bytes 计算，格式固定为 `sha256:<64 lowercase hex>`。
- 上传前做大小限制与 secret warning。
- 下载先验证 compressed size/hash，再验证 archive 仍满足相同 canonical entry 元数据、路径、碰撞和 regular-file-only 契约，最后在资源预算内流式解包。

这些字段都属于 canonical format 的一部分。只要 timestamp、mode、compression level、directory entry 策略或路径规范化任一不同，生成的 bytes 就不同，不允许依赖 zip crate/平台默认值。

### 链接、路径与解包安全

zip-slip 校验只解决 `../`/绝对路径，不能替代文件类型和资源限制。pack 与 unpack 必须共同执行：

- pack 不跟随 symlink、junction 或其他 reparse point；检测到后整个 skill blocked，并报告相对路径。
- unpack 只接受 regular file entry；根据 ZIP Unix mode/external attributes 判定为 symlink、device、FIFO 或其他特殊类型时拒绝。显式 directory entry 也因不符合 canonical format 而拒绝，父目录只由安全文件路径按需创建。
- 每个输出路径在 join 前逐 component 校验，join 后确认仍位于任务临时目录内；不能直接使用 archive 提供的路径创建文件。
- archive 内不允许精确重复路径或 portable collision key 重复；`Docs/README.md` 与 `docs/readme.md` 这类包在所有平台都 blocked。
- 所有内容先解到 RAII task temp；完整校验成功后才进入现有覆盖保护/原子目录替换。失败时清理 temp，正式 skill root 不发生变化。

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

V1 使用四个独立硬限制，不能用 compressed size 代替 unpack 资源预算：

```text
max_skill_zip_bytes             = 20 MiB   # compressed blob bytes
max_skill_files                 = 2,000    # regular file entries
max_single_file_unpacked_bytes  = 50 MiB
max_skill_unpacked_bytes        = 100 MiB  # all extracted file bytes
```

所有值进入 `LimitsConfig`，必须为正数，且 single-file unpack limit 不得大于 total unpack limit。上传 pack 和下载 unpack 使用同一组 limits。

```text
canonical_zip_size <= max_skill_zip_bytes
  -> 可以进入上传计划

canonical_zip_size > max_skill_zip_bytes
  -> 标记为 blocked，Sync Preview 显示警告，不上传 blob，不更新 manifest
```

打包时统计 included file count、每个文件实际读取 bytes 和累计 unpacked bytes，任一超限即 blocked。下载时先检查 ZIP headers 声明的 entry count/单文件/累计 uncompressed size，再在流式写入时重新计数实际 bytes；不能信任 header。实际写入超过任一限制立即终止并清理 temp。这同时防止高压缩比 zip bomb 和海量零字节 entry 耗尽磁盘/inode。

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
- `GithubAuthenticatedClient` 直接 await reqwest 并独占 API base URL、token generation、401 refresh/replay；`GitHubVaultStore` / `GithubRepositoryService` 只能经共享 client 发请求。`LocalVaultStore` 保持同一异步 port，但文件系统实现放入 `spawn_blocking`。
- `build_plan`、`prepare_plan`、`apply_plan`、`upload_skills`、`download_skills` 都定义为 async；canonical pack 等纯文件/CPU 阶段在 blocking closure 内完成，不能阻塞 Tauri 事件循环。
- AppConfig 与 SyncState 的 load/save、系统 keyring 的 load/save/clear 同样是阻塞 I/O：对外 command/API 为 async，内部用 `spawn_blocking` 包住一次完整读写事务并映射 join error。
- Tauri application state 管理一个 `SyncOperationGate(tokio::sync::RwLock<()>)`：`get_app_state`、`scan_skills`、`get_sync_plan` 获取读锁；`save_config`、`apply_sync_plan`、`upload_skills`、`download_skills` 获取写锁，并从 config/state 读取一直持有到本地变更和最终 state 保存完成。engine 内部不重复获取该锁，避免死锁。
- Cargo 显式加入 `async-trait` 与 Tokio（macros、rt-multi-thread）依赖，测试使用 `#[tokio::test]`，避免依赖未声明的传递 runtime。

### GitHubVaultStore

GitHub 是 V1 唯一产品远端。`GitHubVaultStore` 与 `GithubRepositoryService` 共享 Tauri managed `Arc<GithubAuthenticatedClient>`，不各自缓存静态 token。该 client 通过同一个 `GithubCredentialManager` 发送请求：正常路径取得有效 credential；首次 401 后按 credential generation 强制单飞刷新并重放一次；若锁内发现其他 caller 已完成新 generation，则直接使用新 token重放而不重复刷新；第二次 401 清除 credential 并返回 `reauthorization_required`。非 401 不自动重放写请求。实现语义：

- 验证 configured `installation_id`、`repository_id`、owner/repo 与 GitHub 当前结果一致；仓库 rename 可更新展示名称，但 repository id 不得静默变化。
- 读取指定 branch 的 HEAD commit。
- branch 已存在时读取 `manifest.json`；只有该文件的 Contents API 请求返回 404 才是 `missing_manifest`。
- 上传缺失的 `blobs/sha256/<hash>.skill.zip`。
- 更新 `manifest.json`。
- 基于读取时的 `base_commit_sha` 创建一次 commit。
- 移动 branch ref 前做乐观锁校验。
- 如果远端 HEAD 已变化，返回 `RemoteChanged`，上层重新 fetch manifest 并重新生成 sync plan。
- GitHub API base URL 只由 `GithubAuthenticatedClient` 构造函数注入：生产固定官方 endpoint，测试使用 wiremock；store/service 不持 base URL 或 raw reqwest client，默认测试不访问真实网络。

repo、branch 和 manifest 必须分层探测，不能把所有 404 折叠为空 vault：

```text
unauthorized          token 无效、过期且刷新失败，或用户撤销授权
app_not_installed     token 有效，但没有可用 installation
repository_forbidden  installation 不覆盖目标 repo，或用户无权访问
repository_missing    owner/repo 或 repository_id 不存在
repository_unavailable GitHub 用 404 隐藏私有资源且现有 installation 证据不足
empty_repository      repo 可访问，但没有任何 branch/HEAD
branch_missing        repo 非空，但配置 branch 不存在
missing_manifest      branch 存在，仅 manifest.json 不存在
invalid_manifest      manifest.json 存在，但 JSON/schema/path/hash 校验失败
ready                 branch 与合法 manifest 都存在
```

401 一律进入凭据刷新/重新授权路径。403 先检查 `Retry-After`、`X-RateLimit-Remaining`、`X-RateLimit-Reset` 和 GitHub error code：primary/secondary rate limit 返回带可重试时间的 `rate_limited`，其他 403 才映射权限错误。GitHub 会用 404 隐藏无权访问的 private repo，因此先以已穷尽分页的 installation/repository 列表为证据：列表明确包含 configured repository id 后，后续 repo 404 是资源竞态/删除；列表明确不含且完整时是 `repository_forbidden`；列表本身因权限/网络不能完整证明时 fail closed 为 `repository_unavailable`，不猜测 missing。branch 404 只有在 repo identity 已验证后才是 `branch_missing`；只有 branch 已验证且精确 `manifest.json` path 404 才是 `missing_manifest`。HTTP 200 的 manifest 必须经过 validated parser，失败返回 `invalid_manifest`，绝不退化为空 vault。空仓库通过列举 branches 后得到空集合判定，不能仅依赖 repository `size == 0`。

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

### GitHub App 注册与构建配置

V1 使用一个由项目维护者注册、可供普通用户安装的 GitHub App。必须按以下固定配置发布：

- 开启 Device Flow。
- Repository permissions 只有 `Contents: Read and write`；`Metadata: Read-only` 使用 GitHub 自动要求的权限。
- 不申请 account/organization permissions，不订阅 events，不要求 webhook，不启用 OAuth App `repo` scope。
- 开启 user-to-server token expiration。Device Flow token 默认 access token 8 小时、refresh token 6 个月；桌面端负责轮换。
- 安装页面引导用户选择 `Only select repositories`，且 V1 只接受授权结果中总计一个可访问 repository。

GitHub App client id 和 app slug 是公开标识，不是 secret。它们通过 release build 环境变量 `SKILL_SYNC_GITHUB_APP_CLIENT_ID` 与 `SKILL_SYNC_GITHUB_APP_SLUG` 注入编译产物；Rust 使用 `option_env!`/build script 生成只读 `GithubAppPublicConfig`，运行时不读取环境变量。测试注入独立配置。release workflow 在打包前校验两项非空，缺失即失败；App private key、client secret 永远不进入源码、CI artifact 或桌面包。V1 只支持 `github.com`，不增加 GitHub Enterprise Server endpoint 配置。

### GitHub App Device Flow 与 token 轮换

GitHub App Device Flow 是 V1 唯一授权路径：

```text
1. 应用用编译时 client id 请求 device/user code；GitHub App 不发送 OAuth scope。
2. UI 展示 user code，并打开 GitHub verification URI。
3. 用户在浏览器确认授权，应用遵守 interval 轮询 token endpoint。
4. 返回 authorization_pending 时继续；slow_down 时在当前 interval 上增加 5 秒。
5. 成功响应后先用 access token 调用 `/user` 取得 login，再把 access_token、refresh_token、expires_in、refresh_token_expires_in 和 login 一次写入系统 keyring；任一步失败都不保存半成品。
6. 后续请求通过 GithubCredentialManager 获取有效 access token。
7. access token 临近过期时，用 client_id + refresh_token 刷新；Device Flow 生成的 token 刷新不需要 client secret。
8. 刷新响应会轮换 refresh token，必须原子替换整份 credential；失败时保留旧 credential，不能只写一半。
```

`GithubCredential` 至少包含 generation UUID、access token、refresh token、两个绝对过期时间、GitHub login 和授权时的 app client id。它作为单个 versioned JSON value 存在 keyring，不写 YAML/JSON config、不进入前端长期状态、不出现在日志或错误正文。`GithubCredentialManager` 为同一 credential 使用 async mutex 合并到期刷新与 401 强制刷新；刷新前后都重新读取 keyring，generation 用于判断另一个 caller 是否已经轮换，避免多个同步 command 使用同一旧 refresh token。GitHub refresh 请求失败时保留仍可用的旧 credential；GitHub 已返回轮换结果但 keyring 原子覆盖失败时，旧 refresh token 可能已失效，必须 best-effort 清除并返回 `credential_persistence_failed`，要求重新授权，不能继续假装刷新成功。若 refresh token 过期/撤销、GitHub 返回 `bad_refresh_token`，或强制刷新后重放仍 401，则清除 credential 并返回 `reauthorization_required`。

### Installation 与单仓库边界

授权成功不等于 App 已安装。Onboarding 必须调用 user installations API；没有 installation 时打开 `https://github.com/apps/<slug>/installations/new`，用户返回后重新检查。随后穷尽分页列出每个 installation 对当前用户可见的 repositories；任何 installation/repository page 失败都 fail closed，不能用部分列表证明单仓库边界。

GitHub App user token 的实际权限是“App permissions、用户权限、所有可访问 installations”的交集，Device Flow 不能再给 token 添加单 repo scope。为兑现 V1 的单仓库最小权限承诺，Onboarding 对所有可访问 installations 汇总 repository ids：只有总计一个 repository 时允许继续；`repository_selection=all` 或总数大于 1 时阻止保存，并引导用户把 App 安装范围调整为唯一 vault repo。配置保存稳定的 `installation_id`、`repository_id` 以及展示/定位用 owner、repo、branch；所有 vault 请求仍只构造到该 configured repository。

### Vault 初始化

repo 检测返回 `empty_repository` 或 `missing_manifest` 时，Onboarding 展示将创建首个/新的 `manifest.json` commit，并要求用户显式确认。初始化不是普通 sync apply：

```text
initialize_github_vault(config, expected_state)
  -> 重新验证 token、installation、repository id 与单仓库边界
  -> 重新探测 repo/branch/manifest 状态
  -> empty_repository:
       Contents API PUT manifest.json，使用 canonical empty manifest 创建首个 commit 和配置 branch
  -> missing_manifest:
       Contents API PUT manifest.json，在已存在 branch 上创建一个 commit
  -> ready:
       幂等返回当前 manifest/commit，不再提交
  -> invalid_manifest:
       blocked；展示校验错误，不覆盖现有文件
  -> branch_missing 且 repo 非空:
       blocked；要求用户选择已有 branch，不猜测、不自动建 branch
```

`expected_state` 包含 installation id、repository id、branch 和探测时的状态。初始化前若 identity 或状态变化，返回最新 `GithubVaultCheck` 要求 UI 重新确认。Contents API 返回 409/422、rate limit 或网络结果不明时不盲目重试写入，先重新读取：若目标 branch 上已存在 schema 合法且 `skills` 为空的 manifest，则按成功处理（允许另一设备的 updated_by/updated_at 不同）；若存在非空/非法 manifest，则 blocked，不能覆盖。首个 HEAD 建立后，所有 steady-state upload/delete/manifest 更新回到 Git Database API 和 `base_commit_sha` 乐观锁路径。

### Ready vault 绑定与 rebind 事务

`initialize_github_vault` 只改变远端并返回 Ready check，不直接写 AppConfig/SyncState。无论 repo 原本 ready 还是初始化后 ready，Onboarding 都调用独立 `bind_github_vault(request)`：request 携带完整 remote identity、expected HEAD/manifest SHA、当前完整 binding key（installation_id + repository_id + branch，若有）和 `confirm_rebind`。

command 获取 SyncOperationGate write lock，再次验证 credential、全量单 repo installation scope、repository id、branch 和 Ready manifest。首次绑定创建空 state；绑定相同 installation/repository/branch 只更新 owner/repo rename 与 commit SHA；绑定不同 identity 只有 `confirm_rebind=true` 且 expected previous binding key 与锁内当前值完整匹配时才归档旧 state。同 repo branch 已被另一窗口切换也返回 stale binding，不能只按 repository id 覆盖。普通 preview/apply 永远不调用 bind/rebind。

AppConfig 与 SyncState 是两个文件，跨文件 rename 不是天然原子。`VaultBindingStore` 使用 `<config-dir>/skill-sync/bind-transaction.json` journal：先写并 fsync 新 config/state temp 与 journal，再依次 replace 两个目标，最后清 journal；启动/任何 config load 前检测 journal，根据其中 previous/new hashes 完成未完成的 replace 或恢复 previous bytes。任一步失败不得留下“新 config + 旧 base”可被正常同步读取的状态。测试覆盖每个故障注入点和恢复后的唯一一致结果。

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

Apply 响应使用 tagged union：`Applied { result }`、`PlanChanged { reason, latest_plan }` 或 `RecoveryRequired { recovery }`。`PlanChanged.reason` 只取 `remote_changed` 与 `plan_changed`：remote commit 不同优先判为 `remote_changed`；commit 相同但 fingerprint 不同判为 `plan_changed`。`Applied.result.state_updated` 返回本次完成 base adoption/removal 的 skill id；仅状态协调时 `applied` 为空、`state_updated` 非空、`remote_commit` 为空。

`RecoveryRequired` 只用于已经发生远端或本地持久化副作用、不能安全当作全新 Apply 重跑的情况。payload 至少包含 `task_id`、`phase`、nullable `remote_commit`、`completed_action_ids`、`pending_action_ids` 和可公开的错误摘要；phase 固定为 `remote_outcome_unknown`、`local_replace_failed`、`trash_move_failed` 或 `state_save_failed`。收到它后 UI 禁止新的 preview/apply，并只允许调用 `resume_sync_recovery(task_id)` 或打开相关路径，不得自动重发原 Apply 请求。

- `selected_action_ids` 是用户本次明确选择的普通动作集合。上传/下载可以由 UI 默认选中，删除动作永不默认选中。
- `base_adoptions` 与 `base_removals` 是后端根据三方表生成的本机状态协调，不属于 `selected_action_ids`。只要 Apply 请求通过版本与指纹校验，就自动执行这些协调项；它们不改变远端，也不触发删除确认。
- 冲突 decision 白名单按当前 `ConflictReason` 固定：`same_name_first_seen` / `both_changed` 允许 `keep_local | use_remote | skip`；`local_deleted_remote_changed` 允许 `delete_remote | restore_remote | skip`；`remote_deleted_local_changed` 允许 `keep_local | accept_delete | skip`。未提供 decision 表示该冲突不执行并返回 warning；不存在的 skill_id、重复 action id、不可执行 action id或 reason 不允许的 decision 返回 `Blocked`，整次 Apply 不得部分执行。
- `delete_guard_tripped == true` 且选择集合或冲突决策包含删除时，`delete_guard_ack` 必须为 true。
- 后端执行前重新生成计划，并先比较 `expected_remote_commit` 与 `plan_fingerprint`。任一不一致时，在任何持久化副作用（本地 skill 写入、trash move、远端变更或 sync_state 更新）之前返回 `PlanChanged { reason, latest_plan }`。重新扫描和 pack 产生的临时文件必须清理。
- UI 收到 `PlanChanged` 后替换为 `latest_plan`，清空旧选择、旧 conflict decisions 和删除熔断确认，要求用户重新确认；禁止自动重试 Apply。手动 recheck 或其他 query 更新导致展示中的 `plan_fingerprint` 变化时，也执行同样的清空逻辑。
- 预检通过后，GitHub 更新 branch ref 时仍使用 `base_commit_sha` 乐观锁，覆盖预检通过后到提交前的竞态窗口。

执行阶段内部使用 `PreparedSyncPlan` 同时持有 `SyncPlan`、本次重新生成且已参与 fingerprint 校验的 `PackedSkill` 以及只保存 zip/blob 的任务临时目录。Apply 必须使用这些已校验 bytes，不能在指纹校验后再次 pack；未进入本地事务前，`PreparedSyncPlan` 在成功、拒绝或错误退出时都通过 RAII 清理临时产物。下载解包结果不能留在可能跨卷的系统 temp；它必须转入目标 namespace root 内的同盘 staging，并在 apply journal 建立后由本地事务接管其生命周期。计划只有 base adoption/removal 时，`selected_action_ids` 和 `decisions` 可以为空，Apply 仍返回 `Applied` 并写入本机状态。

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
5. 使用 PreparedSyncPlan 中已校验的 pack bytes，并将 download 解包结果放入目标 namespace root 内的同盘 staging：
   - 拉取 blob
   - 校验 compressed size/hash 和 canonical archive
   - 解包后要求根级 `SKILL.md` 为 regular file，重新解析 metadata，并验证 `skill_id(namespace, SKILL.md.name)` 与 manifest map key/entry id 完全一致
   - 检查本地目标路径是否可安全覆盖
   - 如果目标路径存在但不属于当前 base，标记为冲突或 blocked
   - 所有下载都验证完成前不改正式 target
6. 对选中的 upload 动作：
   - 收集本次执行阶段重新生成的 canonical zip blob
   - 更新 next_manifest
7. 对明确选中的 delete_remote 动作：从 next_manifest 移除该 skill 条目（blob 暂不 GC）
8. 对明确选中的 delete_local 动作准备同 namespace root 内的 trash target；预先生成完整 next sync_state bytes
9. 原子写入 apply journal，记录远端意图、每个 stage/target/rollback/trash 路径、before/after hash、next state hash 和当前 phase
10. 如果需要改云端，先提交云端变更：
   - await commit_changes(base_commit_sha, blobs, next_manifest)  // 上传与删云端条目合并为 1 个 commit
   - GitHub update ref 必须 force=false；non-fast-forward / 409 / 422 映射为 RemoteChanged
   - RemoteChanged 时重新生成 latest_plan，返回 PlanChanged(RemoteChanged)，不提交已暂存的本地写入
   - update-ref 响应不明时必须重新读取 branch ref：HEAD 等于本次 candidate commit 才按成功继续，HEAD 仍为 base 才可判定未提交，其他 HEAD 视为 RemoteChanged；无法取得证据时返回 RecoveryRequired(remote_outcome_unknown)
11. 远端提交成功或本次不需要远端提交后，逐项提交暂存的本地目录替换与同盘 trash move；每完成一项就原子推进 journal
12. 原子写入 sync_state：
   - 更新 remote commit_sha
   - 更新成功同步项的 base_hash
   - 写入 base_adoptions：base_hash / last_remote_hash = adoption.hash，并更新 last_synced_at
   - 执行 base_removals：从 sync_state.skills 移除 both_deleted 项
   - 已执行的 local_deleted / remote_deleted 删除项同样从 sync_state 移除（base 清空）
   - 计划只有 base adoption/removal 时也必须原子保存 sync_state，且不调用 commit_changes
13. state 持久化成功后删除 rollback/staging 并清理 journal；清理失败不回退业务结果，返回 Applied + cleanup warning，启动时继续清理
14. 返回 ApplySyncResponse::Applied
```

### 本地写入事务与失败语义

GitHub branch、多个 namespace root 和 config directory 不在同一个事务系统中，V1 不宣称全局原子。目标是：正式 target 永远不暴露半解包目录；远端已成功后，本地失败有持久 journal 可以幂等继续；新同步不能跨过未解决事务。

每个下载使用目标 root 内的保留目录，确保最终 rename 不跨卷：

```text
<namespace-root>/.skill-sync-staging/<task-id>/<folder_name>/
<namespace-root>/.skill-sync-rollback/<task-id>/<folder_name>/
<namespace-root>/.skill-sync-trash/<task-id>/<folder_name>/
<config-dir>/skill-sync/apply-transaction.json
```

scanner 必须显式跳过三个 `.skill-sync-*` 保留目录。目标不存在时，将 stage 原子 rename 为 target；目标存在时，先把 target rename 到 rollback，再把 stage rename 到 target。第二次 rename 失败时立即尝试恢复 rollback；只有恢复成功、远端尚未发布且此前没有其他本地动作时，才可作为无副作用普通错误返回，否则保留 journal 并返回 `RecoveryRequired(local_replace_failed)`。覆盖产生的 rollback 在 sync_state 保存成功前不得删除。删本地使用同 root 的 trash 路径做 rename，不使用可能位于另一卷的 config-dir trash。若 namespace root 原本不存在，journal 记录 `created_root=true`；预提交失败只在 root 仍为空且仍由本事务拥有时删除它。

apply journal 和 sync_state 都使用其目标文件同目录的临时文件，执行 `write -> flush/fsync file -> atomic replace/rename -> fsync parent directory（或平台等价的 durable replace）`。journal 记录 schema、task id、phase、remote base/candidate/result commit、canonical next manifest hash、next state bytes/hash，以及每个动作的 expected-before/expected-after hash和路径状态。路径在恢复时重新经过固定 root 与 portable component 校验，journal 不能指挥客户端访问任意路径。

失败结果与重试策略：

| 失败点                                              | 可见副作用                                        | 返回                                       | 重试规则                                                                |
| --------------------------------------------------- | ------------------------------------------------- | ------------------------------------------ | ----------------------------------------------------------------------- |
| staging、blob/hash、`SKILL.md` 或 identity 校验失败 | 正式 target、trash、远端、state 均不变            | `Blocked` 或普通校验错误                   | 修复原因后重新 preview；不得复用旧 stage                                |
| 远端 commit 明确未发布                              | 本地与 state 不变；远端可能只有不可达 Git object  | 普通 retryable error                       | 重新 preview 后由用户重试                                               |
| 远端 HEAD 已变化且不是 candidate                    | 本地与 state 不变                                 | `PlanChanged(remote_changed, latest_plan)` | 清空旧选择，用户重新确认                                                |
| update-ref 结果无法证明成功或失败                   | 本地正式 target 与 state 不变，保留 journal/stage | `RecoveryRequired(remote_outcome_unknown)` | `resume_sync_recovery` 先读取 branch/commit 证据；禁止重发 Apply        |
| 目录替换失败且无法完整恢复                          | 远端可能已成功，部分本地动作可能完成              | `RecoveryRequired(local_replace_failed)`   | 按 journal hash 幂等完成或恢复当前动作，再继续后续动作                  |
| trash move 失败                                     | 远端可能已成功，已完成动作保留                    | `RecoveryRequired(trash_move_failed)`      | 保留源目录与 journal；resume 只重试未完成 move                          |
| sync_state 保存失败                                 | 远端和全部本地动作已完成，旧 state 仍完整         | `RecoveryRequired(state_save_failed)`      | 使用 journal 中已验证的 next state 原子重写，不重复远端 commit/目录替换 |
| state 已保存但清理失败                              | 业务状态已一致，仅残留 rollback/stage/journal     | `Applied`，附 `cleanup_pending` warning    | 启动或下次获取写锁时幂等清理，不重新同步                                |

应用启动/`get_app_state` 和 mutation command 在 `SyncOperationGate` write lock 下检查 pending journal；可由本地证据唯一判定的 cleanup 阶段可以自动恢复，需要网络确认或继续业务写入的阶段通过 `resume_sync_recovery` 处理。preview/scan 继续持 read lock，在锁内发现 journal 时只返回 `recovery_pending`，不得尝试锁升级或读取部分 state。恢复必须逐项比较 expected-before/after hash，使重复调用幂等；证据与 journal 不一致时 fail closed，保留现场并要求用户处理，不能猜测覆盖。

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
  verify compressed size/hash/canonical archive/SKILL.md/identity
  从 namespace/folder_name 解析唯一固定 target，并检查覆盖安全
  解包到 target root 内同盘 staging
  复用 apply journal 提交目录替换并原子更新 sync_state base_hash
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
- 删本地进 trash：删本地把目录 rename 到 `<namespace-root>/.skill-sync-trash/<task-id>/<folder_name>/` 而不是硬删，确保与 target 同盘；scanner 显式排除该保留目录（删云端有 Git 历史兜底）。
- blob 不做 GC：删云端只移除 `manifest.json` 条目，orphan blob 保留（内容寻址、可 dedup、可恢复），GC 延后。

## Tauri command 设计

保留现有 command 名称的同时新增 GitHub/vault 能力。

```text
async get_app_state()
async save_config(config)
async scan_skills()
async get_sync_plan()
async apply_sync_plan(request: ApplySyncRequest) -> Result<ApplySyncResponse>
async resume_sync_recovery(task_id) -> Result<ApplySyncResponse>
open_path(path)

新增：
async start_github_device_flow()
async poll_github_device_flow(device_code, interval)
async get_github_app_info()
async list_github_installations()
async list_installation_repositories(installation_id)
async discover_single_github_repository()
async list_github_repository_branches(config)
async check_github_vault(config)
async initialize_github_vault(request)
async bind_github_vault(request)
async disconnect_github(expected_repository_id)
async list_remote_skills()
async upload_skills(skill_ids)
async download_skills(skill_ids)
```

`AppState.pending_recovery` 返回 nullable `RecoveryInfo`，使应用重启后仍能展示并恢复 journal；它不包含 stage/rollback/trash 的绝对路径。pending recovery 存在时，普通同步 command 返回 `recovery_pending`。

删除 `check_git()`、`check_remote()`、`prepare_repo()` 这类 Git CLI / Git remote command。GitHub vault 模式不要求用户安装 Git，也不保留旧 Git 同步路径。

Tauri 错误边界使用结构化 payload：`kind/message/retry_after?/latest_check?`。`RateLimited` 携带 retry_after，`VaultStateChanged` 携带最新 `GithubVaultCheck`；共享前端 invoke wrapper 必须保留并校验这些字段，不能只抛出 kind/message。所有错误字段经过 token redaction。

## 前端改造范围

### 应用入口与导航门禁

应用启动先读取 `AppState`，不在状态未决时短暂渲染业务侧边栏。这里必须区分两个不同的 `configured`：`GithubAppInfo.configured` 只表示发布包注入了公开 GitHub App client id/slug，不能解锁工作区；`AppState.configured` 表示本机已经通过 `bind_github_vault` 建立完整 Ready Vault binding。前端工作区门禁使用后者，并同时要求 credential 不是 `disconnected` 或 `reauthorization_required`。

```text
未授权 / 需要重新授权 / 尚未完成 Ready binding
  -> 只渲染 Onboarding 引导壳
  -> 不显示 Sync、Settings 或其他业务导航

Ready binding + 有效或正在刷新的 credential
  -> 进入工作区壳
  -> 导航严格只有 Sync、Settings 两项
```

Device Flow 返回 authorized 只是引导中的第一阶段完成，不等于产品连接完成。App installation、唯一 repository scope、branch、manifest 检查/显式初始化和本机 bind 任一未完成，都继续停留在 Onboarding。直接访问 `/app/sync` 或 `/app/settings` 时，未通过门禁必须 replace redirect 到 `/app/onboarding`；已完成绑定的默认入口是 `/app/sync`。

`/app/onboarding` 保留为隐藏路由，不进入 authenticated navigation。已绑定用户普通访问该路径会回到 Sync；Settings 的“重新配置 Vault”使用显式 `/app/onboarding?mode=reconfigure` 进入向导并暂时隐藏工作区导航。旧 binding 在新 bind transaction 成功前继续保留，用户取消可回到 Sync。断开 GitHub 成功后清除本机 binding 并回到 Onboarding。路由门禁只是 UX 层，所有 Rust commands 仍必须独立校验 credential、binding 和 remote identity。

### Onboarding

从“填写 Git remote URL”改成“连接 GitHub vault”：

- 显示当前构建绑定的 GitHub App，并通过 GitHub App Device Flow 完成授权。
- 使用单列、单当前步骤的渐进式向导，显示“第 N/5 步”和进度；每一步只展示当前需要用户完成的操作，已完成步骤只保留简短状态，不一次堆叠全部 GitHub 概念和控件。
- 显示 GitHub user code、授权页面入口、轮询中、已授权、过期、取消、失败等状态。
- 授权后检测 App installation；没有 installation 时打开 App 安装页并支持返回后重新检查。
- 汇总 installation repositories；不是唯一 repo 时阻止继续并提示调整为 `Only select repositories` 的单个 vault repo。
- 显示并保存 installation id、repository id、GitHub owner/repo 和已有 branch。
- 展示 token 有效、自动刷新中、需要重新授权等状态，但不把 token 传给前端。
- 用区分明确的状态检测 repo、branch 和 `manifest.json`。
- `empty_repository` / `missing_manifest` 显示初始化将创建 commit 的确认；用户确认后调用 `initialize_github_vault`。
- ready check（包括初始化后的 ready）再调用 `bind_github_vault`；切换 stable identity 时单独确认归档旧 base，bind 成功后才进入 Sync。
- bind 成功返回更新后的 `AppState` 后才切换到工作区壳并显示 Sync / Settings；仅 token authorized、App installed 或 initialize Ready 都不得提前显示业务导航。
- `invalid_manifest` 显示校验错误和打开仓库入口，禁止初始化覆盖。
- `rate_limited` 显示可重试时间；`repository_unavailable` 要求重新检查授权/installation，不显示“仓库不存在”。
- 非空 repo 的 branch 缺失时要求选择已有 branch，不自动创建或猜测。
- 删除 SSH/Git 教程入口，避免用户误以为同步依赖本机 Git 配置。

### Settings

新增：

- Remote provider：固定为 GitHub App，不提供 provider selector。
- GitHub App slug、授权账号和 installation 状态。
- GitHub repo 信息（含稳定 repository id）。
- 当前 branch。
- 当前 remote commit。
- 本机 device name。
- skill size limit：默认 `20 MiB`，按 canonical zip 后大小计算，超过则拒绝上传。
- unpack safety limits：默认最多 2,000 files、单文件解压 50 MiB、累计解压 100 MiB；同时作用于本地 pack 与远端 download。
- delete guard：`max_auto_delete` 删除熔断阈值，默认 `5`，超过则要求额外确认。
- ignore rules：复用现有 textarea，全局生效，每行一个 glob。
- 三个固定 namespace root：`agents`、`codex`、`claude-code`。Settings 只读显示 namespace、解析后的绝对路径、存在/可读/扫描状态，不提供路径输入、启用开关或新增 root。
- “重新配置 Vault”进入无业务导航的 Onboarding 向导，但不预先破坏当前 binding；“断开 GitHub”成功后返回同一引导入口。两者都不能让旧工作区导航与未完成的新配置同时可操作。

GitHub access/refresh token 和过期时间只写系统 keyring。前端只接收 credential status，不接收 token；YAML/JSON config、日志、错误信息和 crash context 都不得包含 token。用户点击 Disconnect 时清除 keyring credential 和本机 remote config/sync state 前必须二次确认；它不修改 GitHub App installation，也不删除远端 vault。

demo 不迁移旧配置；包含 `hosts`、`hosts.*.paths` 或 `custom_paths` 的旧 YAML 会被当前格式解析器拒绝，也不会映射成新的扫描目录。用户仍可以在固定 root 内自由新增、编辑、移动和删除 skill；只是不允许通过应用改变工具约定的 root 路径。

### Sync

Sync 页面只在工作区门禁通过后可达，统一管理本地、云端、同步计划与冲突状态。第一阶段删除独立 `Conflicts` 页面，不再在侧边栏提供单独冲突入口。

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
- 收到 `RecoveryRequired` 后冻结 preview/apply/批量操作，展示 phase、已完成/待完成数量和 nullable remote commit，只允许按后端 task id 调用 `resume_sync_recovery`；不得重发旧 Apply。
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
  config.rs              # GitHub App repo identity 配置（含 limits.max_auto_delete）
  detect.rs              # 保留，改为三个固定 namespace root registry、状态与碰撞检测
  errors.rs              # 新增 RemoteChanged / Auth / Vault 错误类型
  skill.rs               # 保留 metadata parser，补充 normalized id
  pack.rs                # 新增 canonical zip / hash / unpack
  portable_path.rs       # 新增共享 portable component/path 与 collision 校验
  local_apply.rs         # 新增同盘 staging、apply journal、目录提交与恢复
  vault_manifest.rs      # 新增 manifest schema 与校验
  sync_state.rs          # 新增本地 base_hash/commit_sha 状态
  remote_store.rs        # 新增 trait 与 DTO
  github_app_config.rs   # 编译时 client id / app slug 公共配置
  github_auth.rs         # Device Flow 请求与响应，不负责 repo API
  github_credentials.rs  # keyring、过期判断、单飞刷新与清除
  github_repository.rs   # installation/repo 枚举、状态探测与显式初始化
  github_store.rs        # steady-state GitHub vault adapter
  vault_binding.rs       # ready vault 的 config/state journaled bind/rebind
  local_vault_store.rs   # 新增测试/开发 adapter
  sync_engine.rs         # command-facing boundary
  sync_engine/vault.rs   # 新三方比较与 apply flow（含删除传播与删除护栏）
```

删除 `git_store.rs` 及其调用方，不保留旧适配器。测试和离线调试只使用不依赖 Git 的 `LocalVaultStore`。

## 实施步骤

### 阶段 1：先落数据契约

- 新增 `vault_manifest.rs`、`sync_state.rs`、`portable_path.rs`、`pack.rs` 的类型与测试。
- 前端新增/扩展 `syncPlan.ts` schema。
- 定义 `agents` / `codex` / `claude-code` 固定 namespace registry，manifest、state、plan 统一改用 namespace/folder_name。
- 不改 UI 行为，只保证类型边界成型。

### 阶段 2：实现 canonical pack/hash

- 用测试 fixture 验证不同遍历顺序、mtime、Unix mode 和宿主平台产生完全相同 zip bytes/hash；golden fixture 锁定 entry timestamp、Unix creator system、regular-file external attributes + `0o644`、Deflate level 6、无目录项/extra/comment。
- 验证 portable path、精确重复与 NFC+lowercase 大小写碰撞；拒绝 symlink、Windows junction/reparse point 和 ZIP 特殊文件 entry。
- 验证 zip-slip、entry count、单文件/累计 unpacked bytes 限制，以及 headers 伪报时按实际流式 bytes 二次熔断。
- 扩展内置默认 ignore，并保留 Settings 全局 ignore 配置入口。
- 实现 compressed/file-count/single-unpacked/total-unpacked 四项硬限制，超过上限只进入 blocked plan，不上传或安装。
- 第一阶段采用全量扫描和朴素 canonical zip/hash，不做 fingerprint cache。

### 阶段 3：实现 LocalVaultStore

- 用本地目录模拟 `manifest.json` 与 `blobs/sha256`。
- 扫描只使用三个固定用户 root，删除旧 hosts/custom_paths 参与扫描的路径。
- 重写 `sync_engine` 的三方比较，先不接 GitHub。
- 覆盖 upload/download/update/conflict/base missing/download-only 0 commit 等规则。
- 覆盖删除规则：local_deleted 删云端、remote_deleted 删本地、删改冲突、双方删除清 base。
- 实现删除护栏：root 可读性闸门、`max_auto_delete` 熔断、删本地进 trash、blob 不 GC。

### 阶段 4：接入 GitHubVaultStore

- 注册并记录 GitHub App 的固定权限、Device Flow、token expiration 与安装说明。
- release build 注入公开 client id/app slug，缺失时阻止正式打包；不打包 private key/client secret。
- 实现 GitHub App Device Flow、access/refresh credential keyring 存储和无 client secret 自动刷新。
- 实现 installation/repository 枚举并强制 V1 单仓库边界。
- 增加区分 unauthorized/installation/repo/empty/branch/manifest 的状态探测。
- 实现空 repo 首 commit 与已有 branch 缺 manifest 的显式、幂等初始化。
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

- 主路径切到 GitHub App vault 配置，不保留 OAuth/PAT/Git/SSH provider selector。
- 根路由和 `/app` layout 先读取 `AppState`：未授权、需要重新授权或未完成 Ready binding 时只渲染 Onboarding；绑定完成后默认进入 Sync。
- authenticated navigation 只保留 Sync / Settings；Onboarding 保留为隐藏路由，不作为第三个 Tab。
- Onboarding 用单当前步骤向导串联 Device Flow、App installation、唯一 repo、branch 检测、显式初始化和 bind，并显示明确进度。
- 删除旧 Git remote 相关 UI。
- Settings 删除可编辑 roots，改成三个固定 namespace 的只读状态列表。
- Settings 的重新配置/断开动作返回无业务导航的 Onboarding；重新配置在新 bind 成功前不破坏旧 binding。
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
- root registry 路径不可由 AppConfig 改写；旧 hosts/custom_paths 配置不会被当前 demo 接受。
- 只扫描固定用户 root 的直接子目录；项目级目录、`~/.codex/skills/.system` 和无合法 `SKILL.md` 的聚合目录不会进入计划。
- scanner 永远排除 `.skill-sync-staging`、`.skill-sync-rollback` 和 `.skill-sync-trash`，恢复现场不会被识别成用户 skill。
- 同 namespace normalized ID 或大小写不敏感 folder name 碰撞时全部 blocked，不保留扫描到的第一个；测试同时覆盖纯本地、纯远端和 local/remote 合并后的目标碰撞。
- 云端新增按 namespace/folder_name 落到唯一 root；更新按 sync_state namespace/relative_dir 写回，非法或不一致目标 blocked。
- canonical zip golden bytes/hash 对遍历顺序、mtime、权限和平台稳定；metadata 固定为 ZIP epoch、Unix creator system、regular-file external attributes + 0644、Deflate level 6，无显式目录项、comment 或 extra fields。
- pack 拒绝 symlink、junction/reparse point、非 UTF-8/非 portable path，以及包内 exact/NFC+lowercase collision。
- unpack 拒绝 zip-slip、symlink/device/FIFO/显式目录等非 canonical entry，并在临时目录中执行 file count、单文件和累计实际 bytes 限制。
- 全量扫描后能为所有有效本地 skill 生成 canonical hash 和 zip size。
- 预览和执行阶段的临时 zip 目录会被清理，执行阶段不复用预览阶段 zip。
- 内置默认 ignore 和 Settings 全局 ignore 都会影响 zip/hash/size/status。
- manifest round trip 使用合法 64 位 lowercase hash；parser 拒绝 unsupported schema、重复 map key、key/id 或 namespace 不一致、非法 hash、非 hash-derived blob path、非法 size/folder/collision。
- 下载验证 manifest size == actual compressed bytes、manifest hash == actual bytes hash，再执行 canonical archive/resource validation。
- 下载解包后验证根级 `SKILL.md`、metadata 与 manifest skill identity；失败不触碰正式 target。
- sync_state round trip。
- release build 缺少 GitHub App client id 或 slug 时在打包前失败；测试构造函数可注入公开配置，运行时不读取环境变量。
- Device Flow 不发送 OAuth `scope`，正确处理 pending/slow_down/expired/denied；成功响应把 access/refresh token 与过期时间一次写入 credential store。
- access token 临近过期时自动刷新；Device Flow refresh 请求只发送 client id/refresh token，不发送 client secret；并发调用只发生一次刷新并原子保存轮换后的 refresh token。
- 未过期 token 收到 401 时按 generation 强制单飞刷新并只重放一次；并发 401 只轮换一次，第二次 401 清 credential。manifest/blob/commit 路径分别覆盖该行为。
- refresh token 过期、授权撤销或 bad refresh 会清除 credential 并返回 reauthorization_required；任何 DTO、日志和错误快照都不含 token。
- 没有 installation、`repository_selection=all`、跨所有可访问 installations 的 repo 总数不等于 1 时不能保存 remote config。
- remote config 持久化 installation/repository id；owner/repo rename 不改变 identity，repository id 不一致时 blocked。
- repo/branch/manifest 的 401/403/404 结合 rate-limit headers、GitHub error body/message/documentation URL 与完整 installation/repository 枚举证据映射；无 headers 的 secondary limit 仍是 rate_limited；证据不足时 repository_unavailable，只有精确 manifest path 404 是 missing_manifest；空 repo 通过无 branch 判定。
- HTTP 200 但 manifest JSON/schema/path/hash 非法时返回 invalid_manifest，check 与 initialize 都不得覆盖。
- sync_state remote identity 包含 installation/repository id；普通 preview/apply mismatch 只读 blocked 且 state bytes 不变；只有 bind command 的显式 confirmed rebind 归档旧 state并从空 base 开始。
- bind command 对 config/state 使用可恢复 journal；故障注入覆盖 temp write、journal fsync、两个 replace 与 journal clear 后的启动恢复。
- empty_repository 初始化用 Contents API 创建 canonical empty manifest 首 commit；missing_manifest 在已有 branch 创建 commit；ready 幂等 0 commit；非空 repo branch_missing blocked。
- 除 wiremock 外，release checklist 使用已安装 App 的 disposable empty private repo 跑 gated 首提交集成测试，验证 GitHub 真实无 HEAD 行为；token 从 keyring 读取，不进环境变量或 CI 日志。
- 初始化竞态或 409/422 后重新读取；已存在合法 empty manifest 视为成功，非空/非法 manifest 不覆盖。
- Tauri error payload 的 retry_after/latest_check 能完整序列化，前端 invoke wrapper 保留并二次 Zod 校验；错误/message/details 均通过 token redaction。
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
- canonical zip 超过 compressed/file-count/single-unpacked/total-unpacked 任一 limit 时进入 blocked，不上传 blob、不更新 manifest；下载超限不触碰正式目标。
- download-only 不创建 remote commit。
- 下载目录替换只从目标 root 内同盘 staging rename；覆盖时 rollback 保留到 sync_state 原子保存成功。
- 故障注入覆盖 apply journal write/fsync、远端结果不明、本地 target->rollback、stage->target、rollback restore、trash move、state temp write/fsync/rename 和 cleanup；逐点验证返回类型、正式路径、journal 与幂等 resume 结果。
- 远端成功后本地替换/trash/state 任一步失败返回 RecoveryRequired，resume 不重复远端 commit；pending recovery 阻止新的普通同步。
- upload/update 只创建一次 remote commit。
- 预览后、Apply 预检前 GitHub commit SHA 变化时返回 `PlanChanged(remote_changed, latest_plan)`；预检通过后的 update-ref 竞态先由 store 返回 `RemoteChanged`，再转换成带最新计划的 `PlanChanged` 响应。
- 相同计划内容无论条目生成顺序如何都产生相同 `plan_fingerprint`；本地 hash、远端 commit、冲突原因、动作方向或目标路径变化时 fingerprint 必须变化。
- Apply 的 expected_remote_commit 或 plan_fingerprint 过期时，在任何持久化副作用前返回包含 latest_plan 的 `PlanChanged`。
- 未出现在 selected_action_ids 的普通删除不得执行；delete guard 已触发但未确认时，任何删除不得执行。
- 旧计划的 conflict decision 不得应用到 reason 已变化的新冲突。
- 解包防 zip-slip、特殊文件/链接逃逸、portable path collision 和 zip bomb；header 声明与实际流式 bytes 都必须受限。
- 目标路径存在但无 base 或 hash 与 base 不一致时，不自动覆盖，进入冲突或 blocked。

前端验证：

- 未授权、credential 需要重新授权或尚未完成 Ready binding 时，首次启动和直接访问 Sync/Settings 都只显示 Onboarding，不闪现业务侧边栏。
- Onboarding 一次只显示当前步骤，具有明确进度；Device Flow authorized 后仍需完成 installation、唯一 repo、branch、manifest 和 bind，期间不显示业务 Tab。
- bind 成功后默认进入 Sync，工作区导航严格只有 Sync / Settings；Onboarding、Conflicts、Backups 都不是导航项。
- 已绑定用户普通访问 Onboarding 会回到 Sync；Settings 通过显式 reconfigure mode 进入隐藏 Onboarding 路由并可取消回到 Sync。disconnect 成功后回到 Onboarding，旧业务导航不可继续操作。
- Zod schema 能解析新 `SyncPlan`（含 delete_remote/delete_local/delete_guard_tripped）。
- Sync 页面正确显示 commit 语义。
- Sync 页面搜索和状态筛选能叠加过滤 skill 卡片。
- “删除”筛选能列出 local_deleted / remote_deleted，卡片 badge 区分删云端与删本地。
- 删改冲突详情能写入删除感知决策。
- 冲突详情摘要能写入 `syncDecisions` 的三项决策。
- Apply 请求包含当前计划版本和明确选择；收到 `PlanChanged` 后替换计划并清空选择、决策和删除确认。
- AppState 能在重启后返回 pending recovery；`RecoveryRequired` 冻结普通同步且 resume mutation 不自动重试旧 Apply。
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

### GitHub App 分发与 Device Flow

普通用户能否完成授权依赖项目维护者正确发布 GitHub App。App 注册配置、公开 client id/app slug、Device Flow 开关、Contents read/write 权限、user token expiration 和安装 URL 都必须进入 release checklist。V1 不要求 deep link，不让用户创建 PAT，也不提供 OAuth App fallback；构建配置错误应在 CI 打包前暴露，而不是安装后才发现。

Device Flow 需要处理 user code 过期、用户取消、slow_down、网络错误和 token rotation。刷新不需要 client secret 的前提是 token 由 Device Flow 生成，因此 credential 中保存授权时的 app client id，并拒绝拿其他构建的 refresh token 静默刷新。

### 单仓库权限的真实边界

GitHub App user token 不是按当前 UI 选择动态裁剪的 token；它可以触达 App 已安装且用户有权访问的资源。V1 通过最小 App permissions、列举所有 user installations 并要求总计唯一 repository 来实现单仓库承诺。若用户随后扩大 App 安装范围，下一次 app state/check/sync 在网络副作用前重新验证并 blocked，要求先收窄权限。

### 空仓库初始化

Git Database API 不能依赖不存在的 HEAD 创建常规同步 commit。空 repo 和缺 manifest 使用独立、用户确认的 Contents API 初始化路径；一旦 HEAD 与 manifest 存在，初始化 command 不再用于同步。初始化状态变化、响应丢失或并发请求都通过重新读取实现幂等，绝不覆盖已存在的非空或非法 manifest。

### GitHub API 并发

多台电脑同时同步时，必须同时使用预览版本校验和 commit SHA 乐观锁。Apply 先用 `expected_remote_commit + plan_fingerprint` 防止旧页面决策套到新计划；移动 branch ref 时再用 `base_commit_sha` 覆盖预检后的竞态窗口。任一校验失败都不要强推、不要覆盖，返回最新计划并要求用户重新确认。

### Async runtime 与阻塞 I/O

reqwest 网络请求运行在 Tauri/Tokio async runtime，目录扫描、zip、hash、解包和移动目录属于阻塞工作，必须进入 `spawn_blocking`。禁止在 async command 内直接执行大批量文件 I/O，也禁止为同步 API 创建嵌套 runtime；否则会造成 UI event loop 卡顿或 runtime panic。应用级 SyncOperationGate 负责本机操作串行化，RemoteStore 的 commit SHA 乐观锁负责跨设备远端并发，两者不能互相替代。

### 第一阶段不做备份恢复

第一阶段删除完整备份/恢复能力，风险控制依赖三方比较、冲突阻止和覆盖前目标路径检查。只要目标路径不是“本机 base 记录对应的未修改版本”，就不能被云端下载静默覆盖。后续如果用户反馈需要误操作恢复，再单独设计 Backups 页面、保留周期和清理策略。

### 扫描与打包性能

第一阶段每次预览/执行同步前全量扫描并重新 pack/hash。数百个小 skill 可以先接受；如果后续出现明显卡顿，再增加 fingerprint cache。缓存优化不进入第一阶段，避免引入“缓存是否可信”的额外状态复杂度。

### 大文件与仓库膨胀

GitHub repo 不适合大 blob，远端 zip 也可能是高压缩比 zip bomb。默认排除缓存、临时文件、日志和依赖目录，并同时限制 compressed bytes、file count、single unpacked bytes 与 total unpacked bytes。比如 `skills/skillA/cache/` 里有 100 MiB 缓存时，应通过 ignore 排除；被排除内容不进入 zip/hash。上传超限时 blocked 且不写 manifest；下载超限时只清理 task temp，不触碰正式 skill。用户需要清理大文件或调整在安全范围内的 limits。

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
