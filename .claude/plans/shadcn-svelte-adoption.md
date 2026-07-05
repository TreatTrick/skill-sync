# Adopt shadcn-svelte

## Goal

Replace the hand-built Svelte UI primitives with shadcn-svelte (Svelte 5 + Tailwind v4 + bits-ui), keep the project's `routes -> app -> modules -> shared` architecture and the existing rich token system, and update docs to prefer shadcn-svelte.

## Decisions (confirmed with user)

- **Stock shadcn-svelte component API**; rewrite page usage where APIs differ (CardHeader title/description/action → CardHeader + CardTitle + CardDescription + action snippet; CardBody → CardContent).
- **Extend shadcn Button** with `loading` + `icon` snippet props (common shadcn customization) so the 7 pages' Button usage stays unchanged.
- **Tokens**: add shadcn token aliases to `src/index.css` mapping to existing values. Preserves `surface` / `surface-hover` / `surface-muted` / `primary-muted` / `warning` / `success` / `strong-foreground` etc. No page token-class changes.
- **Location**: `src/shared/ui/<component>/` subfolders (shadcn convention), aliased via `components.json` to `@/shared/ui` + `@/shared/lib/utils`. Keeps the layer rules; no `src/lib` introduced.

## Components

**Replace** (stock shadcn-svelte source, adapted to import `cn` from `@/shared/lib/utils` and resolve via token aliases):

- `button/Button.svelte` + `index.ts` (extended: + `loading`, + `icon` snippet; exports `buttonVariants`)
- `card/{Card,CardHeader,CardTitle,CardDescription,CardContent,CardFooter}.svelte` + `index.ts`
- `input/{Input.svelte,index.ts}`
- `textarea/{Textarea.svelte,index.ts}`
- `checkbox/{Checkbox.svelte,index.ts}` (bits-ui)
- `badge/{Badge.svelte,index.ts}` (exports `badgeVariants`)

**Delete** old flat files: `Button.svelte`, `Card.svelte`, `CardBody.svelte`, `CardHeader.svelte`, `Input.svelte`, `Textarea.svelte`, `Checkbox.svelte`, `Badge.svelte`.

**Keep custom** (no shadcn equivalent): `Spinner.svelte` (lucide Loader2), `EmptyState.svelte` (composite, rebuilt on Card), `StatusBadge.svelte` (custom; used for tone badges). Move into `src/shared/ui/spinner/`, `empty-state/`, `status-badge/` subfolders for consistency, re-export via barrel.

## Token aliases (add to `src/index.css` `:root` + `.dark` + `@theme inline`)

Map shadcn tokens to existing values (light + dark):

- `--card` / `--card-foreground` ← `--surface` / `--foreground`
- `--popover` / `--popover-foreground` ← `--surface` / `--foreground`
- `--secondary` / `--secondary-foreground` ← `--surface-muted` / `--foreground`
- `--muted` ← `--surface-muted` (bg-muted)
- `--accent` / `--accent-foreground` ← `--surface-hover` / `--foreground`
- `--input` ← `--border` (border-input)
- `--ring` already exists
- `--radius-sm/md/lg` ← standard (0.5rem/0.6rem/0.75rem) if shadcn components use `rounded-md` etc. (Tailwind defaults likely suffice — add only if needed)

## Page changes (7 pages: Backups, Skills, Conflicts, Dashboard, Onboarding, Settings, SyncPreview)

- **CardHeader**: `<CardHeader title=.. description=.. action={...}>` → `<CardHeader><CardTitle>..</CardTitle>{#if description}<CardDescription>..</CardDescription>{/if}{#snippet action()}..{/snippet}</CardHeader>` (or action as children).
- **CardBody** → **CardContent** (rename only; same `class` prop).
- **Badge**: variant `default`/`warning`/`destructive` → shadcn `default`/`secondary`/`destructive`/`outline`. `warning` variant (Conflicts/Sync) → keep using `StatusBadge` (already custom, tone="warning") OR Badge with outline — decide per page (prefer StatusBadge for tone semantics).
- **Button**: unchanged (loading/icon preserved via extension).
- **Input/Textarea/Checkbox**: `bind:value` / `bind:checked` preserved (shadcn supports bindable).
- **EmptyState**: keep API (icon/title/description/action snippets); rebuild internals on Card if it currently uses Card.

## Dependencies

Install: `bits-ui`, `tailwind-variants`. Already present: `clsx`, `tailwind-merge`, `@lucide/svelte`, `@tailwindcss/vite`, Tailwind v4.

## components.json (Svelte, new)

```json
{
  "$schema": "https://shadcn-svelte.com/schema.json",
  "style": "default",
  "tailwind": {
    "css": "src/index.css",
    "baseColor": "slate",
    "cssVariables": true,
    "prefix": ""
  },
  "aliases": {
    "components": "@/shared/ui",
    "utils": "@/shared/lib/utils",
    "ui": "@/shared/ui",
    "lib": "@/shared/lib",
    "hooks": "@/shared/hooks"
  },
  "typescript": true,
  "registry": "https://shadcn-svelte.com/registry"
}
```

Enables future `npx shadcn-svelte@latest add <component>` (will land in `src/shared/ui/<component>/`). Components are hand-authored per shadcn-svelte canonical source (CLI `init` would clobber `src/index.css`, so avoided).

## Docs

- **AGENTS.md** "UI Rules": change to "Prefer shadcn-svelte components for new UI; run `npx shadcn-svelte add <name>` to add. Hand-build only when no shadcn equivalent exists (Spinner, EmptyState, StatusBadge). Do not introduce React component libraries."
- **CLAUDE.md**: add a rule "Prefer shadcn-svelte components from `src/shared/ui`; hand-build only when no equivalent."
- **README.md** Stack: add `shadcn-svelte` + `bits-ui`.

## Files

- **New**: `src/shared/ui/{button,card,input,textarea,checkbox,badge,spinner,empty-state,status-badge}/*.svelte` + `index.ts`; `components.json`.
- **Delete**: flat `src/shared/ui/{Button,Card,CardBody,CardHeader,Input,Textarea,Checkbox,Badge}.svelte`.
- **Modify**: `src/index.css` (token aliases), `src/shared/ui/index.ts` (barrel re-exports), 7 page `.svelte` (CardHeader/CardBody/Badge usage), `AGENTS.md`, `CLAUDE.md`, `README.md`.

## Verification

`npm run lint` (typecheck + eslint + responsive + colors + i18n) and `npm run build` green. Then `npm run tauri dev` visual check (all 7 pages render, theme/language switch, no style regressions).

## Risks / watchpoints

- shadcn-svelte components hardcode token classes (`bg-card`, `bg-accent`, `bg-muted`, `border-input`, `bg-secondary`, `bg-popover`, `ring-ring`); the token aliases must cover every class used — audit each generated component after authoring.
- bits-ui Checkbox `checked` is bindable via its own API — verify `bind:checked` flows (may need `checked = $bindable()` on our wrapper).
- CardHeader composition rewrite touches 6 pages — preserve exact classes/layout (the `flex flex-col gap-3 ... sm:flex-row` etc. moves to CardHeader; title/description classes to CardTitle/CardDescription).
- color lint must accept new semantic tokens (`bg-card`, `bg-accent` are semantic, not palette — should pass; verify).
- shadcn Button default variant uses `bg-primary text-primary-foreground shadow-xs hover:bg-primary/90` — matches current tokens; verify the extended loading/icon don't clash with `disabled` handling.
