// tests/fixtures/sample_workspace/src/cli/args.rs

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "sample_server", about = "Sample service server CLI")]
pub struct CliArgs {
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    #[arg(short, long, default_value = "127.0.0.1")]
    pub host: String,
}

impl CliArgs {
    pub fn parse_arguments() -> Self {
        CliArgs::parse()
    }
}
