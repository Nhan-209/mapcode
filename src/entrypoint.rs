use serde::{Deserialize, Serialize};
use crate::store::CodeStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct EntrypointItem {
    pub name: String,
    pub category: String, // "startup", "http", "cli", "worker"
    pub file_path: String,
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_or_cmd: Option<String>,
}

/// Retrieves all detected system entrypoints (application startup, HTTP API routes,
/// CLI command handlers, background workers, event listeners), optionally filtered by category.
pub fn get_categorized_entrypoints(
    store: &CodeStore,
    category_filter: Option<&str>,
) -> Vec<EntrypointItem> {
    let raw_entrypoints = store.get_all_entrypoints(category_filter);

    let mut results: Vec<EntrypointItem> = raw_entrypoints
        .into_iter()
        .map(|ep| EntrypointItem {
            name: ep.name,
            category: ep.category,
            file_path: ep.file_path,
            line: ep.line,
            route_or_cmd: ep.route_or_cmd,
        })
        .collect();

    // Fallback detection: If no startup entrypoint was registered directly in store,
    // detect standard 'main' functions across symbols in project files.
    let wants_startup = category_filter.map(|c| c.eq_ignore_ascii_case("startup")).unwrap_or(true);
    if wants_startup && !results.iter().any(|r| r.category == "startup") {
        for (file_path, syms) in store.get_all_file_symbols() {
            for sym in syms {
                if sym.name == "main" || sym.signature.contains("fn main(") || sym.signature.contains("func main(") {
                    results.push(EntrypointItem {
                        name: sym.name.clone(),
                        category: "startup".to_string(),
                        file_path: file_path.clone(),
                        line: sym.start_line,
                        route_or_cmd: None,
                    });
                }
            }
        }
    }

    // Sort deterministically by category, file_path, and line
    results.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then(a.file_path.cmp(&b.file_path))
            .then(a.line.cmp(&b.line))
    });

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::collections::HashMap;
    use crate::parser::{Entrypoint, ParsedFileResult, Symbol, SymbolKind};

    #[test]
    fn test_entrypoint_categorization() {
        let store = CodeStore::new(PathBuf::from("."));

        store.update_file_result(
            "src/main.rs",
            ParsedFileResult {
                symbols: vec![Symbol {
                    name: "main".to_string(),
                    kind: SymbolKind::Function,
                    file_path: "src/main.rs".to_string(),
                    start_line: 10,
                    end_line: 25,
                    signature: "fn main()".to_string(),
                    doc: None,
                    container_name: None,
                    callees: vec![],
                }],
                callers: HashMap::new(),
                imports: vec![],
                types: vec![],
                entrypoints: vec![
                    Entrypoint {
                        name: "main".to_string(),
                        category: "startup".to_string(),
                        file_path: "src/main.rs".to_string(),
                        line: 10,
                        route_or_cmd: None,
                    },
                    Entrypoint {
                        name: "get_users".to_string(),
                        category: "http".to_string(),
                        file_path: "src/main.rs".to_string(),
                        line: 30,
                        route_or_cmd: Some("GET /api/users".to_string()),
                    },
                ],
            },
        );

        let all = get_categorized_entrypoints(&store, None);
        assert_eq!(all.len(), 2);

        let http_only = get_categorized_entrypoints(&store, Some("http"));
        assert_eq!(http_only.len(), 1);
        assert_eq!(http_only[0].route_or_cmd, Some("GET /api/users".to_string()));
    }
}
