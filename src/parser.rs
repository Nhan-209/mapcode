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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
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
        }
    }
}

pub fn parse_file(
    file_path: &str,
    content: &str,
    lang: SupportedLanguage,
) -> Result<Vec<Symbol>, String> {
    let mut parser = Parser::new();
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
    }

    let tree = parser
        .parse(content, None)
        .ok_or_else(|| "Failed to parse content with tree-sitter".to_string())?;

    let mut symbols = Vec::new();
    let source_bytes = content.as_bytes();
    extract_symbols(
        tree.root_node(),
        source_bytes,
        content,
        file_path,
        lang,
        None,
        &mut symbols,
    );

    Ok(symbols)
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
    } else if let Some(stripped) = trimmed.strip_prefix("//") {
        stripped
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
    )
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
}
