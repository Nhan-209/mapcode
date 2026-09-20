use std::collections::{HashMap, HashSet};
use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::store::{clean_relative_path, CodeStore};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DependencyReport {
    pub path: String,
    pub imports: Vec<String>,
    pub external_imports: Vec<String>,
    pub imported_by: Vec<String>,
    pub circular_dependencies: Vec<Vec<String>>,
}

/// Computes forward and reverse dependencies for a given file or module path,
/// including cycle detection using 3-color DFS traversal.
pub fn get_file_dependencies(store: &CodeStore, path: &str) -> DependencyReport {
    let root = store.root_path();
    let clean_path = clean_relative_path(&root, path);

    // 1. Resolve effective target file key from store
    let target_key = resolve_file_key(store, &clean_path).unwrap_or(clean_path.clone());

    // 2. Extract forward imports for target_key
    let raw_imports = store.get_file_imports(&target_key);
    let mut internal_imports = Vec::new();
    let mut external_imports = Vec::new();

    for item in raw_imports {
        if item.is_external {
            if !external_imports.contains(&item.source_path) {
                external_imports.push(item.source_path.clone());
            }
        } else {
            // Attempt to resolve relative or module path to an indexed project file
            let resolved = resolve_import_path(store, &target_key, &item.source_path);
            let display_name = resolved.unwrap_or(item.source_path);
            if !internal_imports.contains(&display_name) {
                internal_imports.push(display_name);
            }
        }
    }

    // 3. Extract reverse dependencies (imported_by)
    let all_imports = store.get_all_file_imports();
    let mut imported_by_set = HashSet::new();

    for (other_file, items) in &all_imports {
        if other_file == &target_key {
            continue;
        }
        for item in items {
            if !item.is_external {
                if let Some(res) = resolve_import_path(store, other_file, &item.source_path) {
                    if res == target_key || path_suffix_matches(&res, &target_key) {
                        imported_by_set.insert(other_file.clone());
                    }
                } else if path_suffix_matches(&target_key, &item.source_path) {
                    imported_by_set.insert(other_file.clone());
                }
            }
        }
    }

    let mut imported_by: Vec<String> = imported_by_set.into_iter().collect();
    imported_by.sort();

    // 4. Build adjacency graph of internal dependencies for cycle detection
    let mut adj_graph: HashMap<String, Vec<String>> = HashMap::new();
    for (file, items) in &all_imports {
        let mut neighbors = Vec::new();
        for item in items {
            if !item.is_external {
                if let Some(res) = resolve_import_path(store, file, &item.source_path) {
                    if !neighbors.contains(&res) {
                        neighbors.push(res);
                    }
                }
            }
        }
        adj_graph.insert(file.clone(), neighbors);
    }

    // 5. Detect cycles involving target_key using DFS
    let circular_dependencies = find_cycles_for_node(&adj_graph, &target_key);

    DependencyReport {
        path: target_key,
        imports: internal_imports,
        external_imports,
        imported_by,
        circular_dependencies,
    }
}

/// Resolves a file query to an existing indexed key (handles partial suffix queries like 'main.rs')
fn resolve_file_key(store: &CodeStore, query: &str) -> Option<String> {
    let all_files: Vec<String> = store.get_all_file_symbols().into_iter().map(|(p, _)| p).collect();
    if all_files.iter().any(|f| f == query) {
        return Some(query.to_string());
    }
    all_files.into_iter().find(|file| path_suffix_matches(file, query))
}

/// Resolves an import source string (e.g. "./store" or "crate::parser") relative to the importer file
pub fn resolve_import_path(store: &CodeStore, importer_file: &str, source: &str) -> Option<String> {
    let all_files: HashSet<String> = store
        .get_all_file_symbols()
        .into_iter()
        .map(|(p, _)| p)
        .collect();

    let normalized_source = source.replace('\\', "/").replace("::", "/");
    let clean_source = normalized_source.trim_start_matches("crate/").trim_start_matches("super/");

    // 1. Direct path check
    if all_files.contains(clean_source) {
        return Some(clean_source.to_string());
    }

    // 2. Relative path resolution from importer directory
    let importer_dir = Path::new(importer_file).parent().unwrap_or_else(|| Path::new(""));
    let candidate = importer_dir.join(clean_source);
    let candidate_str = candidate.to_string_lossy().replace('\\', "/");
    let clean_cand = candidate_str.trim_start_matches("./");

    if all_files.contains(clean_cand) {
        return Some(clean_cand.to_string());
    }

    // 3. Extension append candidates (.rs, .py, .ts, .js, .go, .c, .h, /index.ts, /mod.rs)
    let extensions = [
        ".rs", ".py", ".ts", ".tsx", ".js", ".jsx", ".go", ".c", ".cpp", ".h", ".hpp", ".lua",
        "/mod.rs", "/index.ts", "/index.js",
    ];

    for ext in &extensions {
        let with_ext = format!("{}{}", clean_cand, ext);
        if all_files.contains(&with_ext) {
            return Some(with_ext);
        }
        let direct_with_ext = format!("{}{}", clean_source, ext);
        if all_files.contains(&direct_with_ext) {
            return Some(direct_with_ext);
        }
    }

    // 4. Suffix match fallback
    for file in &all_files {
        if path_suffix_matches(file, clean_source) {
            return Some(file.clone());
        }
    }

    None
}

