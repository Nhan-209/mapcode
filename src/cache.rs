use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::parser::{parse_file_result, ParsedFileResult, SupportedLanguage, Symbol};
use crate::store::{CodeStore, Entrypoint, ImportItem, TypeRelation};
use crate::watcher::{relative_to_root, should_ignore, strip_unc_prefix};

pub const CACHE_DIR_NAME: &str = ".mapcode";
pub const CACHE_FILE_NAME: &str = "cache.json";
pub const GIT_FALLBACK_FILE_NAME: &str = "mapcode_cache.json";
pub const CACHE_SCHEMA_VERSION: u32 = 1;

/// Deterministic 64-bit FNV-1a hash algorithm for fast, persistent content hashing.
pub fn compute_fnv1a_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Helper to extract modification time in nanoseconds since UNIX epoch.
pub fn get_mtime_nanos(metadata: &fs::Metadata) -> u64 {
    match metadata.modified() {
        Ok(time) => match time.duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_nanos() as u64,
            Err(_) => 0,
        },
        Err(_) => 0,
    }
}

/// Cached AST data for a single file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileCacheEntry {
    pub relative_path: String,
    pub mtime_nanos: u64,
    pub file_size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<u64>,
    pub symbols: Vec<Symbol>,
    #[serde(default)]
    pub imports: Vec<ImportItem>,
    #[serde(default)]
    pub types: Vec<TypeRelation>,
    #[serde(default)]
    pub entrypoints: Vec<Entrypoint>,
}

/// Top-level schema of `.mapcode/cache.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCache {
    pub version: u32,
    pub mapcode_version: String,
    pub created_at_epoch_secs: u64,
    pub root_path: String,
    pub files: HashMap<String, FileCacheEntry>,
}

impl ProjectCache {
    pub fn new(root_path: &Path) -> Self {
        let root_str = root_path.to_string_lossy().replace('\\', "/");
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs();

        Self {
            version: CACHE_SCHEMA_VERSION,
            mapcode_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at_epoch_secs: epoch,
            root_path: root_str,
            files: HashMap::new(),
        }
    }
}

/// Locates an existing cache file on disk.
pub fn find_existing_cache_file(root: &Path) -> Option<PathBuf> {
    let primary = root.join(CACHE_DIR_NAME).join(CACHE_FILE_NAME);
    if primary.is_file() {
        return Some(primary);
    }
    let fallback = root.join(".git").join(GIT_FALLBACK_FILE_NAME);
    if fallback.is_file() {
        return Some(fallback);
    }
    None
}

/// Determines the best writable path for the cache file.
pub fn resolve_cache_write_path(root: &Path) -> Option<PathBuf> {
    let mapcode_dir = root.join(CACHE_DIR_NAME);
    if fs::create_dir_all(&mapcode_dir).is_ok() {
        return Some(mapcode_dir.join(CACHE_FILE_NAME));
    }
    let git_dir = root.join(".git");
    if git_dir.is_dir() {
        return Some(git_dir.join(GIT_FALLBACK_FILE_NAME));
    }
    None
}

/// Loads the persistent cache from disk. Returns `None` on any corruption, missing file, or schema mismatch.
pub fn load_cache(root: &Path) -> Option<ProjectCache> {
    let cache_path = find_existing_cache_file(root)?;
    let file = match File::open(&cache_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[MapCode Cache] Cannot open cache file {}: {}", cache_path.display(), e);
            return None;
        }
    };

    let reader = BufReader::new(file);
    match serde_json::from_reader::<_, ProjectCache>(reader) {
        Ok(cache) => {
            if cache.version != CACHE_SCHEMA_VERSION {
                eprintln!(
                    "[MapCode Cache] Schema version mismatch (found {}, expected {}). Invalidating cache.",
                    cache.version, CACHE_SCHEMA_VERSION
                );
                return None;
            }
            Some(cache)
        }
        Err(e) => {
            eprintln!(
                "[MapCode Cache] Corrupted cache file {}: {}. Falling back to cold scan.",
                cache_path.display(), e
            );
            None
        }
    }
}

/// Atomically saves the cache to disk via temporary file rename.
pub fn save_cache(root: &Path, cache: &ProjectCache) -> Result<PathBuf, String> {
    let target_path = resolve_cache_write_path(root)
        .ok_or_else(|| "Failed to determine writable cache path".to_string())?;

    let tmp_path = target_path.with_extension(format!("tmp.{}", std::process::id()));

    let file = File::create(&tmp_path).map_err(|e| {
        format!("Failed to create temporary cache file {}: {}", tmp_path.display(), e)
    })?;

    let mut writer = BufWriter::new(file);
    serde_json::to_writer(&mut writer, cache).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        format!("Failed to serialize cache: {}", e)
    })?;

    writer.flush().map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        format!("Failed to flush cache writer: {}", e)
    })?;

    drop(writer);

    // Atomically replace target
    if let Err(e) = fs::rename(&tmp_path, &target_path) {
        #[cfg(windows)]
        {
            let _ = fs::remove_file(&target_path);
            if let Err(e2) = fs::rename(&tmp_path, &target_path) {
                let _ = fs::remove_file(&tmp_path);
                return Err(format!("Failed to rename {} to {}: {}", tmp_path.display(), target_path.display(), e2));
            }
        }
        #[cfg(not(windows))]
        {
            let _ = fs::remove_file(&tmp_path);
            return Err(format!("Failed to rename {} to {}: {}", tmp_path.display(), target_path.display(), e));
        }
    }

    Ok(target_path)
}

