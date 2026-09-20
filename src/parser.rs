use std::collections::{HashMap, HashSet};
use std::path::Path;
use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Parser};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Class,
    Interface,
    Trait,
    Enum,
    TypeAlias,
    Module,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Symbol {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// Represents an import statement extracted from source code.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ImportItem {
    pub source_path: String,
    pub specifier: String,
    pub is_external: bool,
    pub line: usize,
}

/// Represents a type definition, its hierarchy, and associated methods.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeRelation {
    pub name: String,
    pub supertypes: Vec<String>,
    pub is_trait: bool,
    pub methods: Vec<String>,
    pub file_path: String,
}

/// Represents an execution or request entrypoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entrypoint {
    pub name: String,
    pub category: String, // "startup", "http", "cli", "worker"
    pub file_path: String,
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_or_cmd: Option<String>,
}

/// Aggregated multi-dimensional AST extraction result for a single source file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedFileResult {
    pub symbols: Vec<Symbol>,
    pub callers: HashMap<String, HashSet<SymbolRef>>,
    pub imports: Vec<ImportItem>,
    pub types: Vec<TypeRelation>,
    pub entrypoints: Vec<Entrypoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
    Lua,
    Go,
    C,
    Cpp,
}

impl SupportedLanguage {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        match ext.as_str() {
            "rs" => Some(SupportedLanguage::Rust),
            "py" => Some(SupportedLanguage::Python),
            "js" | "mjs" | "cjs" | "jsx" => Some(SupportedLanguage::JavaScript),
            "ts" => Some(SupportedLanguage::TypeScript),
            "tsx" => Some(SupportedLanguage::Tsx),
            "lua" => Some(SupportedLanguage::Lua),
            "go" => Some(SupportedLanguage::Go),
            "c" | "h" => Some(SupportedLanguage::C),
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Some(SupportedLanguage::Cpp),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            SupportedLanguage::Rust => "Rust",
            SupportedLanguage::Python => "Python",
            SupportedLanguage::JavaScript => "JavaScript",
            SupportedLanguage::TypeScript => "TypeScript",
            SupportedLanguage::Tsx => "TypeScript (TSX)",
            SupportedLanguage::Lua => "Lua",
            SupportedLanguage::Go => "Go",
            SupportedLanguage::C => "C",
            SupportedLanguage::Cpp => "C++",
        }
    }
}

pub fn set_parser_language(parser: &mut Parser, lang: SupportedLanguage) -> Result<(), String> {
    match lang {
        SupportedLanguage::Rust => {
            parser
                .set_language(tree_sitter_rust::language())
                .map_err(|e| format!("Failed to set Rust language: {:?}", e))?;
        }
        SupportedLanguage::Python => {
            parser
                .set_language(tree_sitter_python::language())
                .map_err(|e| format!("Failed to set Python language: {:?}", e))?;
        }
        SupportedLanguage::JavaScript => {
            parser
                .set_language(tree_sitter_javascript::language())
                .map_err(|e| format!("Failed to set JavaScript language: {:?}", e))?;
        }
        SupportedLanguage::TypeScript => {
            parser
                .set_language(tree_sitter_typescript::language_typescript())
                .map_err(|e| format!("Failed to set TypeScript language: {:?}", e))?;
        }
        SupportedLanguage::Tsx => {
            parser
                .set_language(tree_sitter_typescript::language_tsx())
                .map_err(|e| format!("Failed to set TSX language: {:?}", e))?;
        }
        SupportedLanguage::Lua => {
            parser
                .set_language(tree_sitter_lua::language())
                .map_err(|e| format!("Failed to set Lua language: {:?}", e))?;
        }
        SupportedLanguage::Go => {
            parser
                .set_language(tree_sitter_go::language())
                .map_err(|e| format!("Failed to set Go language: {:?}", e))?;
        }
        SupportedLanguage::C => {
            parser
                .set_language(tree_sitter_c::language())
                .map_err(|e| format!("Failed to set C language: {:?}", e))?;
        }
        SupportedLanguage::Cpp => {
            parser
                .set_language(tree_sitter_cpp::language())
                .map_err(|e| format!("Failed to set C++ language: {:?}", e))?;
        }
    }
    Ok(())
}

/// Comprehensive file parser returning symbols, callers, imports, types, and entrypoints.
pub fn parse_file_result(
    file_path: &str,
    content: &str,
    lang: SupportedLanguage,
) -> Result<ParsedFileResult, String> {
    let mut parser = Parser::new();
    set_parser_language(&mut parser, lang)?;

    let tree = parser
        .parse(content, None)
        .ok_or_else(|| "Failed to parse content with tree-sitter".to_string())?;

    let root = tree.root_node();
    let source_bytes = content.as_bytes();

    let mut symbols = Vec::new();
    let mut imports = Vec::new();
    let mut types = Vec::new();
    let mut entrypoints = Vec::new();

    // 1. Extract symbols & callees
    extract_symbols(
        root,
        source_bytes,
        content,
        file_path,
        lang,
        None,
        &mut symbols,
    );

    // 2. Extract import statements
    extract_imports(
        root,
        source_bytes,
        file_path,
        lang,
        &mut imports,
    );

    // 3. Extract type relations & hierarchies
    extract_types(
        root,
        source_bytes,
        file_path,
        lang,
        &mut types,
    );

    // 4. Extract entrypoints (startup, HTTP, CLI, worker)
    extract_entrypoints(
        root,
        source_bytes,
        content,
        file_path,
        lang,
        &mut entrypoints,
    );

    // 5. Correlate associated methods with types
    correlate_type_methods(&symbols, &mut types);

    // 6. Build callers map for symbols in this file
    let mut callers: HashMap<String, HashSet<SymbolRef>> = HashMap::new();
    for sym in &symbols {
        let sym_ref = SymbolRef::from(sym);
        for callee in &sym.callees {
            callers
                .entry(callee.clone())
                .or_default()
                .insert(sym_ref.clone());
        }
    }

    Ok(ParsedFileResult {
        symbols,
        callers,
        imports,
        types,
        entrypoints,
    })
}

/// Backward-compatible wrapper delegating to parse_file_result.
pub fn parse_file(
    file_path: &str,
    content: &str,
    lang: SupportedLanguage,
) -> Result<Vec<Symbol>, String> {
    parse_file_result(file_path, content, lang).map(|r| r.symbols)
}

