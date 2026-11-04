use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;

use crate::resolve::{language_family_from_path, LanguageFamily};
use crate::store::{clean_relative_path, CodeStore};
use crate::watcher::should_ignore;

const MAX_LINE_CHARS: usize = 200;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMatch {
    pub file_path: String,
    pub line_number: usize,
    pub line_text: String,
    #[serde(default)]
    pub is_definition: bool,
}

fn is_comment_line(trimmed: &str) -> bool {
    trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with("--")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
}

fn clip_line(line: &str) -> String {
    let t = line.trim();
    if t.chars().count() <= MAX_LINE_CHARS {
        t.to_string()
    } else {
        let cut: String = t.chars().take(MAX_LINE_CHARS).collect();
        format!("{}…", cut)
    }
}

/// Turns a user-supplied filter ('.rs', '*.rs', 'src/', 'store.rs', 'src/**/*.rs')
/// into gitignore-style whitelist globs.
fn normalize_globs(filter: &str) -> Vec<String> {
    let g = filter.trim().replace('\\', "/");
    if g.is_empty() {
        return Vec::new();
    }
    if g.contains(['*', '?', '[']) {
        return vec![g];
    }
    if g.starts_with('.') {
        return vec![format!("*{}", g)];
    }
    let d = g.trim_end_matches('/');
    vec![format!("**/{}", d), format!("{}/**", d), format!("**/{}/**", d)]
}

struct SearchOpts<'a> {
    glob: Option<&'a str>,
    limit: usize,
    source_only: bool,
    skip_comments: bool,
}

fn search_impl(store: &CodeStore, regex: &Regex, opts: &SearchOpts) -> Result<Vec<TextMatch>, String> {
    let root = store.root_path();
    let max_matches = if opts.limit == 0 { 50 } else { opts.limit.min(500) };

    let mut walker = WalkBuilder::new(&root);
    walker
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .sort_by_file_path(|a, b| a.cmp(b));

    if let Some(filter) = opts.glob {
        let globs = normalize_globs(filter);
        if !globs.is_empty() {
            let mut ob = OverrideBuilder::new(&root);
            for g in &globs {
                ob.add(g).map_err(|e| format!("Invalid glob '{}': {}", filter, e))?;
            }
            walker.overrides(ob.build().map_err(|e| format!("Invalid glob '{}': {}", filter, e))?);
        }
    }

    let mut matches = Vec::new();

    'files: for result in walker.build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if !path.is_file() || should_ignore(path) {
            continue;
        }

        let rel_path = clean_relative_path(&root, &path.to_string_lossy());
        if opts.source_only && language_family_from_path(&rel_path) == LanguageFamily::Other {
            continue;
        }

        if let Ok(meta) = entry.metadata() {
            if meta.len() > MAX_FILE_BYTES {
                continue;
            }
        }

        // Non-UTF8 / binary files fail here and are skipped.
        let Ok(content) = fs::read_to_string(path) else { continue };
        for (idx, line) in content.lines().enumerate() {
            if !regex.is_match(line) {
                continue;
            }
            if opts.skip_comments && is_comment_line(line.trim()) {
                continue;
            }
            matches.push(TextMatch {
                file_path: rel_path.clone(),
                line_number: idx + 1,
                line_text: clip_line(line),
                is_definition: false,
            });
            if matches.len() >= max_matches {
                break 'files;
            }
        }
    }

    Ok(matches)
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
    let regex = if is_regex {
        Regex::new(pattern).map_err(|e| format!("Invalid regex '{}': {}", pattern, e))?
    } else {
        Regex::new(&regex::escape(pattern)).map_err(|e| e.to_string())?
    };
    search_impl(
        store,
        &regex,
        &SearchOpts { glob: glob_filter, limit, source_only: false, skip_comments: false },
    )
}

/// Finds whole-word references to a symbol in source files (comment lines excluded),
/// flagging the lines that are definitions.
pub fn find_references_in_workspace(
    store: &CodeStore,
    name: &str,
    file_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<TextMatch>, String> {
    if name.trim().is_empty() {
        return Ok(Vec::new());
    }
    let regex = Regex::new(&format!(r"\b{}\b", regex::escape(name.trim())))
        .map_err(|e| e.to_string())?;
    let mut matches = search_impl(
        store,
        &regex,
        &SearchOpts { glob: file_filter, limit, source_only: true, skip_comments: true },
    )?;

    let bare = name.rsplit([':', '.']).next().unwrap_or(name);
    let def_lines: HashSet<(String, usize)> = store
        .find_definition_advanced(bare, None, None)
        .into_iter()
        .map(|d| (d.file_path, d.start_line))
        .collect();
    for m in &mut matches {
        m.is_definition = def_lines.contains(&(m.file_path.clone(), m.line_number));
    }
    Ok(matches)
}
