# SSH 配置引导:onboarding 内置 Prompt + 教程

## 目标

让开发者(尤其在工作电脑上、全局 git config 挂着公司账号的人)首次进入 onboarding 时,能立刻知道如何把本软件的个人 GitHub 仓库用 SSH 关联起来。提供两条路径:

1. **AI 辅助**:onboarding 内一键复制 prompt,粘贴给 Codex / Claude Code 代为配置。
2. **自行配置**:仓库内分步教程 `docs/ssh-setup.md`。

## 背景(已确认的代码事实)

- `git_store.rs::init()` 已用仓库 **local** config 写死 `user.email=skill-sync@local` / `user.name=Skill Sync`,commit 作者不会污染成公司身份。**剩下纯粹是 push 认证问题。**
- `git_store` 是系统 `git` CLI 的薄封装,remote URL 原样传给 git,认证完全由系统 git 决定。填 SSH URL(`git@...`)即可走 SSH。
- onboarding 页:`OnboardingPage.svelte`,文案集中在 i18n `onboarding.*`。
- `src/shared/ui` 仅有 badge/button/card/checkbox/empty-state/input/spinner/status-badge/textarea。**无 Collapsible、无 Toast、无 clipboard 依赖。**
- i18n lint 只查 `.ts`/`.svelte`(`src/shared/i18n` 豁免),**不查 `.md`**。

## 产出物

### 1. `src/modules/onboarding/content/ssh-setup-prompt.md`(新建)

给 AI 的自包含中文 prompt,onboarding 用 `?raw` 导入后供复制。内容要点:

- **任务**:在工作电脑上为本软件的同步仓库配置 SSH 认证到个人 GitHub。
- **上下文**:工作电脑全局 git config 是公司账号(禁止修改);skill-sync 已用 local config 处理 commit 身份;只剩 `git push` 认证;Windows 上 Git Credential Manager 会让公司 HTTPS 凭证和个人 GitHub 串号,故选 SSH。
- **执行步骤**:① 生成专用 ed25519 key(带 passphrase,不复用公司 key);② 把公钥加到个人 GitHub 账号;③ 在 `~/.ssh/config` 为 `github.com` 配 Host 别名(如 `github-personal`),`IdentityFile` 指向新 key,`IdentitiesOnly yes`;④ 确保 ssh-agent 加载该 key(Windows 需启动 OpenSSH Authentication Agent 服务);⑤ 给出最终 remote URL:`git@github-personal:<user>/<repo>.git`;⑥ `ssh -T git@github-personal` 验证。
- **约束**:不改全局 git config、不动公司凭证、所有步骤可回退;遇多账号时仅用 Host 别名隔离。
- **产出**:可用的 remote URL + 已验证的 SSH 连接,填回 onboarding 的 remote 输入框。

放 `onboarding/content/` 是模块私有资源,不污染 `shared`,符合 `routes->app->modules->shared` 流向。

### 2. `docs/ssh-setup.md`(新建)

面向人的分步教程(中文,`.md` 不受 i18n lint 约束):

- **为什么需要**:工作电脑公司 git config / GCM 凭证冲突;软件已处理 commit 作者,只剩认证;SSH 按 host/key 天然隔离,优于 HTTPS+PAT。
- **方式 A · 手动配置**:生成 key → 加 GitHub → `~/.ssh/config` Host 别名 → ssh-agent → remote 用 `git@github-personal:...` → `ssh -T` 验证。Windows / macOS / Linux 分别给出命令。
- **方式 B · AI 辅助**:打开软件 onboarding,点"复制 AI 配置 Prompt",粘贴给 Codex / Claude Code。
- **回到软件**:在 onboarding remote 填 SSH URL,点"检测远程"应显示可达。
- **FAQ**:多 GitHub 账号、passphrase 记忆、Windows OpenSSH Agent 服务、`IdentitiesOnly` 作用。

### 3. `OnboardingPage.svelte`(修改)

在 remote/branch 输入 Card 与检测 Card 之间,插入一个 SSH 提示 Card:

