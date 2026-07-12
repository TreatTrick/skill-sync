# Skill Sync 开发计划

## 当前产品边界

Skill Sync 是一个 Tauri 2 + Rust + Svelte 5 桌面 demo。V1 只有 GitHub App Device Flow 这一种远端 provider，不保留 Git CLI、SSH、OAuth App、PAT、备份页、restore flow 或可配置 root 的产品路径。

用户必须先完成 Ready Vault binding，应用才显示工作区。未绑定时只有单步骤 Onboarding；绑定后导航严格只有 Sync / Settings。

## 远端模型

Vault 是一个私有 GitHub 仓库，内容固定为：

```text
manifest.json
blobs/sha256/<sha256>.skill.zip
```

manifest schema 为 1。Skill hash 取 canonical ZIP bytes 的 SHA-256，blob 路径必须由 hash 推导。初始化空仓库或缺 manifest 时，只有用户显式确认才创建一次初始化 commit；invalid manifest、分支缺失、权限错误和限流状态都不能覆盖远端。

GitHub App installation 必须使用 selected repositories 且只授权一个 Vault 仓库。client id / app slug 是构建时公开配置，access/refresh token 只作为 versioned credential 保存到 keyring。应用不接收或保存 client secret/private key。

## 本地模型

只扫描固定用户级 namespace：

| Namespace     | Root                              |
| ------------- | --------------------------------- |
| `agents`      | `~/.agents/skills`                |
| `codex`       | `~/.codex/skills`，排除 `.system` |
| `claude-code` | `~/.claude/skills`                |

不扫描项目目录，不允许从 Settings 增加、禁用或编辑 root。缺失、不可读、扫描失败或空扫描不得推断删除。同 namespace 的 normalized id 或 folded folder collision 必须 blocked。

本机 config 只包含当前格式的 `remote`、`limits`、`ignore` 和 `device_id`。demo 不提供旧配置迁移或 legacy 兼容；未知字段和旧版本会被拒绝。sync_state 绑定 installation id、repository id 和 branch，identity 变化必须显式 rebind。

## 同步行为

- Preview 只生成计划，不写 sync_state。
- Apply 必须携带 expected remote commit、plan fingerprint、selected action ids 和 delete guard ack。
- 下载-only、删本地-only 和双方删除不创建 GitHub commit。
- 上传、删云端和保留本地重传最多合并为一个 commit。
- 删除项默认不选中；本地删除进入 trash，云端删除只移除 manifest entry，blob 不 GC。
- 远端提交成功后，如果本地替换、trash 或 state 保存失败，保留 apply journal 并进入 recovery；recovery 解决前禁止新的同步。
- canonical ZIP 在临时目录中创建和解包，严格执行压缩大小、文件数、单文件解压大小和总解压大小限制。

## 前端工作流

1. `/`、`/app`、`/app/sync` 和 `/app/settings` 先等待 AppState；未 Ready 时不渲染工作区导航。
2. Onboarding 依次执行 Device Flow、installation/repository discovery、branch 选择、Vault check/init 和 bind。
3. Device Flow authorized、App installed 或 initialize ready 都不能提前解锁工作区。
4. Settings 只读显示稳定 identity、credential status、limits、ignore 和固定 root 状态；重新配置在新绑定成功前保留旧绑定。
5. Sync 页面负责搜索、筛选、删除确认、冲突决策和 recovery resume。

## 分阶段验收

- Rust：`cargo test --manifest-path src-tauri/Cargo.toml`、`cargo fmt --all -- --check`、必要时 `cargo clippy --all-targets --all-features --locked -- -D warnings`。
- Frontend：`npm run typecheck`、`npm run format:check`、`npm run lint`、`npm run build`。
- 禁止项搜索：确认产品代码无 Git CLI fallback、旧 backup/git store、token 泄漏、blocking reqwest、嵌套 runtime 或旧 root 配置。
- 手动烟测需要 Tauri dev 环境、GitHub App credential 和一次性私有仓库，详见 [GitHub App setup](github-app-setup.md)。

## 非目标

当前 demo 不实现 provider 抽象扩展、OAuth/PAT、Git URL、SSH 教程、Backups 页面、恢复流程、文件级 diff、tombstone、项目级 skill 扫描或 per-skill ignore 文件。
