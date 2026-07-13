// GitHub 集成 feature：app_config（应用配置）、auth（设备流认证）、credentials（凭据与认证客户端）、
// repository（仓库发现与 vault 初始化）、store（Git blob/manifest 远端存储）。同 feature 子模块之间
// 通过 crate::github::<name> 显式引用，外部调用方同样使用该路径。

pub(crate) mod app_config;
pub(crate) mod auth;
pub(crate) mod credentials;
pub(crate) mod repository;
pub(crate) mod store;
