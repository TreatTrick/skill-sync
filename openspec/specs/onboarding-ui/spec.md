# Purpose

规定首次配置的六步引导、全部流程状态和现有状态机的呈现约束。

## Requirements

### Requirement: 首次配置 SHALL 使用六个有名称的步骤引导

首次配置 SHALL 显示六步进度轨道、当前步骤名称和当前步骤的主要操作；欢迎内容 SHALL 是流程内页面。GitHub 授权、仓库选择、GitHub App 安装、Vault 检查/初始化及完成绑定 SHALL 按当前流程分组。

#### Scenario: 开始首次配置

- **WHEN** 用户首次进入配置流程
- **THEN** 页面显示内嵌欢迎内容和开始配置操作，不依赖模态欢迎弹窗

#### Scenario: 查看 GitHub App 安装

- **WHEN** 用户处于安装 GitHub App 步骤
- **THEN** 页面展示安装操作及在 GitHub 上选择 Vault 仓库范围的说明

### Requirement: 配置界面 SHALL 覆盖所有现有流程状态

页面 SHALL 为现有 `OnboardingStage` 的每个状态提供可见内容或等待反馈，包括 App 未配置、授权中、设备授权等待、仓库创建、安装范围阻止、公开仓库确认、选择分支、检查中、Vault 不可用、初始化确认、manifest 无效、API 限流和完成绑定。重新配置 SHALL 继续由现有 mode 表达。

#### Scenario: API 限流或 Vault 异常

- **WHEN** 流程进入限流、manifest 无效、安装范围阻止或 Vault 不可用状态
- **THEN** 当前六步位置正确，错误原因和下一步操作清楚可见

#### Scenario: 授权或检查正在进行

- **WHEN** 流程等待设备授权、轮询或 Vault 检查
- **THEN** 页面显示等待/进度状态并禁用重复提交，不丢失当前流程状态

### Requirement: 首次配置 SHALL 继续通过现有流程状态机执行

步骤页面 SHALL 调用现有 `OnboardingFlow` 和 API 端口；布局重组不得直接调用替代 API、跳过授权校验、绕过公开仓库确认或改变 Vault 绑定/基线建立顺序。

#### Scenario: 完成 Vault 绑定

- **WHEN** 用户完成授权、选择仓库/分支并确认初始化
- **THEN** 现有状态机执行绑定及基线流程，并按现有结果导航到同步页
