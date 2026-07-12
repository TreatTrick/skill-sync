# Skill Sync README 重写设计

## 目标

将现有偏技术说明的中英文 README 重写为产品价值优先的项目首页，让首次访问者能快速理解 Skill Sync 解决的问题、主要优势、使用门槛和可信边界，并获得明确的下载或开发入口。

默认 `README.md` 使用英文，`README.zh-CN.md` 使用简体中文。两份文档保持结构、事实和链接一致，但文案按各自语言习惯编写，不做机械直译。

## 目标读者

- 在多台电脑上使用 Codex、Claude Code 或通用 Agent Skills 目录的个人开发者。
- 希望通过自己的私有 GitHub 仓库保存 Skills，但不想配置 Git、SSH 或 PAT 的用户。
- 关注本地优先、授权范围、冲突处理和数据可恢复性的开源项目使用者。
- 希望了解技术栈、构建方式和贡献入口的潜在贡献者。

## 内容策略

README 采用“产品价值优先，工程可信度支撑”的顺序：先解释用户收益和获取方式，再说明工作流程、安全边界和技术实现。同步设计中的完整状态矩阵、manifest 校验细节和恢复协议继续留在 `docs/github-base-refactor-design.md`，README 只保留足以建立信任的摘要和链接。产品现状声明必须由当前代码、测试或已发布产物验证；设计文档只用于解释设计动机和实现模型，不能单独证明能力已经交付。

不使用未经证实的宣传数字，不声称支持 Linux，不虚构产品截图，不把计划中的能力写成已完成能力。当前没有界面截图素材，因此首版只使用现有应用图标、徽章、短文案、列表、表格和代码块。

## 信息结构

1. 首屏：应用图标、项目名、一句话价值主张、语言切换、Release/License/Platform 徽章，以及最新版 Release 下载入口。徽章固定使用 shields.io：Release 徽章显示最新 GitHub Release 并链接到 Releases latest，License 徽章显示仓库协议并链接到 `LICENSE`，Platform 徽章显示当前确有预构建产物的 `Windows x64` 并链接到 Releases latest。
2. Why Skill Sync：说明多设备、多 Agent 工具环境下手工复制 Skills 的痛点，以及项目采用私有 GitHub Vault 的解决方式。
3. Highlights：突出本地优先、无 SaaS、无需 Git/PAT/SSH、同步预览、三方比较、显式冲突决策、安全删除和可恢复 apply。
4. How it works：用三个步骤介绍 GitHub Device Flow 授权、唯一私有 Vault 绑定、预览与应用同步计划。
5. Supported directories：准确列出 `agents`、`codex`、`claude-code` 三个固定用户级目录，并说明项目级目录和自定义 root 不属于 V1。
6. Vault and sync model：简要展示 `manifest.json` 与内容寻址 ZIP blob，并说明判断依据是 base/local/remote hash，而不是文件时间。
7. Privacy and safety：说明最小 GitHub App 权限、凭证进入系统 keyring、不含 client secret/private key、无遥测、删除需显式选择并进入本机 trash。
8. Getting started：优先引导用户从 GitHub Releases 下载 Windows x64 安装包，然后在应用内完成 Onboarding。latest Release 当前只发布 Windows x64 的 EXE 和 MSI；README 不硬编码版本号，不声明 macOS 有可下载产物，也不声称安装包已签名。必要时简短提醒操作系统可能对未签名安装包显示安全提示。
9. Development：列出 Node.js 20+、由 `rust-toolchain.toml` 管理的 Rust 1.88.0、Tauri v2 系统依赖、源码构建命令和完整质量检查命令。完整检查固定为 `npm run typecheck`、`npm run format:check`、`npm run lint`、`npm run build`、`npm run rust:fmt:check`、`npm run rust:clippy` 和 `npm run rust:test`。
10. Tech stack、project structure、contributing 与 license：为贡献者提供简洁入口。仓库没有 `CONTRIBUTING.md`，因此贡献说明直接写在 README：先通过 GitHub Issues 沟通较大改动，提交聚焦的 Pull Request，并在提交前运行完整质量检查；只链接仓库现有页面或标准 GitHub Issues/Pulls URL。

## 链接与事实边界

- Release 链接使用 `https://github.com/TreatTrick/skill-sync/releases/latest`。
- GitHub App 安装链接使用源码内置 slug 对应的 `https://github.com/apps/tt-skills-sync/installations/new`，但正常流程仍建议从应用 Onboarding 发起。
- 平台只列出最新 Release 实际提供预构建产物的 Windows x64。macOS universal 虽然存在 workflow 配置，但最新 Release 没有对应资产，因此不能宣传为当前可下载平台。
- 保留对 `docs/github-base-refactor-design.md` 的技术设计链接。
- 删除对不存在的 `docs/github-app-setup.md` 的引用。
- V1 唯一远端路径是 GitHub App Device Flow + 单个私有 GitHub Vault 仓库。
- 远端只保存 manifest 和 blob；本机同步状态、凭证、trash 与恢复信息不进入 Vault。

## 双语一致性

两份 README 必须具有相同的章节顺序、命令、表格数据、路径、外部链接和能力边界。语言切换链接互相指向对应文件。英文版面向默认 GitHub 访客，中文版使用自然、直接的中文表达。

## 验收标准

- 新访客能在首屏理解项目用途、支持平台并找到下载入口。
- README 清楚说明相对手工复制、Git CLI、PAT 和 SaaS 同步方案的优势。
- 所有已交付功能描述都能由当前代码、测试或发布产物验证；设计文档只支撑原理说明，不作为交付证据。
- 中英文 README 的章节顺序、代码块、表格、路径、命令和 URL 逐项一致。
- 仓库内相对链接指向现有文件，关键外部链接格式有效。
- README 实施完成后运行完整质量检查：`npm run typecheck`、`npm run format:check`、`npm run lint`、`npm run build`、`npm run rust:fmt:check`、`npm run rust:clippy` 和 `npm run rust:test`。
- 使用 Conventional Commit 提交 README 变更。推送前执行 `git fetch origin`，确认工作区没有非本任务改动、`origin/main` 仍是本任务开始时的基线或本地 `main` 的祖先，并确认待推送提交只包含本次设计与 README 工作；若远端前进、分叉或出现不相关的本地 ahead 提交，则停止并报告，不使用 force push。检查通过后执行普通的 `git push origin main`，再 fetch 并验证 `origin/main` 与本地 `HEAD` 一致。
