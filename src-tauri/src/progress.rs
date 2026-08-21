use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub(crate) const EVENT_NAME: &str = "sync-progress";

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProgressEvent {
    pub operation_id: String,
    pub operation: String,
    pub phase: String,
    pub current: usize,
    pub total: Option<usize>,
    pub skill_id: Option<String>,
    pub determinate: bool,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Clone, Default)]
pub(crate) struct ProgressReporter {
    app: Option<AppHandle>,
    operation_id: String,
    operation: String,
}

impl ProgressReporter {
    pub(crate) fn new(app: Option<AppHandle>, operation: &str) -> Self {
        Self {
            app,
            operation_id: uuid::Uuid::new_v4().to_string(),
            operation: operation.into(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit(
        &self,
        phase: &str,
        current: usize,
        total: Option<usize>,
        skill_id: Option<&str>,
        determinate: bool,
        cache_hits: usize,
        cache_misses: usize,
        status: &str,
        error: Option<&str>,
    ) {
        let Some(app) = &self.app else {
            return;
        };
        let event = ProgressEvent {
            operation_id: self.operation_id.clone(),
            operation: self.operation.clone(),
            phase: phase.into(),
            current,
            total,
            skill_id: skill_id.map(str::to_string),
            determinate,
            cache_hits,
            cache_misses,
            status: status.into(),
            error: error.map(str::to_string),
        };
        drop(app.emit(EVENT_NAME, event));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_payload_contains_operation_and_indeterminate_state() {
        let event = ProgressEvent {
            operation_id: "operation".into(),
            operation: "apply".into(),
            phase: "remote_commit".into(),
            current: 0,
            total: None,
            skill_id: None,
            determinate: false,
            cache_hits: 2,
            cache_misses: 1,
            status: "running".into(),
            error: None,
        };
        let value = serde_json::to_value(event).unwrap();
        assert_eq!(value["phase"], "remote_commit");
        assert_eq!(value["determinate"], false);
        assert_eq!(value["total"], serde_json::Value::Null);
    }
}
