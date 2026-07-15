# 新建 GitHub Vault 仓库实施计划

> **供 Claude 使用：** REQUIRED SUB-SKILL：使用 `superpowers:executing-plans` 逐项执行本计划。

**目标：** 让用户能够从初始化向导打开 GitHub 新建仓库页面，并允许用户选择公开或私有 Vault；公开仓库在继续前显示明确的隐私风险确认。

**架构：** 仓库创建仍由 GitHub 完成，并复用现有 `openPath` Tauri 桥接，因此不增加 GitHub App 权限或仓库创建 API。前端复用已经解析的 `GithubRepository.private`，私有仓库直接继续，公开仓库进入一次性的确认阶段；后端继续只强制唯一 installation repository 和稳定 repository identity，不按可见性阻止同步。

**技术栈：** Svelte 5、SvelteKit、TypeScript、i18next、Tailwind CSS v4、Lucide Svelte、Tauri 2、zod

**设计依据：** [新建 GitHub Vault 仓库设计](./2026-07-15-create-github-vault-repository-design.md)

---

### 任务 1：添加 GitHub 新建 Vault 仓库入口

本任务使用 `@ts-development-standards`。

**文件：**

- 修改：`src/modules/onboarding/pages/OnboardingPage.svelte:45`
- 修改：`src/modules/onboarding/components/InstallAppStage.svelte:4`
- 修改：`src/shared/i18n/locales/zh-CN.json:61`
- 修改：`src/shared/i18n/locales/en-US.json:61`

**步骤 1：在页面定义固定的新建仓库 URL**

在 `OnboardingPage.svelte` 的类型声明之后添加：

```ts
const CREATE_GITHUB_REPOSITORY_URL = 'https://github.com/new'
```

这是固定产品 URL，不从用户输入、GitHub API 响应或本地配置读取。

在 `InstallAppStage` 调用处传入：

```svelte
createRepositoryUrl={CREATE_GITHUB_REPOSITORY_URL}
```

**步骤 2：使用尚不存在的 locale key 添加操作**

在 `InstallAppStage.svelte` 的 `Props` 中添加：

```ts
createRepositoryUrl: string
```

从 `$props()` 解构该 prop。

只在 `stage === 'install_app'` 时，在 GitHub App 安装按钮前添加新建操作：

```svelte
{#if stage === 'install_app'}
  <Button
    class="w-fit"
    onclick={(event: MouseEvent) =>
      onOpenExternal(event, createRepositoryUrl)}
    variant="outline"
  >
    {t('github.createRepository')} <ExternalLink class="size-4" />
  </Button>
{/if}
```

将现有安装按钮的文案改为按阶段选择：

```svelte
{stage === 'repository_scope_blocked'
  ? t('github.adjustInstallation')
  : t('github.installApp')}
<ExternalLink class="size-4" />
```

`repository_scope_blocked` 不显示新建按钮，因为新增仓库不能解决授权范围过大的问题。

**步骤 3：运行类型检查并确认 RED**

运行：

```powershell
npm run typecheck
```

预期：失败，因为类型化 locale 契约中尚不存在 `github.createRepository` 和 `github.adjustInstallation`。

**步骤 4：添加中英文安装阶段文案**

在 `zh-CN.json` 的 `github` 下添加：

```json
"adjustInstallation": "调整 GitHub App 安装",
"createRepository": "新建 Vault 仓库"
```

将 `github.installDescription` 修改为：

```json
"installDescription": "已有仓库可直接安装 GitHub App；没有仓库时先在 GitHub 新建一个。推荐使用私有仓库，公开或私有由你选择。"
```

在 `en-US.json` 的 `github` 下添加：

```json
"adjustInstallation": "Adjust GitHub App installation",
"createRepository": "Create Vault repository"
```

将 `github.installDescription` 修改为：

```json
"installDescription": "Install the GitHub App directly if you already have a repository. Otherwise, create one on GitHub first. A private repository is recommended, but the visibility is your choice."
```

文案必须说明创建操作由 GitHub 完成，不能暗示 Skill Sync 已创建或验证仓库。

**步骤 5：运行聚焦前端检查并确认 GREEN**

运行：

```powershell
npm run typecheck
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
```

预期：全部通过。新增按钮使用现有 `Button`、`ExternalLink`、语义颜色和稳定 Tailwind 尺寸。

**步骤 6：提交新建仓库入口**

