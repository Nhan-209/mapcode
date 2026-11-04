use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::parser::CallSite;
use crate::store::{CodeStore, SymbolRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageFamily {
    Rust,
    Python,
    JavaScript,
    Go,
    CAndCpp,
    Lua,
    Other,
}

pub fn language_family_from_path(path: &str) -> LanguageFamily {
    let lower = path.to_lowercase();
    if lower.ends_with(".rs") {
        LanguageFamily::Rust
    } else if lower.ends_with(".py") {
        LanguageFamily::Python
    } else if lower.ends_with(".ts")
        || lower.ends_with(".tsx")
        || lower.ends_with(".js")
        || lower.ends_with(".jsx")
        || lower.ends_with(".mjs")
        || lower.ends_with(".cjs")
    {
        LanguageFamily::JavaScript
    } else if lower.ends_with(".go") {
        LanguageFamily::Go
    } else if lower.ends_with(".c")
        || lower.ends_with(".cpp")
        || lower.ends_with(".cc")
        || lower.ends_with(".cxx")
        || lower.ends_with(".h")
        || lower.ends_with(".hpp")
        || lower.ends_with(".hh")
        || lower.ends_with(".hxx")
    {
        LanguageFamily::CAndCpp
    } else if lower.ends_with(".lua") {
        LanguageFamily::Lua
    } else {
        LanguageFamily::Other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Exact,
    Likely,
    Ambiguous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedCall {
    pub target: SymbolRef,
    pub confidence: Confidence,
}

/// Disambiguates and resolves a CallSite to candidate definitions in the codebase.
/// Eliminates cross-language pollution and assigns an objective confidence rating.
pub fn resolve_call_site(
    store: &CodeStore,
    caller_file: &str,
    caller_container: Option<&str>,
    cs: &CallSite,
) -> Vec<ResolvedCall> {
    let caller_family = language_family_from_path(caller_file);
    if caller_family == LanguageFamily::Other {
        return Vec::new();
    }

    // 1. Fetch raw definitions matching callee name
    let raw_defs = store.find_definition_advanced(&cs.name, None, None);
    if raw_defs.is_empty() {
        return Vec::new();
    }

    // 2. Strict language family filter — cross-language calls are strictly forbidden
    let same_lang_defs: Vec<SymbolRef> = raw_defs
        .into_iter()
        .filter(|d| language_family_from_path(&d.file_path) == caller_family)
        .map(|d| SymbolRef {
            name: d.name,
            kind: d.kind,
            file_path: d.file_path,
            start_line: d.start_line,
            end_line: d.end_line,
            signature: d.signature,
            container_name: d.container_name,
        })
        .collect();

    if same_lang_defs.is_empty() {
        return Vec::new();
    }

    // Gather files directly imported by caller for locality scoring
    let imports = store.get_file_imports(caller_file);
    let imported_specifiers: HashSet<String> = imports.into_iter().map(|i| i.specifier).collect();

    let mut resolved = Vec::new();

    // 3. Disambiguation logic based on qualifier presence
    if let Some(ref q) = cs.qualifier {
        let is_self_or_this = q.eq_ignore_ascii_case("self")
            || q.eq_ignore_ascii_case("this")
            || q == "Self";

        if is_self_or_this {
            if let Some(cont) = caller_container {
                // Find matching method in same container
                for d in &same_lang_defs {
                    if d.container_name.as_deref().map(|c| c.eq_ignore_ascii_case(cont)).unwrap_or(false) {
                        resolved.push(ResolvedCall {
                            target: d.clone(),
                            confidence: Confidence::Exact,
                        });
                    }
                }
            }
        } else {
            // Check if qualifier matches container name exactly (e.g. Service::new or Calculator.calculate)
            for d in &same_lang_defs {
                if d.container_name.as_deref().map(|c| c.eq_ignore_ascii_case(q)).unwrap_or(false) {
                    resolved.push(ResolvedCall {
                        target: d.clone(),
                        confidence: Confidence::Exact,
                    });
                }
            }

            // Fallback for receiver variables (e.g. s.process() where s is instance of Service)
            if resolved.is_empty() {
                // Check if candidate is in the same file
                for d in &same_lang_defs {
                    if d.file_path == caller_file {
                        resolved.push(ResolvedCall {
                            target: d.clone(),
                            confidence: Confidence::Likely,
                        });
                    }
                }

                // Check if candidate matches imported files or containers
                if resolved.is_empty() {
                    for d in &same_lang_defs {
                        let matches_import = imported_specifiers.iter().any(|spec| {
                            let norm_spec = spec.replace("::", "/");
                            spec.contains(&d.name)
                                || d.file_path.contains(&norm_spec)
                                || d.container_name.as_deref().map(|c| spec.contains(c)).unwrap_or(false)
                        });
                        if matches_import {
                            resolved.push(ResolvedCall {
                                target: d.clone(),
                                confidence: Confidence::Likely,
                            });
                        }
                    }
                }

                // Or if only 1 matching method exists across the same language family
                if resolved.is_empty() {
                    if same_lang_defs.len() == 1 {
                        resolved.push(ResolvedCall {
                            target: same_lang_defs[0].clone(),
                            confidence: Confidence::Likely,
                        });
                    } else {
                        for d in &same_lang_defs {
                            resolved.push(ResolvedCall {
                                target: d.clone(),
                                confidence: Confidence::Ambiguous,
                            });
                        }
                    }
                }
            }
        }
    } else {
        // Unqualified call (e.g. run() or helper())
        // A. Same file has highest confidence
        let mut same_file_hits = Vec::new();
        for d in &same_lang_defs {
            if d.file_path == caller_file {
                same_file_hits.push(ResolvedCall {
                    target: d.clone(),
                    confidence: Confidence::Exact,
                });
            }
        }

        if !same_file_hits.is_empty() {
            resolved = same_file_hits;
        } else {
            // B. Imported files check
            let mut imported_hits = Vec::new();
            for d in &same_lang_defs {
                let matches_import = imported_specifiers.iter().any(|spec| {
                    spec.contains(&d.name) || d.file_path.contains(spec) || spec.ends_with(&d.file_path)
                });
                if matches_import {
                    imported_hits.push(ResolvedCall {
                        target: d.clone(),
                        confidence: Confidence::Likely,
                    });
                }
            }

            if !imported_hits.is_empty() {
                resolved = imported_hits;
            } else if same_lang_defs.len() == 1 {
                // Exactly one global candidate in this language
                resolved.push(ResolvedCall {
                    target: same_lang_defs[0].clone(),
                    confidence: Confidence::Likely,
                });
            } else {
                for d in same_lang_defs {
                    resolved.push(ResolvedCall {
                        target: d,
                        confidence: Confidence::Ambiguous,
                    });
                }
            }
        }
    }

    resolved
}
