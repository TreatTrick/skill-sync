use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::config::LimitsConfig;
use crate::errors::{AppError, Result};
use crate::ignore::is_ignored;
use crate::pack::{PackOptions, PackWarning, SecretWarningKind};
use crate::portable_path::{normalize, validate_portable_path};

pub(crate) const CACHE_CAPACITY_BYTES: u64 = 1024 * 1024 * 1024;
const CACHE_VERSION: u32 = 1;
const PACKER_VERSION: u32 = 1;
const CACHE_DIR_NAME: &str = "sync-cache";
const INDEX_FILE_NAME: &str = "index.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct FileMetadata {
    pub relative_path: String,
    pub kind: String,
    pub size: u64,
    pub modified_ns: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CachedWarning {
    relative_path: String,
    kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    version: u32,
    packer_version: u32,
    source_path: String,
    metadata: Vec<FileMetadata>,
    options_fingerprint: String,
    zip_file: String,
    zip_size: u64,
    hash: String,
    warnings: Vec<CachedWarning>,
    last_access: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CacheIndex {
    version: u32,
    entries: BTreeMap<String, CacheEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct CachedPack {
    pub hash: String,
    pub zip_path: PathBuf,
    pub zip_size: u64,
    pub warnings: Vec<PackWarning>,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub(crate) struct CacheStats {
    pub entries: usize,
    pub bytes: u64,
    pub capacity: u64,
}

pub(crate) struct SkillPackCache {
    root: PathBuf,
    index_path: PathBuf,
    index: CacheIndex,
    capacity: u64,
}

impl SkillPackCache {
    pub(crate) fn open(config_dir: &Path) -> Result<Self> {
        Self::open_with_capacity(config_dir, CACHE_CAPACITY_BYTES)
    }

    fn open_with_capacity(config_dir: &Path, capacity: u64) -> Result<Self> {
        let root = config_dir.join(CACHE_DIR_NAME);
        fs::create_dir_all(&root)?;
        let index_path = root.join(INDEX_FILE_NAME);
        let index = fs::read(&index_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<CacheIndex>(&bytes).ok())
            .filter(|index| index.version == CACHE_VERSION)
            .unwrap_or(CacheIndex {
                version: CACHE_VERSION,
                entries: BTreeMap::new(),
            });
        Ok(Self {
            root,
            index_path,
            index,
            capacity,
        })
    }

    pub(crate) fn lookup(
        &mut self,
        source_path: &Path,
        metadata: &[FileMetadata],
        options_fingerprint: &str,
    ) -> Result<Option<CachedPack>> {
        let key = source_key(source_path);
        let Some(entry) = self.index.entries.get_mut(&key) else {
            return Ok(None);
        };
        if entry.version != CACHE_VERSION
            || entry.packer_version != PACKER_VERSION
            || entry.source_path != key
            || entry.metadata != metadata
            || entry.options_fingerprint != options_fingerprint
        {
            return Ok(None);
        }
        let zip_path = self.root.join(&entry.zip_file);
        let valid = zip_path.is_file()
            && fs::metadata(&zip_path)
                .map(|meta| meta.len() == entry.zip_size)
                .unwrap_or(false)
            && File::open(&zip_path)
                .ok()
                .and_then(|file| ZipArchive::new(file).ok())
                .is_some();
        if !valid {
            return Ok(None);
        }
        entry.last_access = now_ns();
        let hash = entry.hash.clone();
        let zip_size = entry.zip_size;
        let warnings = entry
            .warnings
            .iter()
            .map(|warning| PackWarning {
                relative_path: warning.relative_path.clone(),
                kind: warning_kind(&warning.kind),
            })
            .collect();
        Ok(Some(CachedPack {
            hash,
            zip_path,
            zip_size,
            warnings,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn store(
        &mut self,
        source_path: &Path,
        metadata: Vec<FileMetadata>,
        options_fingerprint: String,
        zip_path: &Path,
        hash: String,
        zip_size: u64,
        warnings: &[PackWarning],
    ) -> Result<()> {
        if zip_size > self.capacity {
            return Ok(());
        }
        let key = source_key(source_path);
        let zip_file = format!(
            "{}.skill.zip",
            hash.strip_prefix("sha256:").unwrap_or(&hash)
        );
        let target = self.root.join(&zip_file);
        let temp = target.with_extension("tmp");
        fs::copy(zip_path, &temp)?;
        sync_file(&temp)?;
        fs::rename(&temp, &target)?;
        let previous_zip = self
            .index
            .entries
            .get(&key)
            .map(|entry| entry.zip_file.clone());
        self.index.entries.insert(
            key.clone(),
            CacheEntry {
                version: CACHE_VERSION,
                packer_version: PACKER_VERSION,
                source_path: key,
                metadata,
                options_fingerprint,
                zip_file,
                zip_size,
                hash,
                warnings: warnings
                    .iter()
                    .map(|warning| CachedWarning {
                        relative_path: warning.relative_path.clone(),
                        kind: warning_kind_name(warning.kind).into(),
                    })
                    .collect(),
                last_access: now_ns(),
            },
        );
        if let Some(previous_zip) = previous_zip {
            self.remove_zip_if_unreferenced(&previous_zip);
        }
        self.evict()?;
        self.save_index()
    }

    pub(crate) fn stats(&self) -> CacheStats {
        let mut seen = BTreeSet::new();
        let mut entries = 0;
        let mut bytes = 0;
        for entry in self.index.entries.values() {
            let valid = fs::metadata(self.root.join(&entry.zip_file))
                .map(|meta| meta.len() == entry.zip_size)
                .unwrap_or(false);
            if valid {
                entries += 1;
                if seen.insert(entry.zip_file.as_str()) {
                    bytes += entry.zip_size;
                }
            }
        }
        CacheStats {
            entries,
            bytes,
            capacity: self.capacity,
        }
    }

    pub(crate) fn clear(&mut self) -> Result<()> {
        if let Ok(entries) = fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    drop(fs::remove_file(path));
                }
            }
        }
        self.index.entries.clear();
        self.save_index()
    }

    fn evict(&mut self) -> Result<()> {
        while self.stats().bytes > self.capacity {
            let Some(key) = self
                .index
                .entries
                .iter()
                .min_by(|(left_key, left), (right_key, right)| {
                    (left.last_access, left_key).cmp(&(right.last_access, right_key))
                })
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(entry) = self.index.entries.remove(&key) {
                self.remove_zip_if_unreferenced(&entry.zip_file);
            }
        }
        Ok(())
    }

    fn remove_zip_if_unreferenced(&self, zip_file: &str) {
        if !self
            .index
            .entries
            .values()
            .any(|entry| entry.zip_file == zip_file)
        {
            drop(fs::remove_file(self.root.join(zip_file)));
        }
    }

    fn save_index(&self) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&self.index)
            .map_err(|error| AppError::Vault(format!("cache index serialize failed: {error}")))?;
        let temp = self.index_path.with_extension("tmp");
        fs::write(&temp, bytes)?;
        sync_file(&temp)?;
        fs::rename(&temp, &self.index_path)?;
        Ok(())
    }

    pub(crate) fn flush(&self) -> Result<()> {
        self.save_index()
    }
}

pub(crate) fn collect_metadata(source: &Path, options: &PackOptions) -> Result<Vec<FileMetadata>> {
    let folder = source
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut files = Vec::new();
    walk_metadata(source, source, &folder, options, &mut files)?;
    let total_size: u64 = files.iter().map(|file| file.size).sum();
    if total_size > options.limits.max_skill_unpacked_bytes {
        return Err(AppError::Blocked(format!(
            "skill exceeds max_skill_unpacked_bytes ({})",
            options.limits.max_skill_unpacked_bytes
        )));
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

fn walk_metadata(
    root: &Path,
    current: &Path,
    folder: &str,
    options: &PackOptions,
    files: &mut Vec<FileMetadata>,
) -> Result<()> {
    let entries = fs::read_dir(current)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if is_symlink_or_reparse(&meta) {
            return Err(AppError::Blocked(format!(
                "symlink or reparse point not allowed: {}",
                path.display()
            )));
        }
        let rel = normalize(
            path.strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .as_ref(),
        );
        if meta.is_dir() {
            let dir_rel = format!("{folder}/{rel}/");
            if !is_ignored(&dir_rel, &options.user_ignore) {
                walk_metadata(root, &path, folder, options, files)?;
            }
            continue;
        }
        if !meta.is_file() {
            return Err(AppError::Blocked(format!("unsupported file type: {rel}")));
        }
        let file_rel = format!("{folder}/{rel}");
        if is_ignored(&file_rel, &options.user_ignore) {
            continue;
        }
        if files.len() + 1 > options.limits.max_skill_files {
            return Err(AppError::Blocked(format!(
                "skill exceeds max_skill_files ({})",
                options.limits.max_skill_files
            )));
        }
        validate_portable_path(&rel)?;
        if meta.len() > options.limits.max_single_file_unpacked_bytes {
            return Err(AppError::Blocked(format!(
                "file exceeds single-file unpacked limit ({} bytes): {}",
                options.limits.max_single_file_unpacked_bytes,
                path.display()
            )));
        }
        let modified_ns = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        files.push(FileMetadata {
            relative_path: rel,
            kind: "file".into(),
            size: meta.len(),
            modified_ns,
        });
    }
    Ok(())
}

pub(crate) fn options_fingerprint(options: &PackOptions) -> Result<String> {
    #[derive(Serialize)]
    struct Fingerprint<'a> {
        packer_version: u32,
        ignore: &'a [String],
        limits: &'a LimitsConfig,
    }
    let bytes = serde_json::to_vec(&Fingerprint {
        packer_version: PACKER_VERSION,
        ignore: &options.user_ignore,
        limits: &options.limits,
    })
    .map_err(|error| AppError::Vault(format!("cache fingerprint serialize failed: {error}")))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

fn source_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn sync_file(path: &Path) -> Result<()> {
    let file = File::open(path)?;
    file.sync_all()?;
    Ok(())
}

fn now_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn warning_kind_name(kind: SecretWarningKind) -> &'static str {
    match kind {
        SecretWarningKind::PrivateKey => "private_key",
        SecretWarningKind::Pem => "pem",
        SecretWarningKind::Token => "token",
    }
}

fn warning_kind(kind: &str) -> SecretWarningKind {
    match kind {
        "private_key" => SecretWarningKind::PrivateKey,
        "pem" => SecretWarningKind::Pem,
        _ => SecretWarningKind::Token,
    }
}

pub(crate) fn cache_stats(config_dir: &Path) -> Result<CacheStats> {
    Ok(SkillPackCache::open(config_dir)?.stats())
}

pub(crate) fn clear_cache(config_dir: &Path) -> Result<()> {
    SkillPackCache::open(config_dir)?.clear()
}

#[cfg(windows)]
fn is_symlink_or_reparse(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_type().is_symlink() || meta.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_symlink_or_reparse(meta: &fs::Metadata) -> bool {
    meta.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::{PackOptions, SkillPackInput, SkillPacker};

    fn options() -> PackOptions {
        PackOptions {
            limits: LimitsConfig::default(),
            user_ignore: Vec::new(),
        }
    }

    #[test]
    fn metadata_and_cache_entry_invalidate_when_file_changes() {
        let root = tempfile::tempdir().unwrap();
        let skill = root.path().join("demo");
        fs::create_dir_all(&skill).unwrap();
        fs::write(
            skill.join("SKILL.md"),
            b"name: demo\ndescription: d\n\nbody",
        )
        .unwrap();
        let opts = options();
        let metadata = collect_metadata(&skill, &opts).unwrap();
        let batch = SkillPacker::pack_batch(
            &[SkillPackInput {
                source_path: skill.clone(),
            }],
            &opts,
        )
        .unwrap();
        let packed = match &batch.outcomes[0] {
            crate::pack::PackOutcome::Packed(packed) => packed,
            crate::pack::PackOutcome::Blocked(blocked) => {
                panic!("unexpected blocked pack: {}", blocked.reason)
            }
        };
        let mut cache = SkillPackCache::open(root.path()).unwrap();
        let fingerprint = options_fingerprint(&opts).unwrap();
        cache
            .store(
                &skill,
                metadata.clone(),
                fingerprint.clone(),
                &packed.zip_path,
                packed.hash.clone(),
                packed.zip_size,
                &packed.warnings,
            )
            .unwrap();
        assert!(cache
            .lookup(&skill, &metadata, &fingerprint)
            .unwrap()
            .is_some());

        fs::write(skill.join("extra.txt"), b"changed").unwrap();
        let changed = collect_metadata(&skill, &opts).unwrap();
        assert!(cache
            .lookup(&skill, &changed, &fingerprint)
            .unwrap()
            .is_none());
    }

    #[test]
    fn clear_cache_removes_entries_without_touching_other_files() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("sync_state.json"), b"state").unwrap();
        let mut cache = SkillPackCache::open(root.path()).unwrap();
        let target = cache.root.join("orphan.skill.zip");
        fs::write(&target, b"orphan").unwrap();
        cache.clear().unwrap();
        assert_eq!(cache.stats().entries, 0);
        assert!(root.path().join("sync_state.json").exists());
    }

    #[test]
    fn corrupt_cached_archive_is_a_miss() {
        let root = tempfile::tempdir().unwrap();
        let skill = root.path().join("demo");
        fs::create_dir_all(&skill).unwrap();
        let zip = root.path().join("pack.zip");
        fs::write(&zip, b"not a zip").unwrap();
        let mut cache = SkillPackCache::open(root.path()).unwrap();
        let metadata = Vec::new();
        let fingerprint = "fingerprint".to_string();
        cache
            .store(
                &skill,
                metadata.clone(),
                fingerprint.clone(),
                &zip,
                "sha256:bad".into(),
                9,
                &[],
            )
            .unwrap();
        assert!(cache
            .lookup(&skill, &metadata, &fingerprint)
            .unwrap()
            .is_none());
    }

    #[test]
    fn oversized_entries_are_not_retained_and_lru_evicts_oldest() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        let first_zip = root.path().join("first.zip");
        let second_zip = root.path().join("second.zip");
        fs::write(&first_zip, b"first").unwrap();
        fs::write(&second_zip, b"second").unwrap();
        let mut cache = SkillPackCache::open_with_capacity(root.path(), 8).unwrap();
        cache
            .store(
                &first,
                Vec::new(),
                "a".into(),
                &first_zip,
                "sha256:first".into(),
                5,
                &[],
            )
            .unwrap();
        cache
            .store(
                &second,
                Vec::new(),
                "b".into(),
                &second_zip,
                "sha256:second".into(),
                6,
                &[],
            )
            .unwrap();
        assert_eq!(cache.stats().entries, 1);
        assert_eq!(cache.stats().bytes, 6);

        let tiny_root = tempfile::tempdir().unwrap();
        let oversized = tiny_root.path().join("oversized");
        fs::create_dir_all(&oversized).unwrap();
        let mut tiny_cache = SkillPackCache::open_with_capacity(tiny_root.path(), 1).unwrap();
        tiny_cache
            .store(
                &oversized,
                Vec::new(),
                "c".into(),
                &second_zip,
                "sha256:oversized".into(),
                6,
                &[],
            )
            .unwrap();
        assert_eq!(tiny_cache.stats().entries, 0);
    }
}
