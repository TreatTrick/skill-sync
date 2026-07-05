# Skill Sync 桌面应用分布式执行指南

本文档说明如何把 Tauri 版 Skill Sync MVP 拆给多个 agent、贡献者或并行 worktree 执行，同时避免互相踩代码。

## 执行模型

采用一个协调者加多个专职 worker 的模式。

- 协调者负责范围控制、集成顺序、分支卫生和最终验收。
- Worker 每次只负责一个可测试的纵向切片。
- Reviewer 在合并前检查 UI 行为、后端正确性、安全性和文档。

MVP 是桌面应用，不是命令行工具。所有用户能力都应通过 UI 暴露。开发过程中可以使用构建命令和测试命令，但产品设计不能依赖用户输入命令。

## 共同规则

- 开始前阅读 `D:\project\skill-sync\docs\development-plan.md`。
- 使用 `agent/<task-id>-<short-name>` 格式命名 feature branch。
- Commit 要小，message 要描述清楚。
- 后端逻辑先写 Rust 测试。
- 前端复杂组件至少有 smoke test 或组件测试。
- 不引入托管后端。
- 不实现用户账号系统。
- 没有备份和冲突检查时，不覆盖本地文件。
- 不执行 synced skills 里的脚本。
- 前端不直接访问文件系统，必须走 Tauri command。
- 行为变化必须同步更新文档。

## 分支策略

```text
main
  agent/001-tauri-skeleton
  agent/002-ui-shell-contracts
  agent/003-skill-detection
  agent/004-config-manifest
  agent/005-git-store
  agent/006-sync-preview
  agent/007-apply-backup
  agent/008-onboarding-skills-ui
  agent/009-conflicts-backups-ui
  agent/010-docs-release
```

合并顺序：

1. `agent/001-tauri-skeleton`
2. `agent/002-ui-shell-contracts`
3. `agent/003-skill-detection`
4. `agent/004-config-manifest`
5. `agent/005-git-store`
6. `agent/006-sync-preview`
7. `agent/007-apply-backup`
8. `agent/008-onboarding-skills-ui`
9. `agent/009-conflicts-backups-ui`
10. `agent/010-docs-release`

如果两个分支修改同一个文件，后合并的 worker 必须在前一个分支合并后 rebase。

## 工作包 001：Tauri 项目骨架

**负责人：** Foundation worker

**分支：** `agent/001-tauri-skeleton`

**输入：**

- `docs/development-plan.md` 的阶段 1。

**输出：**

- Tauri + React + TypeScript 项目骨架。
- `src/App.tsx`
- `src-tauri/src/main.rs`
- `src-tauri/tauri.conf.json`
- `package.json`
- `README.md`
- `LICENSE`
- `.github/workflows/ci.yml`

**完成标准：**

- 应用能启动桌面窗口。
- Dashboard 空状态可见。
- 前端类型检查通过。
- Rust 编译检查通过。
- CI workflow 已存在。

**Worker prompt：**

```text
执行 D:\project\skill-sync\docs\distributed-execution.md 中的工作包 001。
以 D:\project\skill-sync\docs\development-plan.md 作为事实来源。
创建分支 agent/001-tauri-skeleton。
初始化 Tauri + React + TypeScript 桌面应用，保留最小 UI，测试通过后停止。
```

## 工作包 002：UI Shell 与 API 契约

**负责人：** UI contract worker

**分支：** `agent/002-ui-shell-contracts`

**输入：**

- 开发计划中的页面设计和 Tauri command 设计。

**输出：**

- `src/app/layouts/AppLayout.tsx`（scaffold 已提供，本包调整）
- `src/app/router/routeConfig.tsx`（注册各模块路由）
- `src/modules/<module>/pages/*.tsx`（各页面骨架）
- `src/shared/lib/tauri.ts`（invoke 封装）
- `src/shared/schemas/apiResponse.ts`
- 各模块 `types/` 与 `schemas/`
- `src-tauri/src/commands.rs`
- `src-tauri/src/errors.rs`

