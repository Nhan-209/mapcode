use std::collections::{HashSet, VecDeque};
use serde::{Deserialize, Serialize};

use crate::entrypoint::{get_categorized_entrypoints, EntrypointItem};
use crate::store::{clean_relative_path, CodeStore, SymbolRef};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ImpactReport {
    pub target: String,
    pub direct_callers: Vec<SymbolRef>,
    pub transitive_callers: Vec<SymbolRef>,
    pub dependent_files: Vec<String>,
    pub reachable_entrypoints: Vec<EntrypointItem>,
    pub risk_score: usize, // 0..=100
    pub risk_rating: String, // "low" | "medium" | "high" | "critical"
}

/// Computes the comprehensive blast-radius impact analysis for a target symbol,
/// type, or file, calculating upstream callers up to max_depth, dependent files,
/// reachable public entrypoints, and an objective risk score.
pub fn compute_impact(store: &CodeStore, target: &str, max_depth: usize) -> ImpactReport {
    let depth_limit = if max_depth == 0 { 3 } else { max_depth.min(10) };
    let clean_target = target.trim();

    let root = store.root_path();
    let is_file_target = clean_target.contains('/') || clean_target.contains('\\') || clean_target.contains('.');

    // 1. Direct callers of the target
    let cg = store.get_call_graph(clean_target);
    let direct_callers = cg.callers;

    // 2. Transitive callers via BFS traversal
    let mut transitive_callers = Vec::new();
    let mut visited_symbols: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    visited_symbols.insert(clean_target.to_string());
    for caller in &direct_callers {
        if !visited_symbols.contains(&caller.name) {
            visited_symbols.insert(caller.name.clone());
            queue.push_back((caller.name.clone(), 1));
        }
    }

    let mut max_depth_reached = if direct_callers.is_empty() { 0 } else { 1 };

    while let Some((curr_sym, depth)) = queue.pop_front() {
        if depth >= depth_limit {
            continue;
        }

        let upstream_cg = store.get_call_graph(&curr_sym);
        for upstream in upstream_cg.callers {
            if !visited_symbols.contains(&upstream.name) {
                visited_symbols.insert(upstream.name.clone());
                max_depth_reached = max_depth_reached.max(depth + 1);
                transitive_callers.push(upstream.clone());
                queue.push_back((upstream.name, depth + 1));
            }
        }
    }

    transitive_callers.sort_by(|a, b| a.file_path.cmp(&b.file_path).then(a.start_line.cmp(&b.start_line)));

    // 3. Dependent files (files importing the target's definition file, or the target file itself)
    let mut target_files = HashSet::new();
    if is_file_target {
        let cleaned = clean_relative_path(&root, clean_target);
        target_files.insert(cleaned);
    } else {
        for def in &cg.definitions {
            target_files.insert(def.file_path.clone());
        }
    }

    let mut dependent_files_set = HashSet::new();
    let all_imports = store.get_all_file_imports();

    for t_file in &target_files {
        for (importer_file, items) in &all_imports {
            if importer_file == t_file {
                continue;
            }
            for item in items {
                if !item.is_external
                    && (item.source_path == *t_file || item.source_path.ends_with(t_file) || t_file.ends_with(&item.source_path))
                {
                    dependent_files_set.insert(importer_file.clone());
                }
            }
        }
    }

    let mut dependent_files: Vec<String> = dependent_files_set.into_iter().collect();
    dependent_files.sort();

    // 4. Reachable Entrypoints
    let all_entrypoints = get_categorized_entrypoints(store, None);
    let mut reachable_entrypoints = Vec::new();

    let all_caller_names: HashSet<String> = direct_callers
        .iter()
        .chain(transitive_callers.iter())
        .map(|c| c.name.clone())
        .collect();

    for ep in all_entrypoints {
        let matches_caller = all_caller_names.contains(&ep.name);
        let matches_file = dependent_files.contains(&ep.file_path) || target_files.contains(&ep.file_path);

        if matches_caller || matches_file {
            reachable_entrypoints.push(ep);
        }
    }

    reachable_entrypoints.dedup_by(|a, b| a.file_path == b.file_path && a.line == b.line);

    // 5. Objective Risk Score calculation:
    // S = min(100, 20*E + 5*C + 8*F + 2*D)
    let e = reachable_entrypoints.len();
    let c = direct_callers.len() + transitive_callers.len();
    let f = dependent_files.len();
    let d = max_depth_reached;

    let raw_score = (20 * e) + (5 * c) + (8 * f) + (2 * d);
    let risk_score = raw_score.min(100);

    let risk_rating = if risk_score >= 70 {
        "critical".to_string()
    } else if risk_score >= 40 {
        "high".to_string()
    } else if risk_score >= 15 {
        "medium".to_string()
    } else {
        "low".to_string()
    };

    ImpactReport {
        target: clean_target.to_string(),
        direct_callers,
        transitive_callers,
        dependent_files,
        reachable_entrypoints,
        risk_score,
        risk_rating,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::collections::HashMap;
    use crate::parser::{Entrypoint, ParsedFileResult, Symbol, SymbolKind};

    #[test]
    fn test_compute_impact_and_risk_score() {
        let store = CodeStore::new(PathBuf::from("."));

        store.update_file_result(
            "src/service.rs",
            ParsedFileResult {
                symbols: vec![
                    Symbol {
                        name: "calc".to_string(),
                        kind: SymbolKind::Function,
                        file_path: "src/service.rs".to_string(),
                        start_line: 1,
                        end_line: 10,
                        signature: "fn calc()".to_string(),
                        doc: None,
                        container_name: None,
                        callees: vec![],
                    },
                    Symbol {
                        name: "handler".to_string(),
                        kind: SymbolKind::Function,
                        file_path: "src/service.rs".to_string(),
                        start_line: 12,
                        end_line: 20,
                        signature: "fn handler()".to_string(),
                        doc: None,
                        container_name: None,
                        callees: vec!["calc".to_string()],
                    },
                ],
                callers: HashMap::new(),
                imports: vec![],
                types: vec![],
                entrypoints: vec![Entrypoint {
                    name: "handler".to_string(),
                    category: "http".to_string(),
                    file_path: "src/service.rs".to_string(),
                    line: 12,
                    route_or_cmd: Some("GET /api/calc".to_string()),
                }],
            },
        );

        let report = compute_impact(&store, "calc", 3);
        assert_eq!(report.direct_callers.len(), 1);
        assert_eq!(report.direct_callers[0].name, "handler");
        assert_eq!(report.reachable_entrypoints.len(), 1);
        assert!(report.risk_score >= 25);
    }
}
