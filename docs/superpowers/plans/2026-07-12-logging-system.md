# Logging System Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Tauri-backed logging system that writes console and date-rotated local files, retains seven days, and records safe error context across Rust and frontend boundaries.

**Architecture:** Register `tauri-plugin-log` during Tauri startup with stdout/debug and application log directory targets. Keep retention and sensitive-field sanitization in a focused Rust logging module, and route frontend command failures through the existing `invokeCmd` boundary. Add source-level logs at backend root-cause boundaries without logging credentials, request bodies, raw response bodies, or skill contents.

**Tech Stack:** Tauri 2, Rust `log` facade, `tauri-plugin-log`, Svelte 5, TypeScript, `@tauri-apps/plugin-log`, Cargo tests, Svelte check, ESLint.

---

## File Map

- Create: `src-tauri/src/logging.rs` for logger initialization, daily file naming/rotation integration, seven-day cleanup, and safe logging helpers.
- Modify: `src-tauri/src/lib.rs` to initialize logging before runtime and command registration.
- Modify: `src-tauri/src/errors.rs` to expose one redacted error representation for logs and Tauri responses.
- Modify: `src-tauri/src/commands.rs` and backend GitHub/config/sync modules to add operation-level error and lifecycle logs.
- Modify: `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` to add `log` and `tauri-plugin-log`.
- Modify: `package.json`, `package-lock.json` to add `@tauri-apps/plugin-log` if frontend log calls are required by the plugin API.
- Modify: `src/shared/lib/tauri.ts` to log command name, error kind, and safe message on failed invokes.
- Modify: `docs/logging-design.md` only if implementation constraints require a design correction.
- Test: `src-tauri/src/logging.rs` unit tests and existing Rust error/backend tests.

## Chunk 1: Dependencies And Logging Core

### Task 1: Add failing retention and redaction tests

**Files:**

- Create: `src-tauri/src/logging.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/errors.rs`

- [ ] **Step 1: Add tests for seven-day cleanup**

Test that a log file older than seven days is removed, a file newer than seven days remains, directories are ignored, and cleanup errors are returned for the caller to log without blocking startup.

- [ ] **Step 2: Add tests for log-safe error output**

Test that access tokens, refresh tokens, bearer values, client secrets, private keys, device codes, and raw JSON token fields do not appear in the string passed to the logger, while the error kind and operation remain available.

- [ ] **Step 3: Run the focused tests and verify failure**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml logging -- --nocapture
```

Expected: compilation/test failure because the logging module and helpers are not implemented yet.

- [ ] **Step 4: Add the minimum logging module API**

Implement `init(app_handle)`, `cleanup_old_logs(log_dir, now)`, a seven-day retention constant, and a `safe_error`/`log_app_error` helper. Keep cleanup deterministic by accepting a clock value in the pure helper. Use the plugin's daily file target/rotation support and application log directory; do not create a parallel custom logger.

- [ ] **Step 5: Register the module and plugin**

Add the dependencies, register `mod logging`, initialize the plugin before `manage(runtime)` and command registration, and run cleanup after the plugin has resolved the application log directory. A cleanup failure emits `warn!` and does not prevent startup.

- [ ] **Step 6: Run focused tests and formatting**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --all
cargo test --manifest-path src-tauri/Cargo.toml logging -- --nocapture
```

Expected: focused tests pass.

## Chunk 2: Rust Error And Operation Instrumentation

### Task 2: Log command and backend failure boundaries

**Files:**

- Modify: `src-tauri/src/errors.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/github_auth.rs`
- Modify: `src-tauri/src/github_credentials.rs`
- Modify: `src-tauri/src/github_repository.rs`
- Modify: `src-tauri/src/github_store.rs`
- Modify: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/local_apply.rs`
- Modify: `src-tauri/src/sync_engine/vault.rs`

- [ ] **Step 1: Add a safe `AppError` log representation**

Reuse the existing redaction rules and add coverage for device/user codes and JSON-style token fields. The log representation must contain only `kind`, operation, and sanitized message; never serialize credential structs or request headers.

- [ ] **Step 2: Log command failures at the Tauri boundary**

Wrap command implementation calls at the command boundary or use a shared helper so errors include the command name and `AppError::kind()`. Do not duplicate logs in both command wrappers and lower layers for the same error.

- [ ] **Step 3: Log GitHub authorization and credential lifecycle**

Log start/poll state, slow-down, denial, expiry, `/user` lookup failure, refresh failure, keyring persistence failure, and reauthorization transitions. Log GitHub login only after successful authorization; never log device code, user code, access token, refresh token, or request form data.

- [ ] **Step 4: Log repository/store and local state failures**

Log operation name, HTTP status, retry-after, discovery status, manifest/branch status, commit outcome, config path category, recovery phase, and sync plan counts. Do not log raw response bodies, full private repository identity, skill files, blob bytes, or tokens.

- [ ] **Step 5: Run the Rust suite and inspect sensitive-output tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --all-features --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
```

Expected: all tests pass and clippy reports no warnings.

## Chunk 3: Frontend Command Failure Logging

### Task 3: Instrument the shared Tauri invoke boundary

**Files:**

- Modify: `package.json`, `package-lock.json`
- Modify: `src/shared/lib/tauri.ts`

- [ ] **Step 1: Add the frontend log plugin package if required by the selected plugin target**

Use the package version compatible with Tauri 2. Do not add a second logging framework.

- [ ] **Step 2: Add a failing boundary test or static assertion**

Verify a failed `invokeCmd` logs the command name and safe error kind/message before rethrowing `SkillSyncError`; verify arguments are not serialized into the log call.

- [ ] **Step 3: Implement failure logging**

Log command failures through the Tauri log plugin, preserving the existing structured error conversion and UI behavior. If the log plugin itself is unavailable during a frontend-only browser build, logging must fail silently and must not mask the original command error.

- [ ] **Step 4: Run frontend checks**

Run:

```bash
npm run typecheck
npm run lint
npm run build
```

Expected: all checks pass and existing error cards remain unchanged.

## Chunk 4: Retention And Manual Verification

### Task 4: Verify log files and document operation

**Files:**

- Modify: `docs/logging-design.md` if actual plugin path/rotation naming differs from the design.

- [ ] **Step 1: Verify a Debug startup log**

Run `npm run tauri dev`; confirm startup and command failure entries appear in the console and application log directory.

- [ ] **Step 2: Verify date rotation and retention**

Create dated test log files in the application log directory, restart the app, and confirm files older than seven days are removed while recent files remain.

- [ ] **Step 3: Verify no sensitive values are logged**

Trigger Device Flow and a mocked auth/API failure, then inspect the log text for absence of token, bearer, device-code, request-header, and raw-response values.

- [ ] **Step 4: Run the final verification suite**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml --all-features --locked
npm run typecheck
npm run format:check
npm run lint
npm run build
```

Expected: all commands exit successfully.

- [ ] **Step 5: Commit the implementation**

Use a Conventional Commit such as:

```bash
git add src-tauri package.json package-lock.json src docs/logging-design.md
git commit -m "feat: add tauri application logging"
```

Do not stage unrelated user files such as `docs/ui-refactor.md`.