fn extract_symbols(
    node: Node,
    source: &[u8],
    content: &str,
    file_path: &str,
    lang: SupportedLanguage,
    container_name: Option<&str>,
    symbols: &mut Vec<Symbol>,
) {
    let kind = node.kind();

    match lang {
        SupportedLanguage::Rust => {
            match kind {
                "function_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        let mut callees = Vec::new();
                        if let Some(body) = node.child_by_field_name("body") {
                            collect_callees(body, source, lang, &mut callees);
                        }
                        callees.sort();
                        callees.dedup();

                        let symbol_kind = if container_name.is_some() {
                            SymbolKind::Method
                        } else {
                            SymbolKind::Function
                        };

                        symbols.push(Symbol {
                            name,
                            kind: symbol_kind,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: container_name.map(|s| s.to_string()),
                            callees,
                        });
                    }
                    return;
                }
                "struct_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::Struct,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: None,
                            callees: Vec::new(),
                        });
                    }
                }
                "enum_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::Enum,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: None,
                            callees: Vec::new(),
                        });
                    }
                }
                "trait_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        symbols.push(Symbol {
                            name: name.clone(),
                            kind: SymbolKind::Trait,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: None,
                            callees: Vec::new(),
                        });

                        // Recurse into trait declarations
                        if let Some(body) = node.child_by_field_name("body") {
                            extract_symbols(
                                body,
                                source,
                                content,
                                file_path,
                                lang,
                                Some(&name),
                                symbols,
                            );
                        }
                    }
                    return;
                }
                "type_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, None);
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::TypeAlias,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: None,
                            callees: Vec::new(),
                        });
                    }
                    return;
                }
                "mod_item" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        symbols.push(Symbol {
                            name: name.clone(),
                            kind: SymbolKind::Module,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: None,
                            callees: Vec::new(),
                        });

                        if let Some(body) = node.child_by_field_name("body") {
                            extract_symbols(
                                body,
                                source,
                                content,
                                file_path,
                                lang,
                                Some(&name),
                                symbols,
                            );
                        }
                    }
                    return;
                }
                "impl_item" => {
                    let impl_target = node
                        .child_by_field_name("type")
                        .map(|t| node_text(t, source).to_string());
                    if let Some(body) = node.child_by_field_name("body") {
                        extract_symbols(
                            body,
                            source,
                            content,
                            file_path,
                            lang,
                            impl_target.as_deref(),
                            symbols,
                        );
                    }
                    return;
                }
                _ => {}
            }
        }
        SupportedLanguage::Python => {
            match kind {
                "class_definition" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_python_body_doc(node, source)
                            .or_else(|| extract_leading_doc(content, node.start_position().row, lang));

                        symbols.push(Symbol {
                            name: name.clone(),
                            kind: SymbolKind::Class,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: None,
                            callees: Vec::new(),
                        });

                        if let Some(body) = node.child_by_field_name("body") {
                            extract_symbols(
                                body,
                                source,
                                content,
                                file_path,
                                lang,
                                Some(&name),
                                symbols,
                            );
                        }
                    }
                    return;
                }
                "function_definition" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_python_body_doc(node, source)
                            .or_else(|| extract_leading_doc(content, node.start_position().row, lang));

                        let mut callees = Vec::new();
                        if let Some(body) = node.child_by_field_name("body") {
                            collect_callees(body, source, lang, &mut callees);
                        }
                        callees.sort();
                        callees.dedup();

                        let symbol_kind = if container_name.is_some() {
                            SymbolKind::Method
                        } else {
                            SymbolKind::Function
                        };

                        symbols.push(Symbol {
                            name,
                            kind: symbol_kind,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: container_name.map(|s| s.to_string()),
                            callees,
                        });
                    }
                    return;
                }
                _ => {}
            }
        }
        SupportedLanguage::JavaScript | SupportedLanguage::TypeScript | SupportedLanguage::Tsx => {
            match kind {
                "class_declaration" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        symbols.push(Symbol {
                            name: name.clone(),
                            kind: SymbolKind::Class,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: None,
                            callees: Vec::new(),
                        });

                        if let Some(body) = node.child_by_field_name("body") {
                            extract_symbols(
                                body,
                                source,
                                content,
                                file_path,
                                lang,
                                Some(&name),
                                symbols,
                            );
                        }
                    }
                    return;
                }
                "interface_declaration" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        symbols.push(Symbol {
                            name: name.clone(),
                            kind: SymbolKind::Interface,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: None,
                            callees: Vec::new(),
                        });

                        if let Some(body) = node.child_by_field_name("body") {
                            extract_symbols(
                                body,
                                source,
                                content,
                                file_path,
                                lang,
                                Some(&name),
                                symbols,
                            );
                        }
                    }
                    return;
                }
                "method_signature" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = normalize_whitespace(
                            node_text(node, source)
                                .trim()
                                .trim_end_matches(';')
                                .trim(),
                        );
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::Method,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: container_name.map(|s| s.to_string()),
                            callees: Vec::new(),
                        });
                    }
                    return;
                }
                "type_alias_declaration" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = normalize_whitespace(node_text(node, source));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::TypeAlias,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: None,
                            callees: Vec::new(),
                        });
                    }
                    return;
                }
                "enum_declaration" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::Enum,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: None,
                            callees: Vec::new(),
                        });
                    }
                    return;
                }
                "function_declaration" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        let mut callees = Vec::new();
                        if let Some(body) = node.child_by_field_name("body") {
                            collect_callees(body, source, lang, &mut callees);
                        }
                        callees.sort();
                        callees.dedup();

                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::Function,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: None,
                            callees,
                        });
                    }
                    return;
                }
                "method_definition" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        let mut callees = Vec::new();
                        if let Some(body) = node.child_by_field_name("body") {
                            collect_callees(body, source, lang, &mut callees);
                        }
                        callees.sort();
                        callees.dedup();

                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::Method,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: container_name.map(|s| s.to_string()),
                            callees,
                        });
                    }
                    return;
                }
                "lexical_declaration" | "variable_declaration" => {
                    for i in 0..node.child_count() {
                        if let Some(decl) = node.child(i) {
                            if decl.kind() == "variable_declarator" {
                                if let (Some(name_node), Some(value_node)) = (
                                    decl.child_by_field_name("name"),
                                    decl.child_by_field_name("value"),
                                ) {
                                    let val_kind = value_node.kind();
                                    if val_kind == "arrow_function" || val_kind == "function_expression" {
                                        let name = node_text(name_node, source).to_string();
                                        let start_line = decl.start_position().row + 1;
                                        let end_line = decl.end_position().row + 1;

                                        let body_start = value_node.child_by_field_name("body").map(|b| b.start_byte());
                                        let sig_end = body_start.unwrap_or_else(|| value_node.start_byte());
                                        let decl_start = decl.start_byte();
                                        let sig_raw = if decl_start < sig_end && sig_end <= source.len() {
                                            std::str::from_utf8(&source[decl_start..sig_end]).unwrap_or("")
                                        } else {
                                            node_text(name_node, source)
                                        };
                                        let signature = normalize_whitespace(
                                            sig_raw.trim().trim_end_matches('{').trim_end_matches(':').trim(),
                                        );
                                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                                        let mut callees = Vec::new();
                                        if let Some(body) = value_node.child_by_field_name("body") {
                                            collect_callees(body, source, lang, &mut callees);
                                        }
                                        callees.sort();
                                        callees.dedup();

                                        symbols.push(Symbol {
                                            name,
                                            kind: SymbolKind::Function,
                                            file_path: file_path.to_string(),
                                            start_line,
                                            end_line,
                                            signature,
                                            doc,
                                            container_name: container_name.map(|s| s.to_string()),
                                            callees,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    return;
                }
                _ => {}
            }
        }
        SupportedLanguage::Lua => {
            match kind {
                "function_declaration" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let (name, container, is_method) = match name_node.kind() {
                            "identifier" => (
                                node_text(name_node, source).to_string(),
                                container_name.map(|s| s.to_string()),
                                container_name.is_some(),
                            ),
                            "dot_index_expression" => {
                                let table = name_node
                                    .child_by_field_name("table")
                                    .map(|t| node_text(t, source).to_string());
                                let field = name_node
                                    .child_by_field_name("field")
                                    .map(|f| node_text(f, source).to_string())
                                    .unwrap_or_else(|| node_text(name_node, source).to_string());
                                (field, table, true)
                            }
                            "method_index_expression" => {
                                let table = name_node
                                    .child_by_field_name("table")
                                    .map(|t| node_text(t, source).to_string());
                                let method = name_node
                                    .child_by_field_name("method")
                                    .map(|m| node_text(m, source).to_string())
                                    .unwrap_or_else(|| node_text(name_node, source).to_string());
                                (method, table, true)
                            }
                            _ => (
                                node_text(name_node, source).to_string(),
                                container_name.map(|s| s.to_string()),
                                false,
                            ),
                        };

                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        let mut callees = Vec::new();
                        if let Some(body) = node.child_by_field_name("body") {
                            collect_callees(body, source, lang, &mut callees);
                        }
                        callees.sort();
                        callees.dedup();

                        let symbol_kind = if is_method {
                            SymbolKind::Method
                        } else {
                            SymbolKind::Function
                        };

                        symbols.push(Symbol {
                            name,
                            kind: symbol_kind,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: container,
                            callees,
                        });
                    }
                    return;
                }
                "assignment_statement" | "variable_declaration" => {
                    extract_lua_assignment(node, source, content, file_path, lang, container_name, symbols);
                    return;
                }
                _ => {}
            }
        }
        SupportedLanguage::Go => {
            match kind {
                "function_declaration" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        let mut callees = Vec::new();
                        if let Some(body) = node.child_by_field_name("body") {
                            collect_callees(body, source, lang, &mut callees);
                        }
                        callees.sort();
                        callees.dedup();

                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::Function,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: None,
                            callees,
                        });
                    }
                    return;
                }
                "method_declaration" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        let container = node.child_by_field_name("receiver").and_then(|recv| {
                            for i in 0..recv.child_count() {
                                if let Some(child) = recv.child(i) {
                                    if child.kind() == "parameter_declaration" {
                                        if let Some(t) = child.child_by_field_name("type") {
                                            let raw = node_text(t, source).trim().trim_start_matches('*').trim();
                                            if !raw.is_empty() {
                                                return Some(raw.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                            None
                        });

                        let mut callees = Vec::new();
                        if let Some(body) = node.child_by_field_name("body") {
                            collect_callees(body, source, lang, &mut callees);
                        }
                        callees.sort();
                        callees.dedup();

                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::Method,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: container,
                            callees,
                        });
                    }
                    return;
                }
                "type_spec" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        let (symbol_kind, signature) = if let Some(type_node) = node.child_by_field_name("type") {
                            match type_node.kind() {
                                "struct_type" => {
                                    (SymbolKind::Struct, format!("type {} struct", name))
                                }
                                "interface_type" => {
                                    (SymbolKind::Interface, format!("type {} interface", name))
                                }
                                _ => {
                                    (SymbolKind::TypeAlias, normalize_whitespace(node_text(node, source)))
                                }
                            }
                        } else {
                            (SymbolKind::TypeAlias, normalize_whitespace(node_text(node, source)))
                        };

                        symbols.push(Symbol {
                            name: name.clone(),
                            kind: symbol_kind,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: None,
                            callees: Vec::new(),
                        });

                        if let Some(type_node) = node.child_by_field_name("type") {
                            if type_node.kind() == "interface_type" {
                                for i in 0..type_node.child_count() {
                                    if let Some(child) = type_node.child(i) {
                                        if child.kind() == "method_elem" || child.kind() == "method_spec" {
                                            if let Some(m_name) = child.child_by_field_name("name") {
                                                let m_name_str = node_text(m_name, source).to_string();
                                                let m_start = child.start_position().row + 1;
                                                let m_end = child.end_position().row + 1;
                                                let m_sig = normalize_whitespace(node_text(child, source));
                                                let m_doc = extract_leading_doc(content, child.start_position().row, lang);

                                                symbols.push(Symbol {
                                                    name: m_name_str,
                                                    kind: SymbolKind::Method,
                                                    file_path: file_path.to_string(),
                                                    start_line: m_start,
                                                    end_line: m_end,
                                                    signature: m_sig,
                                                    doc: m_doc,
                                                    container_name: Some(name.clone()),
                                                    callees: Vec::new(),
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return;
                }
                "type_alias" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = normalize_whitespace(node_text(node, source));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::TypeAlias,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: None,
                            callees: Vec::new(),
                        });
                    }
                    return;
                }
                _ => {}
            }
        }
        SupportedLanguage::C | SupportedLanguage::Cpp => {
            match kind {
                "function_definition" => {
                    if let Some(decl) = node.child_by_field_name("declarator") {
                        let (name, scope) = extract_c_declarator_name(decl, source);
                        if !name.is_empty() {
                            let container = scope.or_else(|| container_name.map(|s| s.to_string()));
                            let start_line = node.start_position().row + 1;
                            let end_line = node.end_position().row + 1;
                            let signature = extract_signature(node, source, Some("body"));
                            let doc = extract_leading_doc(content, node.start_position().row, lang);

                            let mut callees = Vec::new();
                            if let Some(body) = node.child_by_field_name("body") {
                                collect_callees(body, source, lang, &mut callees);
                            }
                            callees.sort();
                            callees.dedup();

                            let symbol_kind = if container.is_some() {
                                SymbolKind::Method
                            } else {
                                SymbolKind::Function
                            };

                            symbols.push(Symbol {
                                name,
                                kind: symbol_kind,
                                file_path: file_path.to_string(),
                                start_line,
                                end_line,
                                signature,
                                doc,
                                container_name: container,
                                callees,
                            });
                        }
                    }
                    return;
                }
                "declaration" | "field_declaration" => {
                    if let Some(decl) = node.child_by_field_name("declarator") {
                        if has_function_declarator(decl) {
                            let (name, scope) = extract_c_declarator_name(decl, source);
                            if !name.is_empty() {
                                let container = scope.or_else(|| container_name.map(|s| s.to_string()));
                                let start_line = node.start_position().row + 1;
                                let end_line = node.end_position().row + 1;
                                let signature = normalize_whitespace(node_text(node, source).trim().trim_end_matches(';').trim());
                                let doc = extract_leading_doc(content, node.start_position().row, lang);

                                let symbol_kind = if container.is_some() {
                                    SymbolKind::Method
                                } else {
                                    SymbolKind::Function
                                };

                                symbols.push(Symbol {
                                    name,
                                    kind: symbol_kind,
                                    file_path: file_path.to_string(),
                                    start_line,
                                    end_line,
                                    signature,
                                    doc,
                                    container_name: container,
                                    callees: Vec::new(),
                                });
                            }
                        }
                    }
                }
                "class_specifier" | "struct_specifier" => {
                    let is_class = kind == "class_specifier";
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        symbols.push(Symbol {
                            name: name.clone(),
                            kind: if is_class { SymbolKind::Class } else { SymbolKind::Struct },
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: container_name.map(|s| s.to_string()),
                            callees: Vec::new(),
                        });

                        if let Some(body) = node.child_by_field_name("body") {
                            extract_symbols(
                                body,
                                source,
                                content,
                                file_path,
                                lang,
                                Some(&name),
                                symbols,
                            );
                        }
                    }
                    return;
                }
                "enum_specifier" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::Enum,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: container_name.map(|s| s.to_string()),
                            callees: Vec::new(),
                        });
                    }
                    return;
                }
                "type_definition" => {
                    if let Some(decl) = node.child_by_field_name("declarator") {
                        let (name, _) = extract_c_declarator_name(decl, source);
                        if !name.is_empty() {
                            let start_line = node.start_position().row + 1;
                            let end_line = node.end_position().row + 1;
                            let signature = normalize_whitespace(node_text(node, source).trim().trim_end_matches(';').trim());
                            let doc = extract_leading_doc(content, node.start_position().row, lang);

                            symbols.push(Symbol {
                                name,
                                kind: SymbolKind::TypeAlias,
                                file_path: file_path.to_string(),
                                start_line,
                                end_line,
                                signature,
                                doc,
                                container_name: None,
                                callees: Vec::new(),
                            });
                        }
                    }
                    return;
                }
                "alias_declaration" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = normalize_whitespace(node_text(node, source).trim().trim_end_matches(';').trim());
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        symbols.push(Symbol {
                            name,
                            kind: SymbolKind::TypeAlias,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: None,
                            callees: Vec::new(),
                        });
                    }
                    return;
                }
                "namespace_definition" => {
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = node_text(name_node, source).to_string();
                        let start_line = node.start_position().row + 1;
                        let end_line = node.end_position().row + 1;
                        let signature = extract_signature(node, source, Some("body"));
                        let doc = extract_leading_doc(content, node.start_position().row, lang);

                        symbols.push(Symbol {
                            name: name.clone(),
                            kind: SymbolKind::Module,
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                            doc,
                            container_name: container_name.map(|s| s.to_string()),
                            callees: Vec::new(),
                        });

                        if let Some(body) = node.child_by_field_name("body") {
                            extract_symbols(
                                body,
                                source,
                                content,
                                file_path,
                                lang,
                                Some(&name),
                                symbols,
                            );
                        }
                    }
                    return;
                }
                _ => {}
            }
        }
    }

    // Traverse all children recursively
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            extract_symbols(child, source, content, file_path, lang, container_name, symbols);
        }
    }
}

