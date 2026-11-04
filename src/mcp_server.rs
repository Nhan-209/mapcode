use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::architecture::get_architecture_tree;
use crate::dependency::get_file_dependencies;
use crate::entrypoint::get_categorized_entrypoints;
use crate::impact::compute_impact;
use crate::store::CodeStore;
use crate::type_graph::get_type_hierarchy;
use crate::watcher::{strip_unc_prefix, WatcherHandle};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let stripped = uri.strip_prefix("file://")?;
    let decoded = url_decode(stripped);

    #[cfg(windows)]
    {
        let trimmed = decoded.trim_start_matches('/');
        Some(PathBuf::from(trimmed.replace('/', "\\")))
    }
    #[cfg(not(windows))]
    {
        Some(PathBuf::from(decoded))
    }
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| (b as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn extract_workspace_from_init(params: Option<&Value>) -> Option<PathBuf> {
    let params = params?;

    // 1. Try rootUri
    if let Some(uri) = params.get("rootUri").and_then(|v| v.as_str()) {
        if let Some(path) = uri_to_path(uri) {
            return Some(path);
        }
    }

    // 2. Try rootPath
    if let Some(p) = params.get("rootPath").and_then(|v| v.as_str()) {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }

    // 3. Try workspaceFolders
    if let Some(folders) = params.get("workspaceFolders").and_then(|v| v.as_array()) {
        if let Some(first) = folders.first() {
            if let Some(uri) = first.get("uri").and_then(|v| v.as_str()) {
                if let Some(path) = uri_to_path(uri) {
                    return Some(path);
                }
            }
        }
    }

    None
}

#[allow(dead_code)]
pub fn get_tools_list() -> Value {
    json!([
        {
            "name": "set_workspace",
            "description": "Switch or re-index the active workspace directory. Allows AI to instantly analyze ANY project on your machine with ZERO manual config changes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory path of the project to analyze (e.g. 'D:/laptrinh/duan/my_app' or '.')"
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "get_file_outline",
            "description": "Get structured outline of symbols (functions, classes, structs, traits, methods) in a file with line numbers and signatures.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file within the project (e.g. 'src/main.rs')"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["json", "text"],
                        "description": "Use 'text' for a compact line-per-symbol outline (~90% fewer tokens). Default 'json'."
                    },
                    "workspace_path": {
                        "type": "string",
                        "description": "Optional workspace root directory to switch to before querying"
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "find_definition",
            "description": "Find definitions of a symbol (function, class, struct, type, trait) across the entire indexed codebase with high precision.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the symbol to find (e.g. 'parse_file' or 'CodeStore::get_call_graph')"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Optional file path or filename suffix to disambiguate identical symbol names"
                    },
                    "container": {
                        "type": "string",
                        "description": "Optional class, struct, or trait name to disambiguate methods"
                    },
                    "workspace_path": {
                        "type": "string",
                        "description": "Optional workspace root directory to switch to before querying"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "get_call_graph",
            "description": "Get bidirectional call graph for a function/method: who calls it (callers with line & signature) and what functions it calls (callees + resolved candidate definitions).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the function or method to query"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Optional file path or filename suffix to disambiguate identical function names"
                    },
                    "container": {
                        "type": "string",
                        "description": "Optional class or struct name to disambiguate methods"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["json", "text"],
                        "description": "Use 'text' for a compact callers/callees listing (far fewer tokens). Default 'json'."
                    },
                    "workspace_path": {
                        "type": "string",
                        "description": "Optional workspace root directory to switch to before querying"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "fuzzy_search_symbols",
            "description": "Fuzzy and substring search for symbols by name across the project, optionally filtered by symbol kind.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Symbol name query to search for"
                    },
                    "kind": {
                        "type": "string",
                        "description": "Optional filter by symbol kind: function, method, struct, class, interface, trait, enum, type_alias",
                        "enum": ["function", "method", "struct", "class", "interface", "trait", "enum", "type_alias"]
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default 30)"
                    },
                    "workspace_path": {
                        "type": "string",
                        "description": "Optional workspace root directory to switch to before querying"
                    }
                },
                "required": ["query"]
            }
        },
        {
            "name": "get_project_stats",
            "description": "Get high-level summary statistics of the indexed codebase (active root path, file counts, language breakdown, total symbols, functions, types).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspace_path": {
                        "type": "string",
                        "description": "Optional workspace root directory to switch to before querying"
                    }
                }
            }
        },
        {
            "name": "get_dependencies",
            "description": "Get forward and reverse import dependencies for a file, plus circular dependency cycle detection.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file within the project (e.g. 'src/main.rs')"
                    },
                    "workspace_path": {
                        "type": "string",
                        "description": "Optional workspace root directory to switch to before querying"
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "get_type_graph",
            "description": "Get bidirectional type hierarchy for a class/struct/interface/trait: supertypes, subtypes, trait implementations, and associated methods.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the class, struct, trait, or interface to inspect"
                    },
                    "workspace_path": {
                        "type": "string",
                        "description": "Optional workspace root directory to switch to before querying"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "get_entrypoints",
            "description": "List all detected system entrypoints (application main startup, HTTP API routes, CLI command handlers, background workers, event listeners).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "description": "Optional category filter: startup, http, cli, worker",
                        "enum": ["startup", "http", "cli", "worker"]
                    },
                    "workspace_path": {
                        "type": "string",
                        "description": "Optional workspace root directory to switch to before querying"
                    }
                }
            }
        },
        {
            "name": "get_architecture_map",
            "description": "Get a topographical hierarchical module tree with architectural layer classification (API, service, model, utility) and coupling metrics (Ca, Ce, Instability).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum tree depth to explore (default 3)"
                    },
                    "workspace_path": {
                        "type": "string",
                        "description": "Optional workspace root directory to switch to before querying"
                    }
                }
            }
        },
        {
            "name": "get_impact_analysis",
            "description": "Compute comprehensive blast-radius impact analysis when modifying a function, type, or file: upstream transitive callers, affected files, reachable entrypoints/APIs, and risk score.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "description": "Name of the function, type, or file path to analyze blast radius for"
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum traversal depth for upstream call chains (default 3)"
                    },
                    "workspace_path": {
                        "type": "string",
                        "description": "Optional workspace root directory to switch to before querying"
                    }
                },
                "required": ["target"]
            }
        },
        {
            "name": "read_symbol",
            "description": "Read exact source code implementation of a symbol (function, method, class, struct) with line numbers and optional surrounding context lines. MUCH cheaper and faster than reading entire files.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the symbol to read (e.g. 'new', 'parse_file', or 'CodeStore::root_path')"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Optional file path or filename suffix to disambiguate identical symbol names"
                    },
                    "container": {
                        "type": "string",
                        "description": "Optional class, struct, or trait name to disambiguate methods"
                    },
                    "context_lines": {
                        "type": "integer",
                        "description": "Number of context lines to include before and after the symbol body (default 0)"
                    },
                    "workspace_path": {
                        "type": "string",
                        "description": "Optional workspace root directory to switch to before querying"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "get_symbol_context",
            "description": "Get complete 360-degree context for a symbol in ONE tool call: source code slice, incoming callers with call site lines, outgoing callees with signatures, and containing type context.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the function or method to inspect"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Optional file path or filename suffix to disambiguate identical symbol names"
                    },
                    "container": {
                        "type": "string",
                        "description": "Optional class or struct name to disambiguate methods"
                    },
                    "workspace_path": {
                        "type": "string",
                        "description": "Optional workspace root directory to switch to before querying"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "find_references",
            "description": "Find all code references (calls, type usages, instantiations, field access) across the codebase for a symbol name, ignoring comments and strings.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the symbol, variable, or type to find usages of"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Optional file path or glob pattern to filter results"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of references to return (default 50)"
                    },
                    "workspace_path": {
                        "type": "string",
                        "description": "Optional workspace root directory to switch to before querying"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "search_text",
            "description": "Ultra-fast text or regex search across the indexed codebase respecting .gitignore. Far more efficient than running broad shell find/grep.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Text substring or regex pattern to search for"
                    },
                    "regex": {
                        "type": "boolean",
                        "description": "Whether to treat pattern as a regular expression (default false)"
                    },
                    "glob": {
                        "type": "string",
                        "description": "Optional file extension or path suffix filter (e.g. '.rs' or 'src/')"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of matching lines to return (default 50)"
                    },
                    "workspace_path": {
                        "type": "string",
                        "description": "Optional workspace root directory to switch to before querying"
                    }
                },
                "required": ["pattern"]
            }
        }
    ])
}

