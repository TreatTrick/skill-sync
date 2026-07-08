# AGENTS.md

## General Development Rules

1. Prefer existing repository patterns. Use this document when the codebase has no stricter rule.
2. Preserve existing comments. If you need to add comments, write them in Chinese.
3. Keep changes focused. Do not perform broad unrelated refactors.
4. Use `rg` / `rg --files` first when searching.
5. Use `apply_patch` for manual file edits.

## AI Collaboration Rules

- Prefer implementing the user's current request directly. Ask only when missing information makes a safe assumption risky.
- Do not delete user code, comments, or uncommitted changes. If unrelated changes exist, leave them alone.
- If the user explicitly says not to write tests, do not add tests. Otherwise choose the smallest verification that matches the risk.
- After UI, layout, color, or copy changes, run the matching lint command. Before committing, run `npm run lint` and `npm run build`.
- Do not add empty abstractions, thin components, thin helpers, or functions that only forward runes state reads or setters.
- Report changed files and verification commands. Do not claim unverified success.

## Git Commit Rules

- Commit messages must follow Conventional Commits, for example `feat: add user filter`, `fix: handle empty response`, or `docs: update usage notes`.
- Allowed types are `build`, `chore`, `ci`, `docs`, `feat`, `fix`, `refactor`, `revert`, and `test`.
- Commit subjects must not use Start Case, PascalCase, or all caps.
- The project uses a `husky` `commit-msg` hook that runs `commitlint`.
- The project uses a `husky` `pre-commit` hook that runs `npm run precommit` before each commit.
- `npm run precommit` runs `lint-staged` and checks only staged files.
- `lint-staged` formats staged files with `prettier --ignore-unknown --write`, and runs staged source lint checks for matching `src` files; do not hand-format around the configured Prettier rules.

## Anti-Thin-Wrapper Rule

- Do not create wrappers that only forward simple runes state reads or setter calls.
- Add a wrapper only when it contains real semantics such as business rules, defaults, validation, unit conversion, error handling, telemetry, cross-state coordination, or stable reuse from 3+ call sites.
- If a function only does `return state.xxx` or `state.setXxx(value)`, inline the state access at the use site.
- When reading state only to apply a fallback, prefer keeping the fallback explicit at the use site, for example `const value = stateValue ?? 0`.
- Before adding any helper, confirm that it adds meaning rather than just another name.

## Svelte Architecture

Use `Svelte 5 + SvelteKit + TypeScript` with lightweight domain-oriented modules. The goal is practical frontend engineering: routing, components, forms, async requests, dialogs, and state management without copying backend DDD layers into the frontend.

### Principles

- Organize by product capability instead of piling everything into generic `components`, `hooks`, and `utils` folders.
- Frontend rules should handle interaction, display, input constraints, and light permission hints. Final business authority belongs to backend responses.
- Do not create empty Entity, Aggregate, Repository, or Domain Service abstractions for architecture theater.

### Recommended Structure

```text
src/
  routes/              # SvelteKit file routing — assembly only, no business logic
    +layout.svelte
    app/
      <module>/+page.svelte
  app/
    router/
    providers/
    layouts/
  shared/
    ui/
    lib/
    schemas/
    state/             # runes state in *.svelte.ts
    i18n/
    theme/
  modules/
    <module-name>/
      pages/
      components/
      api/
      schemas/
      state/           # module-local runes state in *.svelte.ts
      lib/
      types/
```

### Directory Responsibilities

- `src/routes`: SvelteKit file routing only. Each `+page.svelte` is a thin wrapper rendering a module page; routes carry no business logic.
- `src/app`: application assembly only, including route config, global providers, layouts, and error boundaries.
- `src/shared`: stable cross-module foundations such as UI primitives, request clients, date helpers, base zod helpers, and global runes state.
- `src/modules/<module-name>`: private pages, components, API functions, schemas, state, business helpers, and types for one product module.

### Module Boundaries

- Modules must not deep-import another module's private components, state, helpers, or API functions.
- When one module needs another capability, prefer page composition, route params, backend APIs, or a stable capability promoted to `shared`.
- Move code to `shared` only after the concept is stable and reused across modules.
- Do not let `shared` become a junk drawer.
- Layer imports must follow `routes -> app -> modules -> shared`. Lower layers must not import higher layers, including through relative imports.
- Avoid circular imports and self imports; ESLint checks both.
- Do not use `export *` or `export * as namespace`; explicitly name each exported member.
- ESLint uses `eslint-plugin-import-x` with the TypeScript resolver, so architecture violations may appear as import-cycle, self-import, deep-import, or reverse-layer errors.

## State Management

