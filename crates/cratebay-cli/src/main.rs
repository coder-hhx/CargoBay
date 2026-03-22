//! CrateBay CLI — command-line interface.

use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(
    name = "cratebay",
    version,
    about = "CrateBay CLI — manage containers and AI workflows"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List containers
    #[command(alias = "ls")]
    List,
    /// Show system information
    Info,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::List => commands::container::list().await?,
        Commands::Info => commands::system::info()?,
    }

    Ok(())
}
