# GitHub Star Growth Design

## Goal

Improve Skill Sync's passive GitHub discovery and conversion without requiring ongoing personal-account content or a promotional GIF.

## Scope

The work has three parts:

1. Complete the repository presentation with the screenshots already published in v0.1.3, a concise GitHub description, and relevant repository topics.
2. Submit Skill Sync only to established Agent Skills, Codex, and Claude Code resource lists whose contribution rules explicitly accept ecosystem tools.
3. Add a low-pressure, bilingual GitHub support entry to the application settings page.

Continuous social posting, paid promotion, fake stars, a marketing site, and a demo GIF are outside this scope.

## Repository Presentation

The localized screenshots in `docs/images/` and their prominent README sections are already complete in v0.1.3 and must not be duplicated or reordered.

Use this repository description:

> Local-first desktop app that syncs Codex, Claude Code, and Agent Skills across machines through your private GitHub repository.

Use focused GitHub topics that match implemented behavior and likely searches: `agent-skills`, `codex`, `claude-code`, `ai-agents`, `skill-management`, `sync`, `local-first`, `desktop-app`, `tauri`, `svelte`, `rust`, and `github`.

Do not set a homepage URL until the project has a real product site. Pointing the homepage field back to the repository or Release page adds no useful navigation.

## Directory Submissions

Candidate repositories must be checked individually before editing. A candidate is eligible only when its current README or contribution guide has a category for ecosystem tools, desktop applications, managers, or related resources. Lists limited to actual Skill packages are not eligible.

Each submission should add one concise English entry linking directly to `https://github.com/TreatTrick/skill-sync` and describe the concrete value: local-first multi-machine sync for Codex, Claude Code, and Agent Skills through a user-owned private GitHub repository. Follow each target's sorting, punctuation, and pull-request template exactly.

Submit to a small number of high-relevance lists rather than many loosely related lists. If authentication or repository permissions prevent creating a pull request, prepare the exact patch and submission text locally and report the blocked external step instead of weakening credentials or bypassing protections.

## In-App GitHub Entry

Add an unframed support row or a single settings card at the bottom of the configured Settings page, matching the existing dense workspace layout. The Chinese copy should be direct and low pressure:

- Title: `支持 Skill Sync`
- Description: `如果 Skill Sync 对你有帮助，可以在 GitHub 上为项目点个 Star。`
- Action: `在 GitHub 上支持`

The English copy should carry the same meaning:

- Title: `Support Skill Sync`
- Description: `If Skill Sync is useful to you, you can support the project with a Star on GitHub.`
- Action: `Support on GitHub`

The action uses a Lucide GitHub icon and opens the fixed project repository URL in the system browser. The backend command accepts no caller-provided URL and reuses the existing platform open implementation, so the UI cannot turn it into an arbitrary URL launcher. Opening failures use the page's existing normalized error toast behavior.

The entry is shown only in the configured Settings workspace. No modal, startup prompt, repeated toast, or automatic browser opening is allowed.

## Verification

- Confirm the public GitHub API reports the intended description and topics.
- Confirm both README files still reference both localized screenshots.
- Validate every external list against its current contribution rules and review each outgoing diff before submission.
- Run `npm run lint:responsive`, `npm run lint:colors`, and `npm run lint:i18n` after the settings UI change.
- Before committing, run `npm run typecheck`, `npm run format:check`, `npm run lint`, and `npm run build`.
- Run focused Rust formatting, lint, and tests for the new Tauri command, then complete the repository's Rust checks if backend code changes.
