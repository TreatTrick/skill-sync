// 内置 ignore + 用户全局 ignore。
// glob 语义：`*` 匹配单个 path segment 内的任意字符（不跨越 `/`），`?` 匹配单个字符，
// `**` 折叠为 `*`（同样不跨越 `/`）。pattern 与 text 按 `/` 拆段后必须段数相等。

/// 内置 ignore 规则，与 `config::default_ignore` 保持一致。
/// 两个副本在 Task 13 AppConfig 原子迁移时合并到此处。
pub(crate) fn default_ignore() -> Vec<String> {
    vec![
        "**/.git/**".into(),
        "**/node_modules/**".into(),
        "**/.env".into(),
        "**/.DS_Store".into(),
        "**/cache/**".into(),
        "**/.cache/**".into(),
        "**/tmp/**".into(),
        "**/temp/**".into(),
        "**/*.log".into(),
    ]
}

/// 判断相对路径是否被内置 ignore + 用户 ignore 命中。
/// `rel_path` 使用 `/` 分隔（调用方负责传入包含 skill folder 前缀的 namespace 相对路径）。
pub(crate) fn is_ignored(rel_path: &str, user_ignore: &[String]) -> bool {
    let normalized = rel_path.replace('\\', "/");
    for pat in default_ignore().iter().chain(user_ignore.iter()) {
        if glob_match(pat, &normalized) {
            return true;
        }
    }
    false
}

/// 段感知 glob：`*` 与 `?` 不跨越 `/`，`**` 折叠为 `*`。
/// pattern 与 text 段数必须相等，逐段匹配。
pub(crate) fn glob_match(pattern: &str, text: &str) -> bool {
    let pat_segs: Vec<&str> = pattern.split('/').collect();
    let txt_segs: Vec<&str> = text.split('/').collect();
    if pat_segs.len() != txt_segs.len() {
        return false;
    }
    pat_segs
        .iter()
        .zip(txt_segs.iter())
        .all(|(p, t)| seg_match(p, t))
}

/// 单段匹配：`*` 匹配段内任意字符序列，`?` 匹配单字符。段内不含 `/`。
fn seg_match(pat: &str, text: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let mut pi = 0;
    let mut ti = 0;
    let mut star: Option<usize> = None;
    let mut st = 0;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            st = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            st += 1;
            ti = st;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_ignore_excludes_cache_temp_and_logs() {
        assert!(is_ignored("skillA/cache/data.bin", &[]));
        assert!(is_ignored("skillA/.cache/data.bin", &[]));
        assert!(is_ignored("skillA/tmp/file", &[]));
        assert!(is_ignored("skillA/output.log", &[]));
    }

    #[test]
    fn user_ignore_excludes_specific_skill_cache() {
        let rules = vec!["**/skillA/cache/**".to_string()];
        assert!(is_ignored("skills/skillA/cache/big.bin", &rules));
        assert!(!is_ignored("skills/skillB/cache/big.bin", &rules));
    }

    #[test]
    fn default_ignore_contains_core_rules() {
        let rules = default_ignore();
        assert!(rules.iter().any(|r| r == "**/.git/**"));
        assert!(rules.iter().any(|r| r == "**/cache/**"));
        assert!(rules.iter().any(|r| r == "**/*.log"));
    }

    #[test]
    fn glob_does_not_cross_slash() {
        // `**/cache/**` 仅匹配 cache 恰在第二段（含一段前缀）的路径。
        assert!(glob_match("**/cache/**", "skillA/cache/data.bin"));
        assert!(!glob_match("**/cache/**", "skills/skillB/cache/big.bin"));
        // 段数不等直接不匹配。
        assert!(!glob_match("**/cache/**", "cache/data"));
    }

    #[test]
    fn glob_matches_star_suffix() {
        assert!(glob_match("**/*.log", "skillA/output.log"));
        assert!(!glob_match("**/*.log", "skillA/output.txt"));
    }
}
