# 验证记录

日期：2026-11-17。

## 已验证

- `openspec validate add-about-and-app-updates --strict`：通过。
- `npm run typecheck`（也由 lint 和 build 调用）：Svelte 检查 0 错误、0 警告。
- `npm run format:check`：通过。
- `npm run lint`：通过，包含架构/ESLint、响应式、颜色 token 和 i18n 检查。
- `npm run build`：通过，生成静态前端产物。
- `npm run test:updates`：通过，覆盖软件元数据、版本元数据校验、资源释放、检查错误重试、下载去重、下载与安装分离、签名失败状态、安装失败重试、重启失败重试及未完成同步时的操作门槛。
- `cargo check --manifest-path src-tauri/Cargo.toml --locked`：通过，保留 Rust 1.88 工具链。
- `npm run rust:fmt:check`：通过。
- `npm run rust:clippy`：通过，使用 `--all-targets --all-features --locked -- -D warnings`。
- 通过临时 Playwright 脚本访问真实 Svelte 页面：500 / 900 / 1200px 无页面或内容横向溢出；明暗主题、折叠导航、绿色更新箭头、中英文、检查失败重试、下载进度、取消安装、安装失败重试和重启入口正常；往返设置/关于不重复检查；设置不再包含关于分区；重新检查失败隐藏旧更新标记。

## 实际限制

- `npm run rust:test`：Rust 测试编译成功，但 Windows 测试进程在执行测试前退出，状态为 `0xc0000139 / STATUS_ENTRYPOINT_NOT_FOUND`。本次修改前已存在的测试二进制 `skill_sync_lib-43901bd4ea57f730.exe --list` 也以相同状态退出，属于此环境中既有的原生进程启动限制，未将 Rust 测试记录为通过。
- 页面和状态测试在 Tauri IPC 边界使用替身，没有执行本机安装、退出或重新启动。尚未在 Windows 安装器和 macOS 真机完成已发布版本之间的完整升级验证。
- 实际请求 GitHub：最新正式版为 `v1.1.0`，其 `latest.json` 返回 404。历史版本尚无签名更新清单，所以在首个签名新版发布前，真实检查会显示失败并允许重试。

## 发布前置条件

依据 `docs/app-updates.md` 将已生成的配对私钥配置为 GitHub Actions Secret `TAURI_SIGNING_PRIVATE_KEY`，并妥善备份；功能实现阶段未配置远端 Secret，也未发布 Release。CI 完成两个目标平台的草稿构建后才公开 latest；首次具备 updater 的版本需手动安装，随后在测试设备上验证完整签名升级流程。

## v1.1.1 发布准备

- 应用、Rust crate、官网与下载链接已统一为 `1.1.1`，CHANGELOG 已补充本次变更。
- 已核对本机签名密钥与应用公钥配对，并将私钥配置为本仓库 Actions Secret `TAURI_SIGNING_PRIVATE_KEY`；密钥未进入 Git。
- 关于、导航与设置主规范已同步，变更已归档至 `2026-11-17-add-about-and-app-updates`。

## v1.1.1 打包兼容性修复

- 首次 CI 在 Tauri 版本预检阶段失败：前端 API 的宽泛版本范围解析为 `2.12.5`，与锁定的 Rust Tauri `2.11.5` 次版本不一致；已在本地用 `npm run tauri build -- --no-bundle` 复现。
- 将 `@tauri-apps/api` 固定为 `2.11.1` 后，实际 Tauri 打包通过版本预检和前端生产构建，进入原生编译；重新运行 `npm run lint`、`npm run test:updates`、`npm run format:check` 均通过。
- 发布工作流采用 Node 24，使用 Tauri Action 支持的 `includeUpdaterJson` 参数；失败运行已取消，尚未公开 v1.1.1 Release。
