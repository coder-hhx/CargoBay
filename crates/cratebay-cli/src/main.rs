//! CrateBay CLI — command-line interface.

use clap::{ArgAction, Parser, Subcommand};

mod commands;

use commands::OutputFormat;

#[derive(Parser)]
#[command(
    name = "cratebay",
    version,
    about = "CrateBay CLI — Container management from the command line"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Docker host (overrides auto-detection). Examples:
    /// - unix:///var/run/docker.sock
    /// - tcp://127.0.0.1:2375
    #[arg(long, global = true)]
    docker_host: Option<String>,

    /// Output format for structured commands.
    #[arg(long, global = true, default_value = "table")]
    format: OutputFormat,
}

#[derive(Subcommand)]
enum Commands {
    /// Container operations
    #[command(subcommand)]
    Container(ContainerCommands),

    /// Image operations
    #[command(subcommand)]
    Image(ImageCommands),

    /// Runtime management (start/stop/status)
    #[command(subcommand)]
    Runtime(RuntimeCommands),

    /// System information
    #[command(subcommand)]
    System(SystemCommands),

    /// MCP server operations
    #[command(subcommand)]
    Mcp(McpCommands),
}

#[derive(Subcommand)]
enum ContainerCommands {
    /// List containers
    #[command(alias = "ls")]
    List {
        /// Show all containers (including stopped)
        #[arg(long)]
        all: bool,
    },

    /// Create a container
    Create {
        /// Container name
        name: String,
        /// Image reference, e.g. alpine:3.20
        #[arg(long)]
        image: String,
        /// CPU cores limit
        #[arg(long)]
        cpu: Option<u32>,
        /// Memory limit in MB
        #[arg(long)]
        memory: Option<u64>,
        /// Command to run (shell form)
        #[arg(long)]
        command: Option<String>,
        /// Working directory inside the container
        #[arg(long)]
        working_dir: Option<String>,
        /// Environment variables (KEY=VALUE). Can be repeated.
        #[arg(long, action = ArgAction::Append)]
        env: Vec<String>,
        /// Do not auto-start container after creation
        #[arg(long)]
        no_start: bool,
    },

    /// Start a container
    Start { id: String },

    /// Stop a container
    Stop {
        id: String,
        /// Timeout in seconds before SIGKILL (default: 10)
        #[arg(long)]
        timeout: Option<u32>,
    },

    /// Delete a container
    Delete {
        id: String,
        /// Force removal
        #[arg(long)]
        force: bool,
    },

    /// Execute a command inside a container
    Exec {
        id: String,
        /// Working directory inside the container
        #[arg(long)]
        working_dir: Option<String>,
        /// Command to execute (after `--`)
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },

    /// Show container logs
    Logs {
        id: String,
        /// Follow log output
        #[arg(long)]
        follow: bool,
        /// Number of lines to show from the end (default: 100)
        #[arg(long)]
        tail: Option<u32>,
        /// Show timestamps (RFC3339)
        #[arg(long)]
        timestamps: bool,
    },

    /// Inspect a container
    Inspect { id: String },
}

#[derive(Subcommand)]
enum ImageCommands {
    /// List local images
    List,

    /// Search images from registry
    Search {
        query: String,
        /// Max results
        #[arg(long)]
        limit: Option<u32>,
    },

    /// Pull an image
    Pull { image: String },

    /// Delete a local image
    Delete { id: String },
}

#[derive(Subcommand)]
enum RuntimeCommands {
    /// Show runtime status
    Status,
    /// Start the built-in runtime
    Start,
    /// Stop the built-in runtime
    Stop,
    /// Pre-download runtime image without starting
    Provision,
}

#[derive(Subcommand)]
enum McpCommands {
    /// Export MCP config for Claude Desktop, Cursor, or other MCP clients
    Export {
        /// Target client (claude, cursor, generic)
        #[arg(default_value = "claude")]
        target: String,
    },
}

