# 新建私有 GitHub Vault 仓库设计

## 背景

当前初始化流程会在 GitHub Device Flow 授权后，引导用户安装 TT-Skills-Sync GitHub App。GitHub 托管的 App 安装页面只能选择已有仓库，不能在仓库选择器中直接新建仓库。因此，没有合适私有仓库的新用户必须先离开当前流程，在 GitHub 手动创建仓库，再重新回到 App 安装页面，路径不够连贯。

Skill Sync 当前采用最小权限模型：GitHub App 只有 `Contents: Read and write` 以及 GitHub 强制要求的 `Metadata: Read-only`，不申请 account 或 organization permission。现有架构也要求 installation 最终只能覆盖一个私有 Vault 仓库。

本设计在不扩大 GitHub 权限的前提下，为用户补充明确的新建仓库入口，并把“Vault 必须是私有仓库”落实为后端不变量。

## 目标

- 在初始化安装阶段提供“新建私有 Vault 仓库”操作，打开 GitHub 官方新建仓库页面。
- 用户返回应用后，可以继续安装或调整 GitHub App，并重新检查 installation。
- 已有私有仓库的用户可以跳过新建操作，原有路径不受影响。
- 唯一授权仓库为公开仓库时，在进入分支选择和任何远端写入前明确阻止。
- 保持 GitHub App 当前最小权限，不引入仓库创建 API。
- 中英文界面都清楚说明仓库由 GitHub 创建，用户需要把 App 限定到唯一私有仓库。

## 非目标

- 不在 Skill Sync 内填写仓库名称、选择 owner 或设置仓库属性。
- 不调用 `/user/repos`、`/orgs/{org}/repos` 或其他仓库创建 endpoint。
- 不增加 Repository Administration、account 或 organization permission。
- 不自动把新仓库加入 installation，也不静默安装 GitHub App。
- 不自动轮询 GitHub 页面是否完成，不监听浏览器关闭或跳转。
- 不引入模板仓库、预填参数、仓库删除或失败后的自动清理。
- 不修改“installation 必须只覆盖一个仓库”的现有约束。

## 方案比较与决策

### 方案 A：打开 GitHub 新建仓库页面

应用打开 `https://github.com/new`。用户在 GitHub 创建私有仓库后返回 Skill Sync，再安装或调整 GitHub App，只授权该仓库。

优点：

- 不增加权限，保持现有安全承诺。
- GitHub 负责名称冲突、owner 权限、组织策略、SSO 和可见性选择等复杂交互。
- 兼容个人账户和允许用户创建仓库的组织账户。
- 失败发生在 GitHub 页面时，用户能直接看到 GitHub 的准确提示。
- 前后端改动集中，现有用户流程基本不变。

缺点：

- 用户仍需在浏览器和桌面应用之间切换。
- Skill Sync 无法确认用户是否真正创建了仓库，只能在用户返回后重新发现。
- GitHub 页面允许用户创建公开仓库，因此应用必须在发现和远端副作用前再次校验。

### 方案 B：由 Skill Sync 调用 GitHub API 自动创建

应用内收集 owner 和仓库名称，使用 GitHub API 创建私有仓库，再把仓库加入 installation 并初始化 Vault。

优点是步骤更少，应用可以控制仓库属性并继续初始化。缺点是需要更高权限、组织仓库存在额外策略差异、已有 installation 可能需要重新批准权限，而且会产生“仓库已创建但加入 installation 或初始化失败”的部分成功状态。GitHub App 也不能静默安装自己，因此首次使用仍无法完全消除 GitHub 确认步骤。

### 决策

采用方案 A。当前产品更重视最小权限、用户可见的资源归属和可恢复的初始化流程。方案 B 的权限与失败恢复成本明显高于它减少的操作步骤，不进入本次范围。

## 用户体验设计

安装阶段继续使用现有 `InstallAppStage` 卡片，不新增页面、弹窗或本地表单。

### 没有可用仓库或尚未安装 App

卡片按顺序提供三个操作：

1. “新建私有 Vault 仓库”：在系统浏览器打开 `https://github.com/new`。
2. “安装 GitHub App”：打开现有 GitHub App installation URL。
3. “重新检查安装”：重新运行仓库发现流程。

说明文案同时照顾两类用户：已有私有仓库的用户可以直接安装 App；没有仓库的用户先去 GitHub 创建私有仓库。三个按钮并不表示应用知道上一步已经完成，用户可以根据当前情况直接选择需要的操作。

### 唯一授权仓库是公开仓库

进入新的 `public_repository_blocked` 阶段，显示专用说明：Vault 必须使用私有仓库。卡片提供“新建私有 Vault 仓库”“调整 GitHub App 安装”和“重新检查安装”。此状态不能进入分支选择、Vault 初始化或绑定。

### installation 覆盖多个仓库或全部仓库

继续使用 `repository_scope_blocked`。用户需要调整 GitHub App 仓库范围并重新检查。该状态不显示新建仓库操作，因为继续创建仓库不能解决授权范围过大的问题。

### 已有唯一私有仓库

保持原有流程：列出分支或检查空仓库，必要时初始化 `manifest.json`，然后绑定 Vault。新功能不增加额外确认步骤。

## 状态与数据流

1. GitHub Device Flow 授权完成后，前端调用 `discover_single_github_repository`。
2. Rust 服务穷尽分页列出当前用户可见的 installations 和 repositories。
3. 没有 installation 时返回 `app_not_installed`；没有可用仓库时保持现有 unavailable 安装引导。
4. 多仓库或全部仓库授权继续返回现有阻止结果。
5. 恰好一个仓库时，Rust 必须读取 GitHub 响应中的 `private` 字段：
   - `private == false`：返回新的 `public_repository` 结果。
   - `private == true`：返回现有 `single_repository` 结果。
   - 字段缺失或类型错误：返回协议/远端错误，不能猜测为私有仓库。
