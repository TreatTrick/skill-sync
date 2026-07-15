# 新建 GitHub Vault 仓库设计

## 背景

当前初始化流程会在 GitHub Device Flow 授权后，引导用户安装 TT-Skills-Sync GitHub App。GitHub 托管的 App 安装页面只能选择已有仓库，不能在仓库选择器中直接新建仓库。因此，没有合适仓库的新用户必须先离开当前流程，在 GitHub 手动创建仓库，再重新回到 App 安装页面，路径不够连贯。

Skill Sync 当前采用最小权限模型：GitHub App 只有 `Contents: Read and write` 以及 GitHub 强制要求的 `Metadata: Read-only`，不申请 account 或 organization permission。现有架构要求 installation 最终只能覆盖一个 Vault 仓库，以限制应用能够写入的远端范围。

仓库是否公开与这一权限边界是两个独立问题。Vault 协议、分支检查、Contents API 写入、manifest 和 blob 同步都不要求仓库必须私有。私有仓库能降低 Skill 内容和 Git 历史意外公开的风险，应作为推荐默认；公开仓库也有分享 Skills 等合理用途，应由用户自行选择。

本设计在不扩大 GitHub 权限的前提下，为用户补充明确的新建仓库入口，并在使用公开仓库前提供一次清晰的风险提示和继续操作。

## 已确认决策

- installation 必须只覆盖一个仓库，继续作为后端强制约束。
- Vault 仓库可以公开或私有，可见性由用户在 GitHub 选择。
- 私有仓库无需额外确认，按现有流程继续。
- 公开仓库在初始化向导中显示风险提示；用户明确点击“继续使用公开仓库”后继续。
- 公开仓库不是错误或 blocked 状态，后端不拒绝公开仓库。
- 不持久化“已确认公开仓库”标记；确认只推进当前初始化会话。
- 新建仓库仍由 GitHub 完成，Skill Sync 不调用仓库创建 API。

## 目标

- 在初始化安装阶段提供“新建 Vault 仓库”操作，打开 GitHub 官方新建仓库页面。
- 用户返回应用后，可以继续安装或调整 GitHub App，并重新检查 installation。
- 已有仓库的用户可以跳过新建操作，原有路径不受影响。
- 唯一授权仓库为公开仓库时，在初始化或绑定前明确说明内容与 Git 历史对所有人可见。
- 用户确认风险后，公开仓库能够正常进入分支选择、Vault 初始化和绑定流程。
- 保持 GitHub App 当前最小权限，不引入仓库创建 API。
- 中英文界面都清楚说明仓库由 GitHub 创建，并且 App 只能授权一个 Vault 仓库。

## 非目标

- 不在 Skill Sync 内填写仓库名称、选择 owner 或设置仓库属性。
- 不调用 `/user/repos`、`/orgs/{org}/repos` 或其他仓库创建 endpoint。
- 不增加 Repository Administration、account 或 organization permission。
- 不自动把新仓库加入 installation，也不静默安装 GitHub App。
- 不自动轮询 GitHub 页面是否完成，不监听浏览器关闭或跳转。
- 不引入模板仓库、预填参数、仓库删除或失败后的自动清理。
- 不持久化仓库可见性或公开风险确认状态。
- 不把公开仓库升级为后端安全错误，也不阻止远端副作用。
- 不修改“installation 必须只覆盖一个仓库”的现有约束。

## 方案比较与决策

### 方案 A：打开 GitHub 新建仓库页面

应用打开 `https://github.com/new`。用户在 GitHub 选择 owner、名称和可见性，创建仓库后返回 Skill Sync，再安装或调整 GitHub App，只授权该仓库。

优点：

- 不增加权限，保持现有安全承诺。
- GitHub 负责名称冲突、owner 权限、组织策略、SSO 和可见性选择等复杂交互。
- 兼容个人账户和允许用户创建仓库的组织账户。
- 失败发生在 GitHub 页面时，用户能直接看到 GitHub 的准确提示。
- 前后端改动集中，现有用户流程基本不变。

缺点：

- 用户仍需在浏览器和桌面应用之间切换。
- Skill Sync 无法确认用户是否真正创建了仓库，只能在用户返回后重新发现。
- 应用不能控制仓库可见性，因此需要在发现公开仓库时提示风险。

### 方案 B：由 Skill Sync 调用 GitHub API 自动创建

应用内收集 owner、仓库名称和可见性，使用 GitHub API 创建仓库，再把仓库加入 installation 并初始化 Vault。

优点是步骤更少，应用可以控制仓库属性并继续初始化。缺点是需要更高权限、组织仓库存在额外策略差异、已有 installation 可能需要重新批准权限，而且会产生“仓库已创建但加入 installation 或初始化失败”的部分成功状态。GitHub App 也不能静默安装自己，因此首次使用仍无法完全消除 GitHub 确认步骤。

