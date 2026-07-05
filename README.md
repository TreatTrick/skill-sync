# React Frontend App

This project was generated with the React Frontend Scaffold skill. It is a production-minded React TypeScript starting point with architecture rules, lint checks, AI collaboration guidance, and a tiny demo dashboard.

## Quick Start

```bash
npm install
npm run dev
```

Before committing, run:

```bash
npm run lint
npm run build
```

## Scripts

| Command                   | Purpose                                                                 |
| ------------------------- | ----------------------------------------------------------------------- |
| `npm run dev`             | Start the Vite dev server                                               |
| `npm run build`           | Type-check and build the production bundle                              |
| `npm run typecheck`       | Run TypeScript project references with `tsc -b`                         |
| `npm run lint`            | Run typecheck, ESLint, responsive lint, color-token lint, and i18n lint |
| `npm run lint:responsive` | Detect risky responsive layout patterns                                 |
| `npm run lint:colors`     | Detect direct Tailwind palette colors and literal colors outside tokens |
| `npm run lint:i18n`       | Detect hardcoded Chinese UI copy outside locale files                   |
| `npm run precommit`       | Run lint-staged formatting and then the full lint pipeline              |
| `npm run format`          | Format the project with Prettier                                        |
| `npm run format:check`    | Check Prettier formatting without writing files                         |

## Architecture Rules

- Organize product code under `src/app`, `src/shared`, and `src/modules`.
- Do not deep-import another module's private components, stores, API calls, or helpers.
- Keep layer imports flowing `app -> modules -> shared`; lower layers must not import higher layers.
- Avoid circular imports, self imports, and `export *` barrels.
- Use explicit named exports in barrels and import another module only through its root entry.
- Put server state in TanStack Query first; keep Zustand for small client-only state.
- Validate external data with zod before rendering or business handling.
- Use semantic Tailwind color tokens from `src/index.css`; do not use direct palette colors in components.
- Put user-facing Chinese copy in `src/shared/i18n/locales` and read it with `t(...)`.
- Give complex tables a local horizontal scroll fallback on narrow screens.
- Avoid arbitrary Tailwind text sizes and radius classes. Use stable text and radius tokens.
- Large arbitrary width/height/min/max sizing classes need clear layout meaning, responsive variants, or a local overflow fallback.

## AI Collaboration Rules

AI agents working in this repo must:

- Read `AGENTS.md` first; Claude Code should also read `CLAUDE.md`.
- Preserve existing comments. If new comments are necessary, write them in Chinese.
- Never overwrite, revert, or delete user changes.
- Use `apply_patch` for manual file edits.
- Prefer `rg` and `rg --files` when searching.
- Avoid empty abstractions, thin helpers, thin components, and helpers that only forward Zustand `getState()` or setters.
- Run the relevant lint commands after UI, layout, color, or copy changes; run `npm run lint` and `npm run build` before reporting completion.

## Git Commits

Commit messages must follow Conventional Commits. Allowed types:

```text
build, chore, ci, docs, feat, fix, refactor, revert, test
```

Examples:

```bash
git commit -m "feat: add dashboard filter"
git commit -m "fix: handle empty response"
```

Husky runs `npm run precommit` in `pre-commit` and `commitlint` in `commit-msg`.