pub async fn run_stdio_server(
    store: Arc<CodeStore>,
    watcher: Arc<WatcherHandle>,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut stdout = tokio::io::stdout();
    let mut line = String::new();

    eprintln!("[MapCode] MCP stdio server ready for requests.");

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[MapCode] Parse error: {}", e);
                let err_resp = JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                let out = serde_json::to_string(&err_resp)? + "\n";
                stdout.write_all(out.as_bytes()).await?;
                stdout.flush().await?;
                continue;
            }
        };

        if req.id.is_none() {
            if req.method == "notifications/initialized" {
                eprintln!("[MapCode] Client initialized notification received.");
            }
            continue;
        }

        let resp = handle_request(&req, &store, &watcher).await;
        let out = serde_json::to_string(&resp)? + "\n";
        stdout.write_all(out.as_bytes()).await?;
        stdout.flush().await?;
    }

    eprintln!("[MapCode] MCP server connection closed.");
    Ok(())
}

async fn handle_request(
    req: &JsonRpcRequest,
    store: &Arc<CodeStore>,
    watcher: &Arc<WatcherHandle>,
) -> JsonRpcResponse {
    let id = req.id.clone();
    match req.method.as_str() {
        "initialize" => {
            // Auto-detect workspace from client initialization if provided
            if let Some(ws_path) = extract_workspace_from_init(req.params.as_ref()) {
                if ws_path.exists() && ws_path.is_dir() {
                    eprintln!(
                        "[MapCode] Auto-detected workspace from client initialize: {}",
                        ws_path.display()
                    );
                    let _ = watcher.switch_workspace(ws_path);
                }
            }

            let result = json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "mapcode",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "instructions": "ALWAYS use MapCode tools before reading files or running broad shell greps:\n- To inspect a function or struct body: use 'read_symbol' (drastically cheaper token cost than reading files).\n- To understand a symbol's full context (code, callers, callees, types): use 'get_symbol_context' in 1 call.\n- To locate symbols or explore outlines: use 'get_file_outline', 'find_definition', 'fuzzy_search_symbols'.\n- Pass format:'text' to get_file_outline / get_call_graph for compact output.\n- For raw text/regex search use 'search_text' (supports glob like '*.rs' or 'src/'); 'find_references' returns whole-word usages in source files with [def] tags.\n- To trace relationships: use 'get_call_graph' (bidirectional), 'find_references', 'get_dependencies'.\n- To assess blast radius before editing code: use 'get_impact_analysis'.\n- Results are guaranteed fresh: MapCode automatically refreshes edited files before returning responses."
            });
            JsonRpcResponse {
                jsonrpc: "2.0",
                id,
                result: Some(result),
                error: None,
            }
        }
        "ping" => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(json!({})),
            error: None,
        },
        "tools/list" => {
            let tools = get_tools_list();
            JsonRpcResponse {
                jsonrpc: "2.0",
                id,
                result: Some(json!({ "tools": tools })),
                error: None,
            }
        }
        "tools/call" => {
            let result = handle_tool_call(req.params.as_ref(), store, watcher).await;
            match result {
                Ok(content_text) => JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: Some(json!({
                        "content": [
                            {
                                "type": "text",
                                "text": content_text
                            }
                        ],
                        "isError": false
                    })),
                    error: None,
                },
                Err(err_msg) => JsonRpcResponse {
                    jsonrpc: "2.0",
                    id,
                    result: Some(json!({
                        "content": [
                            {
                                "type": "text",
                                "text": err_msg
                            }
                        ],
                        "isError": true
                    })),
                    error: None,
                },
            }
        }
        _ => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", req.method),
                data: None,
            }),
        },
    }
}

