use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

use crate::config::LimitsConfig;
use crate::errors::{AppError, Result};
use crate::ignore::is_ignored;
use crate::portable_path::{normalize, validate_portable_path, validate_portable_paths};

/// 待打包的单个 skill 输入。
#[allow(dead_code)] // Task 8/9 接入 build_plan/apply_plan 后使用
#[derive(Debug, Clone)]
pub(crate) struct SkillPackInput {
    pub skill_id: String,
    pub source_path: PathBuf,
}

/// 打包选项：资源 limit 与用户全局 ignore。
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct PackOptions {
    pub limits: LimitsConfig,
    pub user_ignore: Vec<String>,
}

/// 已打包 skill 的 canonical zip 描述。`zip_path` 指向 task dir 内的临时文件。
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct PackedSkill {
    pub skill_id: String,
    pub hash: String,
    pub zip_path: PathBuf,
    pub zip_size: u64,
    pub file_count: usize,
    pub unpacked_size: u64,
    pub ignored_files: usize,
    pub ignored_bytes: u64,
    pub warnings: Vec<PackWarning>,
}

/// 疑似密钥/Token warning；只记录相对路径与类型，不含原文，避免 Debug 泄露。
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct PackWarning {
    pub relative_path: String,
    pub kind: SecretWarningKind,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SecretWarningKind {
    PrivateKey,
    Pem,
    Token,
}

/// 单个 skill 打包被阻止的原因。
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct PackBlocked {
    pub skill_id: String,
    pub reason: String,
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum PackOutcome {
    Packed(PackedSkill),
    Blocked(PackBlocked),
}

/// 拥有 `temp_dir/skill-sync/<uuid>` 并在 Drop 时递归清理的 RAII guard。
/// 成功、错误与提前返回路径都依赖同一 Drop 语义。
pub(crate) struct PackTaskDir {
    path: PathBuf,
}

impl PackTaskDir {
    fn new() -> Result<Self> {
        let base = std::env::temp_dir().join("skill-sync");
        fs::create_dir_all(&base)?;
        let path = base.join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&path)?;
        Ok(Self { path })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PackTaskDir {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}

/// 一批打包结果：同时持有 task dir 与 outcomes，返回时 zip 仍存在；
/// batch drop 时统一清理临时目录。
#[allow(dead_code)]
pub(crate) struct PackBatch {
    pub task_dir: PackTaskDir,
    pub outcomes: Vec<PackOutcome>,
}

pub(crate) struct SkillPacker;

impl SkillPacker {
    /// 批量打包。task dir 在函数入口创建；任一 systemic 错误经 `?` 提前返回时，
    /// RAII 自动清理 task dir。成功则把 task dir 移入 PackBatch 由调用方持有。
    #[allow(dead_code)]
    pub(crate) fn pack_batch(
        inputs: &[SkillPackInput],
        options: &PackOptions,
    ) -> Result<PackBatch> {
        let task_dir = PackTaskDir::new()?;
        let mut outcomes = Vec::with_capacity(inputs.len());
        for input in inputs {
            let outcome = pack_one(input, options, &task_dir)?;
            outcomes.push(outcome);
        }
        Ok(PackBatch { task_dir, outcomes })
    }
}

/// 解包报告：实际写入的文件数与累计字节。
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct UnpackReport {
    pub file_count: usize,
    pub unpacked_size: u64,
}

fn pack_one(
    input: &SkillPackInput,
    options: &PackOptions,
    task_dir: &PackTaskDir,
) -> Result<PackOutcome> {
    let folder = input
        .source_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let blocked = |reason: String| {
        PackOutcome::Blocked(PackBlocked {
            skill_id: input.skill_id.clone(),
            reason,
        })
    };

    let walk = collect_skill_entries(&input.source_path, &folder, options)?;
    let (mut entries, ignored_files, ignored_bytes) = match walk {
        WalkResult::Ok {
            entries,
            ignored_files,
            ignored_bytes,
        } => (entries, ignored_files, ignored_bytes),
        WalkResult::Blocked(reason) => return Ok(blocked(reason)),
    };

    // 按 NFC 规范化后的 UTF-8 bytes 排序，保证 zip 字节稳定。
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    // 完整路径 NFC+lowercase 折叠 collision 校验。
    let rels: Vec<&str> = entries.iter().map(|(r, _)| r.as_str()).collect();
    if let Err(e) = validate_portable_paths(&rels) {
        return Ok(blocked(e.to_string()));
    }

    let warnings = scan_secrets(&entries);

    let zip_path = task_dir
        .path()
        .join(format!("{}.skill.zip", uuid::Uuid::new_v4()));
    write_canonical_zip(&zip_path, &entries)?;
    let zip_bytes = fs::read(&zip_path)?;
    let zip_size = zip_bytes.len() as u64;
    if zip_size > options.limits.max_skill_zip_bytes {
        return Ok(blocked(format!(
            "skill zip exceeds max_skill_zip_bytes ({})",
            options.limits.max_skill_zip_bytes
        )));
    }
    let hash = format!("sha256:{}", hex::encode(Sha256::digest(&zip_bytes)));
    let file_count = entries.len();
    let unpacked_size: u64 = entries.iter().map(|(_, c)| c.len() as u64).sum();

    Ok(PackOutcome::Packed(PackedSkill {
        skill_id: input.skill_id.clone(),
        hash,
        zip_path,
        zip_size,
        file_count,
        unpacked_size,
        ignored_files,
        ignored_bytes,
        warnings,
    }))
}

enum WalkResult {
    Ok {
        entries: Vec<(String, Vec<u8>)>,
        ignored_files: usize,
        ignored_bytes: u64,
    },
    Blocked(String),
}

fn collect_skill_entries(source: &Path, folder: &str, options: &PackOptions) -> Result<WalkResult> {
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut ignored_files = 0usize;
    let mut ignored_bytes = 0u64;
    match walk_dir(
        source,
        source,
        folder,
        options,
        &mut entries,
        &mut ignored_files,
        &mut ignored_bytes,
    )? {
        Some(reason) => Ok(WalkResult::Blocked(reason)),
        None => Ok(WalkResult::Ok {
            entries,
            ignored_files,
            ignored_bytes,
        }),
    }
}

/// 递归遍历 skill 目录。返回 `Some(reason)` 表示该 skill 被阻止。
fn walk_dir(
    root: &Path,
    current: &Path,
    folder: &str,
    options: &PackOptions,
    entries: &mut Vec<(String, Vec<u8>)>,
    ignored_files: &mut usize,
    ignored_bytes: &mut u64,
) -> Result<Option<String>> {
    let Ok(read_dir) = fs::read_dir(current) else {
        return Ok(Some(format!(
            "cannot read directory: {}",
            current.display()
        )));
    };
    for entry in read_dir {
        let entry = entry?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if is_symlink_or_reparse(&meta) {
            return Ok(Some(format!(
                "symlink or reparse point not allowed: {}",
                rel_of(root, &path)
            )));
        }
        let rel = normalize(&rel_of(root, &path));
        if meta.is_dir() {
            let dir_rel = format!("{folder}/{rel}/");
            if is_ignored(&dir_rel, &options.user_ignore) {
                continue;
            }
            if let Some(reason) = walk_dir(
                root,
                &path,
                folder,
                options,
                entries,
                ignored_files,
                ignored_bytes,
            )? {
                return Ok(Some(reason));
            }
        } else if meta.is_file() {
            let file_rel = format!("{folder}/{rel}");
            if is_ignored(&file_rel, &options.user_ignore) {
                *ignored_files += 1;
                *ignored_bytes += meta.len();
                continue;
            }
            if entries.len() + 1 > options.limits.max_skill_files {
                return Ok(Some(format!(
                    "skill exceeds max_skill_files ({})",
                    options.limits.max_skill_files
                )));
            }
            if let Err(e) = validate_portable_path(&rel) {
                return Ok(Some(e.to_string()));
            }
            let content =
                match read_file_capped(&path, options.limits.max_single_file_unpacked_bytes) {
                    Ok(c) => c,
                    Err(e) => return Ok(Some(e.to_string())),
                };
            let total: u64 =
                entries.iter().map(|(_, c)| c.len() as u64).sum::<u64>() + content.len() as u64;
            if total > options.limits.max_skill_unpacked_bytes {
                return Ok(Some(format!(
                    "skill exceeds max_skill_unpacked_bytes ({})",
                    options.limits.max_skill_unpacked_bytes
                )));
            }
            entries.push((rel, content));
        } else {
            return Ok(Some(format!("unsupported file type: {rel}")));
        }
    }
    Ok(None)
}

fn rel_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// 读取文件并按单文件上限截断；超过返回 `Blocked`。不读取 symlink 目标。
fn read_file_capped(path: &Path, max: u64) -> Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut buf = Vec::new();
    let mut chunk = vec![0u8; 8192];
    loop {
        let n = file.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() as u64 > max {
            return Err(AppError::Blocked(format!(
                "file exceeds single-file unpacked limit ({} bytes): {}",
                max,
                path.display()
            )));
        }
    }
    Ok(buf)
}

fn is_symlink_or_reparse(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// 扫描 included 文本文件的私钥/PEM/常见 Token，产生不含原文的结构化 warning。
fn scan_secrets(entries: &[(String, Vec<u8>)]) -> Vec<PackWarning> {
    let mut warnings = Vec::new();
    for (rel, content) in entries {
        let Ok(text) = std::str::from_utf8(content) else {
            continue;
        };
        if text.contains("-----BEGIN") && text.contains("PRIVATE KEY") {
            warnings.push(PackWarning {
                relative_path: rel.clone(),
                kind: SecretWarningKind::PrivateKey,
            });
        } else if text.contains("-----BEGIN") && text.contains("CERTIFICATE") {
            warnings.push(PackWarning {
                relative_path: rel.clone(),
                kind: SecretWarningKind::Pem,
            });
        }
        const TOKEN_PATTERNS: &[&str] = &[
            "github_pat_",
            "ghp_",
            "gho_",
            "ghs_",
            "ghr_",
            "glpat-",
            "xox",
            "AKIA",
        ];
        if TOKEN_PATTERNS.iter().any(|p| text.contains(p)) {
            warnings.push(PackWarning {
                relative_path: rel.clone(),
                kind: SecretWarningKind::Token,
            });
        }
    }
    warnings
}

fn epoch_dt() -> Result<DateTime> {
    DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0)
        .map_err(|e| AppError::Vault(format!("invalid canonical epoch: {e}")))
}

/// 写入 canonical zip：固定 epoch 时间戳、Unix creator、regular file + 0o644、
/// Deflated level 6、无目录项/comment/extra。
fn write_canonical_zip(zip_path: &Path, entries: &[(String, Vec<u8>)]) -> Result<()> {
    let file = File::create(zip_path)?;
    let mut zip = ZipWriter::new(file);
    let epoch = epoch_dt()?;
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(6))
        .last_modified_time(epoch)
        .unix_permissions(0o644);
    for (rel, content) in entries {
        zip.start_file(rel.as_str(), options).map_err(zip_err)?;
        zip.write_all(content)?;
    }
    let mut inner = zip.finish().map_err(zip_err)?;
    inner.flush()?;
    Ok(())
}

/// 安全解包：先解析原始 central directory 校验 canonical 元数据、declared 大小、
/// 重复/folded collision（`ZipArchive` 会静默去重同名 entry，故必须自行解析），
/// 再用 `ZipArchive` + 限量 reader 流式校验实际字节（不信任 header）。
/// 任一失败清空 task temp，正式 target 不变。
#[allow(dead_code)]
pub(crate) fn unpack_skill(
    zip_path: &Path,
    task_target: &Path,
    limits: &LimitsConfig,
) -> Result<UnpackReport> {
    let zip_bytes = fs::read(zip_path)?;
    if zip_bytes.len() as u64 > limits.max_skill_zip_bytes {
        return clear_and_block(
            task_target,
            format!(
                "zip exceeds max_skill_zip_bytes ({})",
                limits.max_skill_zip_bytes
            ),
        );
    }
    let raw = parse_central_directory(&zip_bytes)?;
    if raw.len() > limits.max_skill_files {
        return clear_and_block(
            task_target,
            format!("zip has too many entries ({})", limits.max_skill_files),
        );
    }

    // 阶段一：原始 central directory 校验 canonical 元数据与 declared 大小。
    let mut declared_total: u64 = 0;
    let mut names: Vec<String> = Vec::with_capacity(raw.len());
    for r in &raw {
        let normalized = normalize(&r.name);
        if r.name.ends_with('/') {
            return clear_and_block(task_target, "directory entry not allowed".into());
        }
        if r.method != 8 {
            return clear_and_block(task_target, "non-deflated entry not allowed".into());
        }
        if (r.mod_time, r.mod_date) != (0, 33) {
            return clear_and_block(task_target, "non-canonical timestamp".into());
        }
        if r.external_attrs >> 16 != 0o100644 {
            return clear_and_block(task_target, "non-canonical file mode".into());
        }
        if !r.comment.is_empty() {
            return clear_and_block(task_target, "entry comment not allowed".into());
        }
        if !r.extra.is_empty() {
            return clear_and_block(task_target, "entry extra field not allowed".into());
        }
        if validate_portable_path(&normalized).is_err() {
            return clear_and_block(task_target, "non-portable entry name".into());
        }
        if u64::from(r.uncompressed_size) > limits.max_single_file_unpacked_bytes {
            return clear_and_block(task_target, "entry exceeds single-file limit".into());
        }
        declared_total += u64::from(r.uncompressed_size);
        if declared_total > limits.max_skill_unpacked_bytes {
            return clear_and_block(task_target, "zip exceeds total unpacked limit".into());
        }
        if names.contains(&normalized) {
            return clear_and_block(task_target, "duplicate entry name".into());
        }
        names.push(normalized);
    }
    let rels: Vec<&str> = names.iter().map(|n| n.as_str()).collect();
    if validate_portable_paths(&rels).is_err() {
        return clear_and_block(task_target, "entry name collision".into());
    }

    // 阶段二：`ZipArchive` 解压 + 限量 reader 流式校验实际字节。
    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(cursor).map_err(zip_err)?;
    if archive.len() != names.len() {
        return clear_and_block(task_target, "central directory mismatch".into());
    }
    let mut actual_total: u64 = 0;
    let mut file_count = 0usize;
    for (i, name) in names.iter().enumerate() {
        let mut zf = archive.by_index(i).map_err(zip_err)?;
        let target_path = task_target.join(name);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create(&target_path)?;
        let mut written: u64 = 0;
        let mut buf = [0u8; 8192];
        let mut exceeded = false;
        loop {
            let n = zf.read(&mut buf)?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            written += n as u64;
            actual_total += n as u64;
            if written > limits.max_single_file_unpacked_bytes
                || actual_total > limits.max_skill_unpacked_bytes
            {
                exceeded = true;
                break;
            }
        }
        file.flush()?;
        if exceeded {
            return clear_and_block(task_target, "actual bytes exceed unpacked limit".into());
        }
        file_count += 1;
    }
    Ok(UnpackReport {
        file_count,
        unpacked_size: actual_total,
    })
}

/// 原始 central directory 条目（直接解析字节，不受 `ZipArchive` 去重影响）。
pub(crate) struct RawCdEntry {
    pub name: String,
    #[allow(dead_code)] // 仅 inspect_zip 测试读取；Task 9 接入后生产使用
    pub creator_system: u8,
    pub method: u16,
    pub mod_time: u16,
    pub mod_date: u16,
    pub external_attrs: u32,
    pub extra: Vec<u8>,
    pub comment: Vec<u8>,
    pub uncompressed_size: u32,
}

/// 解析 central directory 全部条目。EOCD 找不到或签名不匹配返回 `Vault` 错误。
pub(crate) fn parse_central_directory(bytes: &[u8]) -> Result<Vec<RawCdEntry>> {
    let eocd = find_eocd(bytes)
        .ok_or_else(|| AppError::Vault("zip end of central directory not found".into()))?;
    let cd_offset = le_u32(&bytes[eocd + 16..eocd + 20])? as usize;
    let mut out = Vec::new();
    let mut pos = cd_offset;
    while pos + 46 <= bytes.len() && bytes[pos..pos + 4] == [0x50, 0x4b, 0x01, 0x02] {
        let version_made_by = le_u16(&bytes[pos + 4..pos + 6])?;
        let creator_system = (version_made_by >> 8) as u8;
        let method = le_u16(&bytes[pos + 10..pos + 12])?;
        let mod_time = le_u16(&bytes[pos + 12..pos + 14])?;
        let mod_date = le_u16(&bytes[pos + 14..pos + 16])?;
        let uncompressed_size = le_u32(&bytes[pos + 24..pos + 28])?;
        let name_len = le_u16(&bytes[pos + 28..pos + 30])? as usize;
        let extra_len = le_u16(&bytes[pos + 30..pos + 32])? as usize;
        let comment_len = le_u16(&bytes[pos + 32..pos + 34])? as usize;
        let external_attrs = le_u32(&bytes[pos + 38..pos + 42])?;
        let name_start = pos + 46;
        let name = String::from_utf8_lossy(&bytes[name_start..name_start + name_len]).to_string();
        let extra = bytes[name_start + name_len..name_start + name_len + extra_len].to_vec();
        let comment = bytes
            [name_start + name_len + extra_len..name_start + name_len + extra_len + comment_len]
            .to_vec();
        out.push(RawCdEntry {
            name,
            creator_system,
            method,
            mod_time,
            mod_date,
            external_attrs,
            extra,
            comment,
            uncompressed_size,
        });
        pos = name_start + name_len + extra_len + comment_len;
    }
    Ok(out)
}

fn le_u16(bytes: &[u8]) -> Result<u16> {
    bytes
        .try_into()
        .map(u16::from_le_bytes)
        .map_err(|_| AppError::Vault("invalid zip little-endian u16 slice".into()))
}

fn le_u32(bytes: &[u8]) -> Result<u32> {
    bytes
        .try_into()
        .map(u32::from_le_bytes)
        .map_err(|_| AppError::Vault("invalid zip little-endian u32 slice".into()))
}

fn find_eocd(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 22 {
        return None;
    }
    let start = bytes.len().saturating_sub(22 + 65535);
    (start..=bytes.len() - 22)
        .rev()
        .find(|&i| bytes[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])
}

/// 清空 task temp 内所有内容并以 `Blocked` 返回，保证正式 target 不残留。
fn clear_and_block(task_target: &Path, reason: String) -> Result<UnpackReport> {
    if let Ok(entries) = fs::read_dir(task_target) {
        for e in entries.flatten() {
            let path = e.path();
            // entry 可能是文件或目录，分别用 remove_file / remove_dir_all 清理。
            let remove = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
            drop(remove);
        }
    }
    Err(AppError::Blocked(reason))
}

fn zip_err(e: zip::result::ZipError) -> AppError {
    AppError::Vault(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn test_limits() -> LimitsConfig {
        LimitsConfig::default()
    }

    fn limits_with_zip_bytes(n: u64) -> LimitsConfig {
        let mut l = test_limits();
        l.max_skill_zip_bytes = n;
        l
    }

    fn tiny_unpack_limits() -> LimitsConfig {
        LimitsConfig {
            max_skill_zip_bytes: 1024 * 1024,
            max_skill_files: 5,
            max_single_file_unpacked_bytes: 16,
            max_skill_unpacked_bytes: 32,
            max_auto_delete: 10,
        }
    }

    fn pack_options(limits: &LimitsConfig) -> PackOptions {
        PackOptions {
            limits: limits.clone(),
            user_ignore: vec![],
        }
    }

    fn skill_fixture_with_files<const N: usize>(files: [(&str, &str); N]) -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (rel, content) in files {
            let path = dir.path().join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, content).unwrap();
        }
        dir
    }

    fn pack_input(path: &Path) -> SkillPackInput {
        SkillPackInput {
            skill_id: "test:demo".into(),
            source_path: path.to_path_buf(),
        }
    }

    /// 打包单个 skill 并持有整个 PackBatch（含 task dir），返回 batch。
    /// 调用方在读取 `packed.zip_path` 期间必须保持 batch 存活。
    fn pack_batch_single(path: &Path, limits: &LimitsConfig) -> Result<PackBatch> {
        SkillPacker::pack_batch(&[pack_input(path)], &pack_options(limits))
    }

    fn packed_of(batch: &PackBatch) -> &PackedSkill {
        match &batch.outcomes[0] {
            PackOutcome::Packed(p) => p,
            PackOutcome::Blocked(b) => panic!("expected packed, got blocked: {}", b.reason),
        }
    }

    fn outcome_of(batch: &PackBatch) -> &PackOutcome {
        &batch.outcomes[0]
    }

    /// 仅用于只关心 Blocked 的场景：返回 outcome 并丢弃 batch（zip 随之清理）。
    fn pack_fixture_outcome(path: &Path, limits: &LimitsConfig) -> Result<PackOutcome> {
        let batch = pack_batch_single(path, limits)?;
        Ok(batch.outcomes.into_iter().next().unwrap())
    }

    fn epoch_dt_test() -> DateTime {
        DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0).unwrap()
    }

    fn canonical_options() -> SimpleFileOptions {
        SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(6))
            .last_modified_time(epoch_dt_test())
            .unix_permissions(0o644)
    }

    // ---- golden / stability tests ----

    #[test]
    fn pack_bytes_are_stable_across_write_order_mtime_and_permissions() {
        let a =
            skill_fixture_with_files([("SKILL.md", "name: demo"), ("scripts/run.sh", "echo ok")]);
        let b =
            skill_fixture_with_files([("scripts/run.sh", "echo ok"), ("SKILL.md", "name: demo")]);
        set_distinct_mtime_and_permissions(a.path(), b.path()).unwrap();
        let batch_a = pack_batch_single(a.path(), &test_limits()).unwrap();
        let batch_b = pack_batch_single(b.path(), &test_limits()).unwrap();
        let first = packed_of(&batch_a);
        let second = packed_of(&batch_b);
        assert_eq!(
            fs::read(&first.zip_path).unwrap(),
            fs::read(&second.zip_path).unwrap()
        );
        assert_eq!(first.hash, second.hash);
        // golden 字节比较由 golden_demo_zip_matches_packed_bytes 锁定（见下方）。
    }

    /// 一次性生成 checked-in golden zip。运行：
    /// `cargo test --manifest-path src-tauri/Cargo.toml pack::tests::write_golden_demo_zip -- --ignored`
    /// 不得在普通 `cargo test` 中重写 golden。
    #[ignore]
    #[test]
    fn write_golden_demo_zip() {
        let skill =
            skill_fixture_with_files([("SKILL.md", "name: demo"), ("scripts/run.sh", "echo ok")]);
        let batch = pack_batch_single(skill.path(), &test_limits()).unwrap();
        let packed = packed_of(&batch);
        let target =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/canonical/demo.skill.zip");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, fs::read(&packed.zip_path).unwrap()).unwrap();
    }

    #[test]
    fn golden_demo_zip_matches_packed_bytes() {
        let skill =
            skill_fixture_with_files([("SKILL.md", "name: demo"), ("scripts/run.sh", "echo ok")]);
        let batch = pack_batch_single(skill.path(), &test_limits()).unwrap();
        let packed = packed_of(&batch);
        assert_eq!(
            fs::read(&packed.zip_path).unwrap(),
            include_bytes!("../tests/fixtures/canonical/demo.skill.zip")
        );
    }

    #[test]
    fn canonical_zip_has_fixed_metadata_and_no_directory_entries() {
        let skill =
            skill_fixture_with_files([("SKILL.md", "name: demo"), ("nested/run.sh", "echo ok")]);
        let batch = pack_batch_single(skill.path(), &test_limits()).unwrap();
        let packed = packed_of(&batch);
        let inspection = inspect_zip(&packed.zip_path);
        assert_eq!(inspection.names(), ["SKILL.md", "nested/run.sh"]);
        assert!(inspection.iter().all(|e| e.timestamp == zip_epoch()));
        assert!(inspection
            .iter()
            .all(|e| e.creator_system == CreatorSystem::Unix));
        assert!(inspection
            .iter()
            .all(|e| e.file_type == ZipFileType::Regular));
        assert!(inspection.iter().all(|e| e.unix_mode == 0o644));
        assert!(inspection.iter().all(|e| e.method == 8)); // 8 == Deflated
        assert!(inspection
            .iter()
            .all(|e| e.comment.is_empty() && e.extra.is_empty()));
    }

    #[test]
    fn ignored_cache_does_not_affect_hash_or_size() {
        let skill = skill_fixture_with_files([("SKILL.md", "name: demo"), ("cache/data", "one")]);
        let batch_a = pack_batch_single(skill.path(), &test_limits()).unwrap();
        let first_hash = packed_of(&batch_a).hash.clone();
        let first_size = packed_of(&batch_a).zip_size;
        fs::write(skill.path().join("cache/data"), "different and larger").unwrap();
        let batch_b = pack_batch_single(skill.path(), &test_limits()).unwrap();
        let second = packed_of(&batch_b);
        assert_eq!(
            (first_hash, first_size),
            (second.hash.clone(), second.zip_size)
        );
    }

    #[test]
    fn oversized_pack_is_blocked() {
        let skill =
            skill_fixture_with_files([("SKILL.md", "name: demo"), ("payload.bin", "not-empty")]);
        let batch = pack_batch_single(skill.path(), &limits_with_zip_bytes(1)).unwrap();
        assert!(matches!(outcome_of(&batch), PackOutcome::Blocked(_)));
    }

    #[test]
    fn pack_warns_about_probable_secrets_without_blocking() {
        let skill = skill_fixture_with_files([
            ("SKILL.md", "name: demo"),
            (
                "id.pem",
                "-----BEGIN PRIVATE KEY-----\nredacted\n-----END PRIVATE KEY-----",
            ),
            ("token.txt", "github_pat_REDACTED_TEST_VALUE"),
        ]);
        let batch = pack_batch_single(skill.path(), &test_limits()).unwrap();
        let packed = packed_of(&batch);
        assert_eq!(packed.warnings.len(), 2);
        assert!(packed
            .warnings
            .iter()
            .all(|w| !format!("{w:?}").contains("REDACTED_TEST_VALUE")));
    }

    // ---- symlink / reparse rejection ----

    #[cfg(unix)]
    #[test]
    fn pack_rejects_symlink_without_reading_target() {
        let skill = symlink_escape_fixture("outside-secret");
        assert!(matches!(
            pack_fixture_outcome(skill.path(), &test_limits()).unwrap(),
            PackOutcome::Blocked(_)
        ));
        assert!(!pack_debug_output().contains("outside-secret"));
    }

    #[cfg(windows)]
    #[test]
    fn pack_rejects_junction_and_other_reparse_points() {
        match junction_escape_fixture() {
            Some(skill) => {
                assert!(matches!(
                    pack_fixture_outcome(skill.path(), &test_limits()).unwrap(),
                    PackOutcome::Blocked(_)
                ));
            }
            None => {
                // 当前环境无创建 symlink 的权限，跳过；检测逻辑仍由 is_symlink_or_reparse 覆盖。
            }
        }
    }

    #[cfg(unix)]
    fn symlink_escape_fixture(_target_name: &str) -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        // 悬空 symlink：目标不存在也足以被 symlink_metadata 识别为 symlink。
        // 不读取目标，避免 target 名称进入任何字符串。
        std::os::unix::fs::symlink("/nonexistent/outside-secret", dir.path().join("escape"))
            .unwrap();
        dir
    }

    #[cfg(unix)]
    fn pack_debug_output() -> String {
        let skill = symlink_escape_fixture("outside-secret");
        let outcome = pack_fixture_outcome(skill.path(), &test_limits()).unwrap();
        format!("{outcome:?}")
    }

    #[cfg(windows)]
    fn junction_escape_fixture() -> Option<TempDir> {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("outside");
        fs::create_dir_all(&target).ok()?;
        std::os::windows::fs::symlink_dir(&target, dir.path().join("escape")).ok()?;
        Some(dir)
    }

    // ---- RAII cleanup ----

    #[derive(Debug, Clone, Copy)]
    enum PackTestExit {
        Success,
        Error,
        EarlyReturn,
    }

    #[test]
    fn pack_task_dir_is_removed_on_success_error_and_early_return() {
        for exit in [
            PackTestExit::Success,
            PackTestExit::Error,
            PackTestExit::EarlyReturn,
        ] {
            let path = run_pack_scope(exit);
            assert!(!path.exists(), "task dir leaked for {exit:?}");
        }
    }

    fn run_pack_scope(exit: PackTestExit) -> PathBuf {
        let dir = PackTaskDir::new().unwrap();
        let path = dir.path().to_path_buf();
        fs::write(dir.path().join("marker"), "x").unwrap();
        match exit {
            PackTestExit::Success => {
                // 正常离开作用域，drop 清理。
            }
            PackTestExit::Error | PackTestExit::EarlyReturn => {
                // 模拟 `?` 提前返回：函数返回时 drop 仍清理。
                return path;
            }
        }
        drop(dir);
        path
    }

    // ---- unpack rejection tests ----

    #[test]
    fn unpack_rejects_zip_slip_paths() {
        let zip = zip_fixture([("../evil", b"bad".as_slice())]);
        let target = tempfile::tempdir().unwrap();
        assert!(matches!(
            unpack_skill(zip.path(), target.path(), &test_limits()),
            Err(AppError::Blocked(_))
        ));
        assert!(!target.path().join("../evil").exists());
    }

    #[test]
    fn unpack_rejects_symlink_special_directory_and_case_collision_entries() {
        let fixtures: [(&str, ZipFixture); 5] = [
            ("duplicate", duplicate_entry_zip_fixture()),
            ("symlink", symlink_zip_fixture()),
            ("device", device_zip_fixture()),
            ("directory", explicit_directory_fixture()),
            ("case_collision", case_collision_zip_fixture()),
        ];
        for (label, zip) in fixtures {
            let target = tempfile::tempdir().unwrap();
            let result = unpack_skill(zip.path(), target.path(), &test_limits());
            assert!(result.is_err(), "{label}: expected err, got {result:?}");
            assert!(
                target.path().read_dir().unwrap().next().is_none(),
                "{label}: target not cleared"
            );
        }
    }

    #[test]
    fn unpack_enforces_file_single_and_total_actual_byte_limits() {
        let fixtures: [(&str, ZipFixture); 4] = [
            ("too_many", too_many_entries_fixture()),
            ("oversized_single", oversized_single_file_fixture()),
            ("oversized_total", oversized_total_fixture()),
            ("lying_header", lying_header_fixture()),
        ];
        for (label, zip) in fixtures {
            let target = tempfile::tempdir().unwrap();
            let result = unpack_skill(zip.path(), target.path(), &tiny_unpack_limits());
            assert!(result.is_err(), "{label}: expected err, got {result:?}");
            assert!(
                target.path().read_dir().unwrap().next().is_none(),
                "{label}: target not cleared"
            );
        }
    }

    #[test]
    fn unpack_canonical_pack_roundtrips() {
        let skill =
            skill_fixture_with_files([("SKILL.md", "name: demo"), ("scripts/run.sh", "echo ok")]);
        let batch = pack_batch_single(skill.path(), &test_limits()).unwrap();
        let packed = packed_of(&batch);
        let target = tempfile::tempdir().unwrap();
        let report = unpack_skill(&packed.zip_path, target.path(), &test_limits()).unwrap();
        assert_eq!(report.file_count, 2);
        assert_eq!(
            fs::read_to_string(target.path().join("SKILL.md")).unwrap(),
            "name: demo"
        );
        assert_eq!(
            fs::read_to_string(target.path().join("scripts/run.sh")).unwrap(),
            "echo ok"
        );
    }

    // ---- zip fixture helpers ----

    struct ZipFixture {
        _dir: TempDir,
        path: PathBuf,
    }

    impl ZipFixture {
        fn path(&self) -> &Path {
            &self.path
        }
    }

    fn write_zip_fixture(bytes: Vec<u8>) -> ZipFixture {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.zip");
        fs::write(&path, &bytes).unwrap();
        ZipFixture { _dir: dir, path }
    }

    fn zip_fixture<const N: usize>(entries: [(&str, &[u8]); N]) -> ZipFixture {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.zip");
        let file = File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = canonical_options();
        for (name, content) in entries {
            zip.start_file(name, opts).unwrap();
            zip.write_all(content).unwrap();
        }
        let mut f = zip.finish().unwrap();
        drop(f.flush());
        ZipFixture { _dir: dir, path }
    }

    fn symlink_zip_fixture() -> ZipFixture {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.zip");
        let file = File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .last_modified_time(epoch_dt_test());
        zip.add_symlink("link", "target", opts).unwrap();
        let mut f = zip.finish().unwrap();
        drop(f.flush());
        ZipFixture { _dir: dir, path }
    }

    fn explicit_directory_fixture() -> ZipFixture {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.zip");
        let file = File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.add_directory("sub/", canonical_options()).unwrap();
        let mut f = zip.finish().unwrap();
        drop(f.flush());
        ZipFixture { _dir: dir, path }
    }

    fn case_collision_zip_fixture() -> ZipFixture {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.zip");
        let file = File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = canonical_options();
        zip.start_file("Docs/a", opts).unwrap();
        zip.write_all(b"x").unwrap();
        zip.start_file("docs/A", opts).unwrap();
        zip.write_all(b"y").unwrap();
        let mut f = zip.finish().unwrap();
        drop(f.flush());
        ZipFixture { _dir: dir, path }
    }

    fn too_many_entries_fixture() -> ZipFixture {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.zip");
        let file = File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = canonical_options();
        for i in 0..6 {
            zip.start_file(format!("f{i}.txt"), opts).unwrap();
            zip.write_all(b"x").unwrap();
        }
        let mut f = zip.finish().unwrap();
        drop(f.flush());
        ZipFixture { _dir: dir, path }
    }

    fn oversized_single_file_fixture() -> ZipFixture {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.zip");
        let file = File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.start_file("big.txt", canonical_options()).unwrap();
        zip.write_all(&[b'A'; 17]).unwrap();
        let mut f = zip.finish().unwrap();
        drop(f.flush());
        ZipFixture { _dir: dir, path }
    }

    fn oversized_total_fixture() -> ZipFixture {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.zip");
        let file = File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = canonical_options();
        for name in ["a.txt", "b.txt", "c.txt"] {
            zip.start_file(name, opts).unwrap();
            zip.write_all(&[b'A'; 12]).unwrap();
        }
        let mut f = zip.finish().unwrap();
        drop(f.flush());
        ZipFixture { _dir: dir, path }
    }

    // ---- raw zip builder for adversarial (non-canonical) fixtures ----

    struct RawEntrySpec {
        name: String,
        content: Vec<u8>,
        method: u16,
        external_mode: u32,
        forged_uncompressed_size: Option<u32>,
    }

    fn raw_spec(
        name: &str,
        content: &[u8],
        method: u16,
        external_mode: u32,
        forged: Option<u32>,
    ) -> RawEntrySpec {
        RawEntrySpec {
            name: name.into(),
            content: content.to_vec(),
            method,
            external_mode,
            forged_uncompressed_size: forged,
        }
    }

    fn build_raw_zip(specs: &[RawEntrySpec]) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        let mut central: Vec<u8> = Vec::new();
        for spec in specs {
            let name = spec.name.as_bytes();
            let (method, data, crc, uncompressed) = match spec.method {
                0 => {
                    let crc = crc32fast::hash(&spec.content);
                    (0u16, spec.content.clone(), crc, spec.content.len() as u32)
                }
                8 => {
                    let mut enc =
                        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::new(6));
                    enc.write_all(&spec.content).unwrap();
                    let compressed = enc.finish().unwrap();
                    let crc = crc32fast::hash(&spec.content);
                    (8u16, compressed, crc, spec.content.len() as u32)
                }
                _ => panic!("unsupported raw method"),
            };
            let declared = spec.forged_uncompressed_size.unwrap_or(uncompressed);
            let local_offset = out.len() as u32;
            // local file header
            out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes()); // mod_time (epoch)
            out.extend_from_slice(&33u16.to_le_bytes()); // mod_date (1980-01-01)
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&declared.to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name);
            out.extend_from_slice(&data);
            // central directory header
            central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
            central.extend_from_slice(&((3u16 << 8) | 20).to_le_bytes()); // Unix + v2.0
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&method.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&33u16.to_le_bytes());
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central.extend_from_slice(&declared.to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes()); // extra_len
            central.extend_from_slice(&0u16.to_le_bytes()); // comment_len
            central.extend_from_slice(&0u16.to_le_bytes()); // disk
            central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
            central.extend_from_slice(&(spec.external_mode << 16).to_le_bytes());
            central.extend_from_slice(&local_offset.to_le_bytes());
            central.extend_from_slice(name);
        }
        let cd_offset = out.len() as u32;
        let cd_size = central.len() as u32;
        out.extend_from_slice(&central);
        // end of central directory
        out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(specs.len() as u16).to_le_bytes());
        out.extend_from_slice(&(specs.len() as u16).to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    fn duplicate_entry_zip_fixture() -> ZipFixture {
        let bytes = build_raw_zip(&[
            raw_spec("a.txt", b"first", 8, 0o100644, None),
            raw_spec("a.txt", b"second", 8, 0o100644, None),
        ]);
        write_zip_fixture(bytes)
    }

    fn device_zip_fixture() -> ZipFixture {
        let bytes = build_raw_zip(&[raw_spec("dev", b"", 8, 0o20644, None)]);
        write_zip_fixture(bytes)
    }

    fn lying_header_fixture() -> ZipFixture {
        let content = vec![b'A'; 100];
        let bytes = build_raw_zip(&[raw_spec("big.txt", &content, 8, 0o100644, Some(1))]);
        write_zip_fixture(bytes)
    }

    // ---- zip inspection helper (raw central directory parse) ----

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum CreatorSystem {
        Unix,
        Other,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ZipFileType {
        Regular,
        Directory,
        Symlink,
        Other,
    }

    struct ZipInspection {
        entries: Vec<ZipInspectionEntry>,
    }

    struct ZipInspectionEntry {
        name: String,
        timestamp: (u16, u16),
        creator_system: CreatorSystem,
        file_type: ZipFileType,
        unix_mode: u32,
        method: u16,
        comment: Vec<u8>,
        extra: Vec<u8>,
    }

    impl ZipInspection {
        fn names(&self) -> Vec<&str> {
            self.entries.iter().map(|e| e.name.as_str()).collect()
        }

        fn iter(&self) -> std::slice::Iter<'_, ZipInspectionEntry> {
            self.entries.iter()
        }
    }

    fn zip_epoch() -> (u16, u16) {
        (0, 33)
    }

    fn inspect_zip(path: &Path) -> ZipInspection {
        let bytes = fs::read(path).unwrap();
        let raw = parse_central_directory(&bytes).unwrap();
        let entries = raw
            .into_iter()
            .map(|r| {
                let mode = r.external_attrs >> 16;
                let file_type = if r.name.ends_with('/') {
                    ZipFileType::Directory
                } else if mode & 0o170000 == 0o120000 {
                    ZipFileType::Symlink
                } else if mode & 0o170000 == 0o100000 {
                    ZipFileType::Regular
                } else {
                    ZipFileType::Other
                };
                ZipInspectionEntry {
                    name: r.name,
                    timestamp: (r.mod_time, r.mod_date),
                    creator_system: if r.creator_system == 3 {
                        CreatorSystem::Unix
                    } else {
                        CreatorSystem::Other
                    },
                    file_type,
                    unix_mode: mode & 0o777,
                    method: r.method,
                    comment: r.comment,
                    extra: r.extra,
                }
            })
            .collect();
        ZipInspection { entries }
    }

    // ---- mtime / permissions helper for stability test ----

    fn set_distinct_mtime_and_permissions(a: &Path, b: &Path) -> std::io::Result<()> {
        let t1 = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        let t2 = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2_000_000);
        set_tree_modified(a, t1)?;
        set_tree_modified(b, t2)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for f in walk_files(a) {
                std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o600))?;
            }
            for f in walk_files(b) {
                std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        Ok(())
    }

    fn set_tree_modified(root: &Path, t: std::time::SystemTime) -> std::io::Result<()> {
        for f in walk_files(root) {
            // set_modified 需要写权限；默认 File::open 是只读。
            let file = std::fs::OpenOptions::new().write(true).open(&f)?;
            file.set_modified(t)?;
        }
        Ok(())
    }

    fn walk_files(root: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        walk_files_inner(root, &mut out);
        out
    }

    fn walk_files_inner(path: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(path) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk_files_inner(&p, out);
            } else {
                out.push(p);
            }
        }
    }
}