### 决策

采用方案 A。当前产品更重视最小权限、用户可见的资源归属和可恢复的初始化流程。方案 B 的权限与失败恢复成本明显高于它减少的操作步骤，不进入本次范围。

## 用户体验设计

安装阶段继续使用现有 `InstallAppStage` 卡片，不新增本地仓库表单。

### 没有可用仓库或尚未安装 App

卡片按顺序提供三个操作：

1. “新建 Vault 仓库”：在系统浏览器打开 `https://github.com/new`。
2. “安装 GitHub App”：打开现有 GitHub App installation URL。
3. “重新检查安装”：重新运行仓库发现流程。

说明文案同时照顾两类用户：已有仓库的用户可以直接安装 App；没有仓库的用户先去 GitHub 创建。文案推荐私有仓库，但明确说明公开或私有由用户选择。三个按钮并不表示应用知道上一步已经完成，用户可以根据当前情况直接选择需要的操作。

### 唯一授权仓库是公开仓库

进入新的 `confirm_public_repository` 阶段。页面显示仓库 owner/name 和风险说明：Vault 中的 Skill 文件以及 Git 历史对所有人可见，公开后可能被复制或缓存，用户应确认内容不含敏感信息。

提供三个操作：

1. “继续使用公开仓库”：明确确认风险，并继续加载分支或检查 Vault。
2. “新建其他 Vault 仓库”：打开 `https://github.com/new`，让用户创建新的替代仓库。
3. “调整 GitHub App 安装”：打开 installation URL，改选另一个仓库。

继续按钮只推进当前初始化会话，不写入新的配置字段。如果用户重新开始配置，公开仓库会再次显示提示。

### installation 覆盖多个仓库或全部仓库

继续使用 `repository_scope_blocked`。用户需要调整 GitHub App 仓库范围并重新检查。该状态不显示新建仓库操作，因为继续创建仓库不能解决授权范围过大的问题。

### 已有唯一私有仓库

保持原有流程：列出分支或检查空仓库，必要时初始化 `manifest.json`，然后绑定 Vault。新功能不增加额外确认步骤。

## 状态与数据流

1. GitHub Device Flow 授权完成后，前端调用 `discover_single_github_repository`。
2. Rust 服务穷尽分页列出当前用户可见的 installations 和 repositories。
3. 没有 installation 时返回 `app_not_installed`；没有可用仓库时保持现有 unavailable 安装引导。
4. 多仓库或全部仓库授权继续返回现有阻止结果。
5. 恰好一个仓库时，Rust 继续返回现有 `single_repository`，不按可见性分叉。
6. 前端调用现有 `listInstallationRepositories`，从已通过 zod 校验的 `GithubRepository.private` 获取当前可见性。
7. 私有仓库直接加载分支；公开仓库先进入 `confirm_public_repository`。
8. 用户确认公开仓库后，复用与私有仓库相同的分支、Vault 检查、初始化和绑定流程。

打开 `https://github.com/new` 只是本地外链操作，不改变应用状态，也不会写入本地配置。用户返回后必须主动重新检查，最新 GitHub 结果仍是仓库发现的事实来源。

公开仓库确认是隐私提示，不是授权或一致性边界。repository visibility 可能在 GitHub 上变化，因此后端仍以 installation ID、repository ID、owner/repo 和 branch 校验远端身份，不缓存或强制可见性。

## 组件与模块职责

### `src/modules/onboarding/components/InstallAppStage.svelte`

- 负责安装阶段的说明和操作按钮。
- 保存固定的 GitHub 新建仓库 URL。
- 复用父页面现有的 `onOpenExternal` 回调处理外链失败。
- 不保存仓库名称、owner、可见性或“已创建”状态。

### `src/modules/onboarding/components/PublicRepositoryWarningStage.svelte`

- 显示公开仓库 identity 和隐私风险。
- 提供继续使用、新建其他仓库和调整 installation 三个明确操作。
- 不修改远端仓库，不持久化确认状态。

### `src/modules/onboarding/pages/OnboardingPage.svelte`

- 扩展 `OnboardingStage`，识别 `confirm_public_repository`。
- 在现有 repository list 响应中读取 `selectedRepository.private`。
- 私有仓库直接继续；公开仓库等待用户确认。
- 把加载分支和推进后续状态的现有逻辑提取为有实际语义的函数，供私有路径和公开确认路径复用。
- 继续负责阶段进度、错误规范化和页面装配。

### `src/modules/onboarding/api/onboardingApi.ts` 与 schema

- 继续使用现有 `GithubRepository.private: boolean` 解析结果。
- 不新增 `public_repository` discovery 状态。
- 不新增仓库创建、可见性确认或持久化 API。

