use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::channel;
use notify::{recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::parser::{parse_file, SupportedLanguage};
use crate::store::CodeStore;

pub fn strip_unc_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

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
    false
}

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
                        if let Ok(content) = std::fs::read_to_string(path) {
                            if let Some(rel_str) = relative_to_root(path, root) {
                                if let Ok(symbols) = parse_file(&rel_str, &content, lang) {
                                    store.update_file(&rel_str, symbols);
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

pub fn start_watcher(
    store: Arc<CodeStore>,
    root_path: PathBuf,
) -> Result<RecommendedWatcher, notify::Error> {
    let (tx, rx) = channel();
    let mut watcher = recommended_watcher(tx)?;
    watcher.watch(&root_path, RecursiveMode::Recursive)?;

    let store_clone = store.clone();
    let root_clone = root_path.clone();

    std::thread::Builder::new()
        .name("mapcode-watcher".to_string())
        .spawn(move || {
            for res in rx {
                match res {
                    Ok(event) => {
                        handle_event(&store_clone, &root_clone, event);
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

fn read_file_with_retry(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(c) => Some(c),
        Err(_) => {
            // Retry once after brief pause to accommodate atomic saves / temporary file locks
            std::thread::sleep(std::time::Duration::from_millis(30));
            std::fs::read_to_string(path).ok()
        }
    }
}

fn handle_event(store: &Arc<CodeStore>, root: &Path, event: Event) {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {
            for path in event.paths {
                if should_ignore(&path) {
                    continue;
                }
                if let Some(lang) = SupportedLanguage::from_path(&path) {
                    if path.is_file() {
                        if let Some(content) = read_file_with_retry(&path) {
                            if let Some(rel_str) = relative_to_root(&path, root) {
                                match parse_file(&rel_str, &content, lang) {
                                    Ok(symbols) => {
                                        let count = symbols.len();
                                        store.update_file(&rel_str, symbols);
                                        eprintln!(
                                            "[MapCode Watcher] Updated '{}' ({} symbols)",
                                            rel_str, count
                                        );
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "[MapCode Watcher] Failed to parse '{}': {}",
                                            rel_str, e
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        EventKind::Remove(_) => {
            for path in event.paths {
                if let Some(rel_str) = relative_to_root(&path, root) {
                    store.remove_file(&rel_str);
                    eprintln!("[MapCode Watcher] Removed '{}'", rel_str);
                }
            }
        }
        _ => {}
    }
}

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
        let count = scan_and_index_project(&self.store, &clean_root);

        // 3. Start new watcher
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
        let normal_file = Path::new(r"D:\projects\my_app\src\main.rs");
        assert_eq!(relative_to_root(normal_file, unc_root), Some("src/main.rs".to_string()));
    }

    #[test]
    fn test_should_ignore() {
        assert!(should_ignore(Path::new("my_project/node_modules/index.js")));
        assert!(should_ignore(Path::new("my_project/target/debug/build.rs")));
        assert!(should_ignore(Path::new("my_project/.git/config")));
        assert!(!should_ignore(Path::new("my_project/src/lib.rs")));
    }
}
