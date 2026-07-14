# README Screenshots And v0.1.3 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Show both localized application screenshots prominently in both README files and release the result as v0.1.3.

**Architecture:** Keep static documentation assets in `docs/images/` and reference them with repository-relative paths. Preserve the repository's existing release process by changing the same five version files used for v0.1.2, committing on `main`, and publishing an annotated tag.

**Tech Stack:** Markdown, PNG assets, npm package metadata, Cargo metadata, Tauri configuration, Git

---

### Task 1: Add The Screenshot Assets

**Files:**

- Create: `docs/images/skill-sync-english.png`
- Create: `docs/images/skill-sync-chinese.png`

**Step 1: Copy the provided screenshots**

Copy `C:\Users\11541\Downloads\skill-sync-english.png` and `C:\Users\11541\Downloads\skill-sync-chinese.png` into `docs/images/` without modifying their contents.

**Step 2: Verify the copied assets**

Compare the source and destination SHA-256 hashes.

Expected: each destination hash matches its corresponding source file.

### Task 2: Add Localized README Screenshot Sections

**Files:**

- Modify: `README.md`
- Modify: `README.zh-CN.md`

**Step 1: Add the English README section**

Immediately after the badge block, add an `Application preview` section with the English screenshot first and the Chinese screenshot second. Use localized captions and descriptive alternative text.

**Step 2: Add the Chinese README section**

At the matching location, add a `软件截图` section with the Chinese screenshot first and the English screenshot second. Use Chinese captions and alternative text.

**Step 3: Inspect the Markdown diff**

Expected: both README files reference both files under `docs/images/`, and the current README language determines the first screenshot.

### Task 3: Bump The Application Version

**Files:**

- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tauri.conf.json`

**Step 1: Replace the project version**

Change only the application/package version values from `0.1.2` to `0.1.3`, matching the v0.1.2 release commit's file set.

**Step 2: Verify the version values**

Read the project-level version fields directly.

Expected: all five version sources report `0.1.3`; dependency versions remain unchanged.

### Task 4: Commit And Publish v0.1.3

**Files:**

- Stage: `README.md`
- Stage: `README.zh-CN.md`
- Stage: `docs/images/skill-sync-english.png`
- Stage: `docs/images/skill-sync-chinese.png`
- Stage: `docs/plans/2026-07-14-readme-screenshots-v0.1.3.md`
- Stage: `package.json`
- Stage: `package-lock.json`
- Stage: `src-tauri/Cargo.toml`
- Stage: `src-tauri/Cargo.lock`
- Stage: `src-tauri/tauri.conf.json`

**Step 1: Review the focused diff and status**

Expected: only the planned documentation, image, plan, and version files are changed.

Do not run lint, compilation, build, or test commands, per the release request.

**Step 2: Commit the release**

```bash
git commit -m "chore: release v0.1.3"
```

Expected: the commit succeeds through the repository's configured hooks.

**Step 3: Push main**

```bash
git push origin main
```

Expected: `origin/main` advances to the release commit.

**Step 4: Create and push the annotated tag**

```bash
git tag -a v0.1.3 -m "v0.1.3"
git push origin v0.1.3
```

Expected: the remote `v0.1.3` tag resolves to the release commit.
