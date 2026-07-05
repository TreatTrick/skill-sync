# Skill Sync 桌面应用开发计划

> **给 agent 执行者：** 必须使用 `superpowers:subagent-driven-development`（如果环境支持子代理）或 `superpowers:executing-plans` 来执行本计划。所有步骤使用 checkbox（`- [ ]`）语法，便于跟踪进度。

**目标：** 构建一个开源 Tauri 桌面应用，让开发者通过 UI 同步自己在多台电脑上的 agent skills。

**产品形态：** 不做命令行工具。用户通过桌面界面完成首次配置、skills 扫描、同步预览、上传/下载、冲突处理、备份恢复和设置管理。

**架构：** 前端负责交互、列表、状态展示和冲突决策；Tauri/Rust 后端负责本地文件扫描、`SKILL.md` 解析、Git 同步、备份、冲突检测和配置持久化。云端不做自建账号系统，MVP 使用用户自己的 GitHub/GitLab/自托管 Git 仓库作为同步源。

**技术栈：** Tauri（当前稳定大版本）、Rust、React、TypeScript、Vite、CSS Modules 或 Tailwind CSS、系统 Git CLI、JSON/YAML 配置、Vitest、Rust unit tests、GitHub Actions。

---

## 产品判断

Skill Sync 的第一版不应该做公共 marketplace，也不应该先做 SaaS 账号系统。最有价值的第一步是：

> 让你的 agent skills 像 dotfiles 一样跟着你走，但用桌面 UI 完成。

这个产品真正的优势是降低同步门槛。很多用户知道自己有好用的 skills，却不想记路径、写脚本、处理 Git 冲突。桌面应用可以把这些复杂性包起来。

## MVP 范围

包含：

- 首次启动引导。
- 选择或创建本地同步仓库目录。
- 配置 Git remote。
- 自动发现本机已存在的 agent skills。
- 支持 Codex、Claude Code、Cursor、Gemini CLI 和自定义路径。
- 在 UI 中展示每个 skill 的来源、路径、状态和是否参与同步。
- 同步前显示预览：新增、修改、删除、冲突。
- 通过按钮执行同步。
- 冲突页面支持保留本地、保留远程、跳过。
- 写入本地文件前自动创建备份。
- 备份页面支持查看和恢复。
- 设置页面支持管理 agent 路径、Git remote、备份目录和 ignore 规则。
- 所有危险写操作支持预览。

不包含：

- 用户账号系统。
- 公共 marketplace。
- Web dashboard。
- 团队权限。
- 端到端加密。
- 自动后台守护进程。
- 移动端。
- 插件商店。
- 自动执行 skill 内脚本。

## 核心用户故事

- 作为开发者，我第一次打开应用时，可以选择一个 Git 仓库作为 skills 同步源。
- 作为开发者，我可以看到应用自动发现了哪些本地 skills。
- 作为开发者，我可以勾选哪些 skills 要同步。
- 作为开发者，我可以在点击同步前看到所有计划变更。
- 作为开发者，我可以一键把本机 skills 推送到自己的远程 Git 仓库。
- 作为新电脑上的开发者，我可以一键从远程仓库恢复 skills。
- 作为谨慎的开发者，我可以看到冲突项，并逐项决定保留哪一边。
- 作为开发者，我可以从备份页面恢复误覆盖前的版本。

## 用户体验原则

- 首页必须是可用的工作台，不做营销 landing page。
- 所有重要动作都要可预览。
- 所有写操作都要显示目标路径。
- 冲突不要自动猜，必须让用户确认。
- Git 细节尽量隐藏，但错误信息必须可行动。
- UI 不使用大面积装饰卡片，偏安静、工具型、可扫描。
- 状态文案要具体，例如“远程有 3 个更新，本地有 1 个冲突”，不要只写“需要同步”。

## 页面设计

### 1. 首次引导

目的：让用户完成最小配置。

内容：

- 选择本地同步仓库目录。
- 填写 Git remote URL。
- 检查 Git 是否可用。
- 检查 remote 是否可访问。
- 选择默认扫描的 agent 类型。
- 显示将要使用的本地 skills 路径。

