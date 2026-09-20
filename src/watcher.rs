use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant, UNIX_EPOCH};
use notify::{recommended_watcher, Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::parser::{parse_file_result, SupportedLanguage};
use crate::store::CodeStore;

/// Removes the Windows extended path prefix (`\\?\`) if present.
pub fn strip_unc_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

/// Normalizes a path to a forward-slash relative path relative to workspace root.
pub fn relative_to_root(path: &Path, root: &Path) -> Option<String> {
    let clean_path = strip_unc_prefix(path);
    let clean_root = strip_unc_prefix(root);

    if let Ok(rel) = clean_path.strip_prefix(&clean_root) {
        return Some(rel.to_string_lossy().replace('\\', "/"));
    }

    let path_str = clean_path.to_string_lossy().replace('\\', "/");
    let root_str = clean_root.to_string_lossy().replace('\\', "/");
    let clean_root_str = root_str.trim_end_matches('/');

    #[cfg(windows)]
    if path_str.to_lowercase().starts_with(&clean_root_str.to_lowercase()) {
        let rel = path_str[clean_root_str.len()..].trim_start_matches('/');
        return Some(rel.to_string());
    }

    #[cfg(not(windows))]
    if let Some(suffix) = path_str.strip_prefix(clean_root_str) {
        let rel = suffix.trim_start_matches('/');
        return Some(rel.to_string());
    }

    None
}

/// Determines whether a path should be excluded from scanning and indexing.
pub fn should_ignore(path: &Path) -> bool {
    for component in path.components() {
        let s = component.as_os_str().to_string_lossy();
        if s.starts_with('.') && s != "." && s != ".." {
            return true;
        }
        if s == "target"
            || s == "node_modules"
            || s == "__pycache__"
            || s == "venv"
            || s == ".venv"
            || s == "dist"
            || s == "build"
        {
            return true;
        }
    }

    // Ignore editor swap, temporary files, and mapcode cache files
    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
        if file_name.ends_with(".swp")
            || file_name.ends_with(".swo")
            || file_name.ends_with('~')
            || file_name.starts_with(".#")
            || file_name.starts_with('#')
            || file_name == "4913"
            || file_name.ends_with(".tmp")
            || file_name == "mapcode_cache.json"
        {
            return true;
        }
    }

    false
}

