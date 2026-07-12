# GitHub App 配置清单

本文档面向 Skill Sync 维护者，也包含普通用户调整 installation 范围时需要的要点。

## 创建 App

1. 在 GitHub 注册一个可安装的 GitHub App。
2. 开启 Device Flow 和 expiring user tokens。
3. Repository permissions 只开启：Contents read/write、Metadata read-only。
4. 不开启 account/org permissions、events 或 webhook。
5. 记录公开的 client id 和 app slug。client secret、private key 不进入桌面包、源码或 release artifact。
6. 安装页应为 `https://github.com/apps/<slug>/installations/new`。

App installation 必须使用 `Only select repositories`，并且只选择一个私有 Vault 仓库。多个仓库或 `All repositories` 会被应用阻止，不能通过手填 owner/repo 绕过。

## Release 配置

在 GitHub Actions repository variables 配置：

```text
SKILL_SYNC_GITHUB_APP_CLIENT_ID
SKILL_SYNC_GITHUB_APP_SLUG
```

打包前检查：

- 实际打开并核对 `https://github.com/apps/<slug>/installations/new`。
- 缺少 client id 或 slug 时 release 构建失败。
- release artifact 不含 private key、client secret、access token 或 refresh token。
- workflow 只注入公开 client id / slug，不从运行时环境变量读取秘密。

## 普通用户流程

1. 安装维护者提供的 GitHub App。
2. 在 Skill Sync Onboarding 中选择 GitHub Device Flow，打开 GitHub 验证地址并输入 device code。
3. 在安装页选择 `Only select repositories`，只授权一个 Vault 仓库。
4. 返回应用重新检查 installation，选择已有 branch。
5. 空仓库或缺少 `manifest.json` 时，确认创建初始化 commit。
6. Ready check 成功后确认 bind，应用才会显示 Sync / Settings 工作区。

用户可以在 GitHub 的 App installations 页面调整仓库范围或撤销授权。撤销后应用会要求重新授权；应用不会卸载 App、删除远端内容或把 token 写入配置文件和日志。

## 集成测试

默认测试不访问真实 GitHub。需要 gated 的手动测试时，使用 keyring 中的测试 App credential 和 disposable empty private repo：

```bash
SKILL_SYNC_GITHUB_INTEGRATION=1 \
SKILL_SYNC_GITHUB_EMPTY_TEST_REPO=<owner/repo> \
cargo test --manifest-path src-tauri/Cargo.toml github_empty_repo_initialization -- --ignored --nocapture
```

测试仓库只用于演练初始化 commit，不要使用生产 Vault 或包含真实技能的仓库。
