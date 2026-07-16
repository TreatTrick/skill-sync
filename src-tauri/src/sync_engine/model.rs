// 同步计划的数据模型与决策 DTO。前端 schema 与 Tauri boundary 复用本模块的 GitHub Vault 模型。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::skill::SkillNamespace;

/// 单个 skill 在三方比较后的同步状态（13 行真值表 + blocked/unknown）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SyncStatus {
    Synced,
    LocalUpdate,
    RemoteUpdate,
    LocalDeleted,
    RemoteDeleted,
    BothDeleted,
    Conflict,
    Blocked,
    Unknown,
}

/// 冲突原因，对应 decision 白名单的四种情形。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConflictReason {
    SameNameFirstSeen,
    BothChanged,
    LocalDeletedRemoteChanged,
    RemoteDeletedLocalChanged,
}

/// 删除方向：本地 -> trash，或云端 manifest 条目。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeleteDirection {
    DeleteLocal,
    DeleteRemote,
}

/// 用户对冲突或纯删除条目的决策；必须落在对应 ConflictReason 或 SyncStatus 的白名单内。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SyncDecision {
    KeepLocal,
    UseRemote,
    Skip,
    DeleteRemote,
    RestoreRemote,
    AcceptDelete,
}

/// 计划中单个 skill 的条目；同时承载可执行动作、冲突、blocked、unknown 信息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SyncSkillEntry {
    pub action_id: String,
    pub skill_id: String,
    pub name: String,
    pub namespace: SkillNamespace,
    pub folder_name: String,
    pub relative_dir: Option<String>,
    pub status: SyncStatus,
    pub local_hash: Option<String>,
    pub remote_hash: Option<String>,
    pub base_hash: Option<String>,
    pub local_path: Option<String>,
    pub remote_blob: Option<String>,
    pub conflict_reason: Option<ConflictReason>,
    pub delete_direction: Option<DeleteDirection>,
    pub blocked_reason: Option<String>,
    pub warnings: Vec<String>,
}

/// base adoption：base 为空且 local==remote 时生成的本机状态协调（0 commit）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BaseAdoption {
    pub skill_id: String,
    pub hash: String,
}

/// 冲突条目；decisions 按 skill_id 索引，apply 时校验 decision 白名单。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Conflict {
    pub skill_id: String,
    pub name: String,
    pub namespace: SkillNamespace,
    pub folder_name: String,
    pub relative_dir: Option<String>,
    pub conflict_reason: ConflictReason,
    pub local_hash: Option<String>,
    pub remote_hash: Option<String>,
    pub base_hash: Option<String>,
    pub local_path: Option<String>,
    pub remote_blob: Option<String>,
    pub warnings: Vec<String>,
}

/// blocked skill；扫描碰撞、远端超限、identity 非法等不参与任何动作。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BlockedSkill {
    pub skill_id: String,
    pub name: String,
    pub namespace: SkillNamespace,
    pub folder_name: String,
    pub reason: String,
}

/// 提交摘要；`local_state_updates` 为去重后的 adoption/removal skill 数量。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CommitSummary {
    pub uploads: usize,
    pub downloads: usize,
    pub delete_remote: usize,
    pub delete_local: usize,
    pub local_state_updates: usize,
}

/// 完整同步计划。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SyncPlan {
    pub entries: Vec<SyncSkillEntry>,
    pub uploads: Vec<String>,
    pub downloads: Vec<String>,
    pub delete_remote: Vec<String>,
    pub delete_local: Vec<String>,
    pub conflicts: Vec<Conflict>,
    pub blocked: Vec<BlockedSkill>,
    pub warnings: Vec<String>,
    pub delete_guard_tripped: bool,
    pub expected_remote_commit: String,
    pub plan_fingerprint: String,
    pub base_adoptions: Vec<BaseAdoption>,
    pub base_removals: Vec<String>,
    pub will_create_commit: bool,
    pub commit_summary: CommitSummary,
}

/// apply 请求：携带预览时的 expected_remote_commit + plan_fingerprint + 用户选择。
/// selected_action_ids 控制普通可执行项；decisions 控制冲突，并可覆盖纯删除条目。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ApplySyncRequest {
    pub expected_remote_commit: String,
    pub plan_fingerprint: String,
    pub selected_action_ids: Vec<String>,
    pub decisions: HashMap<String, SyncDecision>,
    pub delete_guard_ack: bool,
}

