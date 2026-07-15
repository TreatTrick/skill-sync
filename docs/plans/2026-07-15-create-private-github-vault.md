# 新建私有 GitHub Vault 仓库实施计划

> **供 Claude 使用：** REQUIRED SUB-SKILL：使用 `superpowers:executing-plans` 逐项执行本计划。

**目标：** 让用户能够在初始化向导中打开 GitHub 的新建仓库页面，然后返回 Skill Sync 安装或调整 GitHub App，并绑定唯一的私有 Vault 仓库。

**架构：** 仓库创建操作仍由 GitHub 完成，并复用现有的 `openPath` Tauri 桥接，因此桌面应用不需要申请仓库管理权限，也不调用仓库创建 API。扩展仓库发现流程，增加故障关闭的私有仓库不变量；然后在现有初始化安装阶段中提供明确的新建、安装/调整和重新检查操作。

**技术栈：** Svelte 5、SvelteKit、TypeScript、i18next、Tailwind CSS v4、Lucide Svelte、Tauri 2、Rust、reqwest、zod

**设计依据：** [新建私有 GitHub Vault 仓库设计](./2026-07-15-create-private-github-vault-design.md)

---

### 任务 1：在 Rust 中强制执行私有 Vault 不变量

本任务使用 `@superpowers:test-driven-development`。

**文件：**

- 修改：`src-tauri/src/github/repository.rs:80`
- 测试：`src-tauri/src/github/repository.rs:670`

**步骤 1：添加公开仓库发现的失败测试**

为所有只返回一个可发现仓库的 Rust mock fixture 添加 `"private": true`，因为 GitHub 响应现在必须明确提供仓库可见性信息。目前包括现有的 `discover_single_repository` fixture；实施前再次搜索，避免遗漏后续新增的 fixture：

```powershell
rg -n -C 3 -S 'user/installations/.*/repositories|"repositories": \[' src-tauri/src
```

在现有发现测试附近添加以下测试，复用同样的 `MockServer`、`/user/installations` 和 `/user/installations/100/repositories` 设置：

```rust
#[tokio::test]
async fn discover_public_repository_is_blocked() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user/installations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "installations": [{ "id": 100 }]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/installations/100/repositories"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "repositories": [{
                "id": 200,
                "name": "vault",
                "owner": { "login": "octocat" },
                "private": false
            }]
        })))
        .mount(&server)
        .await;

    let service = build_service(&server).await;
    let discovery = service.discover_single_repository().await.unwrap();

    assert!(matches!(
        discovery,
        GithubRepositoryDiscovery::PublicRepository
    ));
}
```

**步骤 2：运行聚焦测试并确认 RED**

运行：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --locked discover_public_repository_is_blocked
```

预期：编译失败，因为 `GithubRepositoryDiscovery::PublicRepository` 尚不存在。

**步骤 3：添加类型化发现结果和故障关闭的可见性检查**

在 `GithubRepositoryDiscovery` 中添加以下单元变体：

```rust
PublicRepository,
```

在 `discover_single_repository` 的 `total == 1` 分支中，生成 `SingleRepository` 前必须读取 GitHub 返回的 `private` 布尔值：

```rust
let is_private = repo
    .get("private")
    .and_then(serde_json::Value::as_bool)
    .ok_or_else(|| AppError::Vault("repository missing private visibility".into()))?;
if !is_private {
    return Ok(GithubRepositoryDiscovery::PublicRepository);
}
```

将检查放在构造 `GithubRepositorySelection` 之前。更新方法注释，说明唯一公开仓库映射为 `PublicRepository`，只有唯一私有仓库才映射为 `SingleRepository`。

在 `validate_for_side_effect` 中添加明确的 `PublicRepository` 匹配分支，并返回：

```rust
return Err(AppError::Blocked(
    "github vault repository must be private".into(),
));
```

这样即使调用方绕过初始化 UI，或者仓库可见性随后发生变化，initialize、bind、preview 和 apply 仍会采用故障关闭策略。

**步骤 4：运行聚焦 Rust 测试并确认 GREEN**

运行：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --locked github::repository::tests
```

预期：所有 GitHub 仓库服务测试通过，包括私有单仓库发现和公开仓库阻止测试。

**步骤 5：提交后端不变量**

```powershell
git add src-tauri/src/github/repository.rs
git commit -m "fix: require private github vault repository"
```

预期：提交只包含 Rust 枚举、发现校验、明确的副作用防护和测试。

### 任务 2：添加新建仓库的初始化流程

**文件：**

- 修改：`src/shared/schemas/apiResponse.ts:149`
- 修改：`src/modules/onboarding/pages/OnboardingPage.svelte:45`
- 修改：`src/modules/onboarding/components/InstallAppStage.svelte:1`
- 修改：`src/shared/i18n/locales/zh-CN.json:61`
- 修改：`src/shared/i18n/locales/en-US.json:61`

**步骤 1：扩展经过校验的前端发现契约**

在 `githubRepositoryDiscoverySchema` 中添加 Rust 序列化后的结果：

