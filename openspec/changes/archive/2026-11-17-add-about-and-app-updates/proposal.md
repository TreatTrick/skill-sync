## Why

当前软件版本和项目入口隐藏在设置页末尾，用户无法快速查看软件信息或确认是否有新版本。将“关于”提升为与“同步”“设置”并列的导航入口，并提供基于 GitHub 正式版的检查、下载和安装流程。

## What Changes

- 新增独立的关于页面，集中展示软件名称、当前版本、运行平台、发布渠道、许可证及项目链接。
- 从 GitHub Releases 检查正式版更新；发现较新版本时，在左侧“关于”导航项右侧显示绿色圆形更新箭头。
- 在关于页面显示可用版本和发布说明，允许用户下载并安装更新，完成后重新启动应用。
- 为更新检查、下载、安装和失败提供明确状态，并在启动工作区时检查一次、允许用户手动重试。
- 更新 GitHub 发布流程以产出可验证的 Tauri 更新包和签名元数据；签名私钥通过 CI Secret 提供。
- 将“设置”与“关于”作为独立同级页面，移除设置页内嵌的关于分区。

## Capabilities

### New Capabilities

- `about-and-app-updates`: 关于页面的软件信息、GitHub 正式版检查、更新提示及下载安装行为。

### Modified Capabilities

- `workspace-navigation`: 将“关于”加入“同步”“设置”同级主导航，并在有更新时显示状态提示。
- `settings-ui`: 移除设置页内嵌的关于分区，关于内容由独立页面承载。

## Impact

- 影响 `src/app` 导航、`src/modules/settings` 与新增的关于/更新模块、路由和中英文 locale。
- 影响 `src-tauri/tauri.conf.json`、Rust/JavaScript Tauri 插件依赖及 `.github/workflows/release.yml`。
- 需要在 GitHub Actions 中配置 Tauri 更新签名私钥 Secret；公钥随应用配置发布。
