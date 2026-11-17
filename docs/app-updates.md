# 应用内更新发布

关于页面通过 GitHub Releases 的 `latest.json` 检查稳定版，使用 Tauri updater 下载并验证签名。Windows 使用 NSIS 更新安装器，macOS Apple Silicon 使用签名的应用更新归档；首次安装仍可使用 EXE、MSI 或 DMG。

## 签名密钥

`src-tauri/tauri.conf.json` 中的 `plugins.updater.pubkey` 是客户端验证更新的公开密钥。与之配对的私钥保存在本机用户目录 `%USERPROFILE%\.tauri\skill-sync-updater`，公钥副本为同目录的 `.pub` 文件。私钥没有提交到仓库，也不得粘贴到日志、issue 或发布说明。

首次发布前，将私钥文件的完整内容配置为 GitHub Actions repository secret `TAURI_SIGNING_PRIVATE_KEY`。当前生成的密钥未设置密码，`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 可以不配置；使用带密码的密钥时必须同时设置密码 Secret。请妥善离线备份私钥，后续所有更新必须使用同一密钥签名。

如自行维护 fork，在仓库外运行 `npx tauri signer generate --write-keys <用户目录中的密钥路径>` 生成自己的密钥对，将生成的 `.pub` 文件内容填入客户端配置，并在自己的 GitHub 仓库配置配对私钥 Secret，同时修改更新源地址。已发布客户端的信任公钥不能随意更换；它们无法验证其他密钥生成的更新。

## 发布流程

1. 保持 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 版本一致，更新 CHANGELOG。
2. 完成本地类型、lint、格式、前端构建和 Rust 检查。前端开发与 `npm run build` 不需要签名私钥；本地打包签名安装包需要设置签名环境变量。
3. 推送与版本一致的正式标签，例如 `v1.2.0`；预发布标签不会通过此稳定版发布流程。
4. CI 依次构建 macOS Apple Silicon 和 Windows x64，上传签名更新包和 `latest.json` 到草稿 Release。串行构建避免多个作业同时覆盖平台清单。
5. 两个平台全部成功后才公开 Release，并将其设为 latest；构建失败时草稿保持不可见，现有稳定版更新源不切换。
6. 在已有签名客户端上检查更新，核对版本、发布说明、平台、下载进度与安装。Windows 确认安装后应用会退出并启动安装器，安装器完成后重新打开应用；macOS 安装后由用户选择重新启动。

## 首次启用与恢复

当前历史版本没有 updater 元数据；直到首次签名正式版发布，检查更新会显示可重试的检查失败。首次具备更新器的版本需要通过现有安装包安装一次，之后才能应用内更新。不要把历史 Release 缺少 `latest.json` 解释为“已是最新版本”。

签名失败、下载失败或平台资产缺失时应保留当前安装版本；可从官方 Release 页面重新下载安装包恢复。验证失败不能通过关闭签名验证来解决。完整安装升级必须在测试设备上验证，不能在开发者正在使用的工作区自动执行。
