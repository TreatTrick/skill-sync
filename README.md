<p align="center">
  <img src="src-tauri/icons/icon.png" width="140" alt="Skill Sync logo" />
</p>

<h1 align="center">Skill Sync</h1>

<p align="center">
  <strong>Sync your AI agent skills across machines — like dotfiles, with a desktop UI.</strong>
</p>

<p align="center">
  <a href="README.md">English</a> · <a href="README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT" /></a>
  <img src="https://img.shields.io/badge/Tauri-2-orange?logo=tauri&logoColor=white" alt="Tauri 2" />
  <img src="https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white" alt="Svelte 5" />
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey" alt="Platform" />
  <a href="https://github.com/TreatTrick/skill-sync/issues"><img src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg" alt="PRs welcome" /></a>
  <img src="https://img.shields.io/github/stars/TreatTrick/skill-sync?style=social" alt="GitHub Stars" />
</p>

---

## ✨ Why

You've built a great set of Claude Code, Codex, Cursor, or Gemini skills — but moving them between your laptop, desktop, and work machine means remembering paths, copying folders, and hand-merging Git conflicts. **Skill Sync** turns all of that into one click.

Your skills follow you everywhere, through **your own Git repo**. No account. No cloud. No marketplace. No telemetry.

## 🚀 Features

- **🔍 Auto-discover** skills already living on your machine from configured agent paths.
- **🧩 Multi-agent** support — Claude Code, Codex, Cursor, Gemini CLI, and custom paths.
- **👀 Sync preview** — see every upload, download, update, delete, and conflict _before_ anything is written.
- **⚡ One-click sync** — apply the whole plan with a single button.
- **⚔️ Conflict resolution** — keep local, use remote, or skip, per skill.
- **💾 Automatic backups** — every local file is backed up before it's touched, with one-click restore.
- **⚙️ Full settings** — manage agent paths, Git remote, backup directory, and ignore rules.
- **🛡️ Safe by default** — all destructive writes are previewed first.
- **🔒 Local-first & private** — sync runs through your own Git repository. Nothing leaves your control.

## 🤝 Supported Agents

| Agent       | Status |
| ----------- | :----: |
| Claude Code |   ✅   |
| Codex       |   ✅   |
| Cursor      |   ✅   |
| Gemini CLI  |   ✅   |
| Custom path |   ✅   |

## 🧭 How it works

1. **Scan** — discovers skills (e.g. `SKILL.md`) from your configured agent directories.
2. **Diff** — compares local skills against your Git sync repo and computes a change plan.
3. **Preview** — review planned uploads, downloads, updates, deletions, and conflicts.
4. **Sync** — apply changes with one click. Local files are backed up first.
5. **Resolve** — handle conflicts: keep local, use remote, or skip.
6. **Restore** — roll back any file from the backups page, anytime.

## 📦 Install

### Build from source

**Prerequisites:** [Node.js](https://nodejs.org/) 20+, [Rust](https://www.rust-lang.org/) (stable), [Git](https://git-scm.com/), and the [Tauri v2 system dependencies](https://tauri.app/start/prerequisites/).

```bash
git clone https://github.com/TreatTrick/skill-sync.git
cd skill-sync
npm install

npm run tauri dev     # run the app in development
npm run tauri build   # produce installers in src-tauri/target/release/bundle
```

> Prebuilt binaries will ship with the first release — **watch** the repo to be notified ⭐

## 🛠 Tech stack

- **Backend:** Tauri 2 + Rust — skill discovery, `SKILL.md` parsing, Git sync (system `git` CLI), backups, conflict detection.
- **Frontend:** Svelte 5 + SvelteKit (static SPA), TypeScript (strict), Vite.
- **Styling:** Tailwind CSS v4 with semantic color tokens, shadcn-svelte (bits-ui) components.
- **State:** `@tanstack/svelte-query` for server state, Svelte 5 runes for client state.
- **Extras:** `@lucide/svelte` icons, i18next (EN / 简体中文), zod validation.

## 📁 Project structure

```
src/
  routes/    # SvelteKit routing (thin page wrappers)
  app/       # app shell, layout, navigation
  modules/   # dashboard · skills · sync · conflicts · backups · settings · onboarding
  shared/    # shadcn-svelte UI, i18n, tokens, utils
src-tauri/
  src/       # Rust: detect · manifest · git_store · sync_engine · backup · config
```

## 🗺 Roadmap

- [ ] Prebuilt binaries + Tauri auto-update
- [ ] Cross-platform CI (Windows / macOS / Linux)
- [ ] More agent integrations
- [ ] Scheduled / background sync

## 🤝 Contributing

Issues and PRs are welcome! Please read [AGENTS.md](AGENTS.md) and [CLAUDE.md](CLAUDE.md) first — they cover the engineering rules, commit conventions, and AI-collaboration guidelines this repo follows.

```bash
npm run lint     # typecheck + ESLint + responsive/colors/i18n lints
npm run build    # type-check and build
```

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`, …).

## 📄 License

[MIT](LICENSE) © 2026 TreatTrick

---

<sub>Built with Tauri · Svelte 5 · for everyone who keeps losing their skills between machines.</sub>