**完成标准：**

- 页面导航完整。
- 所有页面有可识别空状态。
- TypeScript 类型覆盖主要 DTO。
- Rust stub commands 可返回假数据。
- UI 不依赖真实后端也能运行。

**Worker prompt：**

```text
执行 D:\project\skill-sync\docs\distributed-execution.md 中的工作包 002。
等待 agent/001-tauri-skeleton 合并后，创建分支 agent/002-ui-shell-contracts。
实现桌面 UI shell、页面骨架、TypeScript DTO 和 Rust stub commands。
```

## 工作包 003：Skill 解析与本地发现

**负责人：** Detection worker

**分支：** `agent/003-skill-detection`

**输入：**

- API 契约。
- `tests/fixtures/skills`。

**输出：**

- `src-tauri/src/skill.rs`
- `src-tauri/src/detect.rs`
- `tests/fixtures/skills/*/SKILL.md`
- `scan_skills()` command 真实实现。

**完成标准：**

- 合法 `SKILL.md` 可以解析。
- 无效 metadata 返回结构化错误或 warning。
- 缺失路径不是 fatal error。
- 重复 skill ID 会被报告。
- Rust tests 使用临时目录。

**Worker prompt：**

```text
执行 D:\project\skill-sync\docs\distributed-execution.md 中的工作包 003。
等待 agent/002-ui-shell-contracts 合并后，创建分支 agent/003-skill-detection。
先写 Rust 失败测试，再实现 SKILL.md 解析和本地 skills 扫描。
```

## 工作包 004：配置与 Manifest

**负责人：** State worker

**分支：** `agent/004-config-manifest`

**输入：**

- 开发计划中的配置模型、Skill 数据模型、SyncPlan 数据模型。

**输出：**

- `src-tauri/src/config.rs`
- `src-tauri/src/manifest.rs`
- `save_config(config)` command。
- `get_app_state()` command。

**完成标准：**

- 首次启动能生成默认配置。
- 配置保存到系统 app config 目录。
- 用户路径支持 `~` 展开。
- Manifest hash 稳定。
- Manifest 内路径分隔符统一。
- Rust tests 通过。

**Worker prompt：**

```text
执行 D:\project\skill-sync\docs\distributed-execution.md 中的工作包 004。
等待 agent/003-skill-detection 合并后，创建分支 agent/004-config-manifest。
实现配置持久化和 manifest/hash。重点保证跨平台路径和状态确定性。
```

## 工作包 005：Git Store

**负责人：** Git worker

**分支：** `agent/005-git-store`

**输入：**

- 配置模型。
- 开发计划中的 Git 策略。

**输出：**

- `src-tauri/src/git_store.rs`
- `check_git()` command。
- `check_remote(remote, branch)` command。

**完成标准：**

- 能检测 Git 是否安装。
- 能检测 remote 是否可访问。
- 能报告 dirty repo。
- Git CLI 参数显式传入，不拼接 shell 字符串。
- 测试使用本地临时 Git 仓库。

**Worker prompt：**

```text
执行 D:\project\skill-sync\docs\distributed-execution.md 中的工作包 005。
等待 agent/004-config-manifest 合并后，创建分支 agent/005-git-store。
用系统 Git CLI 实现 Git store。不要执行任意 shell 字符串。
```

## 工作包 006：同步预览

**负责人：** Sync plan worker

**分支：** `agent/006-sync-preview`

**输入：**

- Detection、config、manifest、Git store。
- 开发计划中的 SyncPlan 模型。

**输出：**

- `src-tauri/src/sync_engine.rs`
- `get_sync_plan()` command。
- `src/modules/sync/pages/SyncPreviewPage.tsx` 真实数据接入。

**完成标准：**

- 可以生成 uploads、downloads、updates、deletes、conflicts。
- 生成预览不写任何文件。
- Sync Preview 页面按分组展示计划。
- 冲突数量能反映到 Dashboard。

