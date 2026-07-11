# Knip Review Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a standalone Knip review command and make the existing full lint command enforce it.

**Architecture:** Keep Knip as a project-level static analysis tool configured by one root `knip.json`. Package scripts expose the direct check and compose it once into the existing lint chain; the lockfile pins the installed Knip 6 release. Existing application code remains untouched unless the first scan reports a real unused item and the user separately approves cleanup.

**Tech Stack:** npm, Knip 6, SvelteKit, Svelte 5, TypeScript

---

## Chunk 1: Knip configuration and lint integration

### Task 1: Add the Knip review

**Files:**

- Create: `knip.json`
- Modify: `package.json`
- Modify: `package-lock.json`

- [ ] **Step 1: Verify the review command is initially absent**

Run:

```bash
npm run lint:knip
```

Expected: FAIL with `Missing script: "lint:knip"`. This is the red check proving the new command does not already exist.

- [ ] **Step 2: Install the Knip 6 development dependency**

Run:

```bash
npm install --save-dev knip@6
```

Expected: `package.json` contains a `knip` entry in `devDependencies`, and `package-lock.json` records the resolved Knip 6 package without changing application dependencies.

- [ ] **Step 3: Create the project-level Knip configuration**

Create `knip.json` with exactly this initial configuration:

```json
{
  "$schema": "https://unpkg.com/knip@6/schema.json",
  "entry": [
    "src/routes/**/+*.{js,ts,svelte}",
    "svelte.config.js",
    "vite.config.ts",
    "eslint.config.js",
    "scripts/*.mjs"
  ],
  "project": ["src/**/*.{js,ts,svelte}", "scripts/**/*.mjs"]
}
```

Do not add `ignore`, `ignoreDependencies`, `ignoreExportsUsedInFile`, or other suppressions. The globs intentionally exclude generated output, static assets, and Rust sources.

- [ ] **Step 4: Add the package scripts**

Add the standalone script to `package.json`:

```json
"lint:knip": "knip"
```

Append it exactly once to the existing full lint chain:

```json
"lint": "npm run typecheck && eslint . && npm run lint:responsive && npm run lint:colors && npm run lint:i18n && npm run lint:knip"
```

Do not add Knip to `lint-staged` or `precommit` because Knip requires the complete project graph.

- [ ] **Step 5: Run the direct Knip review**

Run:

```bash
npm run lint:knip
```

Expected: PASS with exit code 0 and no findings.

If Knip reports an implicit framework or tool entry, identify its actual caller, add only the exact missing entry glob, and rerun `npm run lint:knip`. Repeat only for proven entries until the command exits 0. If the reference cannot be expressed as an entry, or Knip reports a genuinely unused file, export, or dependency, stop without deleting or suppressing it and report the finding to the user for a separate scope decision and specification confirmation.

- [ ] **Step 6: Verify script composition**

Run:

```bash
npm pkg get scripts.lint scripts.lint:knip
```

Expected: `lint` ends with one `npm run lint:knip`, and `lint:knip` is `knip`.

- [ ] **Step 7: Run type checking**

Run:

```bash
npm run typecheck
```

Expected: exit 0 with no Svelte or TypeScript errors.

- [ ] **Step 8: Run the format check**

Run:

```bash
npm run format:check
```

Expected: exit 0. If it reports a verified, unrelated baseline failure, record the affected files and do not modify them.

- [ ] **Step 9: Run the full lint chain**

Run:

```bash
npm run lint
```

Expected: exit 0; full lint invokes `lint:knip` exactly once and Knip exits 0.

- [ ] **Step 10: Run the production build**

Run:

```bash
npm run build
```

Expected: exit 0 with the static adapter writing the production output.

- [ ] **Step 11: Review the working-tree diff and configuration**

First verify that no user files are already staged:

```bash
git diff --cached --quiet
```

Expected: exit 0. If it exits nonzero, stop and do not stage or commit until the pre-existing staged changes are handled by the user.

Then run:

```bash
git diff --check -- package.json package-lock.json
git diff -- package.json package-lock.json
Get-Content -Raw knip.json
```

Expected: no scoped whitespace errors; the tracked diff contains only the Knip dependency, scripts, and lockfile records; the displayed `knip.json` exactly matches Step 3. Preserve all unrelated working-tree changes.

- [ ] **Step 12: Stage and inspect only the implementation files**

```bash
git add package.json package-lock.json knip.json
git diff --cached --check
git diff --cached --name-only
git diff --cached -- package.json package-lock.json knip.json
```

Expected: the staged file list contains exactly `knip.json`, `package-lock.json`, and `package.json`; the staged diff has no whitespace errors and matches the reviewed implementation.

- [ ] **Step 13: Commit the implementation**

```bash
git commit -m "build: add knip review"
```
