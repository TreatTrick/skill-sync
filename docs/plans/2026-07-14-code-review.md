层级清晰：shared/ 没有向上引用，模块也不会从 app/ 导入，并且 remote 颜色 token 已正确定义。我已经有足够的信息来撰写评审了。

发现 (Findings)

P0

无。

P1

无。同步模块、共享层以及最近重构的 Rust sync_engine crate 的正确性、错误处理和模块边界均很稳健。未发现正确性、数据损坏或不安全的外部数据处理问题。

P2

src/modules/onboarding/schemas/onboarding.ts:1-25 和 src/modules/settings/schemas/config.ts:1-3 是纯粹的透传 barrel 文件，只是重新导出了 @/shared/schemas，没有任何模块本地的 schema 定义。onboardingApi.ts 和 configApi.ts 从 ../schemas/... 导入，而不是直接从 @/shared/schemas 导入，增加了一个毫无价值的间接层。根据反瘦包装器 (anti-thin-wrapper) 和“禁止添加空抽象”规则，这些属于空抽象。建议修改：要么让 API 文件直接从 @/shared/schemas 导入这些 schema，要么仅在确实添加了模块本地 schema 时才保留该文件。（如果您有意将其用作 facade 占位符，请至少添加一条说明意图的注释。）

src/modules/sync/api/syncApi.ts:29-37, src/modules/settings/api/configApi.ts:22-33, src/modules/onboarding/api/onboardingApi.ts:85-102 使用 zod 重新解析了内部类型的标量参数：resumeSyncRecovery(taskId: string) 执行了 z.string().min(1).parse(taskId)，而 disconnectGithub/listInstallationRepositories 则对已经类型化为 number 的参数执行了 z.number().int().nonnegative().parse(...)。这些标量均源自上游已通过 zod 验证的状态 —— taskId 来自 recovery.task_id (recoveryInfoSchema)，而 repository/installation ID 来自 appStateSchema，该 schema 已经强制要求 int().nonnegative()。这种重新解析重复了上游的保证。响应端验证（例如 applySyncResponseSchema.parse）是完全正确的，应保留。建议修改：删除标量重新解析并直接传递参数；如果为了 FFI 清晰的错误消息而保留，请承认这是纵深防御，并保持一致地应用，而不是仅在这三个地方使用。

src/modules/settings/index.ts:2 导出了 getAppState，但这是一个横切的全局应用状态查询（配置 + 凭据状态 + 恢复），被五个非设置模块的调用者使用：OnboardingPage.svelte:7, SyncPreviewPage.svelte:22, routes/+page.svelte:6, routes/app/+layout.svelte:8, routes/app/+page.svelte:6。这迫使 onboarding 和 sync（在概念上位于设置“之前”或“旁边”）为了获取通用的状态读取而依赖 settings 模块，这既是一个轻微的耦合异味，也使得 settings 成为了一个枢纽。根据架构规则，广泛重用的稳定横切概念属于 shared（已经包含 appStateSchema）。建议修改：考虑将 getAppState（以及类似的 app-state 读取器）提升到 shared 客户端中，以便 settings 仅保留其自身的功能。这是通过公共 index 进行的，因此不算违规，只是一个布局选择。

src-tauri/src/sync_engine/apply.rs:376-870 — apply_plan 约为 500 行。它是线性的、带序的，并带有阶段注释（1–6，然后是构建-执行-提交），所以不是一个“上帝函数”，但请求验证块（约 400–468 行，预期提交/指纹/selected_action_ids/决策白名单/删除保护检查）和两个工作项构建循环可以提取为专注的、可独立测试的辅助函数（validate_apply_request，collect_upload_items 等）。这将使主函数简化为编排过程，并使验证分支能够进行单元测试，而无需 mock 存储。建议修改：提取验证块和项目收集循环；保持 apply_plan 为线性协调器。

待解决问题 (Open Questions)

关于 getAppState 的放置位置（见 P2）：您是否打算让 settings 拥有所有应用状态查询，还是 shared 客户端更符合预期的层级结构？这会影响上述建议是否值得采取行动。
总结 (Summary)

审查范围：已提交的架构（工作树干净，最近 5 次提交主要是文档/规划；唯一相关的代码变更是 SyncFilterBar.svelte），重点关注 sync 模块（最活跃）、shared 层、路由组装以及最近重构的 Rust sync_engine/commands crate。

整体架构稳健：整洁的 routes -> app -> modules -> shared 分层，没有反向或循环导入，所有跨模块消费都通过模块 index.ts 进行，没有 export *，到处都是最小权限的 pub(crate)，基于 trait 的 RemoteStore 抽象，支持干净的测试替身，在 Tauri 边界进行了严格的 zod 响应验证，并通过带有不可变快照的恢复日志进行了正确的乐观并发控制。前端的 i18n/color/responsive token 使用一致。所有发现均为 P2 级别的可维护性/布局问题；未发现正确性、安全性或边界违规问题。

已执行：对上述文件进行静态阅读，通过 grep 验证导入/层级/颜色 token。未执行：npm run typecheck/lint/build 和 cargo 检查（未进行任何更改；仅为审查）。剩余审查风险：我没有深入阅读最大的 Rust 文件 — github/store.rs (1166 行), pack.rs (1291 行), github/repository.rs (961 行), github/credentials.rs (841 行)。鉴于最近对 sync_engine 的功能拆分，这些是接下来进行拆分审查的主要候选对象，但我没有验证它们的内部结构。
