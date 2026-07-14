# Passive GitHub Growth Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Improve passive discovery and convert satisfied users into GitHub supporters through repository metadata, two focused directory submissions, and a bilingual Settings call to action.

**Architecture:** Keep the product change inside the existing Settings page and reuse the existing `openPath` Tauri bridge, which already opens both filesystem paths and onboarding URLs cross-platform. Change repository metadata through GitHub's API, and make external list submissions in isolated forks without adding their files to this repository.

**Tech Stack:** GitHub REST API, Markdown, Svelte 5, TypeScript, i18next, Tailwind CSS, Lucide Svelte, Tauri 2

---

### Task 1: Complete GitHub Repository Metadata

**Files:**

- Verify: `README.md`
- Verify: `README.zh-CN.md`
- Verify: `docs/images/skill-sync-english.png`
- Verify: `docs/images/skill-sync-chinese.png`

**Step 1: Verify the existing screenshot presentation**

Run:

```powershell
rg -n "docs/images|Application preview|软件截图" README.md README.zh-CN.md
```

Expected: each README references both localized screenshots and no GIF is referenced.

**Step 2: Update the public repository metadata**

Use the authenticated GitHub API to set this description:

```text
Local-first desktop app that syncs Codex, Claude Code, and Agent Skills across machines through your private GitHub repository.
```

Set these repository topics:

```text
agent-skills, ai-agents, claude-code, codex, desktop-app, github, local-first, rust, skill-management, svelte, sync, tauri
```

Do not set a homepage URL.

**Step 3: Verify the public metadata**

Run the unauthenticated GitHub repository endpoint and confirm the description and topic set match exactly.

### Task 2: Add the In-App GitHub Support Entry

**Files:**

- Modify: `src/modules/settings/pages/SettingsPage.svelte`
- Modify: `src/shared/i18n/locales/zh-CN.json`
- Modify: `src/shared/i18n/locales/en-US.json`

**Step 1: Add the Settings UI using missing locale keys**

In `SettingsPage.svelte`:

- Import the Lucide `Github` icon.
- Import `Button` and `CardFooter` from shared UI.
- Add a fixed `PROJECT_REPOSITORY_URL` constant.
- Add an async handler that calls `openPath(PROJECT_REPOSITORY_URL)` and reports failures through the existing `errorMessage` and `toast.error` path.
- Add one card after `SkillRootsSection`, inside the configured branch.

Use this structure:

```svelte
<Card>
  <CardHeader>
    <CardTitle>{t('settings.supportProject')}</CardTitle>
    <CardDescription>{t('settings.supportProjectDescription')}</CardDescription>
  </CardHeader>
  <CardFooter>
    <Button variant="outline" onclick={() => void openProjectRepository()}>
      <Github />
      {t('settings.supportOnGithub')}
    </Button>
  </CardFooter>
</Card>
```

**Step 2: Run type checking to confirm the locale contract is incomplete**

Run:

```powershell
npm run typecheck
```

Expected: fail because the three new typed i18n keys are absent.

**Step 3: Add localized copy**

Add to `settings` in `zh-CN.json`:

```json
"supportOnGithub": "在 GitHub 上支持",
"supportProject": "支持 Skill Sync",
"supportProjectDescription": "如果 Skill Sync 对你有帮助，可以在 GitHub 上为项目点个 Star。"
```

Add to `settings` in `en-US.json`:

```json
"supportOnGithub": "Support on GitHub",
"supportProject": "Support Skill Sync",
"supportProjectDescription": "If Skill Sync is useful to you, you can support the project with a Star on GitHub."
```

**Step 4: Run focused frontend checks**

Run:

```powershell
npm run typecheck
npm run lint:responsive
npm run lint:colors
npm run lint:i18n
```

Expected: all pass.

**Step 5: Commit the product change**

```powershell
git add src/modules/settings/pages/SettingsPage.svelte src/shared/i18n/locales/zh-CN.json src/shared/i18n/locales/en-US.json
git commit -m "feat: add github support link"
```

### Task 3: Submit to Relevant Curated Lists

**Files:**

- External modify: `rohitg00/awesome-claude-code-toolkit` `README.md`
- External modify: `tauri-apps/awesome-tauri` `README.md`

**Step 1: Submit to Claude Code Toolkit**

Fork `rohitg00/awesome-claude-code-toolkit`, create a focused branch from its current default branch, and add one row to the existing ecosystem/companion-app table following its local ordering and punctuation:

```markdown
| [Skill Sync](https://github.com/TreatTrick/skill-sync) | new | Local-first desktop app that syncs Claude Code, Codex, and Agent Skills across machines through a user-owned private GitHub repository. |
```

Commit only that README change, push the branch, and open a pull request whose title and body state what was added without promotional claims.

**Step 2: Submit to Awesome Tauri**

Fork `tauri-apps/awesome-tauri`, create a focused branch from `dev`, and add this alphabetically under `Applications > Developer tools`:

```markdown
- [Skill Sync](https://github.com/TreatTrick/skill-sync) ![v2] - Syncs Agent Skills across machines through a private GitHub repository.
```

Verify the description is under 24 words and follows `.github/contributing.md`. Commit only that README change, push the branch, and open one focused pull request using the repository template.

**Step 3: Prepare the human-only Awesome Claude Code recommendation**

Do not submit automatically. Its current rules require a human-created web-form issue and temporarily disable recommendations. Provide this neutral one-line description to the user for later use:

```text
Skill Sync is a local-first desktop app that synchronizes Claude Code, Codex, and Agent Skills across machines through a user-owned private GitHub repository, with previewed changes, explicit conflict choices, and recovery-aware apply.
```

### Task 4: Complete Verification

**Files:**

- Verify: all files changed by Task 2
- Verify: focused external pull request diffs

**Step 1: Run the repository checks**

```powershell
npm run typecheck
npm run format:check
npm run lint
npm run build
```

Expected: all pass.

**Step 2: Inspect repository state**

Run `git status --short`, inspect both local commits, and confirm no unrelated user changes were staged or committed.

**Step 3: Verify external state**

Confirm the GitHub public API returns the new metadata and record the URLs and current status of every submitted pull request.