fn collect_callees(
    node: Node,
    source: &[u8],
    lang: SupportedLanguage,
    callees: &mut Vec<String>,
) {
    let kind = node.kind();

    match lang {
        SupportedLanguage::Rust => {
            if kind == "call_expression" {
                if let Some(func) = node.child_by_field_name("function") {
                    if let Some(c) = extract_rust_callee(func, source) {
                        if !is_ignored_builtin(&c) {
                            callees.push(c);
                        }
                    }
                }
            } else if kind == "macro_invocation" {
                if let Some(mac) = node.child_by_field_name("macro") {
                    let name = node_text(mac, source);
                    if !name.is_empty() && !is_ignored_builtin(name) {
                        callees.push(format!("{}!", name));
                    }
                }
            }
        }
        SupportedLanguage::Python => {
            if kind == "call" {
                if let Some(func) = node.child_by_field_name("function") {
                    if let Some(c) = extract_python_callee(func, source) {
                        if !is_ignored_builtin(&c) {
                            callees.push(c);
                        }
                    }
                }
            }
        }
        SupportedLanguage::JavaScript | SupportedLanguage::TypeScript | SupportedLanguage::Tsx => {
            if kind == "call_expression" {
                if let Some(func) = node.child_by_field_name("function") {
                    if let Some(c) = extract_js_callee(func, source) {
                        if !is_ignored_builtin(&c) {
                            callees.push(c);
                        }
                    }
                }
            }
        }
        SupportedLanguage::Lua => {
            if kind == "function_call" {
                if let Some(func) = node.child_by_field_name("name") {
                    if let Some(c) = extract_lua_callee(func, source) {
                        if !is_ignored_builtin(&c) {
                            callees.push(c);
                        }
                    }
                }
            }
        }
        SupportedLanguage::Go => {
            if kind == "call_expression" {
                if let Some(func) = node.child_by_field_name("function") {
                    if let Some(c) = extract_go_callee(func, source) {
                        if !is_ignored_builtin(&c) {
                            callees.push(c);
                        }
                    }
                }
            }
        }
        SupportedLanguage::C | SupportedLanguage::Cpp => {
            if kind == "call_expression" {
                if let Some(func) = node.child_by_field_name("function") {
                    if let Some(c) = extract_c_callee(func, source) {
                        if !is_ignored_builtin(&c) {
                            callees.push(c);
                        }
                    }
                }
            }
        }
    }

    // Recurse into children, skipping nested function declarations
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            let child_kind = child.kind();
            let is_nested_func = match lang {
                SupportedLanguage::Rust => child_kind == "function_item",
                SupportedLanguage::Python => child_kind == "function_definition",
                SupportedLanguage::JavaScript | SupportedLanguage::TypeScript | SupportedLanguage::Tsx => {
                    child_kind == "function_declaration" || child_kind == "arrow_function"
                }
                SupportedLanguage::Lua => {
                    child_kind == "function_declaration" || child_kind == "function_definition"
                }
                SupportedLanguage::Go => {
                    child_kind == "function_declaration" || child_kind == "func_literal"
                }
                SupportedLanguage::C | SupportedLanguage::Cpp => {
                    child_kind == "function_definition" || child_kind == "lambda_expression"
                }
            };

            if !is_nested_func {
                collect_callees(child, source, lang, callees);
            }
        }
    }
}

fn extract_rust_callee(func: Node, source: &[u8]) -> Option<String> {
    match func.kind() {
        "identifier" => Some(node_text(func, source).to_string()),
        "field_expression" => {
            let field = func.child_by_field_name("field")?;
            Some(node_text(field, source).to_string())
        }
        "scoped_identifier" => {
            let name = func.child_by_field_name("name")?;
            Some(node_text(name, source).to_string())
        }
        "generic_function" => {
            let f = func.child_by_field_name("function")?;
            extract_rust_callee(f, source)
        }
        _ => None,
    }
}

fn extract_python_callee(func: Node, source: &[u8]) -> Option<String> {
    match func.kind() {
        "identifier" => Some(node_text(func, source).to_string()),
        "attribute" => {
            let attr = func.child_by_field_name("attribute")?;
            Some(node_text(attr, source).to_string())
        }
        _ => None,
    }
}

fn extract_js_callee(func: Node, source: &[u8]) -> Option<String> {
    match func.kind() {
        "identifier" => Some(node_text(func, source).to_string()),
        "member_expression" => {
            let prop = func.child_by_field_name("property")?;
            Some(node_text(prop, source).to_string())
        }
        _ => None,
    }
}

fn extract_lua_callee(func: Node, source: &[u8]) -> Option<String> {
    match func.kind() {
        "identifier" => Some(node_text(func, source).to_string()),
        "dot_index_expression" => {
            let field = func.child_by_field_name("field")?;
            Some(node_text(field, source).to_string())
        }
        "method_index_expression" => {
            let method = func.child_by_field_name("method")?;
            Some(node_text(method, source).to_string())
        }
        _ => None,
    }
}

fn extract_go_callee(func: Node, source: &[u8]) -> Option<String> {
    match func.kind() {
        "identifier" => Some(node_text(func, source).to_string()),
        "selector_expression" => {
            let field = func.child_by_field_name("field")?;
            Some(node_text(field, source).to_string())
        }
        _ => None,
    }
}

fn extract_c_callee(func: Node, source: &[u8]) -> Option<String> {
    match func.kind() {
        "identifier" | "field_identifier" => Some(node_text(func, source).to_string()),
        "field_expression" => {
            let field = func.child_by_field_name("field")?;
            Some(node_text(field, source).to_string())
        }
        "scoped_identifier" | "qualified_identifier" => {
            let name = func.child_by_field_name("name")?;
            Some(node_text(name, source).to_string())
        }
        _ => None,
    }
}

