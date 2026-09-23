# MapCode

In-memory code map MCP server for AI coding assistants. Parses codebase with Tree-sitter, indexes symbols and relationships in RAM, and caches persistently to `.mapcode/cache.json`.

## Supported Languages

- Rust (.rs)
- Python (.py)
- TypeScript / JavaScript (.ts, .tsx, .js, .jsx)
- Lua (.lua)
- Go (.go)
- C / C++ (.c, .cpp, .cc, .h, .hpp)

## Installation

Download the binary for your platform from GitHub Releases:
- Windows: mapcode-windows-x64.zip (extract mapcode.exe)
- Linux: mapcode-linux-x64.tar.gz (extract mapcode)

Place the binary in your PATH or an accessible directory.

## MCP Client Configuration

Add to your MCP client config (Claude Desktop, Cursor, Antigravity, Cline, Roo-Code):

```json
{
  "mcpServers": {
    "mapcode": {
      "command": "path/to/mapcode.exe"
    }
  }
}
```

The workspace directory is detected automatically on initialization. You can switch to any other project at runtime using `set_workspace`.

## Tools

| Tool | Parameters | Description |
| --- | --- | --- |
| set_workspace | path | Switch or re-index the active workspace directory at runtime. |
| get_file_outline | path, workspace_path? | Symbols outline in a file (functions, classes, structs, signatures, line numbers). |
| find_definition | name, file_path?, container?, workspace_path? | Find exact definition location of a symbol. |
| get_call_graph | name, file_path?, container?, workspace_path? | Bidirectional call graph (callers with line numbers, callees with candidate definitions). |
| fuzzy_search_symbols | query, kind?, limit?, workspace_path? | Fuzzy search symbols across codebase with Jaro-Winkler ranking. |
| get_project_stats | workspace_path? | Total files, total symbols, functions, types, and language breakdown. |
| get_dependencies | path, workspace_path? | Forward and reverse file dependencies (imports, external imports, imported_by, circular cycles). |
| get_type_graph | name?, workspace_path? | Type inheritance hierarchy (supertypes, subtypes, traits/interfaces, struct embedding, associated methods). |
| get_entrypoints | category?, workspace_path? | Detect entrypoints: startup (main), http (API routes), cli (commands), worker (background handlers). |
| get_architecture_map | workspace_path? | Hierarchical module tree with inferred layer (api, service, model, utility) and Martin coupling metrics (Ca, Ce, Instability). |
| get_impact_analysis | target, max_depth?, workspace_path? | Blast radius analysis: upstream caller chains, affected files, reachable entrypoints, risk score (0-100). |

## How It Works

- Indexing: Walks the project directory respecting .gitignore, parses syntax trees with Tree-sitter, indexes symbols in concurrent DashMaps.
- Cache: Stores parsed metadata in .mapcode/cache.json using atomic file writes. Warm startup loads in ~35ms without re-parsing unchanged files.
- File Watcher: Background watcher updates individual modified files incrementally and syncs cache with debouncing.
- Communication: Standard JSON-RPC 2.0 over stdio. All logs are directed to stderr.

## License

MIT
