use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

pub use crate::parser::{
    parse_file, Entrypoint, ImportItem, ParsedFileResult, SupportedLanguage, Symbol, SymbolKind,
    SymbolRef, TypeRelation,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct FileOutline {
    pub file_path: String,
    pub total_symbols: usize,
    pub symbols: Vec<Symbol>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DefinitionResult {
    pub name: String,
    pub kind: SymbolKind,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
    pub callees: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalleeDetail {
    pub name: String,
    pub candidates: Vec<SymbolRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct CallGraphResult {
    pub symbol_name: String,
    pub definitions: Vec<SymbolRef>,
    pub callers: Vec<SymbolRef>,
    pub callees: Vec<String>,
    pub callee_details: Vec<CalleeDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SymbolMatch {
    pub name: String,
    pub kind: SymbolKind,
    pub file_path: String,
    pub line: usize,
    pub signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
    pub score: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ProjectStats {
    pub root_path: String,
    pub total_files_indexed: usize,
    pub total_symbols_indexed: usize,
    pub total_functions_and_methods: usize,
    pub total_types_and_classes: usize,
    pub language_file_counts: HashMap<String, usize>,
}

pub struct CodeStore {
    root_path: std::sync::RwLock<PathBuf>,
    // relative file path -> symbols in that file
    file_symbols: DashMap<String, Vec<Symbol>>,
    // symbol name -> definitions of that symbol
    definitions: DashMap<String, Vec<SymbolRef>>,
    // callee name -> symbols that call it
    callers: DashMap<String, HashSet<SymbolRef>>,

    // Milestone 1 Multi-Dimensional Indices:
    file_imports: DashMap<String, Vec<ImportItem>>,
    file_types: DashMap<String, Vec<TypeRelation>>,
    type_relations: DashMap<String, Vec<TypeRelation>>,
    file_entrypoints: DashMap<String, Vec<Entrypoint>>,
}

impl CodeStore {
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            root_path: std::sync::RwLock::new(root_path),
            file_symbols: DashMap::new(),
            definitions: DashMap::new(),
            callers: DashMap::new(),
            file_imports: DashMap::new(),
            file_types: DashMap::new(),
            type_relations: DashMap::new(),
            file_entrypoints: DashMap::new(),
        }
    }

    pub fn root_path(&self) -> PathBuf {
        self.root_path.read().unwrap().clone()
    }

    pub fn set_root_path(&self, new_root: PathBuf) {
        *self.root_path.write().unwrap() = new_root;
    }

    pub fn clear(&self) {
        self.file_symbols.clear();
        self.definitions.clear();
        self.callers.clear();
        self.file_imports.clear();
        self.file_types.clear();
        self.type_relations.clear();
        self.file_entrypoints.clear();
    }

    pub fn remove_file(&self, relative_path: &str) {
        if let Some((_, old_symbols)) = self.file_symbols.remove(relative_path) {
            for sym in old_symbols {
                let sym_ref = SymbolRef::from(&sym);

                // Clean definitions
                if let Some(mut defs) = self.definitions.get_mut(&sym.name) {
                    defs.retain(|d| d != &sym_ref);
                }

                // Clean callers for symbols called by sym
                for callee_name in &sym.callees {
                    if let Some(mut caller_set) = self.callers.get_mut(callee_name) {
                        caller_set.remove(&sym_ref);
                    }
                }
            }
        }

        self.file_imports.remove(relative_path);

        if let Some((_, old_types)) = self.file_types.remove(relative_path) {
            for t in old_types {
                if let Some(mut rels) = self.type_relations.get_mut(&t.name) {
                    rels.retain(|r| r.file_path != relative_path);
                }
            }
        }

        self.file_entrypoints.remove(relative_path);
    }

    /// Removes a single file or an entire directory prefix from all collections.
    pub fn remove_path_or_prefix(&self, relative_path: &str) {
        self.remove_file(relative_path);

        let prefix = format!("{}/", relative_path.trim_end_matches('/'));
        let files_to_remove: Vec<String> = self
            .file_symbols
            .iter()
            .filter(|entry| entry.key().starts_with(&prefix))
            .map(|entry| entry.key().clone())
            .collect();

        for file in files_to_remove {
            self.remove_file(&file);
        }
    }

    /// Atomic update using ParsedFileResult from Tree-sitter.
    pub fn update_file_result(&self, relative_path: &str, result: ParsedFileResult) {
        self.remove_file(relative_path);

        for sym in &result.symbols {
            let sym_ref = SymbolRef::from(sym);

            self.definitions
                .entry(sym.name.clone())
                .or_default()
                .push(sym_ref.clone());

            for callee in &sym.callees {
                self.callers
                    .entry(callee.clone())
                    .or_default()
                    .insert(sym_ref.clone());
            }
        }

        if !result.imports.is_empty() {
            self.file_imports
                .insert(relative_path.to_string(), result.imports);
        }

        for tr in &result.types {
            self.type_relations
                .entry(tr.name.clone())
                .or_default()
                .push(tr.clone());
        }
        if !result.types.is_empty() {
            self.file_types
                .insert(relative_path.to_string(), result.types);
        }

        if !result.entrypoints.is_empty() {
            self.file_entrypoints
                .insert(relative_path.to_string(), result.entrypoints);
        }

        self.file_symbols
            .insert(relative_path.to_string(), result.symbols);
    }

    /// Full multi-index atomic update.
    pub fn update_file_full(
        &self,
        relative_path: &str,
        new_symbols: Vec<Symbol>,
        new_imports: Vec<ImportItem>,
        new_types: Vec<TypeRelation>,
        new_entrypoints: Vec<Entrypoint>,
    ) {
        let result = ParsedFileResult {
            symbols: new_symbols,
            callers: HashMap::new(),
            imports: new_imports,
            types: new_types,
            entrypoints: new_entrypoints,
        };
        self.update_file_result(relative_path, result);
    }

    /// Backward-compatible wrapper for updating just symbols.
    pub fn update_file(&self, relative_path: &str, new_symbols: Vec<Symbol>) {
        self.update_file_full(
            relative_path,
            new_symbols,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
    }

    // --- Query Accessors for Downstream Intelligence Engines (M2–M5) ---

    pub fn get_file_imports(&self, path: &str) -> Vec<ImportItem> {
        let root = self.root_path();
        let clean = clean_relative_path(&root, path);
        self.file_imports.get(&clean).map(|v| v.clone()).unwrap_or_default()
    }

    pub fn get_all_file_imports(&self) -> Vec<(String, Vec<ImportItem>)> {
        self.file_imports
            .iter()
            .map(|kv| (kv.key().clone(), kv.value().clone()))
            .collect()
    }

    pub fn get_type_relations(&self, name: &str) -> Vec<TypeRelation> {
        self.type_relations.get(name).map(|v| v.clone()).unwrap_or_default()
    }

    pub fn get_all_type_relations(&self) -> Vec<TypeRelation> {
        let mut list = Vec::new();
        for item in self.type_relations.iter() {
            list.extend(item.value().clone());
        }
        list
    }

    pub fn get_file_types(&self, path: &str) -> Vec<TypeRelation> {
        let root = self.root_path();
        let clean = clean_relative_path(&root, path);
        self.file_types.get(&clean).map(|v| v.clone()).unwrap_or_default()
    }

    pub fn get_file_entrypoints(&self, path: &str) -> Vec<Entrypoint> {
        let root = self.root_path();
        let clean = clean_relative_path(&root, path);
        self.file_entrypoints.get(&clean).map(|v| v.clone()).unwrap_or_default()
    }

    pub fn get_all_entrypoints(&self, category_filter: Option<&str>) -> Vec<Entrypoint> {
        let mut results = Vec::new();
        for item in self.file_entrypoints.iter() {
            for ep in item.value() {
                if let Some(cat) = category_filter {
                    if ep.category.eq_ignore_ascii_case(cat) {
                        results.push(ep.clone());
                    }
                } else {
                    results.push(ep.clone());
                }
            }
        }
        results.sort_by(|a, b| a.file_path.cmp(&b.file_path).then(a.line.cmp(&b.line)));
        results
    }

    pub fn get_all_file_symbols(&self) -> Vec<(String, Vec<Symbol>)> {
        self.file_symbols
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect()
    }

    #[allow(dead_code)]
    pub fn file_count(&self) -> usize {
        self.file_symbols.len()
    }

    pub fn get_file_outline(&self, path: &str) -> Result<FileOutline, String> {
        let root = self.root_path();
        let clean_path = clean_relative_path(&root, path);

        // 1. Direct match
        if let Some(syms) = self.file_symbols.get(&clean_path) {
            let mut sorted = syms.clone();
            sorted.sort_by_key(|s| s.start_line);
            return Ok(FileOutline {
                file_path: clean_path,
                total_symbols: sorted.len(),
                symbols: sorted,
            });
        }

        // 2. Suffix match (component-aware, avoid matching fast_store.rs for store.rs)
        for item in self.file_symbols.iter() {
            if path_suffix_matches(item.key(), &clean_path) {
                let mut sorted = item.value().clone();
                sorted.sort_by_key(|s| s.start_line);
                return Ok(FileOutline {
                    file_path: item.key().clone(),
                    total_symbols: sorted.len(),
                    symbols: sorted,
                });
            }
        }

        // 3. On-demand load from disk
        let full_path = root.join(&clean_path);
        if full_path.is_file() {
            if let Some(lang) = SupportedLanguage::from_path(&full_path) {
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    if let Ok(syms) = parse_file(&clean_path, &content, lang) {
                        self.update_file(&clean_path, syms.clone());
                        let mut sorted = syms;
                        sorted.sort_by_key(|s| s.start_line);
                        return Ok(FileOutline {
                            file_path: clean_path,
                            total_symbols: sorted.len(),
                            symbols: sorted,
                        });
                    }
                }
            }
        }

        Err(format!("File '{}' not found in indexed project.", path))
    }

    #[allow(dead_code)]
    pub fn find_definition(&self, name: &str) -> Vec<DefinitionResult> {
        self.find_definition_advanced(name, None, None)
    }

    pub fn find_definition_advanced(
        &self,
        name: &str,
        file_filter: Option<&str>,
        container_filter: Option<&str>,
    ) -> Vec<DefinitionResult> {
        let mut results = Vec::new();

        if let Some(refs) = self.definitions.get(name) {
            for r in refs.iter() {
                results.push(self.to_definition_result(r));
            }
        }

        // Support container-qualified lookup (e.g. "Point::distance" or "Calculator.calculate")
        if results.is_empty() {
            let parts: Vec<&str> = if name.contains("::") {
                name.split("::").collect()
            } else if name.contains('.') {
                name.split('.').collect()
            } else {
                Vec::new()
            };

            if parts.len() == 2 {
                let container = parts[0];
                let sym_name = parts[1];
                if let Some(refs) = self.definitions.get(sym_name) {
                    for r in refs.iter() {
                        if r.container_name.as_deref().map(|c| c.eq_ignore_ascii_case(container)).unwrap_or(false) {
                            results.push(self.to_definition_result(r));
                        }
                    }
                }
            }
        }

        // Case-insensitive fallback
        if results.is_empty() {
            let name_lower = name.to_lowercase();
            for item in self.definitions.iter() {
                if item.key().to_lowercase() == name_lower {
                    for r in item.value().iter() {
                        results.push(self.to_definition_result(r));
                    }
                }
            }
        }

        // Apply filters
        if let Some(container) = container_filter {
            results.retain(|r| {
                r.container_name
                    .as_deref()
                    .map(|c| c.eq_ignore_ascii_case(container))
                    .unwrap_or(false)
            });
        }

        if let Some(ff) = file_filter {
            results.retain(|r| path_suffix_matches(&r.file_path, ff));
        }

        results
    }

    #[allow(dead_code)]
    pub fn get_call_graph(&self, name: &str) -> CallGraphResult {
        self.get_call_graph_advanced(name, None, None)
    }

    pub fn get_call_graph_advanced(
        &self,
        name: &str,
        file_filter: Option<&str>,
        container_filter: Option<&str>,
    ) -> CallGraphResult {
        // Resolve actual symbol name if given qualified name like Point::distance
        let (query_name, qualified_container) = if let Some((c, s)) = name.split_once("::") {
            (s, Some(c))
        } else if let Some((c, s)) = name.split_once('.') {
            (s, Some(c))
        } else {
            (name, None)
        };

        let effective_container = container_filter.or(qualified_container);

        let all_defs = self.definitions.get(query_name)
            .map(|d| d.value().clone())
            .unwrap_or_default();

        let mut definitions: Vec<SymbolRef> = if let Some(container) = effective_container {
            all_defs
                .into_iter()
                .filter(|d| d.container_name.as_deref().map(|c| c.eq_ignore_ascii_case(container)).unwrap_or(false))
                .collect()
        } else {
            all_defs
        };

        if let Some(ff) = file_filter {
            definitions.retain(|d| path_suffix_matches(&d.file_path, ff));
        }

        let mut callers: Vec<SymbolRef> = self.callers.get(query_name)
            .map(|c| c.value().iter().cloned().collect())
            .unwrap_or_default();

        if let Some(ff) = file_filter {
            callers.retain(|c| path_suffix_matches(&c.file_path, ff));
        }

        callers.sort_by(|a, b| a.file_path.cmp(&b.file_path).then(a.start_line.cmp(&b.start_line)));

        // Resolve callees called by definitions of this symbol
        let mut callee_set = HashSet::new();
        for def in &definitions {
            if let Some(syms) = self.file_symbols.get(&def.file_path) {
                if let Some(sym) = syms.iter().find(|s| s.name == def.name && s.start_line == def.start_line) {
                    for callee in &sym.callees {
                        callee_set.insert(callee.clone());
                    }
                }
            }
        }
        let mut callees: Vec<String> = callee_set.into_iter().collect();
        callees.sort();

        // Build callee details with candidate definitions for high-precision navigation
        let mut callee_details = Vec::new();
        for callee in &callees {
            let candidates = self.definitions.get(callee)
                .map(|d| d.value().clone())
                .unwrap_or_default();
            callee_details.push(CalleeDetail {
                name: callee.clone(),
                candidates,
            });
        }

        CallGraphResult {
            symbol_name: name.to_string(),
            definitions,
            callers,
            callees,
            callee_details,
        }
    }

    pub fn fuzzy_search_symbols(
        &self,
        query: &str,
        kind_filter: Option<&str>,
        limit: usize,
    ) -> Vec<SymbolMatch> {
        let mut matches = Vec::new();
        let max_results = if limit == 0 { 30 } else { limit };

        for entry in self.file_symbols.iter() {
            for sym in entry.value() {
                if let Some(filter) = kind_filter {
                    let kind_str = format!("{:?}", sym.kind).to_lowercase();
                    if kind_str != filter.to_lowercase() {
                        continue;
                    }
                }

                // Check match against symbol name
                let score = score_match(query, &sym.name).or_else(|| {
                    sym.container_name.as_ref().and_then(|container| {
                        score_match(query, &format!("{}::{}", container, sym.name))
                    })
                });

                if let Some(score) = score {
                    matches.push(SymbolMatch {
                        name: sym.name.clone(),
                        kind: sym.kind,
                        file_path: sym.file_path.clone(),
                        line: sym.start_line,
                        signature: sym.signature.clone(),
                        container_name: sym.container_name.clone(),
                        score,
                    });
                }
            }
        }

        matches.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then(a.name.len().cmp(&b.name.len()))
                .then(a.name.cmp(&b.name))
        });

        matches.truncate(max_results);
        matches
    }

    pub fn get_project_stats(&self) -> ProjectStats {
        let mut total_symbols = 0;
        let mut functions_count = 0;
        let mut types_count = 0;
        let mut lang_counts: HashMap<String, usize> = HashMap::new();

        for entry in self.file_symbols.iter() {
            let path = Path::new(entry.key());
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("unknown")
                .to_lowercase();
            let lang_label = match ext.as_str() {
                "rs" => "Rust",
                "py" => "Python",
                "ts" => "TypeScript",
                "tsx" => "TypeScript (TSX)",
                "js" | "mjs" | "cjs" => "JavaScript",
                "jsx" => "JavaScript (JSX)",
                "lua" => "Lua",
                "go" => "Go",
                "c" => "C",
                "h" => "C/C++ Header",
                "cpp" | "cc" | "cxx" => "C++",
                "hpp" | "hh" | "hxx" => "C++ Header",
                other => other,
            };
            *lang_counts.entry(lang_label.to_string()).or_insert(0) += 1;

            for sym in entry.value() {
                total_symbols += 1;
                match sym.kind {
                    SymbolKind::Function | SymbolKind::Method => functions_count += 1,
                    SymbolKind::Class
                    | SymbolKind::Struct
                    | SymbolKind::Interface
                    | SymbolKind::Trait
                    | SymbolKind::Enum
                    | SymbolKind::TypeAlias => types_count += 1,
                    _ => {}
                }
            }
        }

        ProjectStats {
            root_path: self.root_path().to_string_lossy().to_string(),
            total_files_indexed: self.file_symbols.len(),
            total_symbols_indexed: total_symbols,
            total_functions_and_methods: functions_count,
            total_types_and_classes: types_count,
            language_file_counts: lang_counts,
        }
    }

    fn to_definition_result(&self, r: &SymbolRef) -> DefinitionResult {
        let sym = self
            .file_symbols
            .get(&r.file_path)
            .and_then(|syms| syms.iter().find(|s| s.name == r.name && s.start_line == r.start_line).cloned());

        let (doc, callees) = match sym {
            Some(s) => (s.doc, s.callees),
            None => (None, Vec::new()),
        };

        DefinitionResult {
            name: r.name.clone(),
            kind: r.kind,
            file_path: r.file_path.clone(),
            start_line: r.start_line,
            end_line: r.end_line,
            signature: r.signature.clone(),
            doc,
            container_name: r.container_name.clone(),
            callees,
        }
    }
}

pub fn clean_relative_path(root: &Path, input: &str) -> String {
    let normalized = input.replace('\\', "/");
    let normalized = normalized.strip_prefix("//?/").unwrap_or(&normalized);
    let clean = normalized.trim_start_matches("./");

    let root_str = root.to_string_lossy().replace('\\', "/");
    let root_str = root_str.strip_prefix("//?/").unwrap_or(&root_str);
    let clean_root = root_str.trim_end_matches('/');

    #[cfg(windows)]
    let matches_root = clean.to_lowercase().starts_with(&clean_root.to_lowercase());
    #[cfg(not(windows))]
    let matches_root = clean.starts_with(clean_root);

    if matches_root && clean.len() > clean_root.len() {
        clean[clean_root.len()..].trim_start_matches('/').to_string()
    } else {
        clean.to_string()
    }
}

fn path_suffix_matches(full_key: &str, query: &str) -> bool {
    if full_key == query {
        return true;
    }
    let query_suffix = format!("/{}", query.trim_start_matches('/'));
    if full_key.ends_with(&query_suffix) {
        return true;
    }
    let key_suffix = format!("/{}", full_key.trim_start_matches('/'));
    if query.ends_with(&key_suffix) {
        return true;
    }
    false
}

fn score_match(query: &str, target: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }
    let q = query.to_lowercase();
    let t = target.to_lowercase();

    if t == q {
        return Some(100);
    }
    if t.starts_with(&q) {
        return Some(80);
    }
    if t.contains(&q) {
        return Some(60);
    }

    // Subsequence check
    let mut q_chars = q.chars();
    let mut current_q = q_chars.next();
    for c in t.chars() {
        if let Some(qc) = current_q {
            if c == qc {
                current_q = q_chars.next();
            }
        } else {
            break;
        }
    }
    if current_q.is_none() {
        Some(40)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_crud_and_call_graph() {
        let store = CodeStore::new(PathBuf::from("."));

        let sym1 = Symbol {
            name: "caller_fn".to_string(),
            kind: SymbolKind::Function,
            file_path: "src/main.rs".to_string(),
            start_line: 10,
            end_line: 20,
            signature: "fn caller_fn()".to_string(),
            doc: Some("A caller function".to_string()),
            container_name: None,
            callees: vec!["worker_fn".to_string()],
        };

        let sym2 = Symbol {
            name: "worker_fn".to_string(),
            kind: SymbolKind::Function,
            file_path: "src/worker.rs".to_string(),
            start_line: 5,
            end_line: 15,
            signature: "fn worker_fn()".to_string(),
            doc: None,
            container_name: None,
            callees: vec![],
        };

        store.update_file("src/main.rs", vec![sym1]);
        store.update_file("src/worker.rs", vec![sym2]);

        // Definition test
        let defs = store.find_definition("worker_fn");
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].file_path, "src/worker.rs");

        // Call graph test
        let cg = store.get_call_graph("worker_fn");
        assert_eq!(cg.callers.len(), 1);
        assert_eq!(cg.callers[0].name, "caller_fn");

        let cg_caller = store.get_call_graph("caller_fn");
        assert_eq!(cg_caller.callees.len(), 1);
        assert_eq!(cg_caller.callees[0], "worker_fn");
        assert_eq!(cg_caller.callee_details.len(), 1);
        assert_eq!(cg_caller.callee_details[0].candidates.len(), 1);
        assert_eq!(cg_caller.callee_details[0].candidates[0].file_path, "src/worker.rs");

        // Advanced filter test
        let defs_filtered = store.find_definition_advanced("worker_fn", Some("worker.rs"), None);
        assert_eq!(defs_filtered.len(), 1);
        let defs_mismatch = store.find_definition_advanced("worker_fn", Some("other.rs"), None);
        assert_eq!(defs_mismatch.len(), 0);

        // Fuzzy search test
        let matches = store.fuzzy_search_symbols("worker", None, 10);
        assert!(!matches.is_empty());
        assert_eq!(matches[0].name, "worker_fn");

        // Stats test
        let stats = store.get_project_stats();
        assert_eq!(stats.total_files_indexed, 2);
        assert_eq!(stats.total_functions_and_methods, 2);
    }

    #[test]
    fn test_same_name_different_files_callees_isolation() {
        let store = CodeStore::new(PathBuf::from("."));

        // File A has 'run' which calls 'task_a'
        let sym_a = Symbol {
            name: "run".to_string(),
            kind: SymbolKind::Function,
            file_path: "src/a.rs".to_string(),
            start_line: 1,
            end_line: 5,
            signature: "fn run()".to_string(),
            doc: None,
            container_name: None,
            callees: vec!["task_a".to_string()],
        };

        // File B has 'run' which calls 'task_b'
        let sym_b = Symbol {
            name: "run".to_string(),
            kind: SymbolKind::Function,
            file_path: "src/b.rs".to_string(),
            start_line: 1,
            end_line: 5,
            signature: "fn run()".to_string(),
            doc: None,
            container_name: None,
            callees: vec!["task_b".to_string()],
        };

        store.update_file("src/a.rs", vec![sym_a]);
        store.update_file("src/b.rs", vec![sym_b]);

        let cg = store.get_call_graph("run");
        assert!(cg.callees.contains(&"task_a".to_string()));
        assert!(cg.callees.contains(&"task_b".to_string()));

        // Remove file A
        store.remove_file("src/a.rs");

        // File B's callees must still be present and not wiped out!
        let cg_after = store.get_call_graph("run");
        assert!(!cg_after.callees.contains(&"task_a".to_string()));
        assert!(cg_after.callees.contains(&"task_b".to_string()));
    }

    #[test]
    fn test_path_suffix_matching() {
        assert!(path_suffix_matches("src/store.rs", "store.rs"));
        assert!(path_suffix_matches("src/store.rs", "src/store.rs"));
        assert!(!path_suffix_matches("src/fast_store.rs", "store.rs"));
        assert!(!path_suffix_matches("extra.rs", "a.rs"));
        assert!(path_suffix_matches("src/store.rs", "project/src/store.rs"));
    }

    #[test]
    fn test_store_multi_index_extensions() {
        let store = CodeStore::new(PathBuf::from("."));

        let sym = Symbol {
            name: "calculate".to_string(),
            kind: SymbolKind::Method,
            file_path: "src/calc.rs".to_string(),
            start_line: 10,
            end_line: 20,
            signature: "fn calculate(&self)".to_string(),
            doc: None,
            container_name: Some("Calculator".to_string()),
            callees: vec![],
        };

        let import = ImportItem {
            source_path: "src/calc.rs".to_string(),
            specifier: "std::sync::Arc".to_string(),
            is_external: true,
            line: 1,
        };

        let type_rel = TypeRelation {
            name: "Calculator".to_string(),
            supertypes: vec!["Compute".to_string()],
            is_trait: false,
            methods: vec!["calculate".to_string()],
            file_path: "src/calc.rs".to_string(),
        };

        let entrypoint = Entrypoint {
            name: "start_calc".to_string(),
            category: "startup".to_string(),
            file_path: "src/calc.rs".to_string(),
            line: 50,
            route_or_cmd: None,
        };

        let result = ParsedFileResult {
            symbols: vec![sym],
            callers: HashMap::new(),
            imports: vec![import],
            types: vec![type_rel],
            entrypoints: vec![entrypoint],
        };

        store.update_file_result("src/calc.rs", result);

        assert_eq!(store.file_count(), 1);
        let imps = store.get_file_imports("src/calc.rs");
        assert_eq!(imps.len(), 1);
        assert_eq!(imps[0].specifier, "std::sync::Arc");

        let types = store.get_type_relations("Calculator");
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].supertypes, vec!["Compute"]);

        let eps = store.get_all_entrypoints(Some("startup"));
        assert_eq!(eps.len(), 1);
        assert_eq!(eps[0].name, "start_calc");

        // Clear store
        store.clear();
        assert_eq!(store.file_count(), 0);
        assert!(store.get_file_imports("src/calc.rs").is_empty());
        assert!(store.get_type_relations("Calculator").is_empty());
    }

    #[test]
    fn test_remove_path_or_prefix() {
        let store = CodeStore::new(PathBuf::from("."));

        let sym_a = Symbol {
            name: "fn_a".to_string(),
            kind: SymbolKind::Function,
            file_path: "src/module/a.rs".to_string(),
            start_line: 1,
            end_line: 5,
            signature: "fn fn_a()".to_string(),
            doc: None,
            container_name: None,
            callees: vec![],
        };

        let sym_b = Symbol {
            name: "fn_b".to_string(),
            kind: SymbolKind::Function,
            file_path: "src/module/b.rs".to_string(),
            start_line: 1,
            end_line: 5,
            signature: "fn fn_b()".to_string(),
            doc: None,
            container_name: None,
            callees: vec![],
        };

        let sym_other = Symbol {
            name: "fn_other".to_string(),
            kind: SymbolKind::Function,
            file_path: "src/other.rs".to_string(),
            start_line: 1,
            end_line: 5,
            signature: "fn fn_other()".to_string(),
            doc: None,
            container_name: None,
            callees: vec![],
        };

        store.update_file("src/module/a.rs", vec![sym_a]);
        store.update_file("src/module/b.rs", vec![sym_b]);
        store.update_file("src/other.rs", vec![sym_other]);

        assert_eq!(store.file_count(), 3);

        // Remove prefix "src/module"
        store.remove_path_or_prefix("src/module");

        assert_eq!(store.file_count(), 1);
        assert!(store.find_definition("fn_a").is_empty());
        assert!(store.find_definition("fn_b").is_empty());
        assert_eq!(store.find_definition("fn_other").len(), 1);
    }
}