关键状态：

- 未安装 Git。
- remote 不可访问。
- 本地仓库目录为空。
- 本地仓库目录已有 Git repo。
- 用户跳过 remote，只做本地备份。

### 2. Dashboard

目的：让用户一眼知道当前同步状态。

内容：

- 已发现 skills 数量。
- 已启用同步的 skills 数量。
- 待上传数量。
- 待下载数量。
- 冲突数量。
- 最近一次同步时间。
- 主操作按钮：“预览同步”。

### 3. Skills

目的：管理哪些 skills 参与同步。

列表字段：

- 名称。
- 来源 agent。
- 本地路径。
- 同步状态。
- 是否启用。
- 最近修改时间。
- 内容 hash 简写。

操作：

- 启用/禁用同步。
- 打开所在文件夹。
- 查看 `SKILL.md` 摘要。
- 重新扫描。

### 4. Sync Preview

目的：执行同步前展示计划。

分组：

- 将上传到远程。
- 将从远程安装到本地。
- 将更新。
- 将删除。
- 需要用户处理的冲突。

操作：

- 执行同步。
- 跳过本次。
- 查看详情。

### 5. Conflicts

目的：逐项解决冲突。

每个冲突显示：

- skill 名称。
- 本地路径。
- 远程路径。
- 本地修改时间。
- 远程修改时间。
- 差异摘要。

决策：

- 保留本地。
- 使用远程。
- 跳过。

### 6. Backups

目的：恢复写入前的备份。

内容：

- 备份时间。
- 受影响 skill。
- 原路径。
- 备份大小。
- 恢复按钮。

### 7. Settings

目的：管理长期配置。

内容：

- Git remote。
- 当前分支。
- 本地同步仓库路径。
- Agent 路径列表。
- 自定义路径。
- 备份目录。
- Ignore 规则。
- 是否显示高级 Git 日志。

## 项目目录规划

```text
D:\project\skill-sync
  src\
    main.tsx
    App.tsx
    routes\
      Onboarding.tsx
      Dashboard.tsx
      Skills.tsx
      SyncPreview.tsx
      Conflicts.tsx
      Backups.tsx
      Settings.tsx
    components\
      AppShell.tsx
      StatusBadge.tsx
      SkillTable.tsx
      ConfirmDialog.tsx
      PathPicker.tsx
    lib\
      api.ts
      types.ts
      format.ts
    styles\
      globals.css
  src-tauri\
    src\
      main.rs
      commands.rs
      config.rs
      skill.rs
      detect.rs
      manifest.rs
      git_store.rs
      sync_engine.rs
      backup.rs
      errors.rs
    tauri.conf.json
    Cargo.toml
  docs\
    development-plan.md
    distributed-execution.md
  tests\
    fixtures\
      skills\
        codex-sample\
          SKILL.md
        claude-sample\
          SKILL.md
  package.json
  vite.config.ts
  tsconfig.json
  README.md
  LICENSE
```

用户同步仓库结构：

```text
agent-skills-repo
  skills\
    codex\
      my-skill\
        SKILL.md
    claude\
      review-helper\
        SKILL.md
    shared\
      markdown-editor\
        SKILL.md
  skill-sync.yaml
  skill-sync.lock.json
  .skill-syncignore
```

## 配置模型

应用配置保存在用户本地 app config 目录，例如：

```yaml
version: 1
repository:
  local_path: "D:/agent-skills"
  remote: "git@github.com:example/agent-skills.git"
  branch: "main"
defaults:
  backup: true
  install_mode: "copy"
ui:
  show_advanced_git_logs: false
hosts:
  codex:
    enabled: true
    paths:
      - "~/.agents/skills"
      - "~/.codex/skills"
  claude:
    enabled: true
    paths:
      - "~/.claude/skills"
  cursor:
    enabled: false
    paths:
      - "~/.cursor/skills"
  gemini:
    enabled: false
    paths:
      - "~/.gemini/skills"
custom_paths: []
ignore:
  - "**/.git/**"
  - "**/node_modules/**"
  - "**/.env"
```

