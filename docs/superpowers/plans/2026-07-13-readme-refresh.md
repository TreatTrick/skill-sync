# Skill Sync README Refresh Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite the English and Simplified Chinese README files into accurate, product-led project pages, verify the repository, commit the documentation, and push only when the approved safety conditions are satisfied.

**Architecture:** Keep the README pair as parallel entry points with identical structure, facts, commands, tables, paths, and URLs. Use current code, tests, workflow configuration, and published Release assets as evidence; link the detailed vault design instead of duplicating it. No product code, dependencies, screenshots, or speculative platform claims are added.

**Tech Stack:** GitHub-flavored Markdown, Tauri 2, Svelte 5/SvelteKit, Rust 1.88.0, npm repository checks, Git.

---

## Chunk 1: README Content And Delivery

### Task 1: Rewrite The English Project Page

**Files:**

- Modify: `README.md`
- Reference: `docs/superpowers/specs/2026-07-13-readme-refresh-design.md`
- Reference: `docs/github-base-refactor-design.md`
- Reference: `.github/workflows/release.yml`
- Reference: `src-tauri/src/github_app_config.rs`

- [ ] **Step 1: Replace the current English README with the approved product-led structure**

  Keep the existing application icon and language switcher. Add these sections in order: hero and badges, Why Skill Sync, Highlights, How it works, Supported directories, Vault and sync model, Privacy and safety, Getting started, Build from source, Development checks, Tech stack, Project structure, Contributing, License. Put the latest Release download call to action in Getting started.

  Use these badge targets:

  ```text
  Release image: https://img.shields.io/github/v/release/TreatTrick/skill-sync?display_name=tag&sort=semver
  Release target: https://github.com/TreatTrick/skill-sync/releases/latest
  License image: https://img.shields.io/github/license/TreatTrick/skill-sync
  License target: LICENSE
  Platform image: https://img.shields.io/badge/platform-Windows%20x64-0078D4
  Platform target: https://github.com/TreatTrick/skill-sync/releases/latest
  ```

  State that the latest published assets currently provide Windows x64 EXE/MSI installers, without hardcoding a release version. Do not advertise a macOS download, signed/notarized packages, Linux support, custom roots, project roots, or providers other than GitHub App Device Flow. Explicitly state that Skill Sync has no telemetry.

- [ ] **Step 2: Keep technical claims concise and evidence-backed**

  Explain the three-way `base/local/remote` hash comparison, content-addressed vault layout, explicit conflict choices, advisory deletes, local trash, OS keyring credentials, and recovery-aware apply in short user-facing language. Link `docs/github-base-refactor-design.md` for protocol details. Remove the nonexistent `docs/github-app-setup.md` link.

- [ ] **Step 3: Add exact user and contributor commands**

  Use `https://github.com/TreatTrick/skill-sync/releases/latest` as the primary download route and recommend starting GitHub authorization from in-app Onboarding. Include `https://github.com/apps/tt-skills-sync/installations/new` as the direct GitHub App installation link while explaining that Onboarding is the normal entry point. Keep source setup to Node.js 20+, the repository-managed Rust 1.88.0 toolchain, and Tauri v2 system prerequisites. Include these exact checks:

  ```bash
  npm run typecheck
  npm run format:check
  npm run lint
  npm run build
  npm run rust:fmt:check
  npm run rust:clippy
  npm run rust:test
  ```

### Task 2: Rewrite The Simplified Chinese Project Page

**Files:**

- Modify: `README.zh-CN.md`
- Reference: `README.md`

- [ ] **Step 1: Mirror the approved English structure in natural Chinese**

  Preserve the same section order, tables, code blocks, paths, commands, badges, and URLs. Translate meaning rather than English sentence structure. Keep product terms such as Skill, Vault, namespace, manifest, hash, Device Flow, keyring, Onboarding, Sync, and Settings where they make the UI or protocol easier to recognize.