#[derive(Subcommand)]
enum SystemCommands {
    /// Show CrateBay version and platform info
    Info,

    /// Show Docker connection status (does not start runtime)
    DockerStatus,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    if let Some(host) = cli.docker_host.as_ref() {
        std::env::set_var("DOCKER_HOST", host);
    }

    // Runtime manager used by engine ensure for all Docker-dependent commands.
    let runtime = cratebay_core::runtime::create_runtime_manager();

    match cli.command {
        Commands::Container(cmd) => {
            let docker =
                cratebay_core::engine::ensure_docker(runtime.as_ref(), Default::default()).await?;
            match cmd {
                ContainerCommands::List { all } => {
                    commands::container::list(&docker, all, &cli.format).await?
                }
                ContainerCommands::Create {
                    name,
                    image,
                    cpu,
                    memory,
                    command,
                    working_dir,
                    env,
                    no_start,
                } => {
                    commands::container::create(
                        &docker,
                        name,
                        image,
                        cpu,
                        memory,
                        command,
                        working_dir,
                        env,
                        no_start,
                        &cli.format,
                    )
                    .await?
                }
                ContainerCommands::Start { id } => commands::container::start(&docker, &id).await?,
                ContainerCommands::Stop { id, timeout } => {
                    commands::container::stop(&docker, &id, timeout).await?
                }
                ContainerCommands::Delete { id, force } => {
                    commands::container::delete(&docker, &id, force).await?
                }
                ContainerCommands::Exec {
                    id,
                    command,
                    working_dir,
                } => {
                    commands::container::exec(&docker, &id, command, working_dir, &cli.format)
                        .await?
                }
                ContainerCommands::Logs {
                    id,
                    follow,
                    tail,
                    timestamps,
                } => commands::container::logs(&docker, &id, follow, tail, timestamps).await?,
                ContainerCommands::Inspect { id } => {
                    commands::container::inspect(&docker, &id, &cli.format).await?
                }
            }
        }
        Commands::Image(cmd) => {
            match cmd {
                ImageCommands::Search { query, limit } => {
                    // Image search should not require starting the runtime. Prefer any
                    // already-available Docker, otherwise fall back to Docker Hub HTTP API.
                    if let Some(docker) = cratebay_core::docker::try_connect().await {
                        commands::image::search(&docker, &query, limit, &cli.format).await?;
                    } else {
                        let results = cratebay_core::container::image_search_dockerhub(
                            &query,
                            limit.map(u64::from),
                        )
                        .await?;
                        commands::image::print_search_results(&results, &cli.format)?;
                    }
                }
                ImageCommands::List => {
                    let docker =
                        cratebay_core::engine::ensure_docker(runtime.as_ref(), Default::default())
                            .await?;
                    commands::image::list(&docker, &cli.format).await?
                }
                ImageCommands::Pull { image } => {
                    let docker =
                        cratebay_core::engine::ensure_docker(runtime.as_ref(), Default::default())
                            .await?;
                    commands::image::pull(&docker, &image).await?
                }
                ImageCommands::Delete { id } => {
                    let docker =
                        cratebay_core::engine::ensure_docker(runtime.as_ref(), Default::default())
                            .await?;
                    commands::image::delete(&docker, &id).await?
                }
            }
        }
        Commands::Runtime(cmd) => match cmd {
            RuntimeCommands::Status => commands::runtime::status().await?,
            RuntimeCommands::Start => commands::runtime::start().await?,
            RuntimeCommands::Stop => commands::runtime::stop().await?,
            RuntimeCommands::Provision => commands::runtime::provision().await?,
        },
        Commands::System(cmd) => match cmd {
            SystemCommands::Info => commands::system::info()?,
            SystemCommands::DockerStatus => commands::system::docker_status().await?,
        },
        Commands::Mcp(cmd) => match cmd {
            McpCommands::Export { target } => commands::mcp::export_config(&target)?,
        },
    }

    Ok(())
}