说明：

- 路径是候选项，应用必须检查当前机器上哪些路径真实存在。
- 用户可以在 Settings 添加自定义路径。
- MVP 只实现 `copy` 安装模式。
- UI 不暴露命令行语义，内部仍可调用系统 Git。

## 后端数据模型

`Skill`：

- `id`：稳定 ID，默认 `<host>/<directory-name>`。
- `name`：来自 `SKILL.md` front matter 的 `name`。
- `description`：来自 `SKILL.md` front matter 的 `description`。
- `host`：来源 agent，例如 `codex`。
- `source_path`：本地绝对路径。
- `repo_path`：同步仓库中的规范化路径。
- `hash`：忽略规则生效后的目录内容 hash。
- `modified_at`：最后修改时间。
- `enabled`：是否参与同步。

`SyncPlan`：

- `uploads`：本地需要上传的 skills。
- `downloads`：远程需要安装到本地的 skills。
- `updates`：两边都存在但一边较新的 skills。
- `deletes`：计划删除的 skills。
- `conflicts`：需要用户决策的冲突。
- `warnings`：非阻塞 warning。

`Conflict`：

- `skill_id`。
- `local_path`。
- `remote_path`。
- `local_hash`。
- `remote_hash`。
- `reason`。
- `available_actions`。

## Tauri Command 设计

前端只通过 Tauri command 调用后端，不直接访问文件系统。

```text
get_app_state()
save_config(config)
choose_directory()
check_git()
check_remote(remote, branch)
scan_skills()
get_sync_plan()
apply_sync_plan(decisions)
list_backups()
restore_backup(backup_id)
open_path(path)
```

要求：

- 所有 command 返回结构化错误。
- 所有写操作返回 planned actions 和实际 actions。
- Rust 后端不直接弹 UI。
- 前端不拼接危险路径。

## Git 策略

MVP 使用系统 Git CLI，而不是一开始引入复杂 Git library。

原因：

- 用户更容易用现有 SSH key 和 credential helper。
- GitHub/GitLab/自托管 Git 支持更自然。
- 出错时可以把 stderr 转成可读诊断。

限制：

- 应用需要检测 Git 是否安装。
- Windows 上要处理 Git 路径和 SSH agent 问题。
- UI 需要把 Git 错误翻译成用户可行动信息。

## 冲突策略

冲突场景：

- 同一个 skill 本地和远程都改了。
- 同一个 skill ID 出现在多个 host 下。
- 目标路径已经存在，但不是 Skill Sync 管理的内容。
- 远程删除和本地修改冲突。
- `SKILL.md` 的 `name` 改了，但目录 ID 没变。

MVP 行为：

- 遇到冲突不自动覆盖。
- Conflicts 页面展示冲突原因。
- 用户必须选择保留本地、使用远程或跳过。
- 写入前自动备份。
- 暂不提供“全部强制覆盖”按钮。

## 安全与信任

Skills 可能包含私有工作流、脚本、内部路径和公司知识。MVP 必须保守：

- 不提供第三方托管存储。
- 默认不采集 telemetry。
- 不自动执行 skill 目录里的脚本。
- 对 `.env`、`id_rsa`、`*.pem`、常见 API key 格式做基础 warning。
- 尊重 `.skill-syncignore`。
- 所有写操作都必须在 Sync Preview 中可见。
- 远程仓库由用户自己控制。

## 阶段 1：Tauri 项目骨架

### 任务 1：初始化桌面应用

**文件：**

- 创建：`package.json`
- 创建：`vite.config.ts`
- 创建：`tsconfig.json`
- 创建：`src/main.tsx`
- 创建：`src/App.tsx`
- 创建：`src-tauri/Cargo.toml`
- 创建：`src-tauri/src/main.rs`
- 创建：`src-tauri/tauri.conf.json`
- 创建：`README.md`
- 创建：`LICENSE`

- [ ] **步骤 1：初始化 Tauri + React + TypeScript 项目**