**Worker prompt：**

```text
执行 D:\project\skill-sync\docs\distributed-execution.md 中的工作包 006。
等待 agent/005-git-store 合并后，创建分支 agent/006-sync-preview。
实现只读同步预览。该工作包不能写入用户 skill 目录。
```

## 工作包 007：应用同步与备份

**负责人：** Write safety worker

**分支：** `agent/007-apply-backup`

**输入：**

- 同步计划。
- 开发计划中的冲突策略和安全模型。

**输出：**

- `src-tauri/src/backup.rs`
- `apply_sync_plan(decisions)` command。
- `list_backups()` command。
- `restore_backup(backup_id)` command。

**完成标准：**

- 写入前创建备份。
- 冲突未决时不写入。
- 只执行用户确认过的动作。
- 可列出备份。
- 可恢复备份。
- Rust tests 覆盖备份和冲突阻止写入。

**Worker prompt：**

```text
执行 D:\project\skill-sync\docs\distributed-execution.md 中的工作包 007。
等待 agent/006-sync-preview 合并后，创建分支 agent/007-apply-backup。
优先保证安全。没有备份和用户确认时，绝不覆盖本地文件。
```

## 工作包 008：Onboarding 与 Skills UI

**负责人：** Product UI worker

**分支：** `agent/008-onboarding-skills-ui`

**输入：**

- 已实现的 config、scan、Git check commands。

**输出：**

- `src/modules/onboarding/pages/OnboardingPage.tsx`
- `src/modules/skills/pages/SkillsPage.tsx`
- `src/shared/ui/path-picker.tsx`
- `src/modules/skills/components/SkillTable.tsx`
- `src/shared/ui/status-badge.tsx`

**完成标准：**

- 首次启动能完成路径选择和 remote 配置。
- Git 检查结果在 UI 中可见。
- Skills 页面展示真实扫描结果。
- 用户可以启用/禁用 skill 同步。
- 用户可以打开 skill 所在文件夹。

**Worker prompt：**

```text
执行 D:\project\skill-sync\docs\distributed-execution.md 中的工作包 008。
等待 agent/007-apply-backup 合并后，创建分支 agent/008-onboarding-skills-ui。
实现首次引导和 Skills 管理页面。界面要偏工具型、清楚、可扫描。
```

## 工作包 009：Conflicts 与 Backups UI

**负责人：** Recovery UI worker

**分支：** `agent/009-conflicts-backups-ui`

**输入：**

- Sync plan、apply sync、backup commands。

**输出：**

- `src/modules/conflicts/pages/ConflictsPage.tsx`
- `src/modules/backups/pages/BackupsPage.tsx`
- `src/modules/conflicts/components/ConfirmDialog.tsx`

**完成标准：**

- 冲突页面展示原因和可选动作。
- 用户可以选择保留本地、使用远程、跳过。
- 写入前有二次确认。
- Backups 页面可以列出和恢复备份。
- 恢复动作显示目标路径。

**Worker prompt：**

```text
执行 D:\project\skill-sync\docs\distributed-execution.md 中的工作包 009。
等待 agent/008-onboarding-skills-ui 合并后，创建分支 agent/009-conflicts-backups-ui。
实现冲突处理和备份恢复 UI。所有危险动作都要有清楚确认。
```

## 工作包 010：文档、测试与发布

**负责人：** Release worker

**分支：** `agent/010-docs-release`

**输入：**

- 最终桌面应用行为。
- 安全模型。

**输出：**

- `README.md`
- `docs/getting-started.md`
- `docs/configuration.md`
- `docs/safety.md`
- `tests/e2e/smoke.spec.ts`
- `.github/workflows/release.yml`

**完成标准：**

- README 可以引导用户完成第一次图形化同步。
- 文档不要求用户使用命令行完成产品功能。
- Safety 文档明确说明不会执行 skill 脚本。
- Smoke test 覆盖启动、导航、设置、扫描。
- Release workflow 构建 Windows、macOS、Linux 安装包。

