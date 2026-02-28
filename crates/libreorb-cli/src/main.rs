use bollard::Docker;
use bollard::container::{ListContainersOptions, StartContainerOptions, StopContainerOptions, RemoveContainerOptions};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::path::Path;

#[derive(Parser)]
#[command(name = "cargobay", version = "0.1.0", about = "Free, open-source alternative to OrbStack")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// VM management commands
    Vm {
        #[command(subcommand)]
        command: VmCommands,
    },
    /// Docker container management
    Docker {
        #[command(subcommand)]
        command: DockerCommands,
    },
    /// File sharing management (VirtioFS)
    Mount {
        #[command(subcommand)]
        command: MountCommands,
    },
    /// Show system status and platform info
    Status,
}

#[derive(Subcommand)]
enum VmCommands {
    /// Create a new VM
    Create {
        name: String,
        #[arg(long, default_value = "2")]
        cpus: u32,
        #[arg(long, default_value = "2048")]
        memory: u64,
        #[arg(long, default_value = "20")]
        disk: u64,
        /// Enable Rosetta x86_64 translation (macOS Apple Silicon only)
        #[arg(long)]
        rosetta: bool,
    },
    /// Start a VM
    Start { name: String },
    /// Stop a VM
    Stop { name: String },
    /// Delete a VM
    Delete { name: String },
    /// List all VMs
    List,
}

#[derive(Subcommand)]
enum DockerCommands {
    /// List containers
    Ps,
    /// Start a container
    Start { id: String },
    /// Stop a container
    Stop { id: String },
    /// Remove a container
    Rm { id: String },
}

#[derive(Subcommand)]
enum MountCommands {
    /// Mount a host directory into a VM via VirtioFS
    Add {
        /// VM name or ID
        #[arg(long)]
        vm: String,
        /// Tag for the mount
        #[arg(long)]
        tag: String,
        /// Host path to share
        #[arg(long)]
        host_path: String,
        /// Guest mount point
        #[arg(long, default_value = "/mnt/host")]
        guest_path: String,
        /// Mount as read-only
        #[arg(long)]
        readonly: bool,
    },
    /// Unmount a VirtioFS share from a VM
    Remove {
        /// VM name or ID
        #[arg(long)]
        vm: String,
        /// Tag of the mount to remove
        #[arg(long)]
        tag: String,
    },
    /// List VirtioFS mounts for a VM
    List {
        /// VM name or ID
        #[arg(long)]
        vm: String,
    },
}

fn detect_docker_socket() -> Option<String> {
    // Unix socket detection (macOS / Linux)
    #[cfg(unix)]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        let candidates = [
            format!("{}/.colima/default/docker.sock", home),
            format!("{}/.orbstack/run/docker.sock", home),
            "/var/run/docker.sock".to_string(),
            format!("{}/.docker/run/docker.sock", home),
        ];
        if let Some(sock) = candidates.into_iter().find(|p| Path::new(p).exists()) {
            return Some(sock);
        }
    }

    // Windows named pipe detection
    #[cfg(windows)]
    {
        // Docker Desktop for Windows uses named pipes
        let candidates = [
            r"//./pipe/docker_engine",
            r"//./pipe/dockerDesktopLinuxEngine",
        ];
        for pipe in &candidates {
            if Path::new(pipe).exists() {
                return Some(pipe.to_string());
            }
        }
        // WSL2 Docker socket
        let userprofile = std::env::var("USERPROFILE").unwrap_or_default();
        let wsl_sock = format!(r"{}\\.docker\run\docker.sock", userprofile);
        if Path::new(&wsl_sock).exists() {
            return Some(wsl_sock);
        }
    }

    None
}

