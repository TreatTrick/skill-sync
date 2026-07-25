# Skill Sync GitHub Homepage

## Goal

A concise, bilingual (简体中文 / English), light/dark landing page for the Skill Sync repository, built with Svelte (SvelteKit), compiled to static assets, deployed to GitHub Pages, and set as the repo's Homepage URL. It highlights the project's biggest advantages (local-first GitHub Vault, preview-before-apply, explicit conflict choices, no Git/PAT/SSH setup) and links to the v1.0.0 installers.

## Architecture decision: a separate `site/` SvelteKit app

The existing app is a Tauri desktop SPA (`adapter-static` -> `dist/`, served by the WebView, and its routes assume Tauri APIs). It cannot double as a public website, and bundling a marketing page into the desktop app would bloat the Tauri build and ship Tauri-only code to a browser.

So the homepage lives in a **self-contained `site/` subdirectory** with its own `package.json`, configs, and build. It reuses the project's _visual identity_ (teal primary, slate neutrals, GitHub-like dark mode) and _i18n discipline_ (copy externalized into locale files, read via `t(...)`), but is fully decoupled from the Tauri app's toolchain.

## Isolation from root tooling (root `npm run lint` / `npm run build` stay green and untouched)

- Root `eslint.config.js`: add `site` to `globalIgnores([...])` so root eslint skips the site. (The custom layer/import rules already no-op outside `src/`, but the shared `no-restricted-syntax` / `import-x` rules would otherwise fire on site files using their own `@/` alias.)
- Root `.prettierignore`: add `site/` (the site carries its own `.prettierrc.json`, mirrored from root).
- Root `.gitignore`: add `site/node_modules`, `site/build`, `site/.svelte-kit`.
- Root `tsconfig.json` / `svelte-check`: unaffected - the root tsconfig extends `.svelte-kit/tsconfig.json`, which is scoped to the root SvelteKit project; the site has its own `tsconfig.json`.
- Root lint scripts (`check-i18n-literals`, `check-color-tokens`, `check-responsive-layout`) scan `SOURCE_DIRS = ['src']` only, so `site/src` is never touched.

## Site structure

```text
site/
  package.json            # svelte, @sveltejs/kit, @sveltejs/adapter-static, @tailwindcss/vite, tailwindcss, vite, typescript
  package-lock.json       # generated via npm install
  svelte.config.js        # adapter-static + prerender; kit.paths.base = process.env.PAGES_BASE ?? '/skill-sync'
  vite.config.ts          # sveltekit() + tailwindcss(); alias @ -> src
  tsconfig.json           # extends .svelte-kit/tsconfig.json
  eslint.config.js        # minimal svelte flat/recommended + tseslint recommended (site-local)
  .prettierrc.json        # mirror root
  .gitignore              # node_modules, build, .svelte-kit
  static/
    icon.png              # copied from src-tauri/icons/icon.png
  src/
    app.html              # %sveltekit% + inline no-FOUC theme bootstrap script
    app.css               # color tokens (:root light / .dark dark) mirroring src/index.css
    app.d.ts
    lib/
      i18n.svelte.ts      # rune locale state; t(key); persist localStorage; default navigator.language
      theme.svelte.ts     # rune theme state ('light'|'dark'); persist; default prefers-color-scheme
      locales/
        en.ts
        zh.ts
      components/
        Header.svelte
        Hero.svelte
        FeatureCards.svelte
        HowItWorks.svelte
        DownloadSection.svelte
        Footer.svelte
        ThemeToggle.svelte
        LangToggle.svelte
    routes/
      +layout.ts          # export const prerender = true
      +layout.svelte      # <html> theme class + lang attr; Header; slot; Footer
      +page.svelte        # composes Hero / FeatureCards / HowItWorks / DownloadSection
```

## Content (concise, single page)