fn arg_str<'a>(args: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| args.get(*k).and_then(|v| v.as_str()))
}

fn arg_usize(args: &Value, keys: &[&str]) -> Option<usize> {
    keys.iter().find_map(|k| {
        args.get(*k)
            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())))
            .map(|n| n as usize)
    })
}

fn require_str<'a>(args: &'a Value, keys: &[&str]) -> Result<&'a str, String> {
    arg_str(args, keys).ok_or_else(|| {
        format!("Missing required argument '{}' (accepted: {})", keys[0], keys.join(", "))
    })
}

/// Error text for an unknown symbol, with close candidates so the caller can retry immediately.
fn symbol_not_found(store: &CodeStore, name: &str) -> String {
    let bare = name.rsplit([':', '.']).next().unwrap_or(name);
    let cands = store.fuzzy_search_symbols(bare, None, 5);
    if cands.is_empty() {
        format!("Symbol '{}' not found in indexed codebase. Try 'search_text' for raw text.", name)
    } else {
        let list: Vec<String> = cands
            .iter()
            .map(|c| format!("{}{} ({}:{})", c.container_name.as_deref().map(|x| format!("{}::", x)).unwrap_or_default(), c.name, c.file_path, c.line))
            .collect();
        format!("Symbol '{}' not found. Did you mean: {}", name, list.join(", "))
    }
}

