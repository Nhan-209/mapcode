// tests/fixtures/sample_workspace/src/main.rs

mod api;
mod cli;
mod models;
mod service;

use api::routes::create_router;
use cli::args::CliArgs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse_arguments();
    println!("Starting server on {}:{}", args.host, args.port);
    let app = create_router();
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", args.host, args.port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
