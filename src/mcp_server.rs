use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::store::CodeStore;

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

#[allow(dead_code)]
pub fn get_tools_list() -> Value {
    json!([
        {
            "name": "get_file_outline",
            "description": "Get structured outline of symbols (functions, classes, structs, traits, methods) in a file with line numbers and signatures.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file within the project (e.g. 'src/main.rs')"
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "find_definition",
            "description": "Find definitions of a symbol (function, class, struct, type, trait) across the entire indexed codebase.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the symbol to find (e.g. 'parse_file' or 'CodeStore::get_call_graph')"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "get_call_graph",
            "description": "Get bidirectional call graph for a function/method: who calls it (callers) and what functions it calls (callees).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the function or method to query"
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
                    }
                },
                "required": ["query"]
            }
        },
        {
            "name": "get_project_stats",
            "description": "Get high-level summary statistics of the indexed codebase (file counts, language breakdown, total symbols, functions, types).",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }
    ])
}

pub async fn run_stdio_server(store: Arc<CodeStore>) -> Result<(), Box<dyn std::error::Error>> {
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

        let resp = handle_request(&req, &store).await;
        let out = serde_json::to_string(&resp)? + "\n";
        stdout.write_all(out.as_bytes()).await?;
        stdout.flush().await?;
    }

    eprintln!("[MapCode] MCP server connection closed.");
    Ok(())
}

async fn handle_request(req: &JsonRpcRequest, store: &Arc<CodeStore>) -> JsonRpcResponse {
    let id = req.id.clone();
    match req.method.as_str() {
        "initialize" => {
            let result = json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "mapcode",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "instructions": "MapCode provides in-memory code maps, outline, definition lookup, and call graph navigation for fast codebase understanding without searching all files."
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
            let result = handle_tool_call(req.params.as_ref(), store).await;
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

async fn handle_tool_call(
    params: Option<&Value>,
    store: &Arc<CodeStore>,
) -> Result<String, String> {
    let params_obj = params.ok_or_else(|| "Missing params for tools/call".to_string())?;
    let tool_name = params_obj
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| "Missing tool name in tools/call".to_string())?;

    let arguments = params_obj.get("arguments").cloned().unwrap_or(Value::Null);

    match tool_name {
        "get_file_outline" => {
            let path = arguments
                .get("path")
                .and_then(|p| p.as_str())
                .ok_or_else(|| "Missing required argument 'path'".to_string())?;
            let clean_path = path.trim().trim_matches('"').trim_matches('\'');
            let outline = store.get_file_outline(clean_path)?;
            serde_json::to_string_pretty(&outline).map_err(|e| e.to_string())
        }
        "find_definition" => {
            let name = arguments
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or_else(|| "Missing required argument 'name'".to_string())?;
            let defs = store.find_definition(name.trim());
            serde_json::to_string_pretty(&defs).map_err(|e| e.to_string())
        }
        "get_call_graph" => {
            let name = arguments
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or_else(|| "Missing required argument 'name'".to_string())?;
            let cg = store.get_call_graph(name.trim());
            serde_json::to_string_pretty(&cg).map_err(|e| e.to_string())
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
            serde_json::to_string_pretty(&matches).map_err(|e| e.to_string())
        }
        "get_project_stats" => {
            let stats = store.get_project_stats();
            serde_json::to_string_pretty(&stats).map_err(|e| e.to_string())
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
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: Some(json!({})),
        };

        let resp = handle_request(&req, &store).await;
        assert_eq!(resp.id, Some(json!(1)));
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(result["serverInfo"]["name"], "mapcode");
        assert!(result["instructions"].is_string());
    }

    #[tokio::test]
    async fn test_mcp_tools_list() {
        let store = Arc::new(CodeStore::new(PathBuf::from(".")));
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(2)),
            method: "tools/list".to_string(),
            params: None,
        };

        let resp = handle_request(&req, &store).await;
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 5);
        assert!(tools.iter().any(|t| t["name"] == "get_file_outline"));
        assert!(tools.iter().any(|t| t["name"] == "find_definition"));
        assert!(tools.iter().any(|t| t["name"] == "get_call_graph"));
        assert!(tools.iter().any(|t| t["name"] == "fuzzy_search_symbols"));
        assert!(tools.iter().any(|t| t["name"] == "get_project_stats"));
    }

    #[tokio::test]
    async fn test_mcp_tool_call_stats_and_fuzzy() {
        let store = Arc::new(CodeStore::new(PathBuf::from(".")));
        let req_stats = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(3)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "get_project_stats",
                "arguments": {}
            })),
        };

        let resp_stats = handle_request(&req_stats, &store).await;
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
                    "query": "test",
                    "limit": "10"
                }
            })),
        };

        let resp_fuzzy = handle_request(&req_fuzzy, &store).await;
        assert!(resp_fuzzy.error.is_none());
    }
}