/// Initial cold repository scan using `ignore::WalkBuilder`.
#[allow(dead_code)]
pub fn scan_and_index_project(store: &Arc<CodeStore>, root: &Path) -> usize {
    use ignore::WalkBuilder;
    eprintln!("[MapCode] Scanning repository at: {}", root.display());

    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    let mut indexed_count = 0;
    for result in walker {
        match result {
            Ok(entry) => {
                let path = entry.path();
                if path.is_file() && !should_ignore(path) {
                    if let Some(lang) = SupportedLanguage::from_path(path) {
                        if let Ok(content) = fs::read_to_string(path) {
                            if let Some(rel_str) = relative_to_root(path, root) {
                                if let Ok(parsed) = parse_file_result(&rel_str, &content, lang) {
                                    store.update_file_result(&rel_str, parsed);
                                    indexed_count += 1;
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[MapCode] Walk error: {}", e);
            }
        }
    }

    eprintln!("[MapCode] Initial scan complete: indexed {} files into RAM.", indexed_count);
    indexed_count
}

/// Helper to extract modification time in nanoseconds since UNIX epoch.
fn get_mtime_nanos(metadata: &fs::Metadata) -> u64 {
    match metadata.modified() {
        Ok(time) => match time.duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_nanos() as u64,
            Err(_) => 0,
        },
        Err(_) => 0,
    }
}

/// Reads file with microsecond retry backoff to handle atomic editor save locks.
pub fn read_file_with_micro_retry(path: &Path) -> Option<String> {
    // 1. Immediate read attempt (succeeds in 99.9% of normal saves in <0.1ms)
    if let Ok(c) = fs::read_to_string(path) {
        return Some(c);
    }

    // 2. Micro-retry backoff (2 attempts with 1ms pause to handle momentary Windows locks)
    for _ in 0..2 {
        std::thread::sleep(Duration::from_millis(1));
        if let Ok(c) = fs::read_to_string(path) {
            return Some(c);
        }
    }

    None
}

/// Real-time event handler implementing sub-5ms single-file incremental updates.
fn handle_event(
    store: &Arc<CodeStore>,
    root: &Path,
    event: Event,
    debouncer: &Option<CacheDebounceHandle>,
    last_mtimes: &mut HashMap<String, (u64, u64)>,
) {
    for path in event.paths {
        if should_ignore(&path) {
            continue;
        }

        let rel_str = match relative_to_root(&path, root) {
            Some(r) => r,
            None => continue,
        };

        // Case 1: File/Path was removed or renamed away
        if !path.exists() {
            store.remove_path_or_prefix(&rel_str);
            last_mtimes.remove(&rel_str);
            if let Some(deb) = debouncer {
                deb.notify_dirty();
            }
            eprintln!("[MapCode Watcher] Removed '{}'", rel_str);
            continue;
        }

        // Case 2: File is present and supported language
        if path.is_file() {
            if let Some(lang) = SupportedLanguage::from_path(&path) {
                let metadata = match fs::metadata(&path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                let mtime = get_mtime_nanos(&metadata);
                let size = metadata.len();

                // Fast deduplication: skip redundant re-parsing if mtime & size have not changed
                if let Some(&(last_mtime, last_size)) = last_mtimes.get(&rel_str) {
                    if last_mtime == mtime && last_size == size {
                        continue;
                    }
                }

                let start_time = Instant::now();

                if let Some(content) = read_file_with_micro_retry(&path) {
                    match parse_file_result(&rel_str, &content, lang) {
                        Ok(parsed) => {
                            let sym_count = parsed.symbols.len();
                            let import_count = parsed.imports.len();
                            let type_count = parsed.types.len();
                            let entrypoint_count = parsed.entrypoints.len();

                            // Atomic in-memory update
                            store.update_file_result(&rel_str, parsed);
                            last_mtimes.insert(rel_str.clone(), (mtime, size));

                            let elapsed = start_time.elapsed().as_secs_f64() * 1000.0;
                            eprintln!(
                                "[MapCode Watcher] Updated '{}' in {:.2}ms ({} syms, {} imps, {} types, {} eps)",
                                rel_str, elapsed, sym_count, import_count, type_count, entrypoint_count
                            );

                            // Signal non-blocking cache persistence debouncer
                            if let Some(deb) = debouncer {
                                deb.notify_dirty();
                            }
                        }
                        Err(e) => {
                            eprintln!("[MapCode Watcher] Failed to parse '{}': {}", rel_str, e);
                        }
                    }
                }
            }
        }
    }
}

/// Asynchronous cache debouncer managing background disk persistence without blocking watcher threads.
#[derive(Clone)]
pub struct CacheDebounceHandle {
    dirty_tx: std::sync::mpsc::Sender<()>,
}

impl CacheDebounceHandle {
    pub fn spawn(
        store: Arc<CodeStore>,
        root_path: PathBuf,
        debounce_duration: Duration,
    ) -> Self {
        let (dirty_tx, dirty_rx) = std::sync::mpsc::channel();

        std::thread::Builder::new()
            .name("mapcode-cache-debouncer".to_string())
            .spawn(move || {
                loop {
                    match dirty_rx.recv() {
                        Ok(()) => {
                            loop {
                                match dirty_rx.recv_timeout(debounce_duration) {
                                    Ok(()) => {
                                        // Still receiving edits within quiet window; reset timer
                                        continue;
                                    }
                                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                        // Quiet window elapsed; write cache to disk
                                        if let Err(e) = crate::cache::persist_store_to_cache(&store, &root_path) {
                                            eprintln!("[MapCode Cache] Warning: Background cache save failed: {}", e);
                                        }
                                        break;
                                    }
                                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                        // Server shutdown or workspace switch: final flush
                                        let _ = crate::cache::persist_store_to_cache(&store, &root_path);
                                        return;
                                    }
                                }
                            }
                        }
                        Err(_) => return,
                    }
                }
            })
            .expect("Failed to spawn mapcode-cache-debouncer thread");

        Self { dirty_tx }
    }

    pub fn notify_dirty(&self) {
        let _ = self.dirty_tx.send(());
    }
}

/// Starts the background file system watcher and debounced cache synchronizer.
pub fn start_watcher(
    store: Arc<CodeStore>,
    root_path: PathBuf,
) -> Result<RecommendedWatcher, notify::Error> {
    let (tx, rx) = channel();
    let mut watcher = recommended_watcher(tx)?;
    watcher.watch(&root_path, RecursiveMode::Recursive)?;

    let store_clone = store.clone();
    let root_clone = root_path.clone();

    // Spawn cache debouncer with 1-second quiet period
    let debouncer = CacheDebounceHandle::spawn(
        store.clone(),
        root_path,
        Duration::from_millis(1000),
    );

    std::thread::Builder::new()
        .name("mapcode-watcher".to_string())
        .spawn(move || {
            let mut last_mtimes: HashMap<String, (u64, u64)> = HashMap::new();
            let debouncer_opt = Some(debouncer);

            for res in rx {
                match res {
                    Ok(event) => {
                        handle_event(&store_clone, &root_clone, event, &debouncer_opt, &mut last_mtimes);
                    }
                    Err(e) => {
                        eprintln!("[MapCode Watcher] Watch error: {:?}", e);
                    }
                }
            }
        })
        .expect("Failed to spawn watcher thread");

    Ok(watcher)
}

/// Dynamic workspace lifecycle manager allowing on-the-fly workspace switches.
pub struct WatcherHandle {
    watcher: std::sync::Mutex<Option<RecommendedWatcher>>,
    store: Arc<CodeStore>,
}

impl WatcherHandle {
    pub fn new(store: Arc<CodeStore>, initial_watcher: Option<RecommendedWatcher>) -> Self {
        Self {
            watcher: std::sync::Mutex::new(initial_watcher),
            store,
        }
    }

    pub fn switch_workspace(&self, new_root: PathBuf) -> Result<usize, String> {
        let clean_root = strip_unc_prefix(&new_root);
        if !clean_root.exists() || !clean_root.is_dir() {
            return Err(format!(
                "Directory '{}' does not exist or is not a directory.",
                clean_root.display()
            ));
        }

        // 1. Drop previous watcher
        {
            let mut guard = self.watcher.lock().map_err(|e| e.to_string())?;
            *guard = None;
        }

        // 2. Clear store and update root
        self.store.clear();
        self.store.set_root_path(clean_root.clone());

        // 3. Warm startup load or cold scan
        let stats = crate::cache::load_or_scan_project(&self.store, &clean_root);
        let count = stats.total_indexed;

        // 4. Start new watcher and debouncer
        match start_watcher(self.store.clone(), clean_root.clone()) {
            Ok(new_w) => {
                let mut guard = self.watcher.lock().map_err(|e| e.to_string())?;
                *guard = Some(new_w);
                eprintln!(
                    "[MapCode] Realtime watcher active on: {}",
                    clean_root.display()
                );
            }
            Err(e) => {
                eprintln!(
                    "[MapCode] Warning: Failed to start file watcher for {}: {}",
                    clean_root.display(),
                    e
                );
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relative_to_root() {
        let root = Path::new("D:/projects/my_app");
        let file = Path::new("D:/projects/my_app/src/main.rs");
        assert_eq!(relative_to_root(file, root), Some("src/main.rs".to_string()));

        // Windows UNC handling
        let unc_root = Path::new(r"\\?\D:\projects\my_app");
        let normal_file = Path::new(r"D:\projects\my_app/src/main.rs");
        assert_eq!(relative_to_root(normal_file, unc_root), Some("src/main.rs".to_string()));
    }

    #[test]
    fn test_should_ignore() {
        assert!(should_ignore(Path::new("my_project/node_modules/index.js")));
        assert!(should_ignore(Path::new("my_project/target/debug/build.rs")));
        assert!(should_ignore(Path::new("my_project/.git/config")));
        assert!(should_ignore(Path::new("my_project/.mapcode/cache.json")));
        assert!(should_ignore(Path::new("my_project/src/app.rs.swp")));
        assert!(should_ignore(Path::new("my_project/src/4913")));
        assert!(!should_ignore(Path::new("my_project/src/lib.rs")));
    }

    #[test]
    fn test_strip_unc_prefix() {
        let p1 = Path::new(r"\\?\C:\foo\bar");
        assert_eq!(strip_unc_prefix(p1), PathBuf::from(r"C:\foo\bar"));

        let p2 = Path::new(r"C:\foo\bar");
        assert_eq!(strip_unc_prefix(p2), PathBuf::from(r"C:\foo\bar"));
    }
}
