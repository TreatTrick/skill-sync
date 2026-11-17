## Context

当前主应用由 `AppLayout` 和 `routeConfig` 提供左侧导航；设置页末尾包含“关于”分区，只展示 package.json 中的应用版本和项目 GitHub 入口。项目使用 Tauri 2，正式版通过 `.github/workflows/release.yml` 发布到 GitHub Releases，已有 Windows 安装包和 macOS 安装包，但未启用 Tauri updater。

更新功能需要在应用内识别可用正式版、跨页面显示提示并完成受验证的安装。Tauri updater 需要应用内公钥、CI 私钥、更新元数据和平台更新包；CI 私钥只能作为 GitHub Actions Secret 提供。

## Goals / Non-Goals

**Goals:**

- 将“关于”作为“同步”和“设置”的同级路由，展示准确的软件与构建信息。
- 在进入工作区时检查正式版，并支持用户在关于页手动重新检查。
- 有更新时在“关于”导航入口右侧显示绿色圆形更新箭头；在关于页呈现版本、发布说明和安装进度。
- 使用 Tauri updater 校验签名、下载并安装更新；Windows 确认安装时退出并交由安装器重新打开应用，macOS 安装后由用户确认重启。
- 让 GitHub Actions 发布 Tauri updater 所需签名文件和平台更新包。

**Non-Goals:**

- 不自动下载或安装更新；检查到更新后由用户发起安装。
- 不检查预发布版本、开发构建或非 GitHub Releases 的更新源。
- 不更改现有 Vault、同步、授权和用户配置数据。
- 不在仓库中保存 updater 私钥，也不在此变更中创建 GitHub Secret。

## Decisions

### 使用 Tauri updater 处理版本检查与安装

在 Rust 与 JavaScript 侧启用 Tauri 2 updater 插件，配置固定的 Skill Sync GitHub Releases 稳定版更新源和应用公钥。updater 负责按当前平台和架构查找更新、校验签名并安装，UI 负责展示状态、进度和安装后的重启确认。

备选方案是直接调用 GitHub REST API，下载各平台安装程序后再启动安装器。该方式需要自行维护平台资产匹配和安装器生命周期，也无法用 Tauri updater 的签名验证；因此选择官方插件和签名更新包。

### 更新状态由关于模块持有并供导航读取

在 `src/modules/about` 实现更新检查、当前状态和关于页面。远端版本信息使用 TanStack Query 缓存，应用布局与关于页复用同一 query，整个会话不自动重复请求；关于页显式重新检查时释放旧的原生 Update 资源。安装进度和已下载资源由小型 runes 状态协调，下载后禁止重新检查以避免关闭仍在使用的资源。检查失败保留可重试错误，不显示更新可用标记。

该依赖沿用 `app -> modules -> shared` 方向，不把 updater 逻辑加入通用 UI 或设置配置状态。更新检查的结果在一次应用会话内复用，不因导航切换重复请求。

### 只呈现稳定正式版并由用户确认安装

GitHub Release updater 元数据只服务已发布稳定版，Rust comparator 使用语义版本比较并拒绝预发布。只有远端版本高于当前安装版本时才标记为可更新；正式版相同或更旧时展示“已是最新版本”。应用启动检查不会自动下载。用户先下载，通过签名验证后再确认安装；Windows updater 会退出进程并启动安装器，必须提前告知，macOS 安装后允许立即或稍后重启。安装失败保留已下载包以便重试，避免重复下载占用内存。

安装与重启前通过 QueryClient 检查同步/恢复 mutation，存在未完成操作时禁止继续；安装和重启期间布局阻止页面导航，避免启动新的写入操作。

预发布、草稿、签名无效、网络失败及不支持的平台均不会进入安装流程。手动检查保留重新检查入口。Release notes 从 updater 元数据展示，并提供打开对应 GitHub Release 的链接。

### Release CI 为更新产物签名

Tauri 配置公开 updater 公钥，并启用平台更新产物。Release workflow 从 `TAURI_SIGNING_PRIVATE_KEY` 与可选密码 Secret 读取私钥，为 Windows x64 和 macOS Apple Silicon 构建并发布签名元数据和更新包。两个平台串行构建到草稿 Release，避免平台清单覆盖；全部成功后才公开为 latest，避免客户端检查到不完整的版本。私钥不写入仓库、日志或生成的客户端文件。

现有 `.exe`、`.msi` 和 `.dmg` 安装包继续发布，供首次安装和手动恢复使用。启用自动更新的发布前必须配置对应 GitHub Actions Secret；开发和普通前端构建不需要签名私钥。

### 关于页只展示可核验的本地与发布信息

应用名称和当前版本读取 Tauri app 元数据；平台和架构从当前运行环境读取；发布渠道明确显示为稳定版。页面展示项目仓库、最新正式版/检查状态及许可证入口，不把 Vault 配置或凭证带入关于页。

## Risks / Trade-offs

- [GitHub Actions 未配置签名 Secret 时无法生成可用更新] → 发布流程和仓库文档明确要求配置 Secret；构建检查覆盖 updater 配置，发布说明列明密钥要求。
- [更新公钥与发布私钥不匹配会导致更新被拒绝] → 保持签名验证开启，增加签名失败的错误状态，不提供绕过验证的安装入口。
- [平台更新格式或目标架构配置错误会隐藏更新] → 更新源按 Tauri updater 平台目标生成元数据；覆盖 Windows x64 和 macOS Apple Silicon 发布目标。
- [网络错误不应误报“无更新”] → 分别表示检查失败与已是最新版本，并保留手动重试。

## Migration Plan

新增关于路由并更新主导航；设置页移除原“关于”分区，设置其余能力保持原样。加入 updater 插件、权限、签名公钥配置和 Release workflow 产物后，发布管理员需将私钥配置到 GitHub Actions Secret，下一次正式版构建起即可提供应用内更新。发生回归时可关闭 updater 配置并回退关于模块/路由；本地配置和 Vault 数据无需迁移。

## Open Questions

无待确认的产品决策。签名密钥生成与 GitHub Secret 配置是首次启用应用内更新的发布运维步骤。