### Rust GitHub repository service

- 不因可见性修改 `GithubRepositoryDiscovery`。
- 不添加 `PublicRepository` 枚举或公开仓库副作用阻止。
- 继续强制唯一 installation repository 和稳定 repository identity。

### `src/shared/i18n/locales`

- 提供新建仓库、公开仓库风险、继续使用和调整安装的中英文文案。
- 组件内不出现硬编码中英文用户文案。

## 权限与隐私边界

- GitHub App 权限保持 `Contents: Read and write` 和 `Metadata: Read-only`。
- Skill Sync 不获得创建、删除或管理仓库的能力。
- 外链固定为 `https://github.com/new`，不接收用户输入或远端返回的任意 URL。
- GitHub App installation URL 继续来自已校验的应用公开配置或发现结果。
- installation 只能覆盖一个仓库，仍由后端在所有远端副作用前校验。
- 私有仓库只是 GitHub 访问控制，不代表端到端加密；GitHub 和被授权协作者仍可访问内容。
- 公开仓库中的 Skill 文件、历史版本和删除前的 Git 提交可能被任何人读取、复制或缓存，因此必须在首次绑定前明确提示。
- 用户确认公开风险后，应用不再以可见性为由阻止同步。

## 错误与恢复

- 打开外链失败：复用 `openExternal`、`openPath` 和现有错误展示，不改变当前阶段。
- 用户取消创建仓库：应用没有产生状态变化，用户仍可重新打开页面或使用已有仓库。
- 用户不想继续使用公开仓库：可以新建其他仓库或调整 GitHub App installation。
- installation 范围不正确：保持现有 scope blocked 流程，用户调整后重新检查。
- 网络、授权失效或限流：继续使用现有 `SkillSyncError` 分类和 onboarding 错误路径。
- 仓库可见性在绑定后变化：同步继续遵循用户在 GitHub 上的设置；应用不把可见性变化当作 identity 或一致性错误。

## 测试与验证

### 自动化验证

- TypeScript 类型检查验证新阶段、组件 props 和现有 `GithubRepository.private` 使用一致。
- i18n 类型检查验证公开风险和按钮文案均有中英文翻译。
- 响应式、颜色 token 和 i18n lint 验证新增 UI 符合仓库规则。
- Rust 代码与 discovery schema 不应因本功能发生变化。
- 完成前运行 `npm run typecheck`、`npm run format:check`、`npm run lint` 和 `npm run build`。

### 手动验证

- 没有 installation 时可以从应用打开 GitHub 新建仓库页。
- 创建或选择私有仓库后，可以直接继续初始化和绑定，不显示公开风险提示。
- 创建或选择公开仓库后，显示仓库 identity 和风险提示。
- 点击“继续使用公开仓库”后，可以正常进入分支、初始化和绑定步骤。
- 在公开仓库提示页可以打开新建仓库页或调整 GitHub App installation。
- 多仓库授权仍要求收窄范围，并且不显示无助于解决问题的新建按钮。
- 已有唯一私有仓库的现有流程无回归。
- 中英文按钮在窄屏和桌面宽度下均不重叠。

真实 GitHub 验收需要测试账号或一次性仓库，不在自动化测试中创建生产仓库。

## 风险与取舍

1. 用户仍需跨应用操作。这是保持最小权限的直接代价，通过同一卡片内连续、明确的操作降低迷失概率。
2. 公开仓库可能暴露 Skill 内容和历史。设计通过明确说明和主动继续按钮告知风险，但最终尊重用户选择。
3. GitHub 外部页面是否完成不可观测。设计不引入脆弱的定时轮询，用户主动重新检查后再推进状态。
4. 公开风险确认不持久化，也不能防止仓库后续改变可见性。这符合“GitHub 可见性由用户管理”的边界，避免增加易过期的本地状态。
5. 当前前端已解析 `private` 字段，新增确认不需要扩大 Rust 或跨进程契约，减少改动面。

## 验收标准

- 用户无需自行寻找 GitHub 新建仓库入口。
- 新建入口始终打开 GitHub 官方页面，不调用仓库创建 API。
- GitHub App 权限没有扩大。
- 唯一公开或私有仓库都能作为 Vault；多仓库或全部仓库授权仍被阻止。
- 私有仓库直接继续，公开仓库必须经过明确风险提示和主动确认。
- 公开仓库确认后可以完成分支选择、初始化、绑定和后续同步。
- 现有唯一仓库用户流程除公开风险提示外不增加额外步骤。
- 中英文文案、响应式布局、类型契约和完整构建均通过验证。

详细任务拆分与执行命令见 [新建 GitHub Vault 仓库实施计划](./2026-07-15-create-github-vault-repository.md)。