```powershell
git add src/modules/onboarding/pages/OnboardingPage.svelte src/modules/onboarding/components/InstallAppStage.svelte src/shared/i18n/locales/zh-CN.json src/shared/i18n/locales/en-US.json
git commit -m "feat: add github vault repository link"
```

预期：提交只包含固定新建 URL、安装阶段按钮和对应双语文案。

### 任务 2：为公开仓库添加明确确认阶段

本任务使用 `@ts-development-standards`。

**文件：**

- 创建：`src/modules/onboarding/components/PublicRepositoryWarningStage.svelte`
- 修改：`src/modules/onboarding/pages/OnboardingPage.svelte:45`
- 修改：`src/shared/i18n/locales/zh-CN.json:61`
- 修改：`src/shared/i18n/locales/en-US.json:61`

**步骤 1：创建使用缺失 locale key 的公开仓库提示组件**

创建 `PublicRepositoryWarningStage.svelte`：

```svelte
<script lang="ts">
  import { ExternalLink, TriangleAlert } from '@lucide/svelte'

  import type { GithubRepository } from '../schemas/onboarding'
  import { t } from '@/shared/i18n'
  import { Button, Card, CardContent } from '@/shared/ui'

  interface Props {
    repository: GithubRepository
    installUrl: string | null
    createRepositoryUrl: string
    busy: boolean
    onContinue: () => void
    onOpenExternal: (event: MouseEvent, url: string) => void
  }

  let {
    repository,
    installUrl,
    createRepositoryUrl,
    busy,
    onContinue,
    onOpenExternal,
  }: Props = $props()
</script>

<Card>
  <CardContent class="grid gap-4 pt-6">
    <div class="flex items-start gap-3">
      <TriangleAlert class="mt-0.5 size-5 shrink-0 text-warning" />
      <div class="grid gap-1.5">
        <h2 class="font-semibold text-strong-foreground">
          {t('github.publicRepositoryTitle')}
        </h2>
        <p class="text-sm text-muted-foreground">
          {t('github.publicRepositoryDescription', {
            repository: `${repository.owner}/${repository.repo}`,
          })}
        </p>
      </div>
    </div>

    <div class="flex flex-col gap-2 sm:flex-row sm:flex-wrap">
      <Button disabled={busy} loading={busy} onclick={onContinue}>
        {t('github.continuePublicRepository')}
      </Button>
      <Button
        onclick={(event: MouseEvent) =>
          onOpenExternal(event, createRepositoryUrl)}
        variant="outline"
      >
        {t('github.createAnotherRepository')} <ExternalLink class="size-4" />
      </Button>
      {#if installUrl}
        <Button
          onclick={(event: MouseEvent) =>
            onOpenExternal(event, installUrl ?? '')}
          variant="outline"
        >
          {t('github.adjustInstallation')} <ExternalLink class="size-4" />
        </Button>
      {/if}
    </div>
  </CardContent>
</Card>
```

组件只展示风险和命令，不保存确认状态，也不修改仓库。

**步骤 2：添加公开仓库阶段和可复用的后续加载逻辑**

在 `OnboardingPage.svelte`：

1. 导入 `PublicRepositoryWarningStage`。
2. 在 `OnboardingStage` 中添加 `'confirm_public_repository'`。
3. 将该阶段归入进度第 3 步，并使用 `onboarding.stage.branch` 标题。

把 `discoverRepository` 中从 `remote` 已建立开始的分支加载逻辑提取为具有实际语义的函数：

```ts
const continueWithSelectedRepository = async (): Promise<void> => {
  const currentRemote = remote
  if (!currentRemote) return
  branchNames = await listGithubRepositoryBranches(currentRemote)
  if (branchNames.length > 0) {
    selectedBranch = branchNames.includes(currentRemote.branch)
      ? currentRemote.branch
      : branchNames[0]
    stage = 'select_branch'
  } else {
    stage = 'checking_vault'
    await checkVault()
  }
}
```

在 `discoverRepository` 找到仓库后，不再使用 nullable fallback 继续。明确检查：

```ts
const repository = repositories.find(
  (candidate) => candidate.repository_id === discovery.repository.repository_id,
)
if (!repository) {
  stage = 'install_app'
  message = t('github.repositoryUnavailable')
  return
}
selectedRepository = repository
const defaultBranch = repository.default_branch || 'main'
remote = { ...discovery.repository, branch: defaultBranch }
selectedBranch = defaultBranch
if (!repository.private) {
  stage = 'confirm_public_repository'
  return
}
await continueWithSelectedRepository()
```

