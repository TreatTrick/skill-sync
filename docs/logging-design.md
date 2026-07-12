# Skill Sync 日志系统设计

## 目标

为桌面端增加统一日志系统，满足以下要求：

- Debug 开发时输出到控制台。
- Debug 和 Release 都写入本地日志文件。
- 日志按天轮转，文件按日期区分。
- 启动时清理超过 7 天的日志文件，只保留最近 7 天。
- 记录足够的上下文来定位授权、GitHub API、配置、绑定、同步和本地恢复问题。
- 任何日志都不能泄露 access token、refresh token、client secret、private key、验证码或 Authorization header。

## 非目标

- 不增加应用内日志查看器。
- 不上传日志到服务器，不增加遥测和第三方日志 SaaS。
- 不记录完整 GitHub 响应 body、请求 header 或 skill 文件内容。
- 不为了日志重构现有 command、RemoteStore 或同步引擎的业务边界。

## 方案选择

### 方案 A：Tauri Log plugin + 应用统一入口（推荐）

使用 `tauri-plugin-log` 同时配置控制台和文件 target，使用每日轮转；应用启动时扫描 plugin 的日志目录，删除修改时间超过 7 天的日志。Rust 使用 `log` facade，前端使用 `@tauri-apps/plugin-log`，两端共用同一份日志文件配置。

优点是和 Tauri 生命周期、平台日志目录、Debug 控制台天然集成，依赖少，普通用户不需要额外配置。7 天保留属于产品策略，由应用自己的清理步骤保证，而不是依赖不同平台的默认行为。

### 方案 B：自建文件 logger

自行实现按日期命名、文件锁、并发写入和清理。

可完全控制格式，但会重复实现 Tauri plugin 已解决的平台路径、异步写入和控制台 target，增加桌面端并发与崩溃风险。不采用。

### 方案 C：只在前端 `invokeCmd` 记录

只记录前端 command 失败。

能覆盖 UI 调用错误，但无法看到 GitHub API 状态、token 刷新、keyring、文件写入和恢复过程，无法满足定位后端问题的需要。不采用。

## 运行时配置

在 Tauri `Builder` 初始化时注册 log plugin：

- target：文件日志目录 + stdout/Debug 控制台。
- rotation：启动时按本地日期命名当前文件，plugin 使用 `KeepAll` 在文件达到上限时追加日期时间归档。
- level：Debug 构建允许 `Debug`；Release 默认 `Info`，保留 `Warn`/`Error`。
- 当前文件名形如 `skill-sync_YYYY-MM-DD.log`；plugin 的归档文件继续包含日期时间；清理逻辑只处理该日志目录下以 `skill-sync` 开头的应用日志文件。
- 目录使用 Tauri 的应用日志目录，不写入仓库、用户 skill 目录或配置 YAML。
- 启动时清理文件修改时间早于当前时间 7 天的日志；清理失败只记录 warning，不阻止应用启动。

保留策略按文件最后修改时间执行，而不是按文件名解析日期。这样可以覆盖应用长时间未启动、系统时钟变化和 plugin 的具体命名格式。跨午夜运行的进程会继续写入启动日期文件，下一次启动时切换到新的日期文件；这避免了自建定时器和额外线程。

## 日志分层

### Rust

- `error`：command 失败、配置解析/保存失败、keyring 读写失败、Device Flow 终止、GitHub 请求失败、响应解析失败、绑定事务失败、本地 apply/recovery 失败。
- `warn`：Device Flow `slow_down`、即将重试、rate limit、检测到待恢复 journal、删除被护栏阻止、日志清理失败等可恢复异常。
- `info`：应用启动、日志目录清理结果、授权成功的 GitHub login、installation/repository discovery 阶段、绑定成功、同步开始/结束及 commit 数量。
- `debug`：command 开始/结束、GitHub API 操作名和 HTTP status、轮询状态、计划统计。不得输出请求 body、header、token 或 skill 内容。

所有 Rust command 通过统一边界记录错误，避免在每个 UI command 中复制日志逻辑。GitHub client 和 credential manager 在更接近根因的位置记录 HTTP status、operation 名称和错误分类。

### 前端

`invokeCmd` 作为统一入口：

- command 成功不默认逐条记录，避免正常同步产生大量噪声。
- command 失败记录 command 名称、错误 kind 和可安全展示的 message，然后继续抛出给现有 UI 错误处理。
- 不记录传入参数，因为参数可能包含仓库路径、远端 identity 或未来新增的敏感字段。
- Onboarding、Sync、Settings 页面只在无法交给 `invokeCmd` 的 UI 逻辑异常处记录 error。

## 脱敏边界

日志消息使用结构化 operation 和错误分类，不直接格式化 credential 或 reqwest request。以下内容禁止进入日志：

- access token、refresh token、client secret、private key。
- `Authorization`、Cookie 和完整请求 header。
- Device Flow `device_code`、`user_code` 和完整授权响应。
- GitHub 原始响应 body。
- skill 文件内容和压缩包字节。

允许记录：command 名称、阶段、HTTP status、retry-after、错误 kind、installation/repository 数量、manifest 状态、计划统计和恢复 phase。日志脱敏函数继续复用 `AppError` 的敏感字段处理，并增加对 bearer/header/JSON token 字段的覆盖测试。

## 数据流

```text
Tauri startup
  -> initialize log plugin
  -> clean logs older than 7 days
  -> register commands

frontend invokeCmd failure
  -> log command + safe error kind/message
  -> throw SkillSyncError
  -> existing UI error card

Rust operation
  -> log operation/status at source
  -> return AppError on failure
  -> Tauri serializes redacted error payload
```

## 验证计划

- 验证 Debug 控制台和日志目录都收到 startup/error 日志。
- 验证每日轮转配置和日期文件存在。
- 创建超过 7 天的伪日志，启动后确认被删除；7 天内文件保留。
- 用 wiremock 覆盖 GitHub 401、rate limit、invalid JSON、network error，确认日志包含 operation/status/error kind，不包含 token、header 和 response body。
- 触发前端 command 参数错误，确认日志记录 command 和错误 kind，UI 仍显示原有错误卡片。
- 运行 Rust 全量测试、`npm run typecheck`、`npm run lint`、`npm run build` 和 `npm run format:check`。

## 受影响文件（实现阶段）

- `src-tauri/Cargo.toml`、`Cargo.lock`：加入 Tauri log plugin。
- `src-tauri/src/lib.rs`：注册 plugin 和启动清理。
- `src-tauri/src/logging.rs`：日志初始化、7 天清理和安全字段辅助函数。
- `src-tauri/src/errors.rs`、`commands.rs`、GitHub/auth/store/config/sync 模块：在根因边界增加日志。
- `package.json`、`package-lock.json`：加入前端 log plugin。
- `src/shared/lib/tauri.ts`：统一记录 command 错误。
- 相关测试文件：覆盖初始化、清理、脱敏和关键错误分支。