- Prefer `@tanstack/svelte-query` for server data: lists, details, filtered results, audit logs, and similar remote state. Use `createQuery` / `createMutation` with an accessor function (`() => ({ ... })`) so options stay reactive.
- Use Svelte 5 runes (`.svelte.ts` modules exporting a state class instance) only for small client-only state such as current theme, language, sidebar collapse, or temporary cross-page selections.
- Keep form state local to the page with `$state`; do not push it into shared runes state.
- Prefer URL query or route state for filters, pagination, and tabs that should survive refreshes or be shareable.
- Follow the anti-thin-wrapper rule for runes state reads and writes.

## API And Data Validation

- Validate server responses with zod before rendering or business handling.
- Place module-specific API functions in `modules/<module-name>/api`.
- API functions should handle requests, response validation, and error normalization. They should not carry complex UI state.
- Map backend enum values to UI-facing labels or styles inside the owning module.

## UI Rules

- Prefer shadcn-svelte components in `src/shared/ui/<component>/` for new UI; add new ones with `npx shadcn-svelte@latest add <name>`. Hand-build only when no shadcn equivalent exists (Spinner, EmptyState, StatusBadge). Use Tailwind CSS with `@lucide/svelte`. Do not introduce React component libraries.
- Dashboard and workspace screens should be dense, scannable, and task-focused, not marketing landing pages.
- Split components for business readability and real reuse, not for every HTML fragment.
- Tables, filters, pagination, dialogs, drawers, and confirmations can become clear components, but do not create a generic cross-module table framework before stable reuse exists.

## Tailwind Responsive Rules

- Tailwind CSS v4 is configured through `src/index.css` with `@import "tailwindcss";`.
- Global CSS should contain theme variables, fonts, background, and box model basics. Do not set page-level fixed widths globally.
- Do not set fixed minimum widths on `body`, app root containers, or the SvelteKit `src/app.html` shell.
- Use mobile-first responsive classes: default for small screens, then `sm:`, `md:`, `lg:`, `xl:`, `2xl:`.
- Grids must define a narrow-screen default, for example `grid-cols-1 sm:grid-cols-2 xl:grid-cols-4`.
- Tables must have a narrow-screen fallback: wrap with `overflow-x-auto` and set a local `min-w-[...]`.
- Filter bars, headers, and button groups must be allowed to wrap or stack.
- Do not use arbitrary text sizes such as `text-[22px]`, `text-[1.375rem]`, or `text-[vw]`. Prefer stable Tailwind sizes such as `text-sm`, `text-base`, `text-xl`, and `text-2xl`.
- Do not use arbitrary radius classes such as `rounded-[13px]`. Prefer standard radius tokens such as `rounded-md`, `rounded-lg`, and `rounded-full`, or define a semantic token first.
- Large arbitrary `w-[...]`, `h-[...]`, `min-w-[...]`, `min-h-[...]`, `max-w-[...]`, and `max-h-[...]` classes must have clear layout meaning, a local overflow fallback, or a responsive breakpoint. Local table `min-w-[...]` fallbacks are allowed; page root containers should not use them.
- After layout or Tailwind class changes, run `npm run lint:responsive`.

## Tailwind Color Token Rules

- Maintain colors in `src/index.css`: define semantic CSS variables in `:root`, then expose them through `@theme inline`.
- Components should use semantic classes such as `bg-background`, `bg-surface`, `text-foreground`, `text-muted-foreground`, `border-border`, `bg-primary`, `text-warning`, and `text-destructive`.
- Do not use direct Tailwind palette classes in components, such as `bg-slate-100`, `text-teal-800`, `border-red-200`, `bg-white`, or `text-black`.
- Do not hardcode literal colors such as `#fff`, `#ffffff`, `rgb(...)`, `rgba(...)`, or `hsl(...)` outside token definitions.
- After color or UI class changes, run `npm run lint:colors`.

## i18n Copy Rules

- Put user-facing Chinese copy in `src/shared/i18n/locales`.
- Components must read UI copy with `t('...')`, including buttons, table headers, filter options, status labels, empty states, placeholders, nav titles, and hints.
- Chinese translations are allowed in `src/shared/i18n`; other source files (`.ts`, `.svelte`) should add locale keys first.
- After UI copy changes, run `npm run lint:i18n`.

## Naming

- Business module directories use kebab-case.
- shadcn-svelte UI components use lowercase per shadcn convention (for example `src/shared/ui/button/button.svelte`); other Svelte component files use PascalCase.
- Regular TypeScript files use camelCase.
- Types and interfaces use PascalCase; do not prefix interfaces with `I`.
- Constants use SCREAMING_SNAKE_CASE.

## Pre-Completion Checks

```bash
npm run typecheck
npm run format:check
npm run lint
npm run build
```