/// PlanChanged 的原因：远端 commit 变化，或 commit 不变但计划内容变化。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PlanChangeReason {
    RemoteChanged,
    PlanChanged,
}

/// 恢复阶段；RecoveryRequired 只用于已产生持久化副作用或远端结果无法证明的事务。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RecoveryPhase {
    RemoteOutcomeUnknown,
    LocalReplaceFailed,
    StateSaveFailed,
}

/// 恢复信息：task/phase、远端 commit、已完成/待完成 action、消息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RecoveryInfo {
    pub task_id: String,
    pub phase: RecoveryPhase,
    pub remote_commit: Option<String>,
    pub completed_action_ids: Vec<String>,
    pub pending_action_ids: Vec<String>,
    pub message: String,
}

/// apply 成功结果；包含本机状态更新和清理告警。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ApplyResult {
    pub applied: Vec<String>,
    pub state_updated: Vec<String>,
    pub warnings: Vec<String>,
    pub remote_commit: Option<String>,
}

/// apply 响应：成功、计划变更（需重新确认）、或需恢复。
/// `latest_plan` 装箱以避免 enum 变体大小悬殊（serde 透明，JSON 不变）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum ApplySyncResponse {
    Applied {
        result: ApplyResult,
    },
    PlanChanged {
        reason: PlanChangeReason,
        latest_plan: Box<SyncPlan>,
    },
    RecoveryRequired {
        recovery: RecoveryInfo,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- serialization tests ----

    #[test]
    fn status_reason_decision_serialize_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&SyncStatus::LocalUpdate).unwrap(),
            "\"local_update\""
        );
        assert_eq!(
            serde_json::to_string(&SyncStatus::BothDeleted).unwrap(),
            "\"both_deleted\""
        );
        assert_eq!(
            serde_json::to_string(&ConflictReason::SameNameFirstSeen).unwrap(),
            "\"same_name_first_seen\""
        );
        assert_eq!(
            serde_json::to_string(&ConflictReason::LocalDeletedRemoteChanged).unwrap(),
            "\"local_deleted_remote_changed\""
        );
        assert_eq!(
            serde_json::to_string(&SyncDecision::RestoreRemote).unwrap(),
            "\"restore_remote\""
        );
        assert_eq!(
            serde_json::to_string(&DeleteDirection::DeleteLocal).unwrap(),
            "\"delete_local\""
        );
        assert_eq!(
            serde_json::to_string(&RecoveryPhase::StateSaveFailed).unwrap(),
            "\"state_save_failed\""
        );
    }

    fn minimal_plan() -> SyncPlan {
        SyncPlan {
            entries: vec![],
            uploads: vec![],
            downloads: vec![],
            delete_remote: vec![],
            delete_local: vec![],
            conflicts: vec![],
            blocked: vec![],
            warnings: vec![],
            delete_guard_tripped: false,
            expected_remote_commit: "sha".into(),
            plan_fingerprint: "fp".into(),
            base_adoptions: vec![],
            base_removals: vec![],
            will_create_commit: false,
            commit_summary: CommitSummary::default(),
        }
    }

    #[test]
    fn apply_response_uses_status_tag() {
        let applied = ApplySyncResponse::Applied {
            result: ApplyResult {
                applied: vec!["a".into()],
                state_updated: vec![],
                warnings: vec![],
                remote_commit: None,
            },
        };
        let json = serde_json::to_value(&applied).unwrap();
        assert_eq!(json["status"], "applied");
        assert_eq!(json["result"]["applied"][0], "a");

        let plan_changed = ApplySyncResponse::PlanChanged {
            reason: PlanChangeReason::RemoteChanged,
            latest_plan: Box::new(minimal_plan()),
        };
        let json = serde_json::to_value(&plan_changed).unwrap();
        assert_eq!(json["status"], "plan_changed");
        assert_eq!(json["reason"], "remote_changed");
    }

    #[test]
    fn sync_plan_roundtrips() {
        let plan = minimal_plan();
        let json = serde_json::to_string(&plan).unwrap();
        let back: SyncPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(back, plan);
    }
}