fn has_function_declarator(mut node: Node) -> bool {
    loop {
        match node.kind() {
            "function_declarator" => return true,
            "pointer_declarator" | "reference_declarator" | "parenthesized_declarator" => {
                if let Some(inner) = node.child_by_field_name("declarator") {
                    node = inner;
                } else {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

fn extract_c_declarator_name<'a>(mut node: Node<'a>, source: &'a [u8]) -> (String, Option<String>) {
    loop {
        match node.kind() {
            "function_declarator" | "pointer_declarator" | "reference_declarator" | "parenthesized_declarator" => {
                if let Some(inner) = node.child_by_field_name("declarator") {
                    node = inner;
                } else {
                    break;
                }
            }
            "qualified_identifier" => {
                let scope = node.child_by_field_name("scope").map(|s| node_text(s, source).to_string());
                let name = node.child_by_field_name("name").map(|n| node_text(n, source).to_string())
                    .unwrap_or_else(|| node_text(node, source).to_string());
                return (name, scope);
            }
            "identifier" | "field_identifier" | "type_identifier" | "destructor_name" | "operator_name" => {
                return (node_text(node, source).to_string(), None);
            }
            _ => break,
        }
    }
    (node_text(node, source).to_string(), None)
}

fn extract_lua_assignment(
    node: Node,
    source: &[u8],
    content: &str,
    file_path: &str,
    lang: SupportedLanguage,
    container_name: Option<&str>,
    symbols: &mut Vec<Symbol>,
) {
    let mut target_node = node;
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "assignment_statement" {
                target_node = child;
                break;
            }
        }
    }

    let mut var_list = None;
    let mut exp_list = None;

    for i in 0..target_node.child_count() {
        if let Some(child) = target_node.child(i) {
            match child.kind() {
                "variable_list" => var_list = Some(child),
                "expression_list" => exp_list = Some(child),
                _ => {}
            }
        }
    }

    if let (Some(vars), Some(exps)) = (var_list, exp_list) {
        let var_count = vars.named_child_count();
        let exp_count = exps.named_child_count();
        let count = var_count.min(exp_count);

        for idx in 0..count {
            if let (Some(var_node), Some(exp_node)) = (vars.named_child(idx), exps.named_child(idx)) {
                let (name, container, is_method) = match var_node.kind() {
                    "identifier" => (
                        node_text(var_node, source).to_string(),
                        container_name.map(|s| s.to_string()),
                        container_name.is_some(),
                    ),
                    "dot_index_expression" => {
                        let table = var_node.child_by_field_name("table").map(|t| node_text(t, source).to_string());
                        let field = var_node.child_by_field_name("field").map(|f| node_text(f, source).to_string())
                            .unwrap_or_else(|| node_text(var_node, source).to_string());
                        (field, table, true)
                    }
                    _ => (node_text(var_node, source).to_string(), container_name.map(|s| s.to_string()), false),
                };

                if name.is_empty() {
                    continue;
                }

                let start_line = node.start_position().row + 1;
                let end_line = node.end_position().row + 1;
                let doc = extract_leading_doc(content, node.start_position().row, lang);

                if exp_node.kind() == "function_definition" {
                    let mut callees = Vec::new();
                    if let Some(body) = exp_node.child_by_field_name("body") {
                        collect_callees(body, source, lang, &mut callees);
                    }
                    callees.sort();
                    callees.dedup();

                    let sig_end = exp_node.child_by_field_name("body")
                        .map(|b| b.start_byte())
                        .unwrap_or_else(|| exp_node.end_byte());
                    let raw_sig = if node.start_byte() < sig_end && sig_end <= source.len() {
                        std::str::from_utf8(&source[node.start_byte()..sig_end]).unwrap_or("")
                    } else {
                        node_text(var_node, source)
                    };
                    let signature = normalize_whitespace(raw_sig.trim());

                    symbols.push(Symbol {
                        name,
                        kind: if is_method { SymbolKind::Method } else { SymbolKind::Function },
                        file_path: file_path.to_string(),
                        start_line,
                        end_line,
                        signature,
                        doc,
                        container_name: container,
                        callees,
                    });
                } else if exp_node.kind() == "table_constructor" {
                    let signature = format!("{} = {{}}", node_text(var_node, source));
                    symbols.push(Symbol {
                        name: name.clone(),
                        kind: SymbolKind::Class,
                        file_path: file_path.to_string(),
                        start_line,
                        end_line,
                        signature,
                        doc,
                        container_name: container,
                        callees: Vec::new(),
                    });

                    for f_idx in 0..exp_node.named_child_count() {
                        if let Some(field_node) = exp_node.named_child(f_idx) {
                            if field_node.kind() == "field" {
                                if let (Some(f_name_node), Some(f_val_node)) = (
                                    field_node.child_by_field_name("name"),
                                    field_node.child_by_field_name("value"),
                                ) {
                                    if f_val_node.kind() == "function_definition" {
                                        let f_name = node_text(f_name_node, source).to_string();
                                        let f_start_line = field_node.start_position().row + 1;
                                        let f_end_line = field_node.end_position().row + 1;
                                        let f_doc = extract_leading_doc(content, field_node.start_position().row, lang);

                                        let mut f_callees = Vec::new();
                                        if let Some(body) = f_val_node.child_by_field_name("body") {
                                            collect_callees(body, source, lang, &mut f_callees);
                                        }
                                        f_callees.sort();
                                        f_callees.dedup();

                                        let f_sig_end = f_val_node.child_by_field_name("body")
                                            .map(|b| b.start_byte())
                                            .unwrap_or_else(|| f_val_node.end_byte());
                                        let f_raw_sig = if field_node.start_byte() < f_sig_end && f_sig_end <= source.len() {
                                            std::str::from_utf8(&source[field_node.start_byte()..f_sig_end]).unwrap_or("")
                                        } else {
                                            node_text(f_name_node, source)
                                        };

                                        symbols.push(Symbol {
                                            name: f_name,
                                            kind: SymbolKind::Method,
                                            file_path: file_path.to_string(),
                                            start_line: f_start_line,
                                            end_line: f_end_line,
                                            signature: normalize_whitespace(f_raw_sig.trim()),
                                            doc: f_doc,
                                            container_name: Some(name.clone()),
                                            callees: f_callees,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn node_text<'a>(node: Node, source: &'a [u8]) -> &'a str {
    let start = node.start_byte();
    let end = node.end_byte();
    if start <= end && end <= source.len() {
        std::str::from_utf8(&source[start..end]).unwrap_or("")
    } else {
        ""
    }
}

fn extract_signature(node: Node, source: &[u8], body_field: Option<&str>) -> String {
    let body_node = body_field.and_then(|field| node.child_by_field_name(field));
    let raw = if let Some(b) = body_node {
        let start = node.start_byte();
        let end = b.start_byte();
        if start <= end && end <= source.len() {
            std::str::from_utf8(&source[start..end]).unwrap_or("")
        } else {
            node_text(node, source)
        }
    } else {
        node_text(node, source)
    };

    normalize_whitespace(
        raw.trim()
            .trim_end_matches('{')
            .trim_end_matches(':')
            .trim(),
    )
}

fn normalize_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !in_space && !result.is_empty() {
                result.push(' ');
                in_space = true;
            }
        } else {
            result.push(c);
            in_space = false;
        }
    }
    result
}

fn extract_leading_doc(content: &str, start_row: usize, lang: SupportedLanguage) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    if start_row == 0 || start_row > lines.len() {
        return None;
    }

    let mut doc_lines = Vec::new();
    let mut current_row = start_row;

    while current_row > 0 {
        current_row -= 1;
        let line = lines[current_row].trim();
        if line.is_empty() {
            if doc_lines.is_empty() {
                continue;
            } else {
                break;
            }
        }

        // Before collecting any doc lines, skip attribute and decorator lines
        if doc_lines.is_empty() {
            let is_attr = match lang {
                SupportedLanguage::Rust => {
                    line.starts_with("#[") || line.starts_with("#![") || line == "]" || line.ends_with(']')
                }
                SupportedLanguage::Python => line.starts_with('@'),
                SupportedLanguage::JavaScript
                | SupportedLanguage::TypeScript
                | SupportedLanguage::Tsx => line.starts_with('@'),
                SupportedLanguage::C | SupportedLanguage::Cpp => {
                    line.starts_with("[[") || line.ends_with("]]") || line.starts_with("__attribute__")
                }
                SupportedLanguage::Lua | SupportedLanguage::Go => false,
            };
            if is_attr {
                continue;
            }
        }

        let is_doc = match lang {
            SupportedLanguage::Rust => {
                line.starts_with("///")
                    || line.starts_with("//!")
                    || line.starts_with("/**")
                    || line.starts_with('*')
            }
            SupportedLanguage::Python => line.starts_with('#'),
            SupportedLanguage::JavaScript
            | SupportedLanguage::TypeScript
            | SupportedLanguage::Tsx => {
                line.starts_with("/**") || line.starts_with('*') || line.starts_with("//")
            }
            SupportedLanguage::Lua => {
                line.starts_with("---")
                    || line.starts_with("--")
                    || line.starts_with('*')
            }
            SupportedLanguage::Go => {
                line.starts_with("//") || line.starts_with("/*") || line.starts_with('*')
            }
            SupportedLanguage::C | SupportedLanguage::Cpp => {
                line.starts_with("///")
                    || line.starts_with("/**")
                    || line.starts_with("//")
                    || line.starts_with("/*")
                    || line.starts_with('*')
            }
        };

        if is_doc {
            doc_lines.push(clean_comment_prefix(line));
        } else {
            break;
        }
    }

    if doc_lines.is_empty() {
        None
    } else {
        doc_lines.reverse();
        Some(doc_lines.join("\n"))
    }
}

fn clean_comment_prefix(line: &str) -> String {
    let trimmed = line.trim();
    let without_prefix = if let Some(stripped) = trimmed.strip_prefix("///") {
        stripped
    } else if let Some(stripped) = trimmed.strip_prefix("//!") {
        stripped
    } else if let Some(stripped) = trimmed.strip_prefix("/**") {
        stripped.trim_end_matches("*/")
    } else if let Some(stripped) = trimmed.strip_prefix("---") {
        stripped
    } else if let Some(stripped) = trimmed.strip_prefix("--") {
        stripped
    } else if let Some(stripped) = trimmed.strip_prefix("//") {
        stripped
    } else if let Some(stripped) = trimmed.strip_prefix("/*") {
        stripped.trim_end_matches("*/")
    } else if let Some(stripped) = trimmed.strip_prefix('#') {
        stripped
    } else if let Some(stripped) = trimmed.strip_prefix('*') {
        stripped.trim_end_matches("*/")
    } else {
        trimmed
    };
    without_prefix.trim().to_string()
}

fn extract_python_body_doc(node: Node, source: &[u8]) -> Option<String> {
    let body = node.child_by_field_name("body")?;
    for i in 0..body.child_count() {
        if let Some(child) = body.child(i) {
            let child_kind = child.kind();
            if child_kind == "comment" {
                continue;
            }
            if child_kind == "expression_statement" {
                if let Some(str_node) = child.child(0) {
                    if str_node.kind() == "string" {
                        let text = node_text(str_node, source).trim();
                        let trimmed = text.trim_start_matches(['r', 'R', 'u', 'U', 'b', 'B', 'f', 'F']);
                        let unquoted = trimmed
                            .trim_start_matches("\"\"\"")
                            .trim_end_matches("\"\"\"")
                            .trim_start_matches("'''")
                            .trim_end_matches("'''")
                            .trim();
                        return Some(unquoted.to_string());
                    }
                }
            }
            break;
        }
    }
    None
}

fn is_ignored_builtin(name: &str) -> bool {
    matches!(
        name,
        "println"
            | "println!"
            | "print"
            | "print!"
            | "eprintln"
            | "eprintln!"
            | "eprint"
            | "eprint!"
            | "format"
            | "format!"
            | "panic"
            | "panic!"
            | "vec"
            | "vec!"
            | "clone"
            | "unwrap"
            | "expect"
            | "as_ref"
            | "to_string"
            | "len"
            | "is_empty"
            | "range"
            | "int"
            | "str"
            | "dict"
            | "list"
            | "set"
            | "super"
            | "log"
            | "push"
            | "pop"
            | "map"
            | "filter"
            | "forEach"
            | "ok"
            | "err"
            | "Some"
            | "None"
            | "Ok"
            | "Err"
            // Lua builtins
            | "type"
            | "tostring"
            | "tonumber"
            | "pairs"
            | "ipairs"
            | "next"
            | "assert"
            | "error"
            | "pcall"
            | "xpcall"
            | "setmetatable"
            | "getmetatable"
            | "rawget"
            | "rawset"
            // Go builtins
            | "make"
            | "new"
            | "append"
            | "copy"
            | "delete"
            | "close"
            | "recover"
            | "real"
            | "imag"
            // C / C++ common library builtins
            | "printf"
            | "fprintf"
            | "sprintf"
            | "snprintf"
            | "scanf"
            | "malloc"
            | "calloc"
            | "realloc"
            | "free"
            | "sizeof"
            | "memset"
            | "memcpy"
            | "memmove"
    )
}

pub fn is_external_import(specifier: &str, lang: SupportedLanguage) -> bool {
    let s = specifier.trim();
    if s.is_empty() {
        return false;
    }
    match lang {
        SupportedLanguage::Rust => {
            !(s.starts_with("crate") || s.starts_with("super") || s.starts_with("self"))
        }
        SupportedLanguage::Python => {
            !s.starts_with('.')
        }
        SupportedLanguage::JavaScript
        | SupportedLanguage::TypeScript
        | SupportedLanguage::Tsx => {
            !(s.starts_with("./") || s.starts_with("../") || s.starts_with('/'))
        }
        SupportedLanguage::Go => {
            !(s.starts_with("./") || s.starts_with("../"))
        }
        SupportedLanguage::C | SupportedLanguage::Cpp => {
            !s.starts_with('"')
        }
        SupportedLanguage::Lua => {
            if s.starts_with("./") || s.starts_with("../") {
                false
            } else {
                matches!(
                    s,
                    "math" | "string" | "table" | "io" | "os" | "coroutine"
                        | "package" | "debug" | "cjson" | "socket" | "lfs"
                )
            }
        }
    }
}

fn extract_imports(
    node: Node,
    source: &[u8],
    file_path: &str,
    lang: SupportedLanguage,
    imports: &mut Vec<ImportItem>,
) {
    let kind = node.kind();
    match lang {
        SupportedLanguage::Rust => {
            if kind == "use_declaration" {
                let line = node.start_position().row + 1;
                let raw_text = node_text(node, source).trim();
                let clean_text = raw_text
                    .strip_prefix("pub ")
                    .unwrap_or(raw_text)
                    .strip_prefix("use ")
                    .unwrap_or(raw_text)
                    .trim_end_matches(';')
                    .trim();

                if let Some((prefix, sub_list)) = clean_text.split_once("::{") {
                    let sub_items = sub_list.trim_end_matches('}');
                    for item in sub_items.split(',') {
                        let item_clean = item.trim();
                        if !item_clean.is_empty() {
                            let spec = format!("{}::{}", prefix.trim(), item_clean);
                            let is_external = is_external_import(&spec, lang);
                            imports.push(ImportItem {
                                source_path: file_path.to_string(),
                                specifier: spec,
                                is_external,
                                line,
                            });
                        }
                    }
                } else {
                    let spec = if let Some((base, _)) = clean_text.split_once(" as ") {
                        base.trim().to_string()
                    } else {
                        clean_text.to_string()
                    };
                    if !spec.is_empty() {
                        let is_external = is_external_import(&spec, lang);
                        imports.push(ImportItem {
                            source_path: file_path.to_string(),
                            specifier: spec,
                            is_external,
                            line,
                        });
                    }
                }
            }
        }
        SupportedLanguage::Python => {
            if kind == "import_statement" {
                let line = node.start_position().row + 1;
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if child.kind() == "dotted_name" {
                            let spec = node_text(child, source).trim().to_string();
                            if !spec.is_empty() {
                                let is_external = is_external_import(&spec, lang);
                                imports.push(ImportItem {
                                    source_path: file_path.to_string(),
                                    specifier: spec,
                                    is_external,
                                    line,
                                });
                            }
                        } else if child.kind() == "aliased_import" {
                            if let Some(name_child) = child.child_by_field_name("name") {
                                let spec = node_text(name_child, source).trim().to_string();
                                if !spec.is_empty() {
                                    let is_external = is_external_import(&spec, lang);
                                    imports.push(ImportItem {
                                        source_path: file_path.to_string(),
                                        specifier: spec,
                                        is_external,
                                        line,
                                    });
                                }
                            }
                        }
                    }
                }
            } else if kind == "import_from_statement" {
                let line = node.start_position().row + 1;
                if let Some(module_name) = node.child_by_field_name("module_name") {
                    let spec = node_text(module_name, source).trim().to_string();
                    if !spec.is_empty() {
                        let is_external = is_external_import(&spec, lang);
                        imports.push(ImportItem {
                            source_path: file_path.to_string(),
                            specifier: spec,
                            is_external,
                            line,
                        });
                    }
                } else {
                    let raw = node_text(node, source).trim();
                    if let Some(rest) = raw.strip_prefix("from ") {
                        if let Some((mod_part, _)) = rest.split_once(" import") {
                            let spec = mod_part.trim().to_string();
                            if !spec.is_empty() {
                                let is_external = is_external_import(&spec, lang);
                                imports.push(ImportItem {
                                    source_path: file_path.to_string(),
                                    specifier: spec,
                                    is_external,
                                    line,
                                });
                            }
                        }
                    }
                }
            }
        }
        SupportedLanguage::JavaScript
        | SupportedLanguage::TypeScript
        | SupportedLanguage::Tsx => {
            if kind == "import_statement" {
                let line = node.start_position().row + 1;
                if let Some(source_node) = node.child_by_field_name("source") {
                    let raw = node_text(source_node, source).trim();
                    let spec = raw.trim_matches(['\'', '"']).to_string();
                    if !spec.is_empty() {
                        let is_external = is_external_import(&spec, lang);
                        imports.push(ImportItem {
                            source_path: file_path.to_string(),
                            specifier: spec,
                            is_external,
                            line,
                        });
                    }
                }
            } else if kind == "export_statement" {
                let line = node.start_position().row + 1;
                if let Some(source_node) = node.child_by_field_name("source") {
                    let raw = node_text(source_node, source).trim();
                    let spec = raw.trim_matches(['\'', '"']).to_string();
                    if !spec.is_empty() {
                        let is_external = is_external_import(&spec, lang);
                        imports.push(ImportItem {
                            source_path: file_path.to_string(),
                            specifier: spec,
                            is_external,
                            line,
                        });
                    }
                }
            } else if kind == "call_expression" {
                if let Some(func) = node.child_by_field_name("function") {
                    let fn_name = node_text(func, source).trim();
                    if fn_name == "require" || fn_name == "import" {
                        if let Some(args) = node.child_by_field_name("arguments") {
                            for i in 0..args.child_count() {
                                if let Some(arg) = args.child(i) {
                                    if arg.kind() == "string" {
                                        let line = node.start_position().row + 1;
                                        let raw = node_text(arg, source).trim();
                                        let spec = raw.trim_matches(['\'', '"']).to_string();
                                        if !spec.is_empty() {
                                            let is_external = is_external_import(&spec, lang);
                                            imports.push(ImportItem {
                                                source_path: file_path.to_string(),
                                                specifier: spec,
                                                is_external,
                                                line,
                                            });
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        SupportedLanguage::Go => {
            if kind == "import_spec" {
                let line = node.start_position().row + 1;
                if let Some(path_node) = node.child_by_field_name("path") {
                    let raw = node_text(path_node, source).trim();
                    let spec = raw.trim_matches('"').to_string();
                    if !spec.is_empty() {
                        let is_external = is_external_import(&spec, lang);
                        imports.push(ImportItem {
                            source_path: file_path.to_string(),
                            specifier: spec,
                            is_external,
                            line,
                        });
                    }
                }
            }
        }
        SupportedLanguage::C | SupportedLanguage::Cpp => {
            if kind == "preproc_include" {
                let line = node.start_position().row + 1;
                if let Some(path_node) = node.child_by_field_name("path") {
                    let raw = node_text(path_node, source).trim();
                    let is_sys = raw.starts_with('<');
                    let spec = raw.trim_matches(['<', '>', '"']).to_string();
                    if !spec.is_empty() {
                        imports.push(ImportItem {
                            source_path: file_path.to_string(),
                            specifier: spec,
                            is_external: is_sys,
                            line,
                        });
                    }
                }
            }
        }
        SupportedLanguage::Lua => {
            if kind == "function_call" {
                let text = node_text(node, source).trim();
                if text.starts_with("require") {
                    let line = node.start_position().row + 1;
                    if let Some(prefix) = text.strip_prefix("require") {
                        let trimmed = prefix.trim().trim_start_matches('(').trim_end_matches(')').trim();
                        let spec = trimmed.trim_matches(['"', '\'']).to_string();
                        if !spec.is_empty() {
                            let is_external = is_external_import(&spec, lang);
                            imports.push(ImportItem {
                                source_path: file_path.to_string(),
                                specifier: spec,
                                is_external,
                                line,
                            });
                        }
                    }
                }
            }
        }
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            extract_imports(child, source, file_path, lang, imports);
        }
    }
}

fn extract_types(
    node: Node,
    source: &[u8],
    file_path: &str,
    lang: SupportedLanguage,
    types: &mut Vec<TypeRelation>,
) {
    let kind = node.kind();
    match lang {
        SupportedLanguage::Rust => {
            if kind == "struct_item" || kind == "enum_item" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source).to_string();
                    let mut supertypes = Vec::new();
                    
                    // Check preceding attribute_item siblings
                    let mut cur = node.prev_named_sibling();
                    while let Some(prev) = cur {
                        if prev.kind() == "attribute_item" {
                            let attr_text = node_text(prev, source);
                            if let Some(derive_pos) = attr_text.find("derive(") {
                                let after = &attr_text[derive_pos + 7..];
                                if let Some(end_paren) = after.find(')') {
                                    let derive_list = &after[..end_paren];
                                    for d in derive_list.split(',') {
                                        let d_clean = d.trim().to_string();
                                        if !d_clean.is_empty() && !supertypes.contains(&d_clean) {
                                            supertypes.push(d_clean);
                                        }
                                    }
                                }
                            }
                        } else {
                            break;
                        }
                        cur = prev.prev_named_sibling();
                    }

                    // Check within node itself
                    let raw = node_text(node, source);
                    if let Some(derive_pos) = raw.find("derive(") {
                        let after = &raw[derive_pos + 7..];
                        if let Some(end_paren) = after.find(')') {
                            let derive_list = &after[..end_paren];
                            for d in derive_list.split(',') {
                                let d_clean = d.trim().to_string();
                                if !d_clean.is_empty() && !supertypes.contains(&d_clean) {
                                    supertypes.push(d_clean);
                                }
                            }
                        }
                    }
                    types.push(TypeRelation {
                        name,
                        supertypes,
                        is_trait: false,
                        methods: Vec::new(),
                        file_path: file_path.to_string(),
                    });
                }
            } else if kind == "trait_item" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source).to_string();
                    let mut supertypes = Vec::new();
                    if let Some(bounds) = node.child_by_field_name("bounds") {
                        let text = node_text(bounds, source);
                        for b in text.split('+') {
                            let b_clean = b.trim().trim_start_matches(':').trim().to_string();
                            if !b_clean.is_empty() {
                                supertypes.push(b_clean);
                            }
                        }
                    }
                    let mut methods = Vec::new();
                    if let Some(body) = node.child_by_field_name("body") {
                        for i in 0..body.child_count() {
                            if let Some(child) = body.child(i) {
                                if child.kind() == "function_item" || child.kind() == "function_signature_item" {
                                    if let Some(fn_name) = child.child_by_field_name("name") {
                                        methods.push(node_text(fn_name, source).to_string());
                                    }
                                }
                            }
                        }
                    }
                    types.push(TypeRelation {
                        name,
                        supertypes,
                        is_trait: true,
                        methods,
                        file_path: file_path.to_string(),
                    });
                }
            } else if kind == "impl_item" {
                let target_type = node.child_by_field_name("type").map(|n| node_text(n, source).to_string());
                let trait_opt = node.child_by_field_name("trait").map(|n| node_text(n, source).to_string());
                if let Some(struct_name) = target_type {
                    let mut methods = Vec::new();
                    if let Some(body) = node.child_by_field_name("body") {
                        for i in 0..body.child_count() {
                            if let Some(child) = body.child(i) {
                                if child.kind() == "function_item" {
                                    if let Some(fn_name) = child.child_by_field_name("name") {
                                        methods.push(node_text(fn_name, source).to_string());
                                    }
                                }
                            }
                        }
                    }
                    let supertypes = if let Some(tr) = trait_opt {
                        vec![tr]
                    } else {
                        Vec::new()
                    };
                    types.push(TypeRelation {
                        name: struct_name,
                        supertypes,
                        is_trait: false,
                        methods,
                        file_path: file_path.to_string(),
                    });
                }
            }
        }
        SupportedLanguage::Python => {
            if kind == "class_definition" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source).to_string();
                    let mut supertypes = Vec::new();
                    if let Some(superclasses) = node.child_by_field_name("superclasses") {
                        for i in 0..superclasses.child_count() {
                            if let Some(child) = superclasses.child(i) {
                                let ck = child.kind();
                                if ck == "identifier" || ck == "attribute" {
                                    let super_name = node_text(child, source).trim().to_string();
                                    if !super_name.is_empty() {
                                        supertypes.push(super_name);
                                    }
                                }
                            }
                        }
                    }
                    let is_trait = supertypes.iter().any(|s| {
                        s == "ABC" || s == "abc.ABC" || s == "Protocol" || s == "typing.Protocol"
                    });
                    let mut methods = Vec::new();
                    if let Some(body) = node.child_by_field_name("body") {
                        for i in 0..body.child_count() {
                            if let Some(child) = body.child(i) {
                                if child.kind() == "function_definition" {
                                    if let Some(fn_name) = child.child_by_field_name("name") {
                                        methods.push(node_text(fn_name, source).to_string());
                                    }
                                }
                            }
                        }
                    }
                    types.push(TypeRelation {
                        name,
                        supertypes,
                        is_trait,
                        methods,
                        file_path: file_path.to_string(),
                    });
                }
            }
        }
        SupportedLanguage::JavaScript
        | SupportedLanguage::TypeScript
        | SupportedLanguage::Tsx => {
            if kind == "class_declaration" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source).to_string();
                    let mut supertypes = Vec::new();
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            if child.kind() == "class_heritage" {
                                for j in 0..child.child_count() {
                                    if let Some(clause) = child.child(j) {
                                        if clause.kind() == "extends_clause" || clause.kind() == "implements_clause" {
                                            for k in 0..clause.child_count() {
                                                if let Some(item) = clause.child(k) {
                                                    if item.kind() == "identifier" || item.kind() == "type_identifier" {
                                                        let sname = node_text(item, source).trim().to_string();
                                                        if !sname.is_empty() {
                                                            supertypes.push(sname);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let mut methods = Vec::new();
                    if let Some(body) = node.child_by_field_name("body") {
                        for i in 0..body.child_count() {
                            if let Some(child) = body.child(i) {
                                if child.kind() == "method_definition" {
                                    if let Some(fn_name) = child.child_by_field_name("name") {
                                        methods.push(node_text(fn_name, source).to_string());
                                    }
                                }
                            }
                        }
                    }
                    types.push(TypeRelation {
                        name,
                        supertypes,
                        is_trait: false,
                        methods,
                        file_path: file_path.to_string(),
                    });
                }
            } else if kind == "interface_declaration" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source).to_string();
                    let mut supertypes = Vec::new();
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            if child.kind() == "extends_type_clause" || child.kind() == "extends_clause" {
                                for j in 0..child.child_count() {
                                    if let Some(item) = child.child(j) {
                                        if item.kind() == "identifier" || item.kind() == "type_identifier" {
                                            let sname = node_text(item, source).trim().to_string();
                                            if !sname.is_empty() {
                                                supertypes.push(sname);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let mut methods = Vec::new();
                    if let Some(body) = node.child_by_field_name("body") {
                        for i in 0..body.child_count() {
                            if let Some(child) = body.child(i) {
                                if child.kind() == "method_signature" || child.kind() == "property_signature" {
                                    if let Some(fn_name) = child.child_by_field_name("name") {
                                        methods.push(node_text(fn_name, source).to_string());
                                    }
                                }
                            }
                        }
                    }
                    types.push(TypeRelation {
                        name,
                        supertypes,
                        is_trait: true,
                        methods,
                        file_path: file_path.to_string(),
                    });
                }
            }
        }
        SupportedLanguage::Go => {
            if kind == "type_spec" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source).to_string();
                    if let Some(type_node) = node.child_by_field_name("type") {
                        if type_node.kind() == "struct_type" {
                            let mut supertypes = Vec::new();
                            for i in 0..type_node.child_count() {
                                if let Some(c) = type_node.child(i) {
                                    let mut field_nodes = Vec::new();
                                    if c.kind() == "field_declaration_list" {
                                        for j in 0..c.child_count() {
                                            if let Some(f) = c.child(j) {
                                                if f.kind() == "field_declaration" {
                                                    field_nodes.push(f);
                                                }
                                            }
                                        }
                                    } else if c.kind() == "field_declaration" {
                                        field_nodes.push(c);
                                    }

                                    for field in field_nodes {
                                        if field.child_by_field_name("name").is_none() {
                                            let text = node_text(field, source).trim().trim_start_matches('*').trim();
                                            let id = text.split_whitespace().next().unwrap_or(text);
                                            if !id.is_empty() && !supertypes.contains(&id.to_string()) {
                                                supertypes.push(id.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                            types.push(TypeRelation {
                                name,
                                supertypes,
                                is_trait: false,
                                methods: Vec::new(),
                                file_path: file_path.to_string(),
                            });
                        } else if type_node.kind() == "interface_type" {
                            let mut supertypes = Vec::new();
                            let mut methods = Vec::new();
                            for i in 0..type_node.child_count() {
                                if let Some(child) = type_node.child(i) {
                                    if child.kind() == "type_identifier" {
                                        let iname = node_text(child, source).trim().to_string();
                                        if !iname.is_empty() {
                                            supertypes.push(iname);
                                        }
                                    } else if child.kind() == "method_spec" || child.kind() == "method_elem" {
                                        if let Some(m_name) = child.child_by_field_name("name") {
                                            methods.push(node_text(m_name, source).to_string());
                                        }
                                    }
                                }
                            }
                            types.push(TypeRelation {
                                name,
                                supertypes,
                                is_trait: true,
                                methods,
                                file_path: file_path.to_string(),
                            });
                        }
                    }
                }
            }
        }
        SupportedLanguage::C | SupportedLanguage::Cpp => {
            if kind == "class_specifier" || kind == "struct_specifier" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source).to_string();
                    let mut supertypes = Vec::new();
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            if child.kind() == "base_class_clause" {
                                let text = node_text(child, source);
                                let trimmed = text.trim_start_matches(':').trim();
                                for part in trimmed.split(',') {
                                    let clean = part
                                        .replace("public", "")
                                        .replace("protected", "")
                                        .replace("private", "")
                                        .replace("virtual", "");
                                    let s_clean = clean.trim().to_string();
                                    if !s_clean.is_empty() && !supertypes.contains(&s_clean) {
                                        supertypes.push(s_clean);
                                    }
                                }
                            }
                        }
                    }
                    let mut methods = Vec::new();
                    if let Some(body) = node.child_by_field_name("body") {
                        for i in 0..body.child_count() {
                            if let Some(child) = body.child(i) {
                                if child.kind() == "function_definition" || child.kind() == "declaration" {
                                    if let Some(decl) = child.child_by_field_name("declarator") {
                                        let decl_text = node_text(decl, source);
                                        let fn_name = decl_text.split('(').next().unwrap_or("").trim().to_string();
                                        if !fn_name.is_empty() {
                                            methods.push(fn_name);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    types.push(TypeRelation {
                        name,
                        supertypes,
                        is_trait: false,
                        methods,
                        file_path: file_path.to_string(),
                    });
                }
            }
        }
        SupportedLanguage::Lua => {
            if kind == "variable_declaration" || kind == "assignment_statement" {
                let text = node_text(node, source).trim();
                if text.contains("= {}") || text.contains("={}") {
                    let name = text.split('=').next().unwrap_or("").replace("local", "").trim().to_string();
                    if !name.is_empty() && !name.contains('.') {
                        types.push(TypeRelation {
                            name,
                            supertypes: Vec::new(),
                            is_trait: false,
                            methods: Vec::new(),
                            file_path: file_path.to_string(),
                        });
                    }
                }
            }
        }
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            extract_types(child, source, file_path, lang, types);
        }
    }
}

fn extract_entrypoints(
    node: Node,
    source: &[u8],
    content: &str,
    file_path: &str,
    lang: SupportedLanguage,
    entrypoints: &mut Vec<Entrypoint>,
) {
    let kind = node.kind();
    let line = node.start_position().row + 1;

    match lang {
        SupportedLanguage::Rust => {
            if kind == "function_item" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source).to_string();
                    if name == "main" {
                        entrypoints.push(Entrypoint {
                            name: name.clone(),
                            category: "startup".to_string(),
                            file_path: file_path.to_string(),
                            line,
                            route_or_cmd: None,
                        });
                    }

                    let lines: Vec<&str> = content.lines().collect();
                    let start_row = node.start_position().row;
                    for r in (start_row.saturating_sub(5)..start_row).rev() {
                        let l = lines.get(r).map(|s| s.trim()).unwrap_or("");
                        if l.starts_with("#[get(")
                            || l.starts_with("#[post(")
                            || l.starts_with("#[put(")
                            || l.starts_with("#[delete(")
                            || l.starts_with("#[route(")
                        {
                            let method = if l.starts_with("#[get") {
                                "GET"
                            } else if l.starts_with("#[post") {
                                "POST"
                            } else if l.starts_with("#[put") {
                                "PUT"
                            } else if l.starts_with("#[delete") {
                                "DELETE"
                            } else {
                                "ROUTE"
                            };
                            let route_path = l.split('"').nth(1).unwrap_or("/");
                            entrypoints.push(Entrypoint {
                                name: name.clone(),
                                category: "http".to_string(),
                                file_path: file_path.to_string(),
                                line,
                                route_or_cmd: Some(format!("{} {}", method, route_path)),
                            });
                            break;
                        }
                    }

                    if name.to_lowercase().contains("worker") || name.to_lowercase().contains("consumer") {
                        entrypoints.push(Entrypoint {
                            name: name.clone(),
                            category: "worker".to_string(),
                            file_path: file_path.to_string(),
                            line,
                            route_or_cmd: None,
                        });
                    }
                }
            }
        }
        SupportedLanguage::Python => {
            if kind == "if_statement" {
                let text = node_text(node, source);
                if text.contains("__name__") && text.contains("__main__") {
                    entrypoints.push(Entrypoint {
                        name: "__main__".to_string(),
                        category: "startup".to_string(),
                        file_path: file_path.to_string(),
                        line,
                        route_or_cmd: Some("__main__".to_string()),
                    });
                }
            } else if kind == "function_definition" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source).to_string();
                    if name == "main" {
                        entrypoints.push(Entrypoint {
                            name: name.clone(),
                            category: "startup".to_string(),
                            file_path: file_path.to_string(),
                            line,
                            route_or_cmd: None,
                        });
                    }

                    let lines: Vec<&str> = content.lines().collect();
                    let start_row = node.start_position().row;
                    for r in (start_row.saturating_sub(6)..start_row).rev() {
                        let l = lines.get(r).map(|s| s.trim()).unwrap_or("");
                        if l.starts_with('@') {
                            if l.contains(".get(")
                                || l.contains(".post(")
                                || l.contains(".put(")
                                || l.contains(".delete(")
                                || l.contains(".route(")
                            {
                                let method = if l.contains(".get") {
                                    "GET"
                                } else if l.contains(".post") {
                                    "POST"
                                } else if l.contains(".put") {
                                    "PUT"
                                } else if l.contains(".delete") {
                                    "DELETE"
                                } else {
                                    "ROUTE"
                                };
                                let route = l.split(['\'', '"']).nth(1).unwrap_or("/");
                                entrypoints.push(Entrypoint {
                                    name: name.clone(),
                                    category: "http".to_string(),
                                    file_path: file_path.to_string(),
                                    line,
                                    route_or_cmd: Some(format!("{} {}", method, route)),
                                });
                                break;
                            } else if l.contains("click.command") || l.contains(".command(") {
                                entrypoints.push(Entrypoint {
                                    name: name.clone(),
                                    category: "cli".to_string(),
                                    file_path: file_path.to_string(),
                                    line,
                                    route_or_cmd: Some(name.clone()),
                                });
                                break;
                            } else if l.contains(".task") || l.contains("shared_task") {
                                entrypoints.push(Entrypoint {
                                    name: name.clone(),
                                    category: "worker".to_string(),
                                    file_path: file_path.to_string(),
                                    line,
                                    route_or_cmd: None,
                                });
                                break;
                            }
                        } else if !l.is_empty() {
                            break;
                        }
                    }
                }
            }
        }
        SupportedLanguage::JavaScript
        | SupportedLanguage::TypeScript
        | SupportedLanguage::Tsx => {
            if kind == "function_declaration" || kind == "method_definition" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source).to_string();
                    if name == "main" || name == "bootstrap" || name == "startServer" {
                        entrypoints.push(Entrypoint {
                            name,
                            category: "startup".to_string(),
                            file_path: file_path.to_string(),
                            line,
                            route_or_cmd: None,
                        });
                    }
                }
            } else if kind == "call_expression" {
                let text = node_text(node, source);
                if text.starts_with("app.get(")
                    || text.starts_with("app.post(")
                    || text.starts_with("router.get(")
                    || text.starts_with("router.post(")
                    || text.starts_with("fastify.get(")
                {
                    let method = if text.contains(".get(") { "GET" } else { "POST" };
                    let route = text.split(['\'', '"']).nth(1).unwrap_or("/");
                    entrypoints.push(Entrypoint {
                        name: format!("{}_{}", method.to_lowercase(), route.replace('/', "_")),
                        category: "http".to_string(),
                        file_path: file_path.to_string(),
                        line,
                        route_or_cmd: Some(format!("{} {}", method, route)),
                    });
                } else if text.starts_with("app.listen(") || text.starts_with("server.listen(") {
                    entrypoints.push(Entrypoint {
                        name: "listen".to_string(),
                        category: "startup".to_string(),
                        file_path: file_path.to_string(),
                        line,
                        route_or_cmd: None,
                    });
                } else if text.contains(".command(") {
                    let cmd = text.split(['\'', '"']).nth(1).unwrap_or("command");
                    entrypoints.push(Entrypoint {
                        name: cmd.to_string(),
                        category: "cli".to_string(),
                        file_path: file_path.to_string(),
                        line,
                        route_or_cmd: Some(cmd.to_string()),
                    });
                }
            }
        }
        SupportedLanguage::Go => {
            if kind == "function_declaration" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source).to_string();
                    if name == "main" || name == "init" {
                        entrypoints.push(Entrypoint {
                            name,
                            category: "startup".to_string(),
                            file_path: file_path.to_string(),
                            line,
                            route_or_cmd: None,
                        });
                    }
                }
            } else if kind == "call_expression" {
                let text = node_text(node, source);
                if text.contains(".GET(") || text.contains(".POST(") || text.contains(".HandleFunc(") {
                    let method = if text.contains(".GET(") {
                        "GET"
                    } else if text.contains(".POST(") {
                        "POST"
                    } else {
                        "HTTP"
                    };
                    let route = text.split('"').nth(1).unwrap_or("/");
                    entrypoints.push(Entrypoint {
                        name: format!("{}_{}", method.to_lowercase(), route.replace('/', "_")),
                        category: "http".to_string(),
                        file_path: file_path.to_string(),
                        line,
                        route_or_cmd: Some(format!("{} {}", method, route)),
                    });
                }
            }
        }
        SupportedLanguage::C | SupportedLanguage::Cpp => {
            if kind == "function_definition" {
                if let Some(decl) = node.child_by_field_name("declarator") {
                    let decl_text = node_text(decl, source);
                    let name = decl_text.split('(').next().unwrap_or("").trim();
                    if name == "main" {
                        entrypoints.push(Entrypoint {
                            name: "main".to_string(),
                            category: "startup".to_string(),
                            file_path: file_path.to_string(),
                            line,
                            route_or_cmd: None,
                        });
                    }
                }
            }
        }
        SupportedLanguage::Lua => {
            if kind == "function_declaration" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source).to_string();
                    if name == "main" {
                        entrypoints.push(Entrypoint {
                            name,
                            category: "startup".to_string(),
                            file_path: file_path.to_string(),
                            line,
                            route_or_cmd: None,
                        });
                    }
                }
            }
        }
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            extract_entrypoints(child, source, content, file_path, lang, entrypoints);
        }
    }
}

