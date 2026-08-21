## Context

`get_sync_plan` 和 `apply_sync_plan` 当前在单个 Tauri 调用内完成远端 snapshot、固定目录扫描、全部本地 skill 打包/hash 以及后续动作，因此前端只能显示一次性的 loading 状态。`apply_plan` 会先重新准备计划，再执行本地副作用、一次远端提交、状态持久化和清理。

打包结果是规范化 zip，hash 由 zip 字节决定。远端适配器在提交前仍必须校验上传 blob 的 SHA-256，因此缓存不能削弱最终上传完整性校验。

## Goals / Non-Goals

**Goals:**

- 在计划准备和执行期间提供可持续更新的阶段进度。
- 在本地文件元数据和打包选项未变化时复用规范化 zip/hash，避免重复读取和压缩。
- 保持一次同步的单 commit 事务边界、现有 recovery 语义和远端完整性校验。
- 提供有上限、可观测、可手动清理的本地缓存。

**Non-Goals:**

- 不把一次同步拆成多个远端 commit。
- 不提供中途取消或撤销本地/远端副作用。
- 不缓存或改变远端 manifest/blob 的权威数据。
- 不承诺远端 commit 内部的百分比进度。

## Decisions

### 1. 使用 Tauri event 推送进度

为计划准备和执行共用 `sync-progress` 事件，命令在开始工作前生成 `operation_id`，前端先注册监听再调用命令。事件 payload 使用稳定的机器字段：`operation_id`、`operation`、`phase`、`current`、`total`、`skill_id`、`determinate`、`cache_hits`、`cache_misses` 和可选错误码；用户文案由前端 i18n 根据 phase 映射。`total` 在未知阶段为空，`determinate=false`。

扫描、打包、下载、替换、删除、保存状态和清理阶段发送确定进度。单次远端 commit 发送 `remote_commit` 阶段的不确定事件，并在成功/失败时发送终态。命令仍在同一个 gate 和事务中运行；事件只提供观测，不改变执行顺序。

相比轮询，events 能在阻塞的单次调用中即时反馈且不需要暴露临时任务存储；相比拆分命令，不会改变 recovery 或 commit 语义。

### 2. 用元数据指纹复用持久化 zip

新增本地 cache 模块，默认目录为应用配置目录下的 `sync-cache`。每个 entry 保存 cache/schema/packer 算法版本、skill identity、规范化 source path、被纳入打包的文件清单（相对路径、文件类型、字节大小、修改时间纳秒）、打包选项指纹（ignore 规则和全部 limits）、zip 相对路径、zip size、hash、warnings 和最近访问时间。

计划准备先按现有安全规则枚举文件元数据并执行数量/大小/符号链接限制，再比较 entry。所有字段一致且 zip 文件存在、大小匹配并能被 zip reader 打开时复用缓存；否则删除坏 entry 并走现有完整读取、规范化压缩、hash 和 warning 扫描路径。新的 packer/cache 版本会使旧 entry 全部失效。缓存写入使用临时文件、fsync 和原子替换，index 更新与 zip 写入保持可恢复顺序。

元数据快路径不能发现“内容改变但大小和修改时间被刻意恢复”的极端情况；发生缓存 miss、格式损坏、规则变化或无法安全枚举时必须完整重算。实际上传时仍通过 `RemoteStore` 的 blob 校验再次计算上传字节 hash。

### 3. 以总容量和 LRU 管理缓存

缓存 index 记录 entry size 和 last-access。默认总容量上限为 1 GiB；写入新 entry 后按最近访问时间和稳定 tie-breaker 删除最旧 entry，直到总量不超过上限。单个超过上限的 zip 不进入缓存但不阻塞同步。缓存只存本机可重建数据，清空或损坏不会影响 sync state、远端内容或 recovery journal。

新增只读缓存统计命令和清空命令，均受同步 operation gate 保护。设置页展示 entry 数、占用字节和上限，并提供清空操作；同步运行时清空按钮禁用。

### 4. 前端按阶段显示，不伪造总体百分比

同步页监听事件并显示当前 phase、当前 skill 和 `current/total`。确定阶段使用进度条；`remote_commit` 使用不确定进度样式；失败显示失败阶段和错误，不显示 100% 成功。完成事件到达后由现有 query invalidation 重新读取计划和 app state。

## Risks / Trade-offs

- [元数据误判] 内容被修改且元数据保持不变时可能复用旧 hash → 缓存只作为性能快路径；提供手动清空，并在缓存 miss/校验失败时回退完整打包。最终远端 blob 校验仍阻止错误上传。
- [磁盘占用] 1 GiB 缓存可能占用较多空间 → LRU 自动清理、设置页展示占用并允许手动清空。
- [事件丢失] 前端监听建立前可能错过早期事件 → 监听器必须在 invoke 前注册；后续事件带 operation_id，命令结果仍是最终权威。
- [进度粒度] 单个 skill 内的压缩和单个远端 commit 无法提供可靠百分比 → 使用阶段级确定进度与不确定进度，展示当前项而不伪造细粒度数值。
- [并发访问] 多个 cache writer 可能覆盖 index → 复用现有同步 gate，并在 cache 模块内部采用原子 index 更新；缓存损坏时可删除 entry 重建。

## Migration Plan

首次运行创建 `sync-cache` 目录和空 index，不需要迁移现有 `config.yaml` 或 `sync_state.json`。发布后第一次同步建立缓存，后续同步逐步命中。回滚代码时可安全忽略或删除该目录；旧版本不会读取新缓存格式。

## Open Questions

无。取消同步和远端 commit 内部百分比均明确排除在本变更之外。
