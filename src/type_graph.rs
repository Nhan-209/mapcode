use serde::{Deserialize, Serialize};
use crate::store::{CodeStore, SymbolKind, SymbolRef};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct TypeGraphReport {
    pub name: String,
    pub is_trait: bool,
    pub supertypes: Vec<String>,
    pub subtypes: Vec<String>,
    pub associated_methods: Vec<SymbolRef>,
}

/// Retrieves bidirectional type hierarchy, implemented traits, base classes,
/// and associated methods for a struct, class, interface, or trait.
pub fn get_type_hierarchy(store: &CodeStore, name: &str) -> Option<TypeGraphReport> {
    let clean_name = name.trim();
    if clean_name.is_empty() {
        return None;
    }

    // 1. Fetch direct type relations for clean_name
    let mut direct_relations = store.get_type_relations(clean_name);
    let mut resolved_name = clean_name.to_string();
    let mut is_trait = false;

    // Case-insensitive fallback if direct lookup is empty
    if direct_relations.is_empty() {
        let all_types = store.get_all_type_relations();
        for tr in all_types {
            if tr.name.eq_ignore_ascii_case(clean_name) {
                resolved_name = tr.name.clone();
                direct_relations = store.get_type_relations(&resolved_name);
                break;
            }
        }
    }

    // 2. Extract supertypes (traits implemented, classes extended)
    let mut supertypes = Vec::new();
    for tr in &direct_relations {
        if tr.is_trait {
            is_trait = true;
        }
        for sup in &tr.supertypes {
            if !supertypes.contains(sup) {
                supertypes.push(sup.clone());
            }
        }
    }
    supertypes.sort();

    // 3. Extract subtypes (classes/structs that implement or extend resolved_name)
    let mut subtypes = Vec::new();
    let all_types = store.get_all_type_relations();
    for other_tr in &all_types {
        if other_tr.name.eq_ignore_ascii_case(&resolved_name) {
            continue;
        }
        let implements_or_extends = other_tr
            .supertypes
            .iter()
            .any(|s| s.eq_ignore_ascii_case(&resolved_name));

        if implements_or_extends && !subtypes.contains(&other_tr.name) {
            subtypes.push(other_tr.name.clone());
        }
    }
    subtypes.sort();

    // 4. Extract associated methods
    let mut associated_methods = Vec::new();

    // Look in definitions for methods having container_name matching resolved_name
    for (_path, syms) in store.get_all_file_symbols() {
        for sym in syms {
            let matches_container = sym
                .container_name
                .as_deref()
                .map(|c| c.eq_ignore_ascii_case(&resolved_name))
                .unwrap_or(false);

            if matches_container && (sym.kind == SymbolKind::Method || sym.kind == SymbolKind::Function) {
                let sym_ref = SymbolRef::from(&sym);
                if !associated_methods.contains(&sym_ref) {
                    associated_methods.push(sym_ref);
                }
            }
        }
    }

    // Also include method names recorded in TypeRelation.methods
    for tr in &direct_relations {
        for m_name in &tr.methods {
            if !associated_methods.iter().any(|m| &m.name == m_name) {
                let defs = store.find_definition_advanced(m_name, None, Some(&resolved_name));
                if let Some(first) = defs.into_iter().next() {
                    associated_methods.push(SymbolRef {
                        name: first.name,
                        kind: first.kind,
                        file_path: first.file_path,
                        start_line: first.start_line,
                        end_line: first.end_line,
                        signature: first.signature,
                        container_name: first.container_name,
                    });
                }
            }
        }
    }

    associated_methods.sort_by(|a, b| a.file_path.cmp(&b.file_path).then(a.start_line.cmp(&b.start_line)));

    // 5. If nothing was found in TypeRelations, check if it exists in Symbol definitions (e.g. cold struct)
    if direct_relations.is_empty() {
        let defs = store.find_definition_advanced(&resolved_name, None, None);
        let is_known_type = defs.iter().any(|d| {
            matches!(
                d.kind,
                SymbolKind::Struct
                    | SymbolKind::Class
                    | SymbolKind::Interface
                    | SymbolKind::Trait
                    | SymbolKind::Enum
                    | SymbolKind::TypeAlias
            )
        });

        if !is_known_type && associated_methods.is_empty() {
            return None;
        }

        if defs.iter().any(|d| d.kind == SymbolKind::Trait || d.kind == SymbolKind::Interface) {
            is_trait = true;
        }
    }

    Some(TypeGraphReport {
        name: resolved_name,
        is_trait,
        supertypes,
        subtypes,
        associated_methods,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::collections::HashMap;
    use crate::parser::{ParsedFileResult, Symbol, SymbolKind, TypeRelation};

    #[test]
    fn test_type_hierarchy_and_methods() {
        let store = CodeStore::new(PathBuf::from("."));

        // Define Animal trait and Dog struct implementing Animal
        store.update_file_result(
            "src/animal.rs",
            ParsedFileResult {
                symbols: vec![
                    Symbol {
                        name: "bark".to_string(),
                        kind: SymbolKind::Method,
                        file_path: "src/animal.rs".to_string(),
                        start_line: 10,
                        end_line: 15,
                        signature: "fn bark(&self)".to_string(),
                        doc: None,
                        container_name: Some("Dog".to_string()),
                        callees: vec![],
                    },
                ],
                callers: HashMap::new(),
                imports: vec![],
                types: vec![
                    TypeRelation {
                        name: "Dog".to_string(),
                        supertypes: vec!["Animal".to_string()],
                        is_trait: false,
                        methods: vec!["bark".to_string()],
                        file_path: "src/animal.rs".to_string(),
                    },
                    TypeRelation {
                        name: "Animal".to_string(),
                        supertypes: vec![],
                        is_trait: true,
                        methods: vec![],
                        file_path: "src/animal.rs".to_string(),
                    },
                ],
                entrypoints: vec![],
            },
        );

        let dog_report = get_type_hierarchy(&store, "Dog").expect("Dog should be found");
        assert_eq!(dog_report.name, "Dog");
        assert_eq!(dog_report.supertypes, vec!["Animal".to_string()]);
        assert_eq!(dog_report.associated_methods.len(), 1);
        assert_eq!(dog_report.associated_methods[0].name, "bark");

        let animal_report = get_type_hierarchy(&store, "Animal").expect("Animal should be found");
        assert_eq!(animal_report.name, "Animal");
        assert_eq!(animal_report.subtypes, vec!["Dog".to_string()]);
        assert!(animal_report.is_trait);
    }
}