/// Serializes current in-memory store state directly to disk cache.
pub fn persist_store_to_cache(store: &Arc<CodeStore>, root: &Path) -> Result<PathBuf, String> {
    let clean_root = strip_unc_prefix(root);
    let mut project_cache = load_cache(&clean_root).unwrap_or_else(|| ProjectCache::new(&clean_root));

    let mut current_files = HashSet::new();

    for (rel_path, symbols) in store.get_all_file_symbols() {
        current_files.insert(rel_path.clone());

        let full_path = clean_root.join(&rel_path);
        let (mtime_nanos, file_size) = if let Ok(meta) = fs::metadata(&full_path) {
            (get_mtime_nanos(&meta), meta.len())
        } else {
            (0, 0)
        };

        let imports = store.get_file_imports(&rel_path);
        let types = store.get_file_types(&rel_path);
        let entrypoints = store.get_file_entrypoints(&rel_path);

        let existing_hash = project_cache.files.get(&rel_path).and_then(|f| f.content_hash);

        project_cache.files.insert(
            rel_path.clone(),
            FileCacheEntry {
                relative_path: rel_path,
                mtime_nanos,
                file_size,
                content_hash: existing_hash,
                symbols,
                imports,
                types,
                entrypoints,
            },
        );
    }

    // Prune deleted files
    project_cache.files.retain(|k, _| current_files.contains(k));

    save_cache(&clean_root, &project_cache)
}

/// Statistics collected during initial project loading.
#[derive(Debug, Clone)]
pub struct WarmStartupStats {
    pub cached_files: usize,
    pub re_parsed_files: usize,
    pub total_indexed: usize,
    pub elapsed_millis: f64,
    pub warm: bool,
}