不要添加新的 zod schema、Rust discovery 状态或持久化配置字段。

**步骤 3：添加公开仓库继续处理和阶段渲染**

在页面中添加处理函数，沿用现有 busy/error 规范：

```ts
const continueWithPublicRepository = async (): Promise<void> => {
  busy = true
  message = ''
  try {
    await continueWithSelectedRepository()
  } catch (error) {
    setError(error)
  } finally {
    busy = false
  }
}
```

在 `select_branch` 之前渲染：

```svelte
{:else if stage === 'confirm_public_repository' && selectedRepository}
  <PublicRepositoryWarningStage
    repository={selectedRepository}
    installUrl={appInfo?.install_url ?? null}
    createRepositoryUrl={CREATE_GITHUB_REPOSITORY_URL}
    busy={busy}
    onContinue={() => void continueWithPublicRepository()}
    onOpenExternal={(event, url) => void openExternal(event, url)}
  />
```

确认只存在于当前组件会话。用户重新进入初始化或重新配置时，如果仓库仍公开，应再次显示提示。

**步骤 4：运行类型检查并确认 RED**

运行：

```powershell
npm run typecheck
```

预期：失败，因为公开仓库提示使用的 locale key 尚不存在。

**步骤 5：添加公开仓库中英文风险文案**

在 `zh-CN.json` 的 `github` 下添加：

```json
"continuePublicRepository": "继续使用公开仓库",
"createAnotherRepository": "新建其他 Vault 仓库",
"publicRepositoryDescription": "{{repository}} 是公开仓库。Skill 文件和 Git 历史对所有人可见，并可能被复制或缓存。确认不含敏感信息后再继续。",
"publicRepositoryTitle": "使用公开 Vault 仓库"
```

在 `en-US.json` 的 `github` 下添加：

```json
"continuePublicRepository": "Continue with public repository",
"createAnotherRepository": "Create another Vault repository",
"publicRepositoryDescription": "{{repository}} is public. Its Skill files and Git history are visible to everyone and may be copied or cached. Continue only after confirming it contains no sensitive information.",
"publicRepositoryTitle": "Use a public Vault repository"
```

不要使用“公开仓库不安全”或“禁止公开仓库”等绝对文案；提示具体暴露面，由用户决定。

**步骤 6：运行聚焦前端检查并确认 GREEN**

运行：

```powershell
npm run typecheck
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
```

预期：全部通过。三个操作在小屏纵向排列，在 `sm` 及以上允许换行，不出现文字或图标重叠。

**步骤 7：提交公开仓库确认流程**

```powershell
git add src/modules/onboarding/components/PublicRepositoryWarningStage.svelte src/modules/onboarding/pages/OnboardingPage.svelte src/shared/i18n/locales/zh-CN.json src/shared/i18n/locales/en-US.json
git commit -m "feat: confirm public github vault"
```

预期：提交只包含公开仓库风险提示、用户继续操作、阶段路由和双语文案；没有 Rust 或跨进程 schema 改动。

### 任务 3：统一用户文档和架构说明

**文件：**

- 修改：`README.md`
- 修改：`README.zh-CN.md`
- 修改：`docs/github-base-refactor-design.md`

**步骤 1：找出所有强制私有仓库表述**

运行：

```powershell
rg -n -S "private|私有|public|公开" README.md README.zh-CN.md docs/github-base-refactor-design.md
```

逐处区分：

- 产品说明中的强制私有表述需要改成“推荐私有，允许公开”。
- 私有仓库特有的 404 行为说明、凭证 private key、安全术语和私有测试 fixture 保持原义，不做机械替换。

**步骤 2：更新英文 README**

在 `README.md` 中统一说明：

- Skill Sync 使用用户控制的 GitHub Vault repository，不强制可见性。
- 私有仓库是保护 Skill 内容和历史的推荐默认。
- 公开仓库适合用户主动分享，但内容和 Git 历史对所有人可见。
- installation 仍必须使用 selected repositories，并且只授权一个 Vault 仓库。
- 没有仓库时可以从 Onboarding 打开 GitHub 新建页；返回后安装或调整 App 并重新检查。

**步骤 3：更新中文 README**

在 `README.zh-CN.md` 中镜像同样的事实和顺序，不保留“只能使用私有仓库”或“必须是私有仓库”的表述。

**步骤 4：更新基础架构设计**

