# CLAUDE.md

Follow the same engineering rules defined in `AGENTS.md`. Key reminders:

1. Preserve existing comments. If new comments are necessary, write them in Chinese.
2. Use `apply_patch` for manual file edits.
3. Organize Svelte code under `src/routes`, `src/app`, `src/shared`, and `src/modules`.
4. Do not add wrappers that only forward runes state reads or setter calls.
5. Put user-facing Chinese copy in `src/shared/i18n/locales` and read it with `t(...)`.
6. Use only semantic color tokens exposed by `src/index.css`.
7. Do not use arbitrary Tailwind text sizes, arbitrary radius classes, or large arbitrary sizing classes without responsive or local-overflow guardrails.
8. Keep imports flowing `routes -> app -> modules -> shared`; do not create circular imports, self imports, or `export *` barrels.
9. Let Prettier own formatting; use `npm run format` or rely on `lint-staged`, and check with `npm run format:check` before larger handoffs.
10. After UI or layout changes, run `npm run lint:responsive`, `npm run lint:colors`, and `npm run lint:i18n` as appropriate. Before completion, run `npm run lint` and `npm run build`.
11. Prefer shadcn-svelte components from `src/shared/ui` for UI; add new ones with `npx shadcn-svelte@latest add <name>`. Hand-build only when no shadcn equivalent exists (Spinner, EmptyState, StatusBadge).

## AI Collaboration Rules

- Prefer directly completing the user's request. Ask only when key information is missing and a safe assumption is not possible.
- Do not overwrite or revert user changes.
- If the user explicitly says not to write tests, do not add tests.
- Do not report completion based on assumptions. Report actual verification command output.
- Do not move module-private code into `shared` unless the concept is stable, clearly named, and reused.
