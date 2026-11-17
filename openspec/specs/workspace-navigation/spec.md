# Purpose

规定主应用左侧导航及当前 Vault 上下文的显示方式。

## Requirements

### Requirement: 主应用导航 SHALL 位于工作区左侧

已配置的主应用 SHALL 在左侧提供“同步”和“设置”两个入口，并清晰标记当前路由。宽屏展开时 SHALL 同时显示图标和文字；窄窗口或折叠状态 SHALL 使用紧凑图标轨，并通过本地化标题和无障碍名称标明入口。

#### Scenario: 宽屏切换主页面

- **WHEN** 用户在宽屏主应用中打开同步或设置页面
- **THEN** 左侧导航显示带文字的两个入口，并标明当前页面

#### Scenario: 窄窗口使用图标轨

- **WHEN** 主应用视口不足以容纳展开导航
- **THEN** 导航保持在左侧并收窄为图标轨，每个入口仍可通过名称识别和访问

### Requirement: 工作区顶栏 SHALL 显示当前 Vault 上下文

已配置主应用的顶栏 SHALL 显示当前 Vault 仓库 owner/repository 和分支；这些值 SHALL 来自已校验的应用状态，而非展示层常量。

#### Scenario: Vault 已绑定

- **WHEN** 主应用已完成 Vault 绑定
- **THEN** 顶栏显示对应仓库和当前分支

#### Scenario: 重新配置期间

- **WHEN** 用户进入首次配置或重新配置流程
- **THEN** 页面继续使用独立配置布局，不显示过期的主应用 Vault 上下文