- [ ] **Step 2: Compare the two README files for factual parity**

  Confirm both versions contain the same three namespaces, two remote paths, seven development commands, latest Release URL, GitHub App install URL, vault tree, platform claim, safety boundaries, and existing local documentation links.

  Run `rg -n "^#{1,2} " README.md README.zh-CN.md` and compare the heading levels and order against the section list in Task 1, Step 1. Expected: both files have the same number and ordering of H1/H2 sections, with language-appropriate titles.

### Task 3: Verify Documentation And Repository Quality

**Files:**

- Verify: `README.md`
- Verify: `README.zh-CN.md`
- Verify: repository-wide configured checks

- [ ] **Step 1: Check formatting, links, and unintended changes**

  Run:

  ```powershell
  git diff --check
  rg -n "docs/github-app-setup|macOS installer|Linux support|PAT setup|SSH setup" README.md README.zh-CN.md
  git diff -- README.md README.zh-CN.md
  ```

  Expected: `git diff --check` passes; the stale or unsupported phrases have no matches except explicit statements that PAT/SSH are not required; the diff contains only the intended bilingual rewrite.

  Verify local targets with:

  ```powershell
  @('README.md', 'README.zh-CN.md', 'LICENSE', 'docs/github-base-refactor-design.md', 'src-tauri/icons/icon.png') | ForEach-Object { if (-not (Test-Path -LiteralPath $_)) { throw "Missing README target: $_" } }
  ```

  Verify external targets with:

  ```powershell
  $urls = @(
    'https://github.com/TreatTrick/skill-sync/releases/latest',
    'https://github.com/apps/tt-skills-sync/installations/new',
    'https://github.com/TreatTrick/skill-sync/issues',
    'https://github.com/TreatTrick/skill-sync/pulls',
    'https://img.shields.io/github/v/release/TreatTrick/skill-sync?display_name=tag&sort=semver',
    'https://img.shields.io/github/license/TreatTrick/skill-sync',
    'https://img.shields.io/badge/platform-Windows%20x64-0078D4'
  )
  $urls | ForEach-Object {
    $response = Invoke-WebRequest -Uri $_ -Method Head -MaximumRedirection 5
    if ($response.StatusCode -lt 200 -or $response.StatusCode -ge 400) { throw "README URL failed: $_ ($($response.StatusCode))" }
  }
  ```

  Expected: all local targets exist and every external endpoint resolves to a 2xx or 3xx response.

- [ ] **Step 2: Run the complete repository checks**

  Run each command independently and require exit code 0:

  ```bash
  npm run typecheck
  npm run format:check
  npm run lint
  npm run build
  npm run rust:fmt:check
  npm run rust:clippy
  npm run rust:test
  ```

- [ ] **Step 3: Commit the README implementation**

  Run:

  ```bash
  git add README.md README.zh-CN.md docs/superpowers/plans/2026-07-13-readme-refresh.md
  git commit -m "docs: rewrite project readme"
  ```

  Expected: commitlint and pre-commit hooks pass, and the new commit contains only the two README files and this implementation plan.

### Task 4: Apply The Push Safety Gate

**Files:**

- Inspect: Git refs and commits only

- [ ] **Step 1: Fetch and inspect local-only commits**

  Run:

  ```bash
  git fetch origin
  git status --short --branch
  git log --oneline origin/main..HEAD
  git merge-base --is-ancestor origin/main HEAD
  ```

  Expected: the worktree is clean, `origin/main` is an ancestor of `HEAD`, and every local-only commit belongs to the README implementation. Commit `737d1c0 fix: align release rust toolchain` is already present on both local and remote `main` before implementation and therefore is not part of this push. If fetched refs show a different state, or any unrelated local-only commit appears, stop without pushing or rewriting history.

- [ ] **Step 2: Push normally only after the gate passes**

  Run:

  ```bash
  git push origin main
  git fetch origin
  git rev-parse HEAD
  git rev-parse origin/main
  ```

  Expected: a normal non-force push succeeds and the two final SHA values are identical. Never use `--force` or rewrite the user's commit.