fn connect_docker() -> Result<Docker, String> {
    // Check DOCKER_HOST env first
    if std::env::var("DOCKER_HOST").is_ok() {
        return Docker::connect_with_local_defaults()
            .map_err(|e| format!("Failed to connect via DOCKER_HOST: {}", e));
    }

    if let Some(sock) = detect_docker_socket() {
        #[cfg(unix)]
        {
            Docker::connect_with_socket(&sock, 120, bollard::API_DEFAULT_VERSION)
                .map_err(|e| format!("Failed to connect to Docker at {}: {}", sock, e))
        }
        #[cfg(windows)]
        {
            Docker::connect_with_named_pipe(&sock, 120, bollard::API_DEFAULT_VERSION)
                .map_err(|e| format!("Failed to connect to Docker at {}: {}", sock, e))
        }
        #[cfg(not(any(unix, windows)))]
        {
            Docker::connect_with_local_defaults()
                .map_err(|e| format!("Failed to connect to Docker: {}", e))
        }
    } else {
        Err("No Docker socket found. Set DOCKER_HOST or install Docker/Colima/OrbStack.".into())
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Vm { command } => handle_vm(command),
        Commands::Docker { command } => {
            if let Err(e) = handle_docker(command).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Mount { command } => handle_mount(command),
        Commands::Status => {
            println!("CargoBay v0.1.0");
            println!("Platform: {}", libreorb_core::platform_info());
            let hv = libreorb_core::create_hypervisor();
            println!("Rosetta x86_64: {}", if hv.rosetta_available() { "available" } else { "not available" });
            match detect_docker_socket() {
                Some(sock) => println!("Docker: connected ({})", sock),
                None => println!("Docker: not found"),
            }
        }
    }
}

fn handle_vm(cmd: VmCommands) {
    let hv = libreorb_core::create_hypervisor();
    match cmd {
        VmCommands::Create { name, cpus, memory, disk, rosetta } => {
            use libreorb_core::hypervisor::VmConfig;
            let config = VmConfig {
                name: name.clone(),
                cpus,
                memory_mb: memory,
                disk_gb: disk,
                rosetta,
                shared_dirs: vec![],
            };
            match hv.create_vm(config) {
                Ok(id) => {
                    println!("Created VM '{}' (id: {})", name, id);
                    if rosetta {
                        println!("  Rosetta x86_64 translation: enabled");
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        VmCommands::Start { name } => {
            match hv.start_vm(&name) {
                Ok(()) => println!("Started VM '{}'", name),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        VmCommands::Stop { name } => {
            match hv.stop_vm(&name) {
                Ok(()) => println!("Stopped VM '{}'", name),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        VmCommands::Delete { name } => {
            match hv.delete_vm(&name) {
                Ok(()) => println!("Deleted VM '{}'", name),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        VmCommands::List => {
            match hv.list_vms() {
                Ok(vms) => {
                    if vms.is_empty() {
                        println!("No VMs found.");
                        return;
                    }
                    println!("{:<12} {:<20} {:<10} {:<6} {:<8} {:<8} {}", "ID", "NAME", "STATE", "CPUS", "MEMORY", "ROSETTA", "MOUNTS");
                    for vm in vms {
                        println!(
                            "{:<12} {:<20} {:<10} {:<6} {:<8} {:<8} {}",
                            vm.id,
                            vm.name,
                            format!("{:?}", vm.state),
                            vm.cpus,
                            format!("{}MB", vm.memory_mb),
                            if vm.rosetta_enabled { "yes" } else { "no" },
                            vm.shared_dirs.len(),
                        );
                    }
                }
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
    }
}

fn handle_mount(cmd: MountCommands) {
    let hv = libreorb_core::create_hypervisor();
    match cmd {
        MountCommands::Add { vm, tag, host_path, guest_path, readonly } => {
            use libreorb_core::hypervisor::SharedDirectory;
            let share = SharedDirectory {
                tag: tag.clone(),
                host_path: host_path.clone(),
                guest_path: guest_path.clone(),
                read_only: readonly,
            };
            match hv.mount_virtiofs(&vm, &share) {
                Ok(()) => {
                    println!("Mounted '{}' → {} (tag: {}{})", host_path, guest_path, tag, if readonly { ", read-only" } else { "" });
                }
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        MountCommands::Remove { vm, tag } => {
            match hv.unmount_virtiofs(&vm, &tag) {
                Ok(()) => println!("Unmounted tag '{}'", tag),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        MountCommands::List { vm } => {
            match hv.list_virtiofs_mounts(&vm) {
                Ok(mounts) => {
                    if mounts.is_empty() {
                        println!("No VirtioFS mounts for VM '{}'.", vm);
                        return;
                    }
                    println!("{:<16} {:<30} {:<20} {}", "TAG", "HOST PATH", "GUEST PATH", "MODE");
                    for m in mounts {
                        println!("{:<16} {:<30} {:<20} {}", m.tag, m.host_path, m.guest_path, if m.read_only { "ro" } else { "rw" });
                    }
                }
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
    }
}

async fn handle_docker(cmd: DockerCommands) -> Result<(), String> {
    let docker = connect_docker()?;
    match cmd {
        DockerCommands::Ps => {
            let mut filters = HashMap::new();
            filters.insert("status", vec!["running", "exited", "paused", "created", "restarting", "dead"]);
            let opts = ListContainersOptions { all: true, filters, ..Default::default() };
            let containers = docker.list_containers(Some(opts)).await.map_err(|e| e.to_string())?;

            println!("{:<16} {:<24} {:<24} {:<16} {}", "CONTAINER ID", "NAME", "IMAGE", "STATUS", "PORTS");
            for c in containers {
                let id = c.id.as_deref().unwrap_or("").chars().take(12).collect::<String>();
                let name = c.names.as_ref().and_then(|n| n.first()).map(|n| n.trim_start_matches('/')).unwrap_or("").to_string();
                let image = c.image.as_deref().unwrap_or("");
                let status = c.status.as_deref().unwrap_or("");
                let ports = c.ports.as_ref().map(|ps| {
                    ps.iter().filter_map(|p| {
                        let private = p.private_port;
                        let public = p.public_port;
                        let typ = p.typ.as_ref().map(|t| format!("{}", t)).unwrap_or_default();
                        match public {
                            Some(pub_port) => Some(format!("{}:{}->{}/{}", p.ip.as_deref().unwrap_or("0.0.0.0"), pub_port, private, typ)),
                            None => Some(format!("{}/{}", private, typ)),
                        }
                    }).collect::<Vec<_>>().join(", ")
                }).unwrap_or_default();
                println!("{:<16} {:<24} {:<24} {:<16} {}", id, name, image, status, ports);
            }
        }
        DockerCommands::Start { id } => {
            docker.start_container(&id, None::<StartContainerOptions<String>>).await.map_err(|e| e.to_string())?;
            println!("Started container {}", id);
        }
        DockerCommands::Stop { id } => {
            docker.stop_container(&id, Some(StopContainerOptions { t: 10 })).await.map_err(|e| e.to_string())?;
            println!("Stopped container {}", id);
        }
        DockerCommands::Rm { id } => {
            docker.remove_container(&id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await.map_err(|e| e.to_string())?;
            println!("Removed container {}", id);
        }
    }
    Ok(())
}
