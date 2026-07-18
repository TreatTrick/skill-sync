# Localized README Tutorial Images Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Help repository visitors understand Skill Sync's core value quickly while keeping a complete localized seven-step setup walkthrough in each README.

**Architecture:** Store the supplied PNG files under language-specific documentation directories and reference them with repository-relative Markdown paths. Replace the current bilingual preview block with four result-focused screenshots in the README's own language, then place all seven steps inside a collapsible walkthrough under the existing getting-started section.

**Tech Stack:** Markdown, HTML `details`/`summary`, PNG assets, PowerShell, Prettier

---

### Task 1: Add Localized Tutorial Assets

**Files:**

- Create: `docs/images/tutorial/en/01-authorize-github.png`
- Create: `docs/images/tutorial/en/02-create-vault.png`
- Create: `docs/images/tutorial/en/03-install-github-app.png`
- Create: `docs/images/tutorial/en/04-select-vault-repository.png`
- Create: `docs/images/tutorial/en/05-bind-vault.png`
- Create: `docs/images/tutorial/en/06-preview-upload.png`
- Create: `docs/images/tutorial/en/07-sync-another-machine.png`
- Create: `docs/images/tutorial/zh-CN/01-authorize-github.png`
- Create: `docs/images/tutorial/zh-CN/02-create-vault.png`
- Create: `docs/images/tutorial/zh-CN/03-install-github-app.png`
- Create: `docs/images/tutorial/zh-CN/04-select-vault-repository.png`
- Create: `docs/images/tutorial/zh-CN/05-bind-vault.png`
- Create: `docs/images/tutorial/zh-CN/06-preview-upload.png`
- Create: `docs/images/tutorial/zh-CN/07-sync-another-machine.png`

**Step 1: Create the language directories**

Run:

```powershell
New-Item -ItemType Directory -Force docs/images/tutorial/en, docs/images/tutorial/zh-CN
```

Expected: both directories exist.

**Step 2: Copy the supplied files with normalized names**

Copy each `C:\Users\11541\Downloads\[number]-[description]-[language].png` file to the matching language directory and normalized filename listed above. Do not transform or recompress the PNG data.

**Step 3: Verify source and destination hashes**

Run `Get-FileHash -Algorithm SHA256` for each source/destination pair.

Expected: all 14 destination hashes match their sources.

### Task 2: Add the English Value Tour and Walkthrough

**Files:**

- Modify: `README.md`

**Step 1: Replace the current application preview**

Replace the bilingual preview block after the badges with `See Skill Sync in action`. Show steps 2, 4, 6, and 7 in that order, with short result-focused headings and descriptions covering:

- a user-owned private GitHub Vault;
- selected-repository GitHub App access;
- review before applying uploads, downloads, deletes, and conflicts;
- cross-machine downloads and explicit conflict choices.

Use only `docs/images/tutorial/en/` images and descriptive English alt text.

**Step 2: Add the complete walkthrough**

After the existing getting-started guidance, add a `<details>` block with a `Complete 7-step walkthrough` summary. Include steps 1 through 7, each with one action/result sentence and its English screenshot.

### Task 3: Add the Chinese Value Tour and Walkthrough

**Files:**

- Modify: `README.zh-CN.md`

**Step 1: Replace the current application preview**

Mirror the English structure with `快速了解 Skill Sync`, Chinese result-focused copy, and only `docs/images/tutorial/zh-CN/` images.

**Step 2: Add the complete walkthrough**

After the existing getting-started guidance, add a `<details>` block with a `完整 7 步教程` summary. Include steps 1 through 7, each with one action/result sentence and its Chinese screenshot.

### Task 4: Verify and Commit the Documentation

**Files:**

- Verify: `README.md`
- Verify: `README.zh-CN.md`
- Verify: `docs/images/tutorial/en/*.png`
- Verify: `docs/images/tutorial/zh-CN/*.png`

**Step 1: Check localized references**

Expected: `README.md` has no `tutorial/zh-CN/` paths, `README.zh-CN.md` has no `tutorial/en/` paths, and each README references all seven localized assets.

**Step 2: Check repository-relative image paths**

Parse image destinations from both README files and verify that every local path exists.

Expected: no missing paths.

**Step 3: Run documentation formatting checks**

Run:

```bash
npm run format:check
git diff --check
```

Expected: both commands exit successfully.

**Step 4: Review and commit the focused diff**

Run:

```bash
git status --short
git diff -- README.md README.zh-CN.md
git add README.md README.zh-CN.md docs/images/tutorial docs/plans/2026-07-18-readme-tutorial-images.md
git commit -m "docs: add localized tutorial walkthrough"
```

Expected: the commit passes the configured hooks and contains only the tutorial assets, README updates, and implementation plan.