```ts
z.object({ status: z.literal('public_repository') }),
```

将其放在 `single_repository` 附近。不要添加仓库创建请求类型、API 函数或 Tauri command。

**步骤 2：添加公开仓库阻止阶段**

在 `OnboardingPage.svelte` 的 `OnboardingStage` 中添加 `'public_repository_blocked'`。

与 `install_app` 和 `repository_scope_blocked` 一样，将其归入初始化进度第 2 步，并使用 `onboarding.stage.installApp` 作为阶段标题。

在 `discoverRepository` 中，在处理 `selection_all` 和 `multiple_repositories` 之前添加：

```ts
if (discovery.status === 'public_repository') {
  stage = 'public_repository_blocked'
  return
}
```

三个安装阶段都渲染 `InstallAppStage`，并直接传递新阶段；不要添加页面级表单状态或只做转发的薄 helper。

**步骤 3：添加 GitHub 新建仓库操作**

在 `InstallAppStage.svelte` 中扩展 `stage` prop 联合类型：

```ts
stage: 'install_app' | 'repository_scope_blocked' | 'public_repository_blocked'
```

添加一个模块局部常量：

```ts
const CREATE_GITHUB_REPOSITORY_URL = 'https://github.com/new'
```

根据阶段渲染不同的说明文案：

```svelte
{#if stage === 'repository_scope_blocked'}
  {t('github.adjustScope')}
{:else if stage === 'public_repository_blocked'}
  {t('github.publicRepositoryBlocked')}
{:else}
  {t('github.installDescription')}
{/if}
```

在 `install_app` 和 `public_repository_blocked` 阶段，在 GitHub App 安装按钮前添加以下外链按钮：

```svelte
<Button
  class="w-fit"
  onclick={(event: MouseEvent) =>
    onOpenExternal(event, CREATE_GITHUB_REPOSITORY_URL)}
  variant="outline"
>
  {t('github.createPrivateRepository')} <ExternalLink class="size-4" />
</Button>
```

保留现有的安装 URL 按钮。两个 blocked 阶段使用 `github.adjustInstallation`，正常安装阶段使用 `github.installApp`。最后仍保留 `github.checkInstallation` 操作。

最终操作顺序为：

1. 用户需要 Vault 仓库时，打开 GitHub 新建仓库页面。
2. 用户返回后安装或调整 GitHub App，并且只授权该私有仓库。
3. 在 Skill Sync 中重新检查仓库发现结果。

不要添加本地仓库名称输入框、owner 选择器、access token、`/user/repos` 调用、自动轮询或新的 runes 状态。

**步骤 4：运行类型检查并确认缺少文案导致失败**

运行：

```powershell
npm run typecheck
```

预期：检查失败，因为类型化 locale 契约中尚不存在 `github.publicRepositoryBlocked`、`github.createPrivateRepository` 和 `github.adjustInstallation`。

**步骤 5：添加中英文文案**

在 `zh-CN.json` 的 `github` 下添加：

```json
"adjustInstallation": "调整 GitHub App 安装",
"createPrivateRepository": "新建私有 Vault 仓库",
"publicRepositoryBlocked": "当前唯一授权的仓库是公开仓库。Vault 必须使用私有仓库，请新建私有仓库并调整 GitHub App 的仓库范围。"
```

将 `github.installDescription` 修改为：

```json
"installDescription": "已有私有仓库可直接安装 GitHub App；没有仓库时先新建一个私有 Vault 仓库，再只授权该仓库。"
```

在 `en-US.json` 的 `github` 下添加对应键：

```json
"adjustInstallation": "Adjust GitHub App installation",
"createPrivateRepository": "Create private Vault repository",
"publicRepositoryBlocked": "The only authorized repository is public. A Vault must use a private repository; create one and adjust the GitHub App repository access."
```

将 `github.installDescription` 修改为：

```json
"installDescription": "Install the GitHub App directly if you already have a private repository. Otherwise, create a private Vault repository first and grant access only to it."
```

文案必须说明创建操作由 GitHub 完成；不能暗示用户返回前 Skill Sync 已经创建或验证了仓库。

**步骤 6：运行聚焦前端检查**

运行：

```powershell
npm run typecheck
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
```

预期：所有命令通过。组件使用语义化颜色类和稳定的 Tailwind 尺寸，所有面向用户的文案都来自 locale key。

**步骤 7：提交前端流程**

```powershell
git add src/shared/schemas/apiResponse.ts src/modules/onboarding/pages/OnboardingPage.svelte src/modules/onboarding/components/InstallAppStage.svelte src/shared/i18n/locales/zh-CN.json src/shared/i18n/locales/en-US.json
git commit -m "feat: add private vault repository setup link"
```

预期：提交只包含经过校验的发现状态、初始化阶段处理、外部新建链接和双语文案。

### 任务 3：更新用户文档和架构文档

**文件：**

