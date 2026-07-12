<p align="center">
  <img src="src-tauri/icons/icon.png" width="140" alt="Skill Sync 图标" />
</p>

<h1 align="center">Skill Sync</h1>

<p align="center">
  <strong>用轻量桌面工作区，在多台机器之间同步 AI agent skills。</strong>
</p>

<p align="center">
  <a href="README.md">English</a> · <a href="README.zh-CN.md">简体中文</a>
</p>

Skill Sync 是一个本地优先的 Tauri 桌面应用，通过一个私有 GitHub Vault 仓库同步技能。V1 唯一支持的远端方式是 GitHub App Device Flow，不提供 SaaS 账号、自建同步服务、OAuth App、PAT、Git CLI 配置、SSH 配置或遥测。

## 工作方式

Vault 绑定完成前，应用只显示受门禁保护的 Onboarding。用户依次完成 GitHub 授权、GitHub App installation 检查、唯一仓库选择、分支选择，并明确确认空仓库或缺少 manifest 时创建初始化 commit。只有 Ready Vault 绑定成功后，才会进入工作区；工作区导航严格只有 Sync 和 Settings。

远端仓库只包含：

```text
manifest.json
blobs/sha256/<sha256>.skill.zip
```

manifest 会整体校验。Skill hash 来源于 canonical ZIP 字节。同步状态保存在本机，授权凭证存入操作系统 keyring，并在可用时自动刷新。

V1 扫描三个固定的用户级目录：

| Namespace     | 目录               |
| ------------- | ------------------ |
| `agents`      | `~/.agents/skills` |
| `codex`       | `~/.codex/skills`  |
| `claude-code` | `~/.claude/skills` |

项目级目录和用户自定义 root 不属于 V1。删除是 advisory 行为：会先在预览中显示，必须显式选择；删除本地目录时会移入本机 trash。删除云端只移除 manifest 条目，不回收 blob。

## GitHub App

release 构建时必须注入公开的 GitHub App client id 和 slug。App 只需要 Contents read/write 与 Metadata read-only 权限。installation 必须使用 selected repositories，并且只包含一个 Vault 仓库。用户可以在 GitHub 中调整或撤销 installation；应用不会保存 client secret 或 private key。

维护者请阅读 [GitHub App 配置清单](docs/github-app-setup.md)，普通用户安装 App 后直接按 Onboarding 操作即可。

## 从源码构建

前置条件：Node.js 20+、Rust stable 和 Tauri v2 系统依赖。应用不依赖本机 Git。

```bash
git clone https://github.com/TreatTrick/skill-sync.git
cd skill-sync
npm install

npm run tauri dev
npm run tauri build
```

release workflow 只通过 repository variables 注入公开 GitHub App client id 和 slug。生成安装包前请完成维护者清单中的核对。

## 开发检查

```bash
npm run typecheck
npm run format:check
npm run lint
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

后端负责本地扫描、canonical ZIP、资源限制、manifest 校验、GitHub API、三方比较、删除护栏和可恢复本地 apply。前端负责 Onboarding、展示、筛选、冲突决策和 command 调用。所有前端 API 响应都会先经过 zod 校验。

## 项目结构

```text
src/
  routes/    # SvelteKit 路由薄包装
  app/       # 应用外壳、路由门禁、导航
  modules/   # onboarding、settings、skills、sync
  shared/    # UI、schemas、i18n、state、请求工具
src-tauri/
  src/       # Rust scanner、packer、vault、GitHub client、sync engine
docs/
  github-base-refactor-design.md
  github-app-setup.md
```

## 协议

[MIT](LICENSE) © 2026 TreatTrick