/// Footer listing other definitions that were not chosen, so ambiguity is never silent.
fn ambiguity_note(defs: &[crate::store::DefinitionResult]) -> String {
    if defs.len() < 2 {
        return String::new();
    }
    let others: Vec<String> = defs
        .iter()
        .skip(1)
        .take(8)
        .map(|d| format!("{}{} ({}:{})", d.container_name.as_deref().map(|x| format!("{}::", x)).unwrap_or_default(), d.name, d.file_path, d.start_line))
        .collect();
    format!(
        "\nNOTE: {} definitions match; showing the first. Others: {}. Pass 'file_path' or 'container' to choose.\n",
        defs.len(),
        others.join(", ")
    )
}

/// Drops the per-call-site `calls` detail (internal resolver input) from outline JSON.
fn outline_json(outline: &crate::store::FileOutline) -> Result<String, String> {
    let mut v = serde_json::to_value(outline).map_err(|e| e.to_string())?;
    if let Some(syms) = v.get_mut("symbols").and_then(|s| s.as_array_mut()) {
        for s in syms {
            if let Some(o) = s.as_object_mut() {
                o.remove("calls");
            }
        }
    }
    serde_json::to_string(&v).map_err(|e| e.to_string())
}

async fn handle_tool_call(
    params: Option<&Value>,
    store: &Arc<CodeStore>,
    watcher: &Arc<WatcherHandle>,
) -> Result<String, String> {
    let params_obj = params.ok_or_else(|| "Missing params for tools/call".to_string())?;
    let tool_name = params_obj
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| "Missing tool name in tools/call".to_string())?;

    let arguments = params_obj.get("arguments").cloned().unwrap_or(Value::Null);

    // If workspace_path is passed in arguments and differs from current root, switch on the fly
    if let Some(ws) = arguments.get("workspace_path").and_then(|p| p.as_str()) {
        let clean = ws.trim().trim_matches('"').trim_matches('\'');
        let ws_path = PathBuf::from(clean);
        if ws_path != store.root_path() && ws_path.exists() && ws_path.is_dir() {
            let _ = watcher.switch_workspace(ws_path);
        }
    }

    match tool_name {
        "set_workspace" => {
            let path = arguments
                .get("path")
                .and_then(|p| p.as_str())
                .ok_or_else(|| "Missing required argument 'path'".to_string())?;
            let clean_path = path.trim().trim_matches('"').trim_matches('\'');
            let raw = PathBuf::from(clean_path);
            let target_dir = match std::fs::canonicalize(&raw) {
                Ok(p) => strip_unc_prefix(&p),
                Err(_) => strip_unc_prefix(&raw),
            };
            watcher.switch_workspace(target_dir)?;
            let stats = store.get_project_stats();
            serde_json::to_string(&stats).map_err(|e| e.to_string())
        }
        "get_file_outline" => {
            let path = arguments
                .get("path")
                .and_then(|p| p.as_str())
                .ok_or_else(|| "Missing required argument 'path'".to_string())?;
            let clean_path = path.trim().trim_matches('"').trim_matches('\'');
            let format_mode = arguments.get("format").and_then(|f| f.as_str()).unwrap_or("json");
            let outline = store.get_file_outline(clean_path)?;
            if format_mode == "text" {
                Ok(crate::format::format_outline_concise(&outline))
            } else {
                outline_json(&outline)
            }
        }
        "find_definition" => {
            let name = arguments
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or_else(|| "Missing required argument 'name'".to_string())?;
            let file_filter = arguments.get("file_path").and_then(|f| f.as_str());
            let container_filter = arguments.get("container").and_then(|c| c.as_str());
            let defs = store.find_definition_advanced(name.trim(), file_filter, container_filter);
            if defs.is_empty() {
                return Err(symbol_not_found(store, name));
            }
            serde_json::to_string(&defs).map_err(|e| e.to_string())
        }
        "get_call_graph" => {
            let name = arguments
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or_else(|| "Missing required argument 'name'".to_string())?;
            let file_filter = arguments.get("file_path").and_then(|f| f.as_str());
            let container_filter = arguments.get("container").and_then(|c| c.as_str());
            let format_mode = arguments.get("format").and_then(|f| f.as_str()).unwrap_or("json");
            let cg = store.get_call_graph_advanced(name.trim(), file_filter, container_filter);
            if format_mode == "text" {
                Ok(crate::format::format_call_graph_concise(&cg))
            } else {
                serde_json::to_string(&cg).map_err(|e| e.to_string())
            }
        }
        "read_symbol" => {
            let name = arguments
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or_else(|| "Missing required argument 'name'".to_string())?;
            let file_filter = arguments.get("file_path").and_then(|f| f.as_str());
            let container_filter = arguments.get("container").and_then(|c| c.as_str());
            let context_lines = arguments
                .get("context_lines")
                .and_then(|l| l.as_u64().or_else(|| l.as_str().and_then(|s| s.parse::<u64>().ok())))
                .unwrap_or(0) as usize;

            let defs = store.find_definition_advanced(name.trim(), file_filter, container_filter);
            if defs.is_empty() {
                return Err(symbol_not_found(store, name));
            }
            let def = &defs[0];
            let slice = store.read_symbol_slice(&def.file_path, def.start_line, def.end_line, context_lines)?;
            let sym = crate::store::Symbol {
                name: def.name.clone(),
                kind: def.kind,
                file_path: def.file_path.clone(),
                start_line: def.start_line,
                end_line: def.end_line,
                signature: def.signature.clone(),
                doc: def.doc.clone(),
                container_name: def.container_name.clone(),
                callees: def.callees.clone(),
                calls: Vec::new(),
            };
            Ok(crate::format::format_symbol_code(&sym, &slice) + &ambiguity_note(&defs))
        }
        "get_symbol_context" => {
            let name = arguments
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or_else(|| "Missing required argument 'name'".to_string())?;
            let file_filter = arguments.get("file_path").and_then(|f| f.as_str());
            let container_filter = arguments.get("container").and_then(|c| c.as_str());

            let defs = store.find_definition_advanced(name.trim(), file_filter, container_filter);
            if defs.is_empty() {
                return Err(symbol_not_found(store, name));
            }
            let def = &defs[0];
            let slice = store.read_symbol_slice(&def.file_path, def.start_line, def.end_line, 0)?;
            let cg = store.get_call_graph_advanced(name.trim(), file_filter, container_filter);

            let mut report = format!("=== SYMBOL CONTEXT: {} ===\n", name);
            let sym = crate::store::Symbol {
                name: def.name.clone(),
                kind: def.kind,
                file_path: def.file_path.clone(),
                start_line: def.start_line,
                end_line: def.end_line,
                signature: def.signature.clone(),
                doc: def.doc.clone(),
                container_name: def.container_name.clone(),
                callees: def.callees.clone(),
                calls: Vec::new(),
            };
            report.push_str(&crate::format::format_symbol_code(&sym, &slice));
            report.push('\n');
            report.push_str(&crate::format::format_call_graph_concise(&cg));
            report.push_str(&ambiguity_note(&defs));
            Ok(report)
        }
        "find_references" => {
            let name = require_str(&arguments, &["name", "symbol"])?;
            let file_filter = arg_str(&arguments, &["file_path", "glob"]);
            let limit = arg_usize(&arguments, &["limit", "max_results"]).unwrap_or(50);
            let matches = crate::search::find_references_in_workspace(store, name.trim(), file_filter, limit)?;
            Ok(crate::format::format_matches(&matches, limit, "references"))
        }
        "search_text" => {
            let pattern = require_str(&arguments, &["pattern", "query"])?;
            let is_regex = arguments.get("regex").and_then(|r| r.as_bool()).unwrap_or(false);
            let glob_filter = arg_str(&arguments, &["glob", "file_pattern", "file_path"]);
            let limit = arg_usize(&arguments, &["limit", "max_results"]).unwrap_or(50);
            let matches = crate::search::search_text_in_workspace(store, pattern.trim(), is_regex, glob_filter, limit)?;
            Ok(crate::format::format_matches(&matches, limit, "matches"))
        }
        "fuzzy_search_symbols" => {
            let query = arguments
                .get("query")
                .and_then(|q| q.as_str())
                .ok_or_else(|| "Missing required argument 'query'".to_string())?;
            let kind = arguments.get("kind").and_then(|k| k.as_str());
            let limit = arguments
                .get("limit")
                .and_then(|l| l.as_u64().or_else(|| l.as_str().and_then(|s| s.parse::<u64>().ok())))
                .map(|l| l as usize)
                .unwrap_or(30);
            let matches = store.fuzzy_search_symbols(query.trim(), kind, limit);
            serde_json::to_string(&matches).map_err(|e| e.to_string())
        }
        "get_project_stats" => {
            let stats = store.get_project_stats();
            serde_json::to_string(&stats).map_err(|e| e.to_string())
        }
        "get_dependencies" => {
            let path = arguments
                .get("path")
                .and_then(|p| p.as_str())
                .ok_or_else(|| "Missing required argument 'path'".to_string())?;
            let report = get_file_dependencies(store, path.trim());
            serde_json::to_string(&report).map_err(|e| e.to_string())
        }
        "get_type_graph" => {
            let name = arguments
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or_else(|| "Missing required argument 'name'".to_string())?;
            match get_type_hierarchy(store, name.trim()) {
                Some(report) => serde_json::to_string(&report).map_err(|e| e.to_string()),
                None => Err(format!("Type '{}' not found in indexed codebase.", name)),
            }
        }
        "get_entrypoints" => {
            let category = arguments.get("category").and_then(|c| c.as_str());
            let entrypoints = get_categorized_entrypoints(store, category);
            serde_json::to_string(&entrypoints).map_err(|e| e.to_string())
        }
        "get_architecture_map" => {
            let max_depth = arguments
                .get("max_depth")
                .and_then(|d| d.as_u64().or_else(|| d.as_str().and_then(|s| s.parse::<u64>().ok())))
                .map(|d| d as usize)
                .unwrap_or(3);
            let tree = get_architecture_tree(store, max_depth);
            serde_json::to_string(&tree).map_err(|e| e.to_string())
        }
        "get_impact_analysis" => {
            let target = arguments
                .get("target")
                .and_then(|t| t.as_str())
                .ok_or_else(|| "Missing required argument 'target'".to_string())?;
            let max_depth = arguments
                .get("max_depth")
                .and_then(|d| d.as_u64().or_else(|| d.as_str().and_then(|s| s.parse::<u64>().ok())))
                .map(|d| d as usize)
                .unwrap_or(3);
            let impact = compute_impact(store, target.trim(), max_depth);
            serde_json::to_string(&impact).map_err(|e| e.to_string())
        }
        _ => Err(format!("Unknown tool: '{}'", tool_name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_mcp_initialize() {
        let store = Arc::new(CodeStore::new(PathBuf::from(".")));
        let watcher = Arc::new(WatcherHandle::new(store.clone(), None));
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: Some(json!({})),
        };

        let resp = handle_request(&req, &store, &watcher).await;
        assert_eq!(resp.id, Some(json!(1)));
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(result["serverInfo"]["name"], "mapcode");
        assert!(result["instructions"].is_string());
    }

    #[tokio::test]
    async fn test_mcp_tools_list() {
        let store = Arc::new(CodeStore::new(PathBuf::from(".")));
        let watcher = Arc::new(WatcherHandle::new(store.clone(), None));
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(2)),
            method: "tools/list".to_string(),
            params: None,
        };

        let resp = handle_request(&req, &store, &watcher).await;
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 15);
        assert!(tools.iter().any(|t| t["name"] == "set_workspace"));
        assert!(tools.iter().any(|t| t["name"] == "get_file_outline"));
        assert!(tools.iter().any(|t| t["name"] == "find_definition"));
        assert!(tools.iter().any(|t| t["name"] == "get_call_graph"));
        assert!(tools.iter().any(|t| t["name"] == "fuzzy_search_symbols"));
        assert!(tools.iter().any(|t| t["name"] == "get_project_stats"));
        assert!(tools.iter().any(|t| t["name"] == "get_dependencies"));
        assert!(tools.iter().any(|t| t["name"] == "get_type_graph"));
        assert!(tools.iter().any(|t| t["name"] == "get_entrypoints"));
        assert!(tools.iter().any(|t| t["name"] == "get_architecture_map"));
        assert!(tools.iter().any(|t| t["name"] == "get_impact_analysis"));
        assert!(tools.iter().any(|t| t["name"] == "read_symbol"));
        assert!(tools.iter().any(|t| t["name"] == "get_symbol_context"));
        assert!(tools.iter().any(|t| t["name"] == "find_references"));
        assert!(tools.iter().any(|t| t["name"] == "search_text"));
    }

    #[tokio::test]
    async fn test_mcp_tool_call_stats_and_fuzzy() {
        let store = Arc::new(CodeStore::new(PathBuf::from(".")));
        let watcher = Arc::new(WatcherHandle::new(store.clone(), None));
        let req_stats = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(3)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "get_project_stats",
                "arguments": {}
            })),
        };

        let resp_stats = handle_request(&req_stats, &store, &watcher).await;
        assert!(resp_stats.error.is_none());
        let content = resp_stats.result.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(content.contains("total_files_indexed"));

        // Fuzzy search tool call with string limit
        let req_fuzzy = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(4)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "fuzzy_search_symbols",
                "arguments": {
                    "query": "nonexistent",
                    "limit": "5"
                }
            })),
        };

        let resp_fuzzy = handle_request(&req_fuzzy, &store, &watcher).await;
        assert!(resp_fuzzy.error.is_none());
        let fuzzy_content = resp_fuzzy.result.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert_eq!(fuzzy_content.trim(), "[]");
    }

    #[test]
    fn test_uri_to_path_conversion() {
        #[cfg(windows)]
        {
            let path = uri_to_path("file:///D:/projects/my%20app").unwrap();
            assert!(path.to_string_lossy().contains("my app"));
        }
        #[cfg(not(windows))]
        {
            let path = uri_to_path("file:///home/user/my%20app").unwrap();
            assert_eq!(path.to_string_lossy(), "/home/user/my app");
        }
    }
}
