<p align="center">
  <img src="src-tauri/icons/icon.png" width="140" alt="Skill Sync logo" />
</p>

<h1 align="center">Skill Sync</h1>

<p align="center">
  <strong>Sync your AI agent skills between machines with a small desktop workspace.</strong>
</p>

<p align="center">
  <a href="README.md">English</a> · <a href="README.zh-CN.md">简体中文</a>
</p>

Skill Sync is a local-first Tauri desktop app for syncing skills through one private GitHub Vault repository. V1 uses the GitHub App Device Flow as its only remote provider. There is no SaaS account, custom sync server, OAuth App, PAT, Git CLI setup, SSH setup, or telemetry.

## How It Works

Before a Vault is bound, the app shows only a guarded Onboarding flow. The flow authorizes GitHub, checks the GitHub App installation, requires exactly one selected repository, lets you choose a branch, and explicitly confirms an empty or missing-manifest initialization commit. Only a successful Ready Vault binding unlocks the workspace, whose navigation contains Sync and Settings.

The remote repository contains only:

```text
manifest.json
blobs/sha256/<sha256>.skill.zip
```

The manifest is validated as a whole. Skill hashes are hashes of canonical ZIP bytes. Local sync state and credentials stay on the machine; credentials are stored in the OS keyring and are refreshed when possible.

V1 scans three fixed user-level roots:

| Namespace     | Root               |
| ------------- | ------------------ |
| `agents`      | `~/.agents/skills` |
| `codex`       | `~/.codex/skills`  |
| `claude-code` | `~/.claude/skills` |

Project-level directories and user-configured roots are outside V1. Deletes are advisory: they are shown in the preview, require explicit selection, and move local directories to the local trash area. Remote deletes remove manifest entries only; blobs are not garbage-collected.

## GitHub App

The app includes the public GitHub App client id and slug. The App needs only Contents read/write and Metadata read-only permissions. Its installation must use selected repositories and contain exactly one Vault repository. Users can adjust or revoke the installation from GitHub; the app never stores a client secret or private key.

Maintainers should follow [GitHub App setup](docs/github-app-setup.md). Users can start with the Onboarding screen after installing the app.

## Build From Source

Prerequisites: Node.js 20+, Rust stable, and the Tauri v2 system dependencies. Git is not required by the app.

```bash
git clone https://github.com/TreatTrick/skill-sync.git
cd skill-sync
npm install

npm run tauri dev
npm run tauri build
```

The release workflow uses the same built-in public App configuration. See the maintainer checklist before producing an installer.

## Development Checks

```bash
npm run typecheck
npm run format:check
npm run lint
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

The backend owns local scanning, canonical ZIP packing, limits, manifest validation, GitHub API access, comparison, delete protection, and journaled local apply. The frontend owns onboarding, display, filters, conflict decisions, and command calls. Frontend API responses are validated with zod before rendering.

## Project Structure

```text
src/
  routes/    # SvelteKit route wrappers
  app/       # app shell, route gate, navigation
  modules/   # onboarding, settings, skills, sync
  shared/    # UI, schemas, i18n, state, request helpers
src-tauri/
  src/       # Rust scanner, packer, vault, GitHub client, sync engine
docs/
  github-base-refactor-design.md
  github-app-setup.md
```

## License

[MIT](LICENSE) © 2026 TreatTrick