pub fn correlate_type_methods(symbols: &[Symbol], types: &mut [TypeRelation]) {
    let mut methods_by_container: HashMap<String, Vec<String>> = HashMap::new();

    for sym in symbols {
        if sym.kind == SymbolKind::Method || sym.kind == SymbolKind::Function {
            if let Some(container) = &sym.container_name {
                methods_by_container
                    .entry(container.clone())
                    .or_default()
                    .push(sym.name.clone());
            }
        }
    }

    for tr in types.iter_mut() {
        if let Some(extra_methods) = methods_by_container.get(&tr.name) {
            for m in extra_methods {
                if !tr.methods.contains(m) {
                    tr.methods.push(m.clone());
                }
            }
        }
        tr.methods.sort();
        tr.methods.dedup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_parser() {
        let code = r#"
/// Adds two numbers together.
#[inline]
pub fn add(a: i32, b: i32) -> i32 {
    validate_input(a);
    a + b
}

type Number = i32;

struct Point {
    x: f64,
    y: f64,
}

impl Point {
    pub fn distance(&self) -> f64 {
        sqrt(self.x * self.x + self.y * self.y)
    }
}
"#;
        let symbols = parse_file("src/test.rs", code, SupportedLanguage::Rust).unwrap();
        assert!(symbols.iter().any(|s| s.name == "add" && s.callees.contains(&"validate_input".to_string())));
        assert!(symbols.iter().any(|s| s.name == "add" && s.doc.as_deref() == Some("Adds two numbers together.")));
        assert!(symbols.iter().any(|s| s.name == "Number" && s.kind == SymbolKind::TypeAlias));
        assert!(symbols.iter().any(|s| s.name == "Point" && s.kind == SymbolKind::Struct));
        assert!(symbols.iter().any(|s| s.name == "distance" && s.kind == SymbolKind::Method && s.container_name.as_deref() == Some("Point")));
    }

    #[test]
    fn test_python_parser() {
        let code = r#"
class Calculator:
    """A simple calculator class."""
    @staticmethod
    def calculate(x, y):
        self.prepare()
        return do_math(x, y)

def helper():
    pass
"#;
        let symbols = parse_file("test.py", code, SupportedLanguage::Python).unwrap();
        assert!(symbols.iter().any(|s| s.name == "Calculator" && s.kind == SymbolKind::Class));
        assert!(symbols.iter().any(|s| s.name == "Calculator" && s.doc.as_deref() == Some("A simple calculator class.")));
        assert!(symbols.iter().any(|s| s.name == "calculate" && s.kind == SymbolKind::Method && s.container_name.as_deref() == Some("Calculator")));
        let calc_sym = symbols.iter().find(|s| s.name == "calculate").unwrap();
        assert!(calc_sym.callees.contains(&"prepare".to_string()) || calc_sym.callees.contains(&"do_math".to_string()));
    }

    #[test]
    fn test_typescript_parser() {
        let code = r#"
interface User {
    id: string;
    name: string;
    getId(): string;
}

export const add = (a: number, b: number): number => {
    return a + b;
};

export function getUser(id: string): User {
    validateId(id);
    return fetchUser(id);
}
"#;
        let symbols = parse_file("test.ts", code, SupportedLanguage::TypeScript).unwrap();
        assert!(symbols.iter().any(|s| s.name == "User" && s.kind == SymbolKind::Interface));
        assert!(symbols.iter().any(|s| s.name == "getId" && s.kind == SymbolKind::Method && s.container_name.as_deref() == Some("User")));
        assert!(symbols.iter().any(|s| s.name == "add" && s.kind == SymbolKind::Function && s.signature.contains("=>") && !s.signature.contains("return")));
        assert!(symbols.iter().any(|s| s.name == "getUser" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_lua_parser() {
        let code = r#"
--- Adds two numbers together.
function add(a, b)
    validate_num(a)
    return a + b
end

local Player = {}

--- Inflicts damage to the player
function Player:take_damage(amount)
    self:check_alive()
    self.hp = self.hp - amount
end

local helper = function(msg)
    log_debug(msg)
end
"#;
        let symbols = parse_file("test.lua", code, SupportedLanguage::Lua).unwrap();
        assert!(symbols.iter().any(|s| s.name == "add" && s.kind == SymbolKind::Function && s.callees.contains(&"validate_num".to_string())));
        assert!(symbols.iter().any(|s| s.name == "add" && s.doc.as_deref() == Some("Adds two numbers together.")));
        assert!(symbols.iter().any(|s| s.name == "Player" && s.kind == SymbolKind::Class));
        assert!(symbols.iter().any(|s| s.name == "take_damage" && s.kind == SymbolKind::Method && s.container_name.as_deref() == Some("Player")));
        assert!(symbols.iter().any(|s| s.name == "take_damage" && s.callees.contains(&"check_alive".to_string())));
        assert!(symbols.iter().any(|s| s.name == "helper" && s.kind == SymbolKind::Function && s.callees.contains(&"log_debug".to_string())));
    }

    #[test]
    fn test_go_parser() {
        let code = r#"
package main

// Server represents the API server.
type Server struct {
    port int
}

// Handler handles incoming requests.
type Handler interface {
    Handle()
}

// Start boots up the server.
func (s *Server) Start() error {
    validate_port(s.port)
    return nil
}

// Utility function
func Calculate(x int) int {
    compute_hash(x)
    return x * 2
}
"#;
        let symbols = parse_file("main.go", code, SupportedLanguage::Go).unwrap();
        assert!(symbols.iter().any(|s| s.name == "Server" && s.kind == SymbolKind::Struct && s.doc.as_deref() == Some("Server represents the API server.")));
        assert!(symbols.iter().any(|s| s.name == "Handler" && s.kind == SymbolKind::Interface));
        assert!(symbols.iter().any(|s| s.name == "Start" && s.kind == SymbolKind::Method && s.container_name.as_deref() == Some("Server")));
        assert!(symbols.iter().any(|s| s.name == "Start" && s.callees.contains(&"validate_port".to_string())));
        assert!(symbols.iter().any(|s| s.name == "Calculate" && s.kind == SymbolKind::Function && s.callees.contains(&"compute_hash".to_string())));
    }

    #[test]
    fn test_c_parser() {
        let code = r#"
/// 2D Point structure
struct Point {
    int x;
    int y;
};

/// Computes the sum of two integers
int add(int a, int b) {
    validate_arg(a);
    return a + b;
}
"#;
        let symbols = parse_file("point.c", code, SupportedLanguage::C).unwrap();
        assert!(symbols.iter().any(|s| s.name == "Point" && s.kind == SymbolKind::Struct));
        assert!(symbols.iter().any(|s| s.name == "add" && s.kind == SymbolKind::Function && s.callees.contains(&"validate_arg".to_string())));
        assert!(symbols.iter().any(|s| s.name == "add" && s.doc.as_deref() == Some("Computes the sum of two integers")));
    }

    #[test]
    fn test_cpp_parser() {
        let code = r#"
/// Calculator class
class Calculator {
public:
    int calculate(int x) {
        verify(x);
        return x * 2;
    }
};

int Calculator::multiply(int a, int b) {
    check_overflow(a);
    return a * b;
}
"#;
        let symbols = parse_file("calc.cpp", code, SupportedLanguage::Cpp).unwrap();
        assert!(symbols.iter().any(|s| s.name == "Calculator" && s.kind == SymbolKind::Class));
        assert!(symbols.iter().any(|s| s.name == "calculate" && s.kind == SymbolKind::Method && s.container_name.as_deref() == Some("Calculator")));
        assert!(symbols.iter().any(|s| s.name == "calculate" && s.callees.contains(&"verify".to_string())));
        assert!(symbols.iter().any(|s| s.name == "multiply" && s.kind == SymbolKind::Method && s.container_name.as_deref() == Some("Calculator")));
        assert!(symbols.iter().any(|s| s.name == "multiply" && s.callees.contains(&"check_overflow".to_string())));
    }

    #[test]
    fn test_rust_ast_extensions() {
        let code = r#"
use std::collections::HashMap;
pub use crate::parser::Symbol;

#[derive(Clone, Debug)]
struct Point {
    x: f64,
    y: f64,
}

trait Formattable {
    fn format(&self) -> String;
}

impl Formattable for Point {
    fn format(&self) -> String {
        format!("({}, {})", self.x, self.y)
    }
}

#[get("/api/v1/ping")]
pub fn ping() -> &'static str {
    "pong"
}

fn main() {
    let p = Point { x: 1.0, y: 2.0 };
}
"#;
        let result = parse_file_result("src/main.rs", code, SupportedLanguage::Rust).unwrap();
        assert_eq!(result.imports.len(), 2);
        assert!(result.imports.iter().any(|i| i.specifier == "std::collections::HashMap" && i.is_external));
        assert!(result.imports.iter().any(|i| i.specifier == "crate::parser::Symbol" && !i.is_external));

        assert!(result.types.iter().any(|t| t.name == "Point" && t.supertypes.contains(&"Clone".to_string())));
        assert!(result.types.iter().any(|t| t.name == "Point" && t.supertypes.contains(&"Formattable".to_string())));
        assert!(result.types.iter().any(|t| t.name == "Formattable" && t.is_trait));

        assert!(result.entrypoints.iter().any(|e| e.name == "main" && e.category == "startup"));
        assert!(result.entrypoints.iter().any(|e| e.name == "ping" && e.category == "http" && e.route_or_cmd == Some("GET /api/v1/ping".to_string())));
    }

    #[test]
    fn test_python_ast_extensions() {
        let code = r#"
import os
from .utils import helper

class BaseService:
    def execute(self):
        pass

class UserService(BaseService):
    def get_users(self):
        return []

@app.get("/users")
def list_users():
    return []

if __name__ == '__main__':
    print("running")
"#;
        let result = parse_file_result("app.py", code, SupportedLanguage::Python).unwrap();
        assert!(result.imports.iter().any(|i| i.specifier == "os" && i.is_external));
        assert!(result.imports.iter().any(|i| i.specifier == ".utils" && !i.is_external));

        assert!(result.types.iter().any(|t| t.name == "UserService" && t.supertypes.contains(&"BaseService".to_string())));
        assert!(result.entrypoints.iter().any(|e| e.name == "__main__" && e.category == "startup"));
        assert!(result.entrypoints.iter().any(|e| e.name == "list_users" && e.category == "http" && e.route_or_cmd == Some("GET /users".to_string())));
    }

    #[test]
    fn test_typescript_ast_extensions() {
        let code = r#"
import { helper } from './utils';
import React from 'react';

interface Animal {
    makeSound(): void;
}

class Dog implements Animal {
    makeSound(): void {
        console.log("bark");
    }
}

app.get("/health", (req, res) => {
    res.send("ok");
});

function main() {
    const d = new Dog();
}
"#;
        let result = parse_file_result("index.ts", code, SupportedLanguage::TypeScript).unwrap();
        assert!(result.imports.iter().any(|i| i.specifier == "./utils" && !i.is_external));
        assert!(result.imports.iter().any(|i| i.specifier == "react" && i.is_external));

        assert!(result.types.iter().any(|t| t.name == "Animal" && t.is_trait));
        assert!(result.types.iter().any(|t| t.name == "Dog" && t.supertypes.contains(&"Animal".to_string())));
        assert!(result.entrypoints.iter().any(|e| e.category == "http" && e.route_or_cmd == Some("GET /health".to_string())));
        assert!(result.entrypoints.iter().any(|e| e.name == "main" && e.category == "startup"));
    }

    #[test]
    fn test_go_ast_extensions() {
        let code = r#"
package main

import (
    "fmt"
    "./local"
)

type Config struct {
    Port int
}

type Server struct {
    Config
}

func (s *Server) Start() {
    fmt.Println("start")
}

func main() {
    s := Server{}
    s.Start()
}
"#;
        let result = parse_file_result("main.go", code, SupportedLanguage::Go).unwrap();
        assert!(result.imports.iter().any(|i| i.specifier == "fmt" && i.is_external));
        assert!(result.imports.iter().any(|i| i.specifier == "./local" && !i.is_external));

        assert!(result.types.iter().any(|t| t.name == "Server" && t.supertypes.contains(&"Config".to_string())));
        assert!(result.types.iter().any(|t| t.name == "Server" && t.methods.contains(&"Start".to_string())));
        assert!(result.entrypoints.iter().any(|e| e.name == "main" && e.category == "startup"));
    }

    #[test]
    fn test_c_cpp_ast_extensions() {
        let code = r#"
#include <stdio.h>
#include "myheader.h"

class Base {
public:
    virtual void run() {}
};

class Derived : public Base {
public:
    void run() override {}
};

int main() {
    return 0;
}
"#;
        let result = parse_file_result("main.cpp", code, SupportedLanguage::Cpp).unwrap();
        assert!(result.imports.iter().any(|i| i.specifier == "stdio.h" && i.is_external));
        assert!(result.imports.iter().any(|i| i.specifier == "myheader.h" && !i.is_external));

        assert!(result.types.iter().any(|t| t.name == "Derived" && t.supertypes.contains(&"Base".to_string())));
        assert!(result.entrypoints.iter().any(|e| e.name == "main" && e.category == "startup"));
    }

    #[test]
    fn test_lua_ast_extensions() {
        let code = r#"
local cjson = require("cjson")
local utils = require("./utils")

local Player = {}

function Player:take_damage(amount)
    self.hp = self.hp - amount
end

function main()
    local p = Player
end
"#;
        let result = parse_file_result("main.lua", code, SupportedLanguage::Lua).unwrap();
        assert!(result.imports.iter().any(|i| i.specifier == "cjson" && i.is_external));
        assert!(result.imports.iter().any(|i| i.specifier == "./utils" && !i.is_external));

        assert!(result.types.iter().any(|t| t.name == "Player" && t.methods.contains(&"take_damage".to_string())));
        assert!(result.entrypoints.iter().any(|e| e.name == "main" && e.category == "startup"));
    }
}
