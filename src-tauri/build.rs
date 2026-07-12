fn main() {
    // GitHub App 公开构建配置：通过 cargo:rustc-env 注入，编译期由 env! 读取。
    // 这是公开 repository variables，不是 GitHub secrets；GITHUB_TOKEN 仍只用于发布 release。
    for key in [
        "SKILL_SYNC_GITHUB_APP_CLIENT_ID",
        "SKILL_SYNC_GITHUB_APP_SLUG",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
        let value = std::env::var(key).unwrap_or_default();
        if std::env::var("PROFILE").as_deref() == Ok("release") && value.trim().is_empty() {
            panic!("missing required release build variable: {key}");
        }
        println!("cargo:rustc-env={key}={value}");
    }
    tauri_build::build();
}
