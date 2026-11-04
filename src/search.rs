use ignore::WalkBuilder;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;

use crate::store::{clean_relative_path, CodeStore};
use crate::watcher::should_ignore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMatch {
    pub file_path: String,
    pub line_number: usize,
    pub line_text: String,
}

/// Fast codebase text search respecting .gitignore with security boundaries.
pub fn search_text_in_workspace(
    store: &CodeStore,
    pattern: &str,
    is_regex: bool,
    glob_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<TextMatch>, String> {
    if pattern.trim().is_empty() {
        return Ok(Vec::new());
    }

    let root = store.root_path();
    let max_matches = if limit == 0 { 50 } else { limit.min(500) };

    let regex = if is_regex {
        Regex::new(pattern).map_err(|e| format!("Invalid regex '{}': {}", pattern, e))?
    } else {
        Regex::new(&regex::escape(pattern)).map_err(|e| e.to_string())?
    };

    let mut walker = WalkBuilder::new(&root);
    walker
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true);

    let mut matches = Vec::new();

    for result in walker.build() {
        if matches.len() >= max_matches {
            break;
        }

        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if !path.is_file() || should_ignore(path) {
            continue;
        }

        let rel_path = clean_relative_path(&root, &path.to_string_lossy());

        if let Some(glob) = glob_filter {
            let clean_glob = glob.trim_start_matches('*').trim_start_matches('.');
            if !rel_path.ends_with(clean_glob) && !rel_path.contains(glob) {
                continue;
            }
        }

        // Limit file size to 2 MB to prevent memory spikes
        if let Ok(meta) = entry.metadata() {
            if meta.len() > 2 * 1024 * 1024 {
                continue;
            }
        }

        if let Ok(content) = fs::read_to_string(path) {
            for (idx, line) in content.lines().enumerate() {
                if regex.is_match(line) {
                    matches.push(TextMatch {
                        file_path: rel_path.clone(),
                        line_number: idx + 1,
                        line_text: line.trim().to_string(),
                    });

                    if matches.len() >= max_matches {
                        break;
                    }
                }
            }
        }
    }

    Ok(matches)
}

/// Finds code references to a symbol across the indexed codebase, excluding comments and string literals.
pub fn find_references_in_workspace(
    store: &CodeStore,
    name: &str,
    file_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<TextMatch>, String> {
    let word_pattern = format!(r"\b{}\b", regex::escape(name));
    let mut matches = search_text_in_workspace(store, &word_pattern, true, file_filter, limit)?;

    // Filter out obvious comment lines across languages
    matches.retain(|m| {
        let trimmed = m.line_text.trim();
        !trimmed.starts_with("//")
            && !trimmed.starts_with('#')
            && !trimmed.starts_with("--")
            && !trimmed.starts_with("/*")
            && !trimmed.starts_with('*')
    });

    Ok(matches)
}