在 `docs/github-base-refactor-design.md` 的产品范围、GitHub App、onboarding 和验收章节中记录：

- 唯一仓库范围是权限不变量，可见性不是。
- `GithubRepository.private` 只用于 onboarding 风险提示。
- 私有仓库直接继续，公开仓库由用户明确确认后继续。
- 后端不添加 `PublicRepository` 状态，也不按可见性阻止 initialize、bind、preview 或 apply。
- Skill Sync 打开 `https://github.com/new`，不调用仓库创建 endpoint。

保留 GitHub 对私有资源返回 404 的错误分类说明，以及 release checklist 使用一次性私有仓库的测试建议。

**步骤 5：验证文档不再声称强制私有**

运行：

```powershell
rg -n -S "exactly one private|must use.*private|唯一.*私有|必须.*私有|只能.*私有" README.md README.zh-CN.md docs/github-base-refactor-design.md
```

预期：没有把私有仓库描述为产品强制条件的匹配。若仍有匹配，逐条确认它是否只是 GitHub 私有资源错误语义，而不是 Vault 可见性限制。

**步骤 6：提交文档**

```powershell
git add README.md README.zh-CN.md docs/github-base-refactor-design.md
git commit -m "docs: document github vault visibility choice"
```

预期：提交只包含与已实现流程一致的英文、中文和架构说明。

### 任务 4：完成自动化与手动验证

宣称完成前使用 `@superpowers:verification-before-completion`。

**文件：**

- 验证：`src/modules/onboarding/pages/OnboardingPage.svelte`
- 验证：`src/modules/onboarding/components/InstallAppStage.svelte`
- 验证：`src/modules/onboarding/components/PublicRepositoryWarningStage.svelte`
- 验证：`src/shared/i18n/locales/zh-CN.json`
- 验证：`src/shared/i18n/locales/en-US.json`
- 验证：`README.md`
- 验证：`README.zh-CN.md`
- 验证：`docs/github-base-refactor-design.md`

**步骤 1：运行仓库要求的全部自动化检查**

运行：

```powershell
npm run typecheck
npm run format:check
npm run lint
npm run build
```

预期：全部通过。`lint` 包含初始化 UI 和文案改动所要求的响应式、颜色 token 和 i18n 检查。

**步骤 2：确认没有后端或权限改动**

从实施前基线检查文件差异：

```powershell
git diff --name-only <implementation-base>..HEAD
rg -n -S "PublicRepository|public_repository" src-tauri src/shared/schemas
```

预期：差异不包含 `src-tauri`、GitHub App 注册配置或权限配置；Rust 和共享响应 schema 中没有新增公开仓库状态。

**步骤 3：检查提交和工作区范围**

运行：

```powershell
git status --short
git log -4 --oneline --decorate
git diff --check <implementation-base>..HEAD
```

预期：没有空白错误；实现由计划中的三个 Conventional Commit 表示；实施前已有的未跟踪文件保持未修改、未提交。

**步骤 4：使用一次性 GitHub 环境手动验证初始化流程**

启动桌面应用：

```powershell
npm run tauri dev
```

使用非生产数据验证以下场景：

1. 没有 App installation 时，安装卡片显示“新建 Vault 仓库”“安装 GitHub App”和“重新检查安装”，在窄屏和桌面宽度下都没有重叠。
2. 新建操作在系统浏览器中打开 `https://github.com/new`，且不会发起仓库创建 API 请求。
3. 创建或选择私有仓库后，重新检查可直接进入分支、初始化和绑定流程，不显示公开风险提示。
4. 创建或选择公开仓库后，显示 owner/repo 和具体风险说明，不会自动继续。
5. 点击“继续使用公开仓库”后，可以正常进入分支、初始化、绑定和同步流程。
6. 公开提示页的“新建其他 Vault 仓库”和“调整 GitHub App 安装”分别打开正确页面。
7. installation 包含多个仓库或全部仓库时，仍显示授权范围警告，不显示无助于解决问题的新建按钮。
8. 切换语言，验证中英文文案，并确认长按钮和风险说明不会发生重叠或溢出。

预期：以上八种场景都符合描述。验证完成后停止开发进程。

**步骤 5：确认权限和产品边界**

检查最终差异和 GitHub App 注册设置。

预期：Repository permissions 仍然是 `Contents: Read and write` 和 `Metadata: Read-only`；没有 account 或 organization permission、`/user/repos` 请求、仓库名称表单、自动仓库变更或公开仓库后端阻止。
