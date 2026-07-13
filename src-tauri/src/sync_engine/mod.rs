// GitHub Vault 同步引擎：model（DTO/决策模型）、plan（merge/fingerprint/build_plan）、
// apply（apply_plan 及批量入口）。三者按 feature 拆分，互不持有持久化副作用之外的隐式状态。

pub(crate) mod apply;
pub(crate) mod model;
pub(crate) mod plan;