- 复用现有 `Card`/`CardContent`/`Button`,**不引入新依赖**(无 shadcn Collapsible,用 `$state` 布尔 + Button 切换显示/隐藏,视觉与现有 Card 一致)。
- 折叠态:标题 + 一个 outline Button(`sshHintToggle`)。
- 展开态:`sshHintDescription` + 主按钮"复制 AI 配置 Prompt"(`copySshPrompt`)+ 复制后临时切文案为 `sshPromptCopied`(约 2.5s 复位)+ 一行 `sshTutorialHint` 指向 `docs/ssh-setup.md`。
- 复制实现:`navigator.clipboard.writeText(sshPromptText)`(Tauri 2 webview 支持;若实测不可用,备用方案是加 `@tauri-apps/plugin-clipboard-manager`,本次先用原生 API)。
- 导入:`import sshPromptText from '../content/ssh-setup-prompt.md?raw'`。

### 4. i18n(zh-CN.json / en-US.json,`onboarding` 节点新增)

| key                  | zh-CN                                                                                                   | en-US                                                                                                                                                                   |
| -------------------- | ------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `sshHintTitle`       | 工作电脑 / SSH 认证(可选)                                                                               | Work computer / SSH auth (optional)                                                                                                                                     |
| `sshHintToggle`      | 如何配置 SSH                                                                                            | How to set up SSH                                                                                                                                                       |
| `sshHintDescription` | 如果工作电脑挂着公司 git 凭证,直接 push 个人仓库会因凭证冲突失败。建议用 SSH key 关联个人 GitHub 仓库。 | If your work computer holds company git credentials, pushing to a personal repo may fail due to credential conflicts. Use an SSH key to link your personal GitHub repo. |
| `copySshPrompt`      | 复制 AI 配置 Prompt                                                                                     | Copy AI setup prompt                                                                                                                                                    |
| `sshPromptCopied`    | 已复制,粘贴给 Codex / Claude Code 即可                                                                  | Copied — paste it into Codex / Claude Code                                                                                                                              |
| `sshTutorialHint`    | 完整分步教程见仓库 docs/ssh-setup.md                                                                    | Full step-by-step guide: docs/ssh-setup.md in the repo                                                                                                                  |

prompt 本体不进 i18n(走 `?raw` import)。

## 实现决策

- **折叠方式**:用现有 `Card` + `Button` + `$state` 布尔,不引入 shadcn Collapsible(避免联网 `npx add`,且对简单 disclosure 偏重)。若后续要统一,可再 `npx shadcn-svelte@latest add collapsible` 替换。
- **prompt 单一来源**:`ssh-setup-prompt.md` 是权威文本,onboarding 复制按钮 `?raw` 读它;教程 `docs/ssh-setup.md` 的"方式 B"章节只写指引(指向 onboarding 复制按钮),不重复 prompt 全文,避免漂移。
- **`?raw` 类型**:SvelteKit 默认 `vite/client` 类型已含 `*?raw`。若 `npm run build` 报类型错,在 `src/app.d.ts` 补 `declare module '*.md?raw'`。
- **颜色/尺寸**:全部用 `src/index.css` 语义 token(`text-muted-foreground` 等),无任意 text/radius/size 类,过 colors/responsive lint。

## 验证

- `npm run lint:i18n` —— `.svelte` 无中文字面量(全走 `t(...)`)。
- `npm run lint:colors` —— 仅用语义 token。
- `npm run lint:responsive` —— 无任意尺寸类。
- `npm run lint` —— 全量。
- `npm run build` —— 验证 `?raw` import 与 Svelte 编译通过。

## 不做(范围边界)

- 不改 `git_store.rs` / `commands.rs`(软件行为不变,只是引导文档 + onboarding UI)。
- 不让 commit identity 可配(仍 `skill-sync@local`,如需贡献图归属是另一个独立需求)。
- 不打包教程进应用内路由(教程是仓库 docs,onboarding 仅文字指引 + 复制 prompt)。
- 不写测试(prompt/教程为静态文档;onboarding 复制按钮为 trivial UI,项目规则未要求 UI 测试)。
