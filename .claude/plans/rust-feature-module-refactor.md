# Rust 源码 feature-based 拆分重构

## 目标

将 `src-tauri/src` 下过于庞大的文件按 feature 拆解为小模块，提升可维护性。
不改变任何运行时行为；保留全部既有注释（规则 #2）；不新增测试，仅迁移既有测试到对应模块。

## 现状（问题文件）

| 文件 | 行数 | 问题 |
|---|---|---|
| `sync_engine/vault.rs` | 3859 | DTO 模型 + plan 推导 + fingerprint + apply 执行 + 巨型 tests 全堆一个文件 |
| `commands.rs` | 1464 | AppRuntime + 21 个 tauri command + 共享 helper + tests |
| `github_*.rs`（5 个根文件） | 68~1166 | 同属 GitHub feature 却散落在 crate 根 |
| `pack.rs` | 1291（生产 ~565） | pack/unpack 较内聚，生产代码不大 |

爆炸半径很小：仅 `commands.rs` 引用 `vault::*`；仅 `commands.rs` + 4 个 github 文件引用 `crate::github_*`。

## 约束（来自 AGENTS.md / CLAUDE.md / Cargo.toml）

- 保留既有注释；新注释用中文。
- Anti-thin-wrapper：不建只做 re-export 的空 barrel。`mod.rs` 只声明子模块 + 必要的 `#[tauri::command]` 入口 re-export；调用方用显式子模块路径。
- `unreachable_pub = "deny"` + `unsafe_code = "forbid"` + clippy `-D warnings`：全部维持 `pub(crate)`，不用裸 `pub`。
- import 流向不形成循环（github_auth↔github_credentials 现有互相引用 Rust 可处理，保持）。

---

## Tier 1：拆分 `sync_engine/vault.rs`（3859 → 3 个生产模块 + 各自 tests）

`sync_engine.rs`（2 行）改为 `sync_engine/mod.rs`，声明子模块。

### `sync_engine/model.rs`（~220 行，原 1–220）
DTO 与决策模型：`SyncStatus` `ConflictReason` `DeleteDirection` `SyncDecision`
`SyncSkillEntry` `BaseAdoption` `Conflict` `BlockedSkill` `CommitSummary` `SyncPlan`
`ApplySyncRequest` `PlanChangeReason` `RecoveryPhase` `RecoveryInfo` `ApplyResult`
`ApplySyncResponse` `LocalSkillInfo`。全部 `pub(crate)`。
配套序列化测试（`status_reason_decision_serialize_to_snake_case`、`apply_response_uses_status_tag`、`sync_plan_roundtrips`）迁入本模块 `mod tests`。

### `sync_engine/plan.rs`（~700 行，原 221–971 "Task 8" + "plan fingerprint"）
plan 推导 + fingerprint + build_plan 入口：
`RawEntry` `derive_entry` `action_kind_for` `ensure_unique_action_ids`
`collision_entry` `namespace_matches_id` `merge_plan` `block_entry`
`FpEntry` `FpAdoption` `FpInput` `fingerprint_input` `canonical_fingerprint_bytes`
`compute_fingerprint` `validate_remote` `build_plan`。
- `merge_plan` `build_plan` `compute_fingerprint` 维持/提升为 `pub(crate)`（apply 与 commands 跨模块用）。
- 其余 helper 保持私有。
- merge 真值表 / fingerprint golden / collision / delete-guard 测试迁入本模块 `mod tests`。

### `sync_engine/apply.rs`（~900 行，原 972–1883 "Task 9"）
apply 执行：`PackedLocal` `PreparedSyncPlan` `prepare_plan` `now_iso`
`is_executable` `allowed_decisions` `UploadItem` `DownloadItem` `DeleteLocalItem`
`download_target` `delete_local_target` `execute_download` `save_recovery_journal`
`cleanup_task_artifacts` `apply_plan` `upload_skills` `download_skills` `batch_apply`。
- `apply_plan` `upload_skills` `download_skills` 为 `pub(crate)`；`prepare_plan` 为 `pub(super)`/`pub(crate)` 供 apply 内部。
- ApplyMockStore / apply 流程测试迁入本模块 `mod tests`。