All copy lives in `src/lib/locales/{en,zh}.ts` and is read with `t('...')`. No Chinese literals in components (mirrors the app's i18n rule).

**Hero**

- Logo (`icon.png`) + "Skill Sync"
- EN headline: "Keep your AI agent Skills in sync across machines."
- EN subhead: "A local-first GitHub Vault for Codex, Claude Code, and agent Skills. No SaaS, no telemetry, no Git/PAT/SSH setup."
- CTAs: **Download** (-> latest release), **View on GitHub**
- Platform line: Windows x64 · Apple Silicon

**Feature cards** (the four biggest advantages, from README "Highlights")

1. Local-first & private - Skills stay in their tool directories; sync through a GitHub repo you control (private recommended). No SaaS, no sync server, no telemetry; credentials in the OS keyring.
2. Preview before apply - review uploads, downloads, conflicts, and proposed deletes before anything changes.
3. Explicit conflict choices - three-way `base` / `local` / `remote` content-hash comparison; you keep local, use remote, or skip.
4. No Git, PAT, or SSH - authorize with the GitHub App Device Flow. No Git CLI, no PAT, no SSH keys.

**How it works** (3 steps)

1. Authorize with GitHub (Device Flow).
2. Bind one repository as your Vault (private recommended).
3. Preview the plan, resolve conflicts, apply.

**Download** - installer table reusing the README v1.0.0 links: Win x64 EXE, Win x64 MSI, Apple Silicon DMG (-> releases page for more).

**Footer** - MIT © 2026 TreatTrick · GitHub · Releases · License.

## Light / dark mode

- `app.css` defines CSS variables on `:root` (light) and `.dark` (dark) - same token names and values as the app's `src/index.css` (teal `--primary`, slate neutrals, dark `#0d1117`).
- `theme.svelte.ts`: rune state; persisted to `localStorage('skill-sync-site-theme')`; defaults to `prefers-color-scheme`.
- Inline script in `app.html` reads localStorage / media query and sets `document.documentElement.classList` **before paint** (no FOUC).
- `ThemeToggle.svelte`: sun/moon icon button (inline SVG, no extra dep).

## i18n (zh / en)

- `i18n.svelte.ts`: rune `locale` state; persisted to `localStorage('skill-sync-site-lang')`; defaults to `navigator.language` (`zh-CN` -> `zh`, else `en`).
- `t(key)`: returns the string for the current locale; components calling `t()` re-render on locale switch (rune dependency), mirroring the app's `t()` wrapper.
- `LangToggle.svelte`: EN / 中 switch.
- `<html lang>` is updated reactively (`'zh'` / `'en'`).

## Build & deploy to GitHub Pages

- `site/svelte.config.js`: `adapter-static` (prerendered single page); `kit.paths.base = process.env.PAGES_BASE ?? '/skill-sync'` (project-pages URL `https://treattrick.github.io/skill-sync/`). The env override lets a future custom domain set `PAGES_BASE=/`.
- `cd site && npm install && npm run build` -> `site/build/` (static `index.html` + prefixed assets).
- **New workflow** `.github/workflows/pages.yml`:
  - `on`: push to `main` with `paths: ['site/**', '.github/workflows/pages.yml']` + `workflow_dispatch`.
  - `permissions`: `pages: write`, `id-token: write`.
  - `concurrency: { group: pages, cancel-in-progress: false }`.
  - Steps: checkout@v4 -> setup-node@v4 (20) -> `working-directory: site`, `npm ci` -> `npm run build` -> `actions/upload-pages-artifact@v3` (`path: site/build`) -> `actions/deploy-pages@v4`.
- **One-time manual setting** (documented, not automatable): repo Settings -> Pages -> Source = "GitHub Actions".

## Set as repo Homepage URL (outward-facing - will confirm before doing)

After the first successful deploy, set the repo "Homepage" field to `https://treattrick.github.io/skill-sync/`. Two options; I will **not** do this without confirmation:

- Manual: repo Settings -> About -> Homepage.
- API: `PATCH /repos/TreatTrick/skill-sync` `{"homepage":"..."}` using a token from `git credential fill` (no `gh` CLI in this environment).

## Verification

- `cd site && npm install && npm run build` succeeds; `site/build/index.html` exists with `/skill-sync/`-prefixed asset paths.
- `npm run dev` (in site) renders; toggle zh/en and light/dark; confirm no FOUC on reload; `<html lang>` and `.dark` class update.
- Root checks unaffected: `npm run lint` and `npm run build` (Tauri app) still pass with `site/` present.
- Push triggers `pages.yml`; workflow green; site live at the Pages URL.

## Out of scope

- No changes to the Tauri app (`src/`, `src-tauri/`) or its READMEs.
- No custom domain (project pages default; base is env-overridable for later).
- No analytics / telemetry (consistent with the project's no-telemetry stance).
