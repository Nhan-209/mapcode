use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::parser::{parse_file, SupportedLanguage, Symbol, SymbolKind};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SymbolRef {
    pub name: String,
    pub kind: SymbolKind,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
}

impl From<&Symbol> for SymbolRef {
    fn from(sym: &Symbol) -> Self {
        Self {
            name: sym.name.clone(),
            kind: sym.kind,
            file_path: sym.file_path.clone(),
            start_line: sym.start_line,
            end_line: sym.end_line,
            signature: sym.signature.clone(),
            container_name: sym.container_name.clone(),
        }
    }
}

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
}

impl CodeStore {
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            root_path: std::sync::RwLock::new(root_path),
            file_symbols: DashMap::new(),
            definitions: DashMap::new(),
            callers: DashMap::new(),
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
    }

    pub fn update_file(&self, relative_path: &str, new_symbols: Vec<Symbol>) {
        self.remove_file(relative_path);

        for sym in &new_symbols {
            let sym_ref = SymbolRef::from(sym);

            // Index definition
            self.definitions
                .entry(sym.name.clone())
                .or_default()
                .push(sym_ref.clone());

            // Index callers: sym calls each callee in sym.callees
            for callee in &sym.callees {
                self.callers
                    .entry(callee.clone())
                    .or_default()
                    .insert(sym_ref.clone());
            }
        }

        self.file_symbols.insert(relative_path.to_string(), new_symbols);
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
}