/// Loads project into CodeStore using persistent cache when available, only parsing changed/new files.
pub fn load_or_scan_project(store: &Arc<CodeStore>, root: &Path) -> WarmStartupStats {
    let start = Instant::now();
    let clean_root = strip_unc_prefix(root);

    let existing_cache = load_cache(&clean_root);
    let is_warm = existing_cache.is_some();

    let mut project_cache = existing_cache.unwrap_or_else(|| ProjectCache::new(&clean_root));

    use ignore::WalkBuilder;
    let walker = WalkBuilder::new(&clean_root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    let mut seen_rel_paths = HashSet::new();
    let mut cached_count = 0;
    let mut reparsed_count = 0;

    for result in walker {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if !path.is_file() || should_ignore(path) {
            continue;
        }

        let lang = match SupportedLanguage::from_path(path) {
            Some(l) => l,
            None => continue,
        };

        let rel_str = match relative_to_root(path, &clean_root) {
            Some(r) => r,
            None => continue,
        };

        seen_rel_paths.insert(rel_str.clone());

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => match fs::metadata(path) {
                Ok(m) => m,
                Err(_) => continue,
            },
        };

        let mtime_nanos = get_mtime_nanos(&metadata);
        let file_size = metadata.len();

        let is_cache_hit = if let Some(cached) = project_cache.files.get(&rel_str) {
            cached.mtime_nanos == mtime_nanos && cached.file_size == file_size
        } else {
            false
        };

        if is_cache_hit {
            let cached = project_cache.files.get(&rel_str).unwrap();
            store.update_file_full(
                &rel_str,
                cached.symbols.clone(),
                cached.imports.clone(),
                cached.types.clone(),
                cached.entrypoints.clone(),
            );
            cached_count += 1;
        } else {
            if let Ok(content) = fs::read_to_string(path) {
                let parsed = parse_file_result(&rel_str, &content, lang).unwrap_or_else(|_| {
                    ParsedFileResult {
                        symbols: Vec::new(),
                        callers: HashMap::new(),
                        imports: Vec::new(),
                        types: Vec::new(),
                        entrypoints: Vec::new(),
                    }
                });
                let hash = compute_fnv1a_hash(content.as_bytes());

                let entry = FileCacheEntry {
                    relative_path: rel_str.clone(),
                    mtime_nanos,
                    file_size,
                    content_hash: Some(hash),
                    symbols: parsed.symbols.clone(),
                    imports: parsed.imports.clone(),
                    types: parsed.types.clone(),
                    entrypoints: parsed.entrypoints.clone(),
                };

                store.update_file_result(&rel_str, parsed);
                project_cache.files.insert(rel_str, entry);
                reparsed_count += 1;
            }
        }
    }

    // Prune deleted files
    let before_count = project_cache.files.len();
    project_cache.files.retain(|k, _| seen_rel_paths.contains(k));
    let deleted_count = before_count.saturating_sub(project_cache.files.len());

    // Save cache on mutation or cold start
    if reparsed_count > 0 || deleted_count > 0 || !is_warm {
        if let Err(e) = save_cache(&clean_root, &project_cache) {
            eprintln!("[MapCode Cache] Warning: Failed to save cache: {}", e);
        }
    }

    let elapsed = start.elapsed();
    let elapsed_millis = elapsed.as_secs_f64() * 1000.0;

    if is_warm {
        eprintln!(
            "[MapCode Cache] Warm startup in {:.2}ms: {} files loaded from cache, {} files re-parsed.",
            elapsed_millis, cached_count, reparsed_count
        );
    } else {
        eprintln!(
            "[MapCode Cache] Cold scan complete in {:.2}ms: indexed {} files into RAM and saved cache.",
            elapsed_millis, reparsed_count
        );
    }

    WarmStartupStats {
        cached_files: cached_count,
        re_parsed_files: reparsed_count,
        total_indexed: cached_count + reparsed_count,
        elapsed_millis,
        warm: is_warm,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::SymbolKind;

    #[test]
    fn test_fnv1a_hash_stability() {
        let data1 = b"fn main() { println!(\"hello\"); }";
        let hash1 = compute_fnv1a_hash(data1);
        let hash2 = compute_fnv1a_hash(data1);
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, 0);

        let data2 = b"fn main() { println!(\"world\"); }";
        let hash3 = compute_fnv1a_hash(data2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_cache_serialization_roundtrip() {
        let mut cache = ProjectCache::new(Path::new("test_project"));
        let sym = Symbol {
            name: "test_fn".to_string(),
            kind: SymbolKind::Function,
            file_path: "src/test.rs".to_string(),
            start_line: 1,
            end_line: 10,
            signature: "fn test_fn()".to_string(),
            doc: Some("test doc".to_string()),
            container_name: None,
            callees: vec!["helper".to_string()],
        };
        let imp = ImportItem {
            source_path: "src/test.rs".to_string(),
            specifier: "std::io".to_string(),
            is_external: true,
            line: 1,
        };
        let ty = TypeRelation {
            name: "TestStruct".to_string(),
            supertypes: vec!["Clone".to_string()],
            is_trait: false,
            methods: vec!["test_fn".to_string()],
            file_path: "src/test.rs".to_string(),
        };
        let ep = Entrypoint {
            name: "test_fn".to_string(),
            category: "startup".to_string(),
            file_path: "src/test.rs".to_string(),
            line: 1,
            route_or_cmd: None,
        };

        let entry = FileCacheEntry {
            relative_path: "src/test.rs".to_string(),
            mtime_nanos: 123456789,
            file_size: 1024,
            content_hash: Some(42),
            symbols: vec![sym],
            imports: vec![imp],
            types: vec![ty],
            entrypoints: vec![ep],
        };
        cache.files.insert("src/test.rs".to_string(), entry);

        let json = serde_json::to_string(&cache).unwrap();
        let deserialized: ProjectCache = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.version, CACHE_SCHEMA_VERSION);
        assert_eq!(deserialized.files.len(), 1);
        let d_entry = deserialized.files.get("src/test.rs").unwrap();
        assert_eq!(d_entry.relative_path, "src/test.rs");
        assert_eq!(d_entry.mtime_nanos, 123456789);
        assert_eq!(d_entry.symbols.len(), 1);
        assert_eq!(d_entry.imports.len(), 1);
        assert_eq!(d_entry.types.len(), 1);
        assert_eq!(d_entry.entrypoints.len(), 1);
    }

    #[test]
    fn test_cache_corrupted_fallback() {
        let temp_dir = std::env::temp_dir().join(format!("mapcode_test_{}", std::process::id()));
        let _ = fs::create_dir_all(temp_dir.join(CACHE_DIR_NAME));
        let cache_file = temp_dir.join(CACHE_DIR_NAME).join(CACHE_FILE_NAME);
        fs::write(&cache_file, b"NOT_A_VALID_JSON").unwrap();

        let loaded = load_cache(&temp_dir);
        assert!(loaded.is_none());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cache_version_mismatch() {
        let temp_dir = std::env::temp_dir().join(format!("mapcode_test_ver_{}", std::process::id()));
        let _ = fs::create_dir_all(temp_dir.join(CACHE_DIR_NAME));
        let cache_file = temp_dir.join(CACHE_DIR_NAME).join(CACHE_FILE_NAME);
        let mismatch_json = r#"{
            "version": 999,
            "mapcode_version": "0.4.0",
            "created_at_epoch_secs": 1000,
            "root_path": "test",
            "files": {}
        }"#;
        fs::write(&cache_file, mismatch_json).unwrap();

        let loaded = load_cache(&temp_dir);
        assert!(loaded.is_none());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