**Worker prompt：**

```text
执行 D:\project\skill-sync\docs\distributed-execution.md 中的工作包 010。
等待 agent/009-conflicts-backups-ui 合并后，创建分支 agent/010-docs-release。
补齐用户文档、smoke test 和跨平台发布流程。只记录真实行为。
```

## Review 协议

每个工作包合并前需要两轮 review。

### Review 1：正确性

Reviewer 检查：

- 测试覆盖目标行为。
- Tauri command 返回结构化错误。
- 前后端 DTO 一致。
- 跨平台路径处理谨慎。
- UI 状态与后端状态一致。
- 实现没有背离 MVP 范围。

### Review 2：安全性

Reviewer 检查：

- 没有备份时不会覆盖用户文件。
- Sync Preview 不写文件。
- Git 命令显式且作用域明确。
- 不执行 skill 脚本。
- 不把 secrets 打到日志里。
- 冲突未决时会停止写入。
- UI 清楚展示危险动作目标路径。

### Review 3：体验

Reviewer 检查：

- 用户不需要命令行即可完成核心流程。
- 文案具体、可行动。
- 表格和状态能快速扫描。
- 窄窗口下没有明显溢出。
- 错误信息说明下一步。

## 集成检查清单

合并任何分支前：

- [ ] 分支已 rebase 到最新 `main`。
- [ ] 前端类型检查通过。
- [ ] 前端测试通过。
- [ ] Rust tests 通过。
- [ ] Rust clippy 无关键 warning。
- [ ] 新行为有测试。
- [ ] 用户可见行为已有文档或 issue 跟踪。
- [ ] 没有无关重构。
- [ ] 没有提交生成噪音。

发布 `v0.1.0` 前：

- [ ] Windows 上能启动应用。
- [ ] macOS/Linux 至少 CI 构建通过。
- [ ] 在临时 Git 仓库中测试第一次同步。
- [ ] 在干净临时 home 目录中测试恢复。
- [ ] 测试冲突检测。
- [ ] 测试备份恢复。
- [ ] 确认 README 与真实 UI 一致。

## 状态更新模板

Worker 应按以下格式汇报：

```text
Package:
Branch:
Completed:
Tests run:
Files changed:
Open risks:
Needs review:
```

## 交接模板

Worker 完成后：

```text
Work Package <ID> is ready for review.

Branch: agent/<id>-<slug>
Summary:
- ...

Verification:
- frontend typecheck
- frontend tests
- rust tests
- clippy

Notes:
- ...
```

## 冲突解决

如果两个 worker 需要修改同一个文件：

1. 后开始的 worker 暂停。
2. 协调者决定哪个分支先合并。
3. 后开始的 worker rebase。
4. 后开始的 worker 重新运行所有相关测试。
5. 如果 UI 或行为变化，后开始的 worker 更新文档。

不要通过大范围重写解决协作冲突。

## 协调者检查清单

- [ ] 保持 `main` 绿色。
- [ ] 按依赖顺序合并。
- [ ] 拒绝 scope creep。
- [ ] 确保产品功能通过 UI 暴露。
- [ ] 行为变化时要求测试。
- [ ] 保持 README 与真实 UI 一致。
- [ ] 跟踪 MVP 排除项，避免 worker 不小心做成平台。

## 并行化地图

Tauri skeleton 后可以并行：

- UI shell 与类型契约。
- README 初稿。

API 契约后可以并行：

- Skill 解析与发现。
- 页面视觉打磨。

Discovery 后可以开始：

- Config 与 manifest。

Config 与 manifest 后可以开始：

- Git store。

Git store 后可以开始：

- Sync preview。

Sync preview 后可以开始：

- Apply sync 与 backups。

Apply sync 后可以并行：

- Onboarding 与 Skills UI。
- Conflicts 与 Backups UI。
- Safety docs。

最后执行：

- Smoke tests。
- Release workflow。
- End-to-end 手工验收。

