use std::collections::{HashMap, HashSet};
use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::dependency::resolve_import_path;
use crate::store::CodeStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ModuleNode {
    pub path: String,
    pub architectural_layer: String, // "api", "service", "model", "utility", "entrypoint", "core", "general"
    pub afferent_coupling: usize,    // Ca: external modules depending on this module
    pub efferent_coupling: usize,    // Ce: external modules this module depends on
    pub instability: f64,            // Ce / (Ca + Ce)
    pub symbol_count: usize,
    pub submodules: Vec<ModuleNode>,
}

/// Constructs a hierarchical topographical module tree with architectural layer classification
/// and Robert C. Martin coupling metrics (Ca, Ce, Instability).
pub fn get_architecture_tree(store: &CodeStore, max_depth: usize) -> ModuleNode {
    let depth_limit = if max_depth == 0 { 3 } else { max_depth.min(8) };
    let all_files = store.get_all_file_symbols();

    // 1. Group files by directory path
    let mut dir_to_files: HashMap<String, Vec<String>> = HashMap::new();
    let mut file_to_symbols_count: HashMap<String, usize> = HashMap::new();

    for (file_path, syms) in &all_files {
        file_to_symbols_count.insert(file_path.clone(), syms.len());

        let dir = Path::new(file_path)
            .parent()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();

        let dir_key = if dir.is_empty() || dir == "." {
            "root".to_string()
        } else {
            dir
        };

        dir_to_files.entry(dir_key).or_default().push(file_path.clone());
    }

    // 2. Build module-level dependency graph to compute Ca, Ce, Instability
    let all_imports = store.get_all_file_imports();
    let mut file_to_module: HashMap<String, String> = HashMap::new();

    for (module_path, files) in &dir_to_files {
        for file in files {
            file_to_module.insert(file.clone(), module_path.clone());
        }
    }

    // Module outgoing imports (Efferent) and incoming imports (Afferent)
    let mut module_efferent: HashMap<String, HashSet<String>> = HashMap::new();
    let mut module_afferent: HashMap<String, HashSet<String>> = HashMap::new();

    for (importer_file, items) in &all_imports {
        let importer_mod = file_to_module
            .get(importer_file)
            .cloned()
            .unwrap_or_else(|| "root".to_string());

        for item in items {
            if !item.is_external {
                if let Some(imported_file) = resolve_import_path(store, importer_file, &item.source_path) {
                    if let Some(target_mod) = file_to_module.get(&imported_file) {
                        if target_mod != &importer_mod {
                            module_efferent
                                .entry(importer_mod.clone())
                                .or_default()
                                .insert(target_mod.clone());
                            module_afferent
                                .entry(target_mod.clone())
                                .or_default()
                                .insert(importer_mod.clone());
                        }
                    }
                }
            }
        }
    }

    // 3. Build submodules tree up to depth_limit
    build_module_tree(
        "root",
        &dir_to_files,
        &file_to_symbols_count,
        &module_afferent,
        &module_efferent,
        1,
        depth_limit,
    )
}

fn build_module_tree(
    current_path: &str,
    dir_to_files: &HashMap<String, Vec<String>>,
    file_to_symbols_count: &HashMap<String, usize>,
    module_afferent: &HashMap<String, HashSet<String>>,
    module_efferent: &HashMap<String, HashSet<String>>,
    current_depth: usize,
    max_depth: usize,
) -> ModuleNode {
    let files = dir_to_files.get(current_path);
    let direct_symbol_count: usize = files
        .map(|fl| fl.iter().filter_map(|f| file_to_symbols_count.get(f)).sum())
        .unwrap_or(0);

    let ca = module_afferent.get(current_path).map(|s| s.len()).unwrap_or(0);
    let ce = module_efferent.get(current_path).map(|s| s.len()).unwrap_or(0);
    let instability = if ca + ce == 0 {
        0.0
    } else {
        (ce as f64) / ((ca + ce) as f64)
    };

    let layer = infer_architectural_layer(current_path);

    let mut submodules = Vec::new();
    if current_depth < max_depth {
        let prefix = if current_path == "root" {
            String::new()
        } else {
            format!("{}/", current_path)
        };

        for other_dir in dir_to_files.keys() {
            if other_dir == current_path {
                continue;
            }

            let is_direct_child = if prefix.is_empty() {
                !other_dir.contains('/') && other_dir != "root"
            } else if other_dir.starts_with(&prefix) {
                let remainder = &other_dir[prefix.len()..];
                !remainder.contains('/')
            } else {
                false
            };

            if is_direct_child {
                let child_node = build_module_tree(
                    other_dir,
                    dir_to_files,
                    file_to_symbols_count,
                    module_afferent,
                    module_efferent,
                    current_depth + 1,
                    max_depth,
                );
                submodules.push(child_node);
            }
        }
    }

    submodules.sort_by(|a, b| a.path.cmp(&b.path));

    let total_symbols = direct_symbol_count + submodules.iter().map(|s| s.symbol_count).sum::<usize>();

    ModuleNode {
        path: current_path.to_string(),
        architectural_layer: layer,
        afferent_coupling: ca,
        efferent_coupling: ce,
        instability: (instability * 100.0).round() / 100.0,
        symbol_count: total_symbols,
        submodules,
    }
}

fn infer_architectural_layer(path: &str) -> String {
    let lower = path.to_lowercase();
    if lower.contains("route") || lower.contains("controller") || lower.contains("api") || lower.contains("endpoint") {
        "api".to_string()
    } else if lower.contains("service") || lower.contains("domain") || lower.contains("usecase") || lower.contains("logic") {
        "service".to_string()
    } else if lower.contains("model") || lower.contains("entity") || lower.contains("schema") || lower.contains("dto") {
        "model".to_string()
    } else if lower.contains("util") || lower.contains("helper") || lower.contains("tool") || lower.contains("common") || lower.contains("shared") {
        "utility".to_string()
    } else if lower.contains("bin") || lower.contains("cli") || lower.contains("main") {
        "entrypoint".to_string()
    } else if lower.contains("core") || lower.contains("engine") {
        "core".to_string()
    } else {
        "general".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::parser::{ParsedFileResult, Symbol, SymbolKind};

    #[test]
    fn test_architecture_tree_generation() {
        let store = CodeStore::new(PathBuf::from("."));

        store.update_file_result(
            "src/controllers/user_controller.rs",
            ParsedFileResult {
                symbols: vec![Symbol {
                    name: "get_user".to_string(),
                    kind: SymbolKind::Function,
                    file_path: "src/controllers/user_controller.rs".to_string(),
                    start_line: 1,
                    end_line: 10,
                    signature: "fn get_user()".to_string(),
                    doc: None,
                    container_name: None,
                    callees: vec![],
                }],
                callers: HashMap::new(),
                imports: vec![],
                types: vec![],
                entrypoints: vec![],
            },
        );

        let tree = get_architecture_tree(&store, 4);
        assert_eq!(tree.path, "root");
        assert!(tree.symbol_count >= 1);
    }
}
