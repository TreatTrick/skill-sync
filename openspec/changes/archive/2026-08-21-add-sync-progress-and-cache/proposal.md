## Why

同步预览和执行目前都是长时间的黑盒调用：用户看不到扫描、打包、文件应用或提交阶段的进展，也无法判断操作是否仍在工作。每次预览和执行还会重新读取、打包并计算所有本地 skill 的 hash，skill 数量较多时等待时间过长。

## What Changes

- 为同步预览和执行增加实时阶段进度事件与进度条。
- 对可计数的扫描、打包、下载、替换、删除和收尾步骤显示确定进度；单次远端 commit 阶段显示不确定进度。
- 保持一次同步只产生一个远端 commit，不提供中途取消操作。
- 在本机配置目录持久化规范化 skill zip、文件元数据和 hash，元数据未变化时复用缓存，变化或校验失败时自动重算。
- 通过打包算法/缓存格式、ignore 规则和资源限制版本化缓存，避免复用失效结果。
- 为缓存设置默认 1 GB 容量上限并按 LRU 清理，提供设置页查看占用和手动清空入口。

## Capabilities

### New Capabilities

- `sync-progress`: 为同步预览和执行提供阶段、当前项、计数和不确定状态的实时进度反馈。
- `skill-pack-cache`: 缓存经过校验的本地 skill 规范化 zip、元数据和 hash，并在安全条件满足时复用。
- `cache-management`: 提供缓存容量统计、自动 LRU 清理和手动清空能力。

### Modified Capabilities

<!-- No existing capability specs are present in this repository. -->

## Impact

- Rust Tauri 同步命令、扫描/打包流程、同步执行流程和本地配置目录持久化。
- 前端同步页面、设置页面、Tauri invoke/event 边界、i18n 文案和 API schemas。
- 新增本地缓存文件格式、容量清理逻辑及相应 Rust/前端测试；不改变远端 manifest 结构或单次 commit 事务边界。