### 调用方更新
`commands.rs` 的 `use crate::sync_engine::vault::{self, …}` 拆为：
`use crate::sync_engine::model::{…}`、`use crate::sync_engine::plan::build_plan`、
`use crate::sync_engine::apply::{apply_plan, upload_skills, download_skills}`；
`vault::build_plan` → `build_plan`，`vault::CommitSummary` → `CommitSummary` 等同步去前缀。

---

## Tier 2：GitHub feature 收拢到 `github/` 文件夹

5 个根文件 → `github/{app_config,auth,credentials,repository,store}.rs`，加 `github/mod.rs` 声明子模块。

- `lib.rs`：删 5 个 `mod github_*`，加 `mod github;`。
- `crate::github_xxx::` → `crate::github::xxx::`（commands.rs + 各 github 文件内）。
- github 文件间的 sibling 引用改 `super::xxx::`（更清晰）。
- 不做 barrel re-export（anti-thin-wrapper）；调用方一律显式子模块路径。

涉及引用文件（已确认仅 5 个）：`commands.rs`、`github_auth.rs`、`github_credentials.rs`、`github_repository.rs`、`github_store.rs`。

---

## Tier 3：拆分 `commands.rs`（1464 → 4 文件）

`commands.rs` → `commands/mod.rs` + 子模块。`lib.rs` 的 `invoke_handler` 改用 `commands::<sub>::<fn>` 显式路径。

### `commands/mod.rs`（~260 行）
运行时 + 通用 infra + 声明子模块：
`SyncOperationGate` `AppRuntime`（含 `impl AppRuntime::new`）`AppState` `GithubAppInfo`
`GithubDeviceFlowPollResponse` `log_command_result`；通用 helper `run_blocking` `list_json`
`load_config` `config_dir`；send-check / app_state 序列化测试。
子模块通过 `use super::{AppRuntime, …}` 访问共享类型。

### `commands/sync.rs`（~450 行）
核心同步命令 + 恢复逻辑：
`get_app_state` `save_config` `scan_skills` `get_sync_plan` `apply_sync_plan`
`resume_sync_recovery`（含 `_impl`）`RemoteRecoveryDecision` `reconcile_remote_journal`
`resume_remote_recovery` `load_store_and_state` `batch_sync`
`ensure_no_pending_recovery` `ensure_no_pending_recovery_at`
`ensure_preview_has_no_pending_recovery` `journal_to_recovery`；
恢复 reconcile 测试（RecoveryMockStore）迁入。

### `commands/github.rs`（~360 行）
13 个 GitHub 命令 + 绑定 helper：
`start_github_device_flow` `poll_github_device_flow` `get_github_app_info`
`list_github_installations` `list_installation_repositories`
`discover_single_github_repository` `list_github_repository_branches`
`check_github_vault` `initialize_github_vault` `bind_github_vault`（+ `_impl`）
`disconnect_github` `list_remote_skills` `upload_skills` `download_skills`；
`RemoteBindingKey` `BindGithubVaultRequest` `remote_binding_key`
`validate_expected_binding` `remote_identity`；device_flow 响应测试迁入。

### `commands/system.rs`（~50 行）
`open_path` `open_path_platform`（跨平台 cfg 分支）。

---

## 不在本次范围

- `pack.rs`：生产 ~565 行、内聚，暂不拆（可后续按 pack/unpack/zip_parse 拆，按需）。
- `github_store.rs` / `github_repository.rs` / `github_credentials.rs`：仅迁入文件夹，不拆内部（各自单一 service/impl，内聚）。
- `detect.rs` `local_apply.rs` `vault_manifest.rs` `sync_state.rs` 等：体积合理、内聚，不动。

## 验证（每完成一个 Tier 跑一次）

```bash
cd src-tauri
cargo check --all-features --locked
cargo clippy --all-targets --all-features --locked -- -D warnings   # = npm run rust:clippy
cargo test --all-features --locked                                  # = npm run rust:test
cargo fmt --all -- --check                                          # = npm run rust:fmt:check
```
最终另跑 `npm run lint` + `npm run build`（AGENTS.md 完成前检查）。
基线：重构前 `cargo check` 已通过。

## 执行顺序

1. Tier 1（vault 拆分）→ 验证 → commit
2. Tier 2（github 收拢）→ 验证 → commit
3. Tier 3（commands 拆分）→ 验证 → commit

每 Tier 独立 commit（Conventional Commits `refactor:`），便于回滚。
