use crate::store::{CallGraphResult, FileOutline, Symbol};

pub fn format_outline_concise(outline: &FileOutline) -> String {
    let mut out = String::new();
    out.push_str(&format!("{} | {} symbols\n", outline.file_path, outline.total_symbols));

    for sym in &outline.symbols {
        let indent = if sym.container_name.is_some() { "  " } else { "" };
        let kind_str = match sym.kind {
            crate::parser::SymbolKind::Function => "fn",
            crate::parser::SymbolKind::Method => "fn",
            crate::parser::SymbolKind::Struct => "struct",
            crate::parser::SymbolKind::Class => "class",
            crate::parser::SymbolKind::Interface => "interface",
            crate::parser::SymbolKind::Trait => "trait",
            crate::parser::SymbolKind::Enum => "enum",
            crate::parser::SymbolKind::TypeAlias => "type",
            crate::parser::SymbolKind::Module => "mod",
        };

        // Clean signature or fallback to name
        let sig = if !sym.signature.is_empty() {
            sym.signature.trim()
        } else {
            &sym.name
        };

        out.push_str(&format!(
            "{:>4}-{:<4} {}{} {}\n",
            sym.start_line, sym.end_line, indent, kind_str, sig
        ));
    }

    out
}

pub fn format_call_graph_concise(cg: &CallGraphResult) -> String {
    let mut out = String::new();
    out.push_str(&format!("Call Graph for: {}\n", cg.symbol_name));

    if !cg.definitions.is_empty() {
        out.push_str("Definitions:\n");
        for d in &cg.definitions {
            let cont = d.container_name.as_deref().map(|c| format!("{}::", c)).unwrap_or_default();
            out.push_str(&format!("  - {}{} ({}:{})\n", cont, d.name, d.file_path, d.start_line));
        }
    }

    out.push_str(&format!("Callers ({}):\n", cg.callers.len()));
    if cg.callers.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for c in &cg.callers {
            let cont = c.container_name.as_deref().map(|s| format!("{}::", s)).unwrap_or_default();
            out.push_str(&format!("  - {}{} ({}:{})\n", cont, c.name, c.file_path, c.start_line));
        }
    }

    out.push_str(&format!("Callees ({}):\n", cg.callees.len()));
    if cg.callees.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for c in &cg.callees {
            out.push_str(&format!("  - {}\n", c));
        }
    }

    out
}

pub fn format_symbol_code(sym: &Symbol, code_slice: &str) -> String {
    let mut out = String::new();
    let container = sym.container_name.as_deref().map(|c| format!("{}::", c)).unwrap_or_default();
    out.push_str(&format!("Symbol: {}{} ({}:{}) [{} lines]\n", container, sym.name, sym.file_path, sym.start_line, sym.end_line - sym.start_line + 1));
    out.push_str("```\n");
    out.push_str(code_slice);
    if !code_slice.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("```\n");
    out
}

/// One line per match: `path:line: text` (definitions tagged), grouped implicitly by file order.
pub fn format_matches(matches: &[crate::search::TextMatch], limit: usize, what: &str) -> String {
    if matches.is_empty() {
        return format!("No {} found.\n", what);
    }
    let cap = if limit == 0 { 50 } else { limit.min(500) };
    let mut out = format!("{} {}", matches.len(), what);
    if matches.len() >= cap {
        out.push_str(&format!(" (truncated at limit={}; raise 'limit' or narrow with a file filter)", cap));
    }
    out.push('\n');
    for m in matches {
        let tag = if m.is_definition { " [def]" } else { "" };
        out.push_str(&format!("{}:{}:{} {}\n", m.file_path, m.line_number, tag, m.line_text));
    }
    out
}
