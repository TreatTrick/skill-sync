# 为 skill-sync 配置 SSH 认证(工作电脑场景)

适用场景:你在**工作电脑**上使用 skill-sync,而这台机器的全局 git config 与凭证管理器挂的是**公司账号**。直接 push 到个人 GitHub 仓库会因凭证冲突失败。本文教你用 SSH key 把 skill-sync 和个人 GitHub 仓库关联起来,与公司凭证完全隔离。

> 如果是个人电脑、没有公司凭证冲突,直接用 HTTPS + PAT 也能用,可跳过本文。

## 为什么用 SSH

- skill-sync 在初始化仓库时已用**仓库 local** config 写好了 commit 身份(`Skill Sync <skill-sync@local>`),commit 不会署成公司账号——**所以只剩 `git push` 的认证要解决。**
- Windows 上的 Git Credential Manager 按 host 存取 HTTPS 凭证,公司 GitHub 和个人 GitHub 都在 `github.com`,容易串号:拿公司 token 推个人仓库会被拒(401),或反之。
- SSH key 认证走 `~/.ssh/` + ssh-agent,**完全不碰 Credential Manager**,一把 key 绑一个账号,按 host 天然隔离。

---

## 方式 A:手动配置

### 1. 生成专用 SSH key

**不要复用**公司已有 key,为个人用途单独生成一把,并设置 passphrase:

```bash
ssh-keygen -t ed25519 -C "skill-sync-personal" -f ~/.ssh/id_ed25519_personal
```

Windows 上若 `ssh-keygen` 不在 PATH,用 Git Bash 或系统自带的 OpenSSH 客户端。

### 2. 把公钥加到个人 GitHub

查看公钥内容并复制:

```bash
cat ~/.ssh/id_ed25519_personal.pub
```

打开 GitHub → **Settings → SSH and GPG keys → New SSH key**,粘贴公钥,保存。

### 3. 配置 `~/.ssh/config`

为个人 GitHub 配一个 **Host 别名**(如 `github-personal`),让 SSH 用新 key 且不用公司 key:

```sshconfig
Host github-personal
    HostName github.com
    User git
    IdentityFile ~/.ssh/id_ed25519_personal
    IdentitiesOnly yes
```

- 文件不存在就新建。macOS / Linux 需 `chmod 600 ~/.ssh/config`。
- `IdentitiesOnly yes` 很关键:它强制只用指定的 key,避免 ssh-agent 里其它(公司)key 被尝试导致被 GitHub 拒绝。
- 同一台机器若有多个 GitHub 账号,每个账号配一个 Host 别名(如 `github-personal`、`github-work`),各自 `IdentityFile`。

### 4. 启动 ssh-agent 并加载 key

**Windows:**

1. 服务(`services.msc`)→ **OpenSSH Authentication Agent** → 启动类型设为"自动",并启动。
2. 加载 key:
   ```bash
   ssh-add ~/.ssh/id_ed25519_personal
   ```

**macOS:**

```bash
ssh-add --apple-use-keychain ~/.ssh/id_ed25519_personal
```

`--apple-use-keychain` 让 passphrase 存进钥匙串,开机自动加载。

**Linux:**

```bash
ssh-add ~/.ssh/id_ed25519_personal
```

### 5. 验证连接

```bash
ssh -T git@github-personal
```

成功会返回:`Hi <你的用户名>! You've successfully authenticated, but GitHub does not provide shell access.`

常见失败原因:

- `Permission denied (publickey)`:公钥没加到 GitHub、key 没 `ssh-add`、Host 别名拼错。
- Windows 上反复要 passphrase:OpenSSH Authentication Agent 服务没启动。
- 用了公司 key 被拒:确认 `~/.ssh/config` 里有 `IdentitiesOnly yes`。

### 6. 拿到 remote URL

你的 remote URL 形式是(注意用的是别名 `github-personal`,不是 `github.com`):

```
git@github-personal:<你的用户名>/<仓库名>.git
```

---

## 方式 B:让 AI 帮你配置

如果你装了 Codex 或 Claude Code,可以打开 skill-sync 的 onboarding 页面,点 **"复制 AI 配置 Prompt"** 按钮,把 prompt 粘贴给 AI,它会一步步引导你完成上述配置。

---

## 回到 skill-sync 验证

1. 在 onboarding 的 **"Remote Git URL"** 输入框填入上面的 SSH URL。
2. 点 **"Check Remote"**,应显示"远程可访问"。
3. 点 **"Save and continue"** 完成初始化。之后 skill-sync 的同步就会通过 SSH 推送到你的个人仓库。

---

## FAQ

**Q:为什么不直接改全局 git config 的 user.email 成个人邮箱?**
A:那会污染公司所有仓库的 commit 身份。skill-sync 已用仓库 local config 处理了 commit 身份,你不需要改全局配置。

**Q:我不想让 commit 显示成 `skill-sync@local`,想算进 GitHub 贡献图?**
A:目前 commit 身份是软件写死的 local config。如需改成你的个人邮箱,需要在 skill-sync 配置里让它可配——这是另一个独立需求,不在本文范围。

**Q:公司也用 GitHub Enterprise,会冲突吗?**
A:不会。GitHub Enterprise 通常是 `github.<公司域名>.com`,和个人 `github.com` 是不同 host,SSH config 各自独立。只需确保个人的 Host 别名指向 `github.com`。

**Q:passphrase 忘了怎么办?**
A:SSH key 的 passphrase 无法找回,只能重新生成 key 并更新 GitHub 上的公钥。旧 key 删掉即可,不影响公司凭证。

**Q:多台电脑都要用?**
A:每台电脑生成各自 key,都加到同一个 GitHub 账号的 SSH keys 列表即可。不要把私钥在机器间拷贝。
