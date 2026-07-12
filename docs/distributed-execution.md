# 已退役的并行执行说明

本文件曾记录早期的多 agent 并行开发拆分。那些工作包描述的是旧产品模型，不能作为当前实现或新任务的依据。

当前唯一有效的实施计划是 [`superpowers/plans/2026-07-07-github-vault-refactor.md`](superpowers/plans/2026-07-07-github-vault-refactor.md)，产品边界和开发检查以 [`development-plan.md`](development-plan.md) 为准。

当前实现只支持 GitHub App Device Flow、单 Vault 仓库、三个固定用户级 namespace，以及 Sync / Settings 工作区。新改动必须遵守仓库根目录 `AGENTS.md` 的依赖、验证和提交规则。
