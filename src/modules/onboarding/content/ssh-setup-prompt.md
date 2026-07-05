# 任务:为 skill-sync 同步仓库配置 SSH 认证(工作电脑)

你是一个 git / SSH 配置助手。请帮我在**工作电脑**上,为 skill-sync 这个工具所管理的本地 git 仓库配置好 SSH 认证,使其能 `git push` 到我的**个人 GitHub** 仓库。请一步步引导我,并在关键处确认后再继续。

## 背景(必读)

- 这台工作电脑的全局 git config(`user.name` / `user.email`)是**公司账号**,且 Windows 上的 Git Credential Manager 很可能已存了公司 GitHub 凭证。**禁止修改全局 git config,禁止删除或覆盖任何公司凭证。**
- skill-sync 软件在初始化仓库时,已用**仓库 local** config 设好了 commit 身份(`Skill Sync <skill-sync@local>`),所以 commit 作者不会污染成公司身份。**剩下唯一要解决的是 `git push` 的认证。**
- 因为公司 HTTPS 凭证会和个人 GitHub 凭证串号(Git Credential Manager 按 host 存取),所以采用 **SSH key** 方案,与公司凭证完全隔离。
- 请在开始前确认我的操作系统(Windows / macOS / Linux),后续命令按对应平台给出。

## 请按以下步骤执行

1. **生成专用 SSH key**
   - 用 ed25519,文件名区分个人用途(如 `id_ed25519_personal`),**不要复用**任何已有公司 key。
   - 设置 passphrase。
   - 示例:`ssh-keygen -t ed25519 -C "skill-sync-personal" -f ~/.ssh/id_ed25519_personal`

2. **把公钥加到个人 GitHub**
   - 给出查看公钥的命令(`cat ~/.ssh/id_ed25519_personal.pub`)。
   - 指导我复制到 GitHub → Settings → SSH and GPG keys → New SSH key。

3. **配置 `~/.ssh/config`**
   - 为个人 GitHub 配一个 Host 别名(如 `github-personal`),`HostName github.com`,`IdentityFile` 指向新 key,加 `IdentitiesOnly yes` 防止误用公司 key。
   - 文件不存在则新建;macOS/Linux 需 `chmod 600`。
   - 示例片段:
     ```
     Host github-personal
         HostName github.com
         User git
         IdentityFile ~/.ssh/id_ed25519_personal
         IdentitiesOnly yes
     ```

4. **启动 / 配置 ssh-agent**
   - Windows:确认 OpenSSH Authentication Agent 服务为自动启动且已运行,再 `ssh-add` 新 key。
   - macOS / Linux:`ssh-add` 新 key;macOS 可用 `--apple-use-keychain` 记住 passphrase。

5. **验证连接**
   - `ssh -T git@github-personal` 应返回 `Hi <user>! You've successfully authenticated...`。
   - 失败则按常见原因排查:公钥未加到 GitHub、key 未 `ssh-add`、Host 别名拼错、passphrase 未解锁、Windows agent 服务未运行。

6. **给出最终 remote URL**
   - 形式:`git@github-personal:<我的GitHub用户名>/<仓库名>.git`
   - 提示我把这个 URL 填进 skill-sync onboarding 的 "Remote Git URL" 输入框,然后点 "Check Remote" 验证可达。

## 约束

- 全程**不动全局 git config**,不改公司凭证。
- 所有改动可回退(`~/.ssh/config` 仅新增一个 Host 段,不覆盖既有内容)。
- 不要执行任何 `git push` 等写操作,只做配置与验证。
- 遇到我没说清的信息(如我的 GitHub 用户名、仓库名、操作系统),先问我再继续。

## 产出

- 一条可用的 SSH remote URL(使用 `github-personal` Host 别名)。
- 一条已通过 `ssh -T` 验证的 SSH 连接。
- 简要说明:把该 URL 填回 skill-sync onboarding 的 remote 输入框并点 "Check Remote"。
