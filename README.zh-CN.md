<p align="center">
  <img src="src-tauri/icons/icon.png" width="140" alt="Skill Sync 图标" />
</p>

<h1 align="center">Skill Sync</h1>

<p align="center">
  <strong>把你的 AI agent 技能同步到每一台机器——像 dotfiles 一样，但用桌面 UI。</strong>
</p>

<p align="center">
  <a href="README.md">English</a> · <a href="README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT" /></a>
  <img src="https://img.shields.io/badge/Tauri-2-orange?logo=tauri&logoColor=white" alt="Tauri 2" />
  <img src="https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white" alt="Svelte 5" />
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey" alt="Platform" />
  <a href="https://github.com/TreatTrick/skill-sync/issues"><img src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg" alt="PRs welcome" /></a>
  <img src="https://img.shields.io/github/stars/TreatTrick/skill-sync?style=social" alt="GitHub Stars" />
</p>

---

## ✨ 为什么需要

你攒了一套很好用的 Claude Code / Codex / Cursor / Gemini 技能，但在笔记本、台式机、工作机之间搬来搬去，意味着记路径、拷文件夹、手动解决 Git 冲突。**Skill Sync** 把这些一次性变成一个按钮。

你的技能通过**你自己的 Git 仓库**跟着你走。无需账号、无需上云、没有 marketplace、没有遥测。

## 🚀 特性

- **🔍 自动发现**：从配置的 agent 路径扫描本机已有的技能。
- **🧩 多 agent 支持**：Claude Code、Codex、Cursor、Gemini CLI，以及自定义路径。
- **👀 同步预览**：在写入任何文件之前，看清每一次上传、下载、更新、删除与冲突。
- **⚡ 一键同步**：一个按钮执行整个计划。
- **⚔️ 冲突处理**：逐个技能选择保留本地、使用远程或跳过。
- **💾 自动备份**：本地文件被改动前自动备份，支持一键恢复。
- **⚙️ 完整设置**：管理 agent 路径、Git remote、备份目录与 ignore 规则。
- **🛡️ 默认安全**：所有破坏性写入都先预览。
- **🔒 本地优先 · 隐私至上**：同步只走你自己的 Git 仓库，数据始终在你的掌控中。

## 🤝 支持的 Agent

| Agent       | 状态 |
| ----------- | :--: |
| Claude Code |  ✅  |
| Codex       |  ✅  |
| Cursor      |  ✅  |
| Gemini CLI  |  ✅  |
| 自定义路径  |  ✅  |

## 🧭 工作原理

1. **扫描**：从你配置的 agent 目录发现技能（如 `SKILL.md`）。
2. **比对**：将本地技能与 Git 同步仓库比对，生成变更计划。
3. **预览**：查看计划中的上传、下载、更新、删除与冲突。
4. **同步**：一键应用变更，本地文件先备份再写入。
5. **处理冲突**：保留本地 / 使用远程 / 跳过。
6. **恢复**：随时从备份页回滚任意文件。

## 📦 安装

### 从源码构建

**前置条件：**[Node.js](https://nodejs.org/) 20+、[Rust](https://www.rust-lang.org/)（stable）、[Git](https://git-scm.com/)，以及 [Tauri v2 系统依赖](https://tauri.app/start/prerequisites/)。

```bash
git clone https://github.com/TreatTrick/skill-sync.git
cd skill-sync
npm install

npm run tauri dev     # 以开发模式运行
npm run tauri build   # 在 src-tauri/target/release/bundle 生成安装包
```

> 预编译二进制会随首个 release 发布——**watch** 本仓库以获取通知 ⭐

## 🛠 技术栈

- **后端：**Tauri 2 + Rust —— 技能发现、`SKILL.md` 解析、Git 同步（系统 `git` CLI）、备份、冲突检测。
- **前端：**Svelte 5 + SvelteKit（静态 SPA）、TypeScript（strict）、Vite。
- **样式：**Tailwind CSS v4 语义化色彩 token、shadcn-svelte（bits-ui）组件。
- **状态：**`@tanstack/svelte-query` 管理服务端状态，Svelte 5 runes 管理客户端状态。
- **其他：**`@lucide/svelte` 图标、i18next（EN / 简体中文）、zod 校验。

## 📁 项目结构

```
src/
  routes/    # SvelteKit 路由（仅作页面薄包装）
  app/       # 应用外壳、布局、导航
  modules/   # dashboard · skills · sync · conflicts · backups · settings · onboarding
  shared/    # shadcn-svelte UI、i18n、token、工具
src-tauri/
  src/       # Rust：detect · manifest · git_store · sync_engine · backup · config
```

## 🗺 路线图

- [ ] 预编译二进制 + Tauri 自动更新
- [ ] 跨平台 CI（Windows / macOS / Linux）
- [ ] 更多 agent 集成
- [ ] 定时 / 后台同步

## 🤝 参与贡献

欢迎提 Issue 和 PR！请先阅读 [AGENTS.md](AGENTS.md) 与 [CLAUDE.md](CLAUDE.md)，它们说明了本仓库的工程规范、提交约定与 AI 协作规则。

```bash
npm run lint     # 类型检查 + ESLint + responsive/colors/i18n 校验
npm run build    # 类型检查并构建
```

提交信息遵循 [Conventional Commits](https://www.conventionalcommits.org/)（`feat:`、`fix:`、`docs:`…）。

## 📄 协议

[MIT](LICENSE) © 2026 TreatTrick

---

<sub>使用 Tauri · Svelte 5 构建 · 献给所有在机器之间弄丢技能的你。</sub>
