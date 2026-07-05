# 发布 v0.0.1 到 GitHub(含 macOS Universal 安装包)

## 目标
将当前代码作为 `v0.0.1` 打 tag 发布到 `TreatTrick/skill-sync`,并通过 GitHub Actions 自动构建 macOS Universal Binary(`.dmg`)上传到 Release。

## 已确认的决策
- 发布方式:GitHub Actions(`tauri-apps/tauri-action`),推 tag 后自动构建+发布
- macOS 架构:Universal Binary(Intel + Apple Silicon)
- 版本号:`tauri.conf.json` 与 `package.json` 同步为 `0.0.1`
- 当前 5 个未提交的 UI 修改(卡片头部重构、按钮对齐)属于合理的 v0.0.1 内容,一并提交

## 改动清单

### 1. 版本号同步
- `src-tauri/tauri.conf.json`:`"version": "0.1.0"` → `"0.0.1"`
- `package.json`:`"version": "0.0.0"` → `"0.0.1"`

### 2. 新增发布工作流 `.github/workflows/release.yml`
```yaml
name: release
on:
  push:
    tags:
      - 'v*'
permissions:
  contents: write
jobs:
  release:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-apple-darwin,x86_64-apple-darwin
      - run: npm ci
      - uses: tauri-apps/tauri-action@v0
        with:
          tagName: ${{ github.ref_name }}
          releaseName: 'Skill Sync v__VERSION__'
          releaseDraft: false
          prerelease: false
          args: --target universal-apple-darwin
```
要点:
- `permissions: contents: write` — 允许 action 创建 Release、上传 assets
- `dtolnay/rust-toolchain` 一次性装好 arm64 + x64 两个 target,Universal 构建必需
- `args: --target universal-apple-darwin` — 产出单个同时支持两种架构的 `.dmg`
- `tauri-action` 会自动执行 `beforeBuildCommand`(即 `npm run build`)、构建 bundle、创建 Release 并上传 `.dmg`/`.app`/`latest-mac.yml`

## 执行步骤

1. **本地验证(提交前必做)**
   - `npm run lint` — 确认 UI 改动通过 responsive/colors/i18n 检查
   - `npm run build` — 确认 typecheck + 前端静态构建通过(CI 会跑同一条命令,先在本地拦住失败)

2. **改版本号**:编辑 `tauri.conf.json` 与 `package.json`

3. **分两个 commit 提交**(保持 history 清晰,commitlint 合规)
   - commit 1:`refactor: 统一卡片头部布局与按钮对齐` ← 5 个 UI 文件 + docs 空行
   - commit 2:`chore: 准备 v0.0.1 发布` ← 版本号 + release.yml

4. **打 annotated tag**
   - `git tag -a v0.0.1 -m "Release v0.0.1"`

5. **推送**
   - `git push origin main`
   - `git push origin v0.0.1` ← 推送 tag 触发 workflow

6. **观察 CI**
   - GitHub 仓库 → Actions 标签页查看 `release` 工作流
   - 成功后 Releases 页会出现 `Skill Sync v0.0.1`,含 `Skill Sync_0.0.1_universal.dmg`

## 验证方式
- `git tag -l` 显示 `v0.0.1`
- GitHub Actions 里 `release` run 显示绿色通过
- Releases 页面存在 `v0.0.1`,assets 包含 `.dmg`
- 下载 `.dmg` 能在 Intel + Apple Silicon Mac 上打开(未签名,首次需右键→打开)

## 风险与说明
- **未签名**:v0.0.1 不配置 Apple 开发者证书,Gatekeeper 会拦截。用户首次打开需「系统设置 → 隐私与安全性 → 仍要打开」或右键打开。Release notes 会注明。后续版本可加 `APPLE_CERTIFICATE` 等 secret 做签名公证。
- **CI 耗时**:macOS runner 构建 Universal 首次约 15–25 分钟(含 Rust 编译两个架构)。
- **CI 失败回滚**:若 workflow 失败,删除远程 tag `git push origin :refs/tags/v0.0.1`、修 bug 后重新打 tag。tag 未关联成功 Release 前,公开页面不会显示半成品。
- **npm ci 依赖 lockfile**:`package-lock.json` 已存在且与 `package.json` 一致,无需额外处理。
