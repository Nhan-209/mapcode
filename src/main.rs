#![allow(
    dead_code,
    clippy::collapsible_if,
    clippy::if_same_then_else,
    clippy::manual_find,
    clippy::unnecessary_sort_by
)]

use std::path::PathBuf;
use std::sync::Arc;

mod architecture;
mod cache;
mod dependency;
mod entrypoint;
mod impact;
mod mcp_server;
mod parser;
mod store;
mod type_graph;
mod watcher;

use store::CodeStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    // Check CLI flags
    if args.iter().any(|a| a == "-h" || a == "--help") {
        eprintln!("MapCode MCP Server v{}", env!("CARGO_PKG_VERSION"));
        eprintln!("Blazing-fast in-memory code map MCP server for AI assistants\n");
        eprintln!("USAGE:");
        eprintln!("    mapcode [DIRECTORY]");
        eprintln!("\nARGS:");
        eprintln!("    <DIRECTORY>    Root directory of the codebase to index [default: .]");
        eprintln!("\nFLAGS:");
        eprintln!("    -h, --help       Print help information");
        eprintln!("    -v, --version    Print version information");
        return Ok(());
    }

    if args.iter().any(|a| a == "-v" || a == "--version") {
        eprintln!("mapcode {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Determine root directory to index
    let raw_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from(".")
    };

    let root_path = match std::fs::canonicalize(&raw_path) {
        Ok(p) => watcher::strip_unc_prefix(&p),
        Err(_) => watcher::strip_unc_prefix(&raw_path),
    };

    eprintln!(
        "[MapCode] Initializing in-memory code map at: {}",
        root_path.display()
    );

    let store = Arc::new(CodeStore::new(root_path.clone()));

    // 1. Initial repository load (warm cache <10ms or cold scan fallback)
    cache::load_or_scan_project(&store, &root_path);

    // 2. Start realtime background watcher
    let initial_watcher = match watcher::start_watcher(store.clone(), root_path.clone()) {
        Ok(w) => {
            eprintln!("[MapCode] Realtime file watcher active.");
            Some(w)
        }
        Err(e) => {
            eprintln!(
                "[MapCode] Warning: Failed to start file watcher: {}. Running with static index.",
                e
            );
            None
        }
    };

    let watcher_handle = Arc::new(watcher::WatcherHandle::new(store.clone(), initial_watcher));

    // 3. Start Stdio MCP Server loop
    mcp_server::run_stdio_server(store, watcher_handle).await?;

    Ok(())
}