使用官方 Tauri 初始化方式创建应用骨架。初始化后确认项目能启动开发窗口。

- [ ] **步骤 2：创建基础 AppShell**

`App.tsx` 显示固定侧边导航和空状态 Dashboard。

- [ ] **步骤 3：验证**

运行前端类型检查和 Rust 编译检查。

预期：应用可以启动，窗口显示 Dashboard。

- [ ] **步骤 4：提交**

```bash
git add .
git commit -m "chore: initialize tauri desktop app"
```

### 任务 2：添加 CI

**文件：**

- 创建：`.github/workflows/ci.yml`

- [ ] **步骤 1：添加前端检查**

CI 运行 TypeScript check、前端测试。

- [ ] **步骤 2：添加 Rust 检查**

CI 运行 Rust test、format check、clippy。

- [ ] **步骤 3：提交**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add desktop app checks"
```

## 阶段 2：UI 导航与类型契约

### 任务 3：实现页面骨架

**文件：**

- 创建：`src/routes/Onboarding.tsx`
- 创建：`src/routes/Dashboard.tsx`
- 创建：`src/routes/Skills.tsx`
- 创建：`src/routes/SyncPreview.tsx`
- 创建：`src/routes/Conflicts.tsx`
- 创建：`src/routes/Backups.tsx`
- 创建：`src/routes/Settings.tsx`
- 创建：`src/components/AppShell.tsx`
- 创建：`src/styles/globals.css`

- [ ] **步骤 1：写组件测试或 smoke test**

确认每个页面能渲染标题和主要区域。

- [ ] **步骤 2：实现导航**

侧边栏包含 Dashboard、Skills、Sync、Conflicts、Backups、Settings。

- [ ] **步骤 3：验证**

应用窗口中可以切换页面，文本不溢出，布局在窄窗口下可用。

- [ ] **步骤 4：提交**

```bash
git add src
git commit -m "feat: add desktop navigation shell"
```

### 任务 4：定义前后端类型

**文件：**

- 创建：`src/lib/types.ts`
- 创建：`src/lib/api.ts`
- 创建：`src-tauri/src/commands.rs`
- 创建：`src-tauri/src/errors.rs`

- [ ] **步骤 1：定义 TypeScript 类型**

包括 `Skill`、`SyncPlan`、`Conflict`、`BackupEntry`、`AppConfig`、`AppError`。

- [ ] **步骤 2：定义 Rust DTO**

Rust DTO 字段与 TypeScript 类型一致，通过 serde 序列化。

- [ ] **步骤 3：实现 stub commands**

先返回假数据，保证 UI 可以并行开发。

- [ ] **步骤 4：提交**

```bash
git add src/lib src-tauri/src
git commit -m "feat: define app api contracts"
```

## 阶段 3：后端核心能力

### 任务 5：解析 `SKILL.md`

**文件：**

- 创建：`src-tauri/src/skill.rs`
- 创建：`tests/fixtures/skills/codex-sample/SKILL.md`

- [ ] **步骤 1：先写 Rust 失败测试**

覆盖合法 front matter、缺少 `name`、缺少 `description`、没有 front matter。

- [ ] **步骤 2：实现最小 parser**

优先简单解析。除非必须支持复杂 YAML，否则不要引入重依赖。

- [ ] **步骤 3：运行 Rust 测试**

预期：通过。

- [ ] **步骤 4：提交**

```bash
git add src-tauri/src/skill.rs tests/fixtures
git commit -m "feat: parse skill metadata"
```

### 任务 6：发现本地 skills

**文件：**

- 创建：`src-tauri/src/detect.rs`
- 修改：`src-tauri/src/commands.rs`

- [ ] **步骤 1：先写失败测试**

使用临时目录测试存在路径、缺失路径、重复 skill ID、无效 `SKILL.md`。

- [ ] **步骤 2：实现扫描**

返回 skills 和 warnings，不存在路径不是 fatal error。

- [ ] **步骤 3：接入 `scan_skills()` command**

前端可从假数据切换为真实扫描结果。

- [ ] **步骤 4：提交**

```bash
git add src-tauri/src/detect.rs src-tauri/src/commands.rs
git commit -m "feat: discover local skills"
```

### 任务 7：配置持久化

**文件：**

- 创建：`src-tauri/src/config.rs`
- 修改：`src-tauri/src/commands.rs`
- 修改：`src/routes/Settings.tsx`

- [ ] **步骤 1：测试默认配置**

覆盖首次启动、保存配置、读取配置、路径展开。

- [ ] **步骤 2：实现配置读写**

配置保存在系统 app config 目录下。

- [ ] **步骤 3：Settings 页面接入真实配置**

用户可以保存 Git remote、agent 路径、备份目录。

- [ ] **步骤 4：提交**

```bash
git add src-tauri/src/config.rs src-tauri/src/commands.rs src/routes/Settings.tsx
git commit -m "feat: persist app configuration"
```

### 任务 8：Manifest 与 hash

**文件：**

- 创建：`src-tauri/src/manifest.rs`

- [ ] **步骤 1：先写失败测试**

覆盖稳定排序、hash 稳定性、忽略文件、JSON round trip。

- [ ] **步骤 2：实现 manifest**

按排序后的路径和文件内容计算 hash，manifest 内路径分隔符统一为 `/`。

- [ ] **步骤 3：提交**

```bash
git add src-tauri/src/manifest.rs
git commit -m "feat: generate sync manifest"
```

## 阶段 4：Git 与同步引擎

### 任务 9：Git Store

**文件：**

- 创建：`src-tauri/src/git_store.rs`
- 修改：`src-tauri/src/commands.rs`

- [ ] **步骤 1：测试 Git 检测**

覆盖 Git 不存在、Git 存在、dirty repo、remote 不可访问。

- [ ] **步骤 2：封装 Git CLI**

使用显式参数调用系统 Git，不拼接 shell 字符串。

- [ ] **步骤 3：接入 `check_git()` 和 `check_remote()`**

Onboarding 可以展示检查结果。

- [ ] **步骤 4：提交**

```bash
git add src-tauri/src/git_store.rs src-tauri/src/commands.rs
git commit -m "feat: add git-backed store"
```

### 任务 10：同步计划生成

**文件：**

- 创建：`src-tauri/src/sync_engine.rs`
- 修改：`src-tauri/src/commands.rs`
- 修改：`src/routes/SyncPreview.tsx`

- [ ] **步骤 1：先写失败测试**

覆盖新增、远程新增、本地修改、远程修改、双方修改、删除。

- [ ] **步骤 2：实现 `get_sync_plan()`**

只生成计划，不写文件。

- [ ] **步骤 3：Sync Preview 页面展示真实计划**

按 uploads、downloads、updates、deletes、conflicts 分组。

- [ ] **步骤 4：提交**

```bash
git add src-tauri/src/sync_engine.rs src-tauri/src/commands.rs src/routes/SyncPreview.tsx
git commit -m "feat: preview skill sync plan"
```

### 任务 11：应用同步计划与备份

**文件：**

- 创建：`src-tauri/src/backup.rs`
- 修改：`src-tauri/src/sync_engine.rs`
- 修改：`src-tauri/src/commands.rs`
- 修改：`src/routes/Backups.tsx`

- [ ] **步骤 1：先写失败测试**

覆盖备份创建、无冲突写入、冲突阻止写入、恢复备份。

- [ ] **步骤 2：实现备份**

备份保存到 app data 或用户配置的备份目录。

- [ ] **步骤 3：实现 `apply_sync_plan(decisions)`**

只执行用户已确认的无冲突动作。

- [ ] **步骤 4：Backups 页面接入真实数据**

用户可以查看和恢复备份。

- [ ] **步骤 5：提交**

```bash
git add src-tauri/src/backup.rs src-tauri/src/sync_engine.rs src-tauri/src/commands.rs src/routes/Backups.tsx
git commit -m "feat: apply sync with backups"
```

## 阶段 5：前端完整体验

### 任务 12：Onboarding

**文件：**

- 修改：`src/routes/Onboarding.tsx`
- 修改：`src/components/PathPicker.tsx`

- [ ] **步骤 1：实现路径选择**

用户选择本地同步仓库目录和备份目录。

- [ ] **步骤 2：实现 remote 检查**

展示 Git 是否可用、remote 是否可访问。

- [ ] **步骤 3：保存配置后进入 Dashboard**

首次配置完成后不再重复显示引导页。

- [ ] **步骤 4：提交**

```bash
git add src/routes/Onboarding.tsx src/components/PathPicker.tsx
git commit -m "feat: add first-run onboarding"
```

### 任务 13：Skills 管理页面

**文件：**

- 修改：`src/routes/Skills.tsx`
- 创建：`src/components/SkillTable.tsx`
- 创建：`src/components/StatusBadge.tsx`

- [ ] **步骤 1：展示真实扫描结果**

表格显示名称、host、路径、状态、启用开关。

- [ ] **步骤 2：实现重新扫描**

按钮触发 `scan_skills()`。

- [ ] **步骤 3：实现打开文件夹**

通过 Tauri command 打开路径。

- [ ] **步骤 4：提交**

```bash
git add src/routes/Skills.tsx src/components
git commit -m "feat: manage discovered skills"
```

### 任务 14：冲突处理页面

**文件：**

- 修改：`src/routes/Conflicts.tsx`
- 创建：`src/components/ConfirmDialog.tsx`

- [ ] **步骤 1：展示冲突列表**

每个冲突显示原因、本地路径、远程路径、可选动作。

- [ ] **步骤 2：实现决策**

用户可选择保留本地、使用远程、跳过。

- [ ] **步骤 3：同步前二次确认**

执行会写文件的动作前显示确认对话框。

- [ ] **步骤 4：提交**

```bash
git add src/routes/Conflicts.tsx src/components/ConfirmDialog.tsx
git commit -m "feat: resolve sync conflicts in ui"
```

## 阶段 6：打磨、测试与发布

### 任务 15：端到端 smoke test

**文件：**

- 创建：`tests/e2e/smoke.spec.ts`
- 修改：`package.json`

- [ ] **步骤 1：添加基础 e2e**

覆盖启动、导航、打开 Settings、触发 scan 的 smoke flow。

- [ ] **步骤 2：添加临时 fixture repo**

测试同步预览不写真实用户目录。

- [ ] **步骤 3：提交**

```bash
git add tests package.json
git commit -m "test: add desktop smoke tests"
```

### 任务 16：文档与发布

**文件：**

- 修改：`README.md`
- 创建：`docs/getting-started.md`
- 创建：`docs/configuration.md`
- 创建：`docs/safety.md`
- 创建：`.github/workflows/release.yml`

- [ ] **步骤 1：写用户上手文档**

说明安装应用、首次配置、选择 Git remote、扫描和同步。

- [ ] **步骤 2：写安全文档**

说明备份、冲突、secret warning、不会执行 skill 脚本。

- [ ] **步骤 3：添加跨平台构建**

发布 Windows、macOS、Linux 安装包。

- [ ] **步骤 4：提交**

```bash
git add README.md docs .github/workflows/release.yml
git commit -m "docs: prepare desktop app release"
```

## 验收标准

- 应用可以在 Windows 启动并展示 Dashboard。
- 首次引导可以保存本地仓库路径和 Git remote。
- Skills 页面可以发现 fixture skills。
- Sync Preview 可以展示新增、修改、删除、冲突。
- 冲突不会自动覆盖。
- 写入前创建备份。
- Backups 页面可以恢复备份。
- Settings 页面可以修改路径和 remote。
- 前端类型检查通过。
- Rust tests 通过。
- 桌面 smoke test 通过。
- README 可以让新用户完成第一次图形化同步。

## 后续路线图

MVP 之后再考虑：

- 系统托盘和后台同步。
- 更细的 visual diff。
- Symlink 安装模式。
- 加密字段。
- 团队共享仓库。
- 公共 skill 发现索引。
- Codex plugin 打包桥接。
- 从 `gh skill` 和 `npx skills` 工作流导入。