fn path_suffix_matches(full_key: &str, query: &str) -> bool {
    if full_key == query {
        return true;
    }
    let query_suffix = format!("/{}", query.trim_start_matches('/'));
    if full_key.ends_with(&query_suffix) {
        return true;
    }
    let stem = query.split('.').next().unwrap_or(query);
    let stem_suffix = format!("/{}", stem.trim_start_matches('/'));
    if full_key.ends_with(&stem_suffix) {
        return true;
    }
    false
}

/// 3-color DFS to detect all cycles traversing the starting node.
fn find_cycles_for_node(graph: &HashMap<String, Vec<String>>, start_node: &str) -> Vec<Vec<String>> {
    let mut cycles = Vec::new();
    let mut visited = HashSet::new();
    let mut current_path = Vec::new();

    fn dfs(
        u: &str,
        start_node: &str,
        graph: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
        depth: usize,
    ) {
        if depth > 15 {
            return;
        }

        path.push(u.to_string());
        visited.insert(u.to_string());

        if let Some(neighbors) = graph.get(u) {
            for v in neighbors {
                if v == start_node && path.len() > 1 {
                    let mut cycle = path.clone();
                    cycle.push(start_node.to_string());
                    cycles.push(cycle);
                } else if !visited.contains(v) {
                    dfs(v, start_node, graph, visited, path, cycles, depth + 1);
                }
            }
        }

        path.pop();
        visited.remove(u);
    }

    dfs(
        start_node,
        start_node,
        graph,
        &mut visited,
        &mut current_path,
        &mut cycles,
        0,
    );

    cycles.sort_by_key(|a| a.len());
    cycles.dedup();
    cycles
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::parser::{ImportItem, ParsedFileResult, Symbol, SymbolKind};

    #[test]
    fn test_dependency_graph_and_cycle_detection() {
        let store = CodeStore::new(PathBuf::from("."));

        // File A imports B
        store.update_file_result(
            "src/a.rs",
            ParsedFileResult {
                symbols: vec![Symbol {
                    name: "fn_a".to_string(),
                    kind: SymbolKind::Function,
                    file_path: "src/a.rs".to_string(),
                    start_line: 1,
                    end_line: 5,
                    signature: "fn fn_a()".to_string(),
                    doc: None,
                    container_name: None,
                    callees: vec![],
                }],
                callers: HashMap::new(),
                imports: vec![ImportItem {
                    source_path: "src/b.rs".to_string(),
                    specifier: "b".to_string(),
                    is_external: false,
                    line: 1,
                }],
                types: vec![],
                entrypoints: vec![],
            },
        );

        // File B imports A (Cycle!)
        store.update_file_result(
            "src/b.rs",
            ParsedFileResult {
                symbols: vec![],
                callers: HashMap::new(),
                imports: vec![ImportItem {
                    source_path: "src/a.rs".to_string(),
                    specifier: "a".to_string(),
                    is_external: false,
                    line: 1,
                }],
                types: vec![],
                entrypoints: vec![],
            },
        );

        let report_a = get_file_dependencies(&store, "src/a.rs");
        assert_eq!(report_a.imports, vec!["src/b.rs".to_string()]);
        assert_eq!(report_a.imported_by, vec!["src/b.rs".to_string()]);
        assert!(!report_a.circular_dependencies.is_empty());
        assert_eq!(
            report_a.circular_dependencies[0],
            vec!["src/a.rs", "src/b.rs", "src/a.rs"]
        );
    }
}