- 修改：`README.md:95`
- 修改：`README.zh-CN.md:95`
- 修改：`docs/github-base-refactor-design.md:459`
- 修改：`docs/github-base-refactor-design.md:485`
- 修改：`docs/github-base-refactor-design.md:789`

**步骤 1：更新英文初始化说明**

在 `README.md` 中说明：没有 Vault 仓库的用户可以从初始化向导打开 GitHub 新建仓库页面，创建私有仓库，返回后安装或调整 GitHub App，只授权该仓库，然后重新检查安装。

保留 GitHub App 直接安装 URL。不要声称 Skill Sync 会创建仓库或拥有仓库管理权限。

**步骤 2：更新中文初始化说明**

在 `README.zh-CN.md` 中按相同事实和顺序更新：仓库由 GitHub 创建、必须是私有仓库、App 必须只授权该仓库，之后用户返回应用重新检查安装。

**步骤 3：在架构设计中记录不变量**

更新现有 GitHub App 和初始化章节，说明：

- Repository permissions 仍然只有 `Contents: Read and write`，以及 GitHub 强制要求的 `Metadata: Read-only`。
- Skill Sync 只打开 `https://github.com/new`，绝不调用仓库创建 endpoint。
- 唯一仓库为公开仓库时，返回类型化的 blocked 发现结果。
- 每条远端副作用路径都会重新验证唯一授权仓库是私有仓库。
- 初始化安装阶段提供新建、安装/调整和重新检查操作，但不暗示流程已经自动完成。

不要把本任务扩展到 OAuth scope、组织管理、仓库模板、自动删除仓库或 GitHub App 权限变更。

**步骤 4：检查文档一致性**

运行：

```powershell
rg -n -S "github.com/new|private repository|私有.*仓库|Contents: Read and write|Metadata: Read-only" README.md README.zh-CN.md docs/github-base-refactor-design.md
```

预期：两个 README 描述相同的用户流程，架构文档保留最小权限边界。

**步骤 5：提交文档**

```powershell
git add README.md README.zh-CN.md docs/github-base-refactor-design.md
git commit -m "docs: explain private vault repository setup"
```

预期：提交只包含与已实现流程一致的文档改动。

### 任务 4：完成自动化与手动验证

宣称完成前使用 `@superpowers:verification-before-completion`。

**文件：**

- 验证：`src-tauri/src/github/repository.rs`
- 验证：`src/shared/schemas/apiResponse.ts`
- 验证：`src/modules/onboarding/pages/OnboardingPage.svelte`
- 验证：`src/modules/onboarding/components/InstallAppStage.svelte`
- 验证：`src/shared/i18n/locales/zh-CN.json`
- 验证：`src/shared/i18n/locales/en-US.json`
- 验证：`README.md`
- 验证：`README.zh-CN.md`
- 验证：`docs/github-base-refactor-design.md`

**步骤 1：运行 Rust 格式、lint 和测试**

运行：

```powershell
npm run rust:fmt:check
npm run rust:clippy
npm run rust:test
```

预期：所有命令通过，没有格式变化、warning 或失败测试。

**步骤 2：运行仓库要求的全部前端检查**

运行：

```powershell
npm run typecheck
npm run format:check
npm run lint
npm run build
```

预期：所有命令通过。`lint` 包含初始化 UI 和文案改动所要求的响应式、颜色 token 和 i18n 检查。

**步骤 3：检查聚焦差异**

运行：

```powershell
git status --short
git diff --check
git log -4 --oneline --decorate
```

预期：没有空白错误；实现由计划中的三个 Conventional Commit 表示；原有未跟踪文件 `code-review.md` 保持不变且未提交。

**步骤 4：使用一次性 GitHub 环境手动验证初始化流程**

启动桌面应用：

```powershell
npm run tauri dev
```

使用非生产数据验证以下场景：

1. 没有 App installation 时，安装卡片显示“新建私有 Vault 仓库”“安装 GitHub App”和“重新检查安装”，在窄屏和桌面宽度下都没有重叠。
2. 新建操作在系统浏览器中打开 `https://github.com/new`，且不会发起仓库创建 API 请求。
3. 创建私有仓库后返回应用，只向该仓库安装 App；重新检查后可以正常初始化 Vault 并完成绑定。
4. 只授权一个公开仓库时，显示专用的私有仓库警告，且不会进入分支选择、Vault 初始化或绑定阶段。
5. installation 包含多个仓库或全部仓库时，仍然显示授权范围警告和调整操作，不会引导用户继续创建仓库。
6. 已经拥有唯一私有 Vault 仓库的现有用户，仍可按原流程进入分支选择和绑定。
7. 切换语言，验证中英文文案，并确认按钮文字换行后不会与相邻操作重叠。

预期：以上七种场景都符合描述。验证完成后停止开发进程。

**步骤 5：确认权限边界**

检查最终差异和 GitHub App 注册设置。

预期：Repository permissions 仍然是 `Contents: Read and write` 和 `Metadata: Read-only`；没有 account 或 organization permission、`/user/repos` 请求、仓库名称表单或自动仓库变更。