6. 前端 zod schema 校验新的 discriminated union 分支，并把 `public_repository` 映射到 `public_repository_blocked`。
7. 只有 `single_repository` 才能继续列举分支、检查 Vault、初始化和绑定。

打开 `https://github.com/new` 只是本地外链操作，不改变应用状态，也不会写入本地配置。用户返回后必须主动重新检查，最新 GitHub 结果仍是唯一事实来源。

## 组件与模块职责

### `src/modules/onboarding/components/InstallAppStage.svelte`

- 负责按安装阶段显示说明和操作按钮。
- 保存固定的 GitHub 新建仓库 URL。
- 复用父页面现有的 `onOpenExternal` 回调处理外链失败。
- 不保存仓库名称、owner 或“已创建”布尔状态。

### `src/modules/onboarding/pages/OnboardingPage.svelte`

- 扩展 `OnboardingStage`，识别公开仓库阻止状态。
- 维持阶段进度和页面装配职责。
- 继续复用现有 `discoverRepository`、错误规范化和重新检查流程。

### `src/shared/schemas/apiResponse.ts`

- 将 `public_repository` 加入 `GithubRepositoryDiscovery` 的 zod discriminated union。
- 不新增仓库创建请求或响应类型。

### `src-tauri/src/github/repository.rs`

- 将私有仓库要求作为远端仓库发现和副作用校验的不变量。
- 为唯一公开仓库返回类型化的 `PublicRepository` 结果。
- 在 `validate_for_side_effect` 中再次阻止公开仓库，防止绕过前端或仓库可见性在绑定后发生变化。

### `src/shared/i18n/locales`

- 提供新建私有仓库、调整安装和公开仓库阻止状态的中英文文案。
- 组件内不出现硬编码中英文用户文案。

## 权限与安全边界

- GitHub App 权限保持 `Contents: Read and write` 和 `Metadata: Read-only`。
- Skill Sync 不获得创建、删除或管理仓库的能力。
- 外链固定为 `https://github.com/new`，不接收用户输入或远端返回的任意 URL。
- GitHub App installation URL 继续来自已校验的应用公开配置或发现结果。
- 公开仓库阻止同时存在于前端流程和 Rust 副作用入口；前端负责体验，后端负责最终防线。
- 如果 GitHub 响应缺少 `private` 字段，采用故障关闭策略，不允许继续写入。

## 错误与恢复

- 打开外链失败：复用 `openExternal`、`openPath` 和现有错误展示，不改变当前阶段。
- 用户取消创建仓库：应用没有产生状态变化，用户仍可重新打开页面或使用已有仓库。
- 创建成公开仓库：重新检查后进入 `public_repository_blocked`，不产生远端 Vault 副作用。
- installation 范围不正确：保持现有 scope blocked 流程，用户调整后重新检查。
- 网络、授权失效或限流：继续使用现有 `SkillSyncError` 分类和 onboarding 错误路径。
- 仓库绑定后改为公开：下一次远端副作用前由 `validate_for_side_effect` 阻止，不依赖旧的本地配置判断。

## 测试与验证

### 自动化验证

- Rust 测试覆盖唯一私有仓库正常发现。
- Rust 测试覆盖唯一公开仓库返回 `PublicRepository`。
- 缺少 `private` 字段时必须报错，不能继续为 `SingleRepository`。
- 现有多仓库、无 installation、Vault 检查和副作用校验测试保持通过。
- TypeScript 类型检查验证新的 zod union 与 Svelte 阶段处理一致。
- 响应式、颜色 token 和 i18n lint 验证新增 UI 与文案符合仓库规则。
- 完成前运行 Rust 全量检查、`npm run typecheck`、`npm run format:check`、`npm run lint` 和 `npm run build`。

### 手动验证

- 没有 installation 时可以从应用打开 GitHub 新建仓库页。
- 创建私有仓库并只授权该仓库后，可以继续初始化和绑定。
- 唯一公开仓库会被阻止，不能进入分支、初始化或绑定步骤。
- 多仓库授权仍要求收窄范围，并且不显示无助于解决问题的新建按钮。
- 已有唯一私有仓库的现有流程无回归。
- 中英文按钮在窄屏和桌面宽度下均不重叠。

真实 GitHub 验收需要测试账号或一次性仓库，不在自动化测试中创建生产仓库。

## 风险与取舍

1. 用户仍需跨应用操作。这是保持最小权限的直接代价，通过同一卡片内连续、明确的三个操作降低迷失概率。
2. GitHub 新建页面不能由应用强制设为私有。设计通过文案提示、发现时校验和副作用前复核形成完整防线。
3. GitHub 外部页面是否完成不可观测。设计不引入脆弱的定时轮询，用户主动重新检查后再推进状态。
4. 对 `private` 字段采用严格解析可能暴露旧测试 fixture 或异常代理响应。所有单仓库 fixture 必须明确补齐字段，真实缺失响应按错误处理。

## 验收标准

- 用户无需自行寻找 GitHub 新建仓库入口。
- 新建入口始终打开 GitHub 官方页面，不调用仓库创建 API。
- GitHub App 权限没有扩大。
- 只有唯一私有仓库能推进到 Vault 初始化和绑定。
- 公开、多个或全部仓库授权都在任何远端副作用前被阻止，并提供准确下一步。
- 现有唯一私有仓库用户流程不增加额外步骤。
- 中英文文案、响应式布局、前后端类型契约和完整构建均通过验证。

详细任务拆分与执行命令见 [新建私有 GitHub Vault 仓库实施计划](./2026-07-15-create-private-github-vault.md)。
