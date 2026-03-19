use bollard::auth::DockerCredentials;
use bollard::container::{
    Config, CreateContainerOptions, InspectContainerOptions, ListContainersOptions, LogsOptions,
    RemoveContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::image::{
    CommitContainerOptions, CreateImageOptions, ImportImageOptions, ListImagesOptions,
    PushImageOptions, RemoveImageOptions, TagImageOptions,
};
use bollard::service::HostConfig;
use bollard::volume::{CreateVolumeOptions, ListVolumesOptions};
use bollard::Docker;
use clap::{CommandFactory, Parser, Subcommand};
use futures_util::stream::TryStreamExt;
use futures_util::StreamExt;
use reqwest::header::{ACCEPT, WWW_AUTHENTICATE};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio_util::codec::{BytesCodec, FramedRead};
use tonic::transport::Channel;

use cratebay_core::proto;
use cratebay_core::proto::vm_service_client::VmServiceClient;
use cratebay_core::validation;

include!("runtime.rs");
include!("vm.rs");
include!("image.rs");
include!("docker.rs");
include!("volume.rs");
include!("k3s.rs");

#[cfg(target_os = "macos")]
use std::sync::OnceLock;

#[derive(Parser)]
#[command(
    name = "cratebay",
    version,
    about = "Free, open-source desktop for containers and Linux VMs"
)]
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
    /// Built-in Docker-compatible runtime (CrateBay Runtime)
    Runtime {
        #[command(subcommand)]
        command: RuntimeCommands,
    },
    /// Docker container management
    Docker {
        #[command(subcommand)]
        command: DockerCommands,
    },
    /// Image management commands
    Image {
        #[command(subcommand)]
        command: ImageCommands,
    },
    /// File sharing management (VirtioFS)
    Mount {
        #[command(subcommand)]
        command: MountCommands,
    },
    /// Docker volume management
    Volume {
        #[command(subcommand)]
        command: VolumeCommands,
    },
    /// K3s (lightweight Kubernetes) management
    K3s {
        #[command(subcommand)]
        command: K3sCommands,
    },
    /// Show system status and platform info
    Status,
    /// Generate shell completions
    Completions {
        /// Target shell (bash, zsh, fish, elvish, powershell)
        shell: clap_complete::Shell,
    },
    #[command(hide = true)]
    InternalRuntimeProxy {
        #[arg(long)]
        guest_ip: String,
        #[arg(long)]
        listen_port: u16,
        #[arg(long)]
        target_port: u16,
    },
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
        /// OS image to use (e.g. "alpine-3.19"). See `cratebay image list-os`
        #[arg(long)]
        os_image: Option<String>,
    },
    /// Start a VM
    Start { name: String },
    /// Stop a VM
    Stop { name: String },
    /// Delete a VM
    Delete { name: String },
    /// List all VMs
    List,
    /// Print an SSH login command for a VM (requires an SSH endpoint)
    LoginCmd {
        name: String,
        #[arg(long, default_value = "root")]
        user: String,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// SSH port (required until VM networking/port-forwarding is implemented)
        #[arg(long)]
        port: Option<u16>,
    },
    /// Port forwarding management
    Port {
        #[command(subcommand)]
        command: PortCommands,
    },
}

#[derive(Subcommand)]
enum PortCommands {
    /// Add a port forward from host to VM guest
    Add {
        /// VM name or ID
        #[arg(long)]
        vm: String,
        /// Host port to listen on
        #[arg(long)]
        host: u16,
        /// Guest port to forward to
        #[arg(long)]
        guest: u16,
        /// Protocol: tcp or udp
        #[arg(long, default_value = "tcp")]
        protocol: String,
    },
    /// List port forwards for a VM
    List {
        /// VM name or ID
        #[arg(long)]
        vm: String,
    },
    /// Remove a port forward
    Remove {
        /// VM name or ID
        #[arg(long)]
        vm: String,
        /// Host port to stop forwarding
        #[arg(long)]
        host: u16,
    },
}

#[derive(Subcommand)]
enum RuntimeCommands {
    /// Start CrateBay Runtime
    Start,
    /// Stop CrateBay Runtime
    Stop,
    /// Show CrateBay Runtime status
    Status,
    /// Print environment variables for terminal usage (DOCKER_HOST)
    Env,
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
    /// Run a new container from an image
    Run {
        image: String,
        /// Optional container name
        #[arg(long)]
        name: Option<String>,
        /// Limit CPU cores (e.g. 2)
        #[arg(long)]
        cpus: Option<u32>,
        /// Limit memory in MB (e.g. 2048)
        #[arg(long)]
        memory: Option<u64>,
        /// Pull image before creating the container
        #[arg(long)]
        pull: bool,
        /// Set environment variables (can be repeated, e.g. --env KEY=VALUE)
        #[arg(long = "env", short = 'e')]
        env: Vec<String>,
    },
    /// Print a shell login command for a container
    LoginCmd {
        container: String,
        #[arg(long, default_value = "/bin/sh")]
        shell: String,
    },
    /// Show logs for a container
    Logs {
        /// Container name or ID
        container: String,
        /// Number of lines to show from the end of the logs (or "all")
        #[arg(long, default_value = "200")]
        tail: String,
        /// Show timestamps
        #[arg(long)]
        timestamps: bool,
    },
    /// Show environment variables of a container
    Env {
        /// Container name or ID
        id: String,
    },
}

#[derive(Subcommand)]
enum ImageCommands {
    /// Search images (Docker Hub / Quay) or list tags for a registry reference
    Search {
        query: String,
        /// dockerhub | quay | all
        #[arg(long, default_value = "all")]
        source: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// List tags for an OCI image reference (e.g. ghcr.io/org/image)
    Tags {
        reference: String,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// List local Docker images
    List,
    /// Remove a local Docker image
    Remove {
        /// Image ID or reference (e.g. nginx:1.27-alpine)
        reference: String,
    },
    /// Tag a local Docker image with a new name
    Tag {
        /// Source image reference (e.g. nginx:1.27-alpine)
        source: String,
        /// Target tag in repo:tag format (e.g. myrepo/nginx:v1)
        target: String,
    },
    /// Inspect a local Docker image (show details as JSON)
    Inspect {
        /// Image ID or reference
        reference: String,
    },
    /// Load a local image archive into Docker (same as `docker load -i`)
    Load { path: String },
    /// Push an image to a registry (same as `docker push`)
    Push { reference: String },
    /// Package an image from an existing container (same as `docker commit`)
    PackContainer { container: String, tag: String },
    /// List available Linux OS images for VM booting
    ListOs,
    /// Download a Linux OS image (kernel + initrd + rootfs) for VM booting
    DownloadOs {
        /// Image id, e.g. "alpine-3.19", "ubuntu-24.04", "debian-12"
        name: String,
    },
    /// Delete a downloaded Linux OS image
    DeleteOs {
        /// Image id, e.g. "alpine-3.19"
        name: String,
    },
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

#[derive(Subcommand)]
enum VolumeCommands {
    /// List all Docker volumes
    List,
    /// Create a Docker volume
    Create {
        /// Volume name
        name: String,
        /// Volume driver (default: local)
        #[arg(long, default_value = "local")]
        driver: String,
    },
    /// Inspect a Docker volume (show details as JSON)
    Inspect {
        /// Volume name
        name: String,
    },
    /// Remove a Docker volume
    Remove {
        /// Volume name
        name: String,
    },
}

#[derive(Subcommand)]
enum K3sCommands {
    /// Show K3s cluster status
    Status,
    /// Download the K3s binary
    Install,
    /// Start the K3s cluster
    Start,
    /// Stop the K3s cluster
    Stop,
    /// Remove K3s binary and data
    Uninstall,
}

fn main() {
    #[cfg(target_os = "macos")]
    prime_macos_runtime_assets_env();
    cratebay_core::logging::init();
    let cli = Cli::parse();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(run_cli(cli));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::Path;
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct EnvVarGuard {
        key: &'static str,
        prev: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set_path(key: &'static str, value: &Path) -> Self {
            let prev = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, prev }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match self.prev.take() {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    struct TempDirGuard {
        path: std::path::PathBuf,
    }

    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), nanos));
            std::fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn daemon_path_prefers_cratebay_daemon_path_env() {
        let _env_guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock");

        let temp = TempDirGuard::new("cratebay-cli-test");
        let fake = temp.path.join("fake-daemon");
        std::fs::write(&fake, b"").expect("write");

        let _daemon_path = EnvVarGuard::set_path("CRATEBAY_DAEMON_PATH", &fake);
        let resolved = daemon_path();
        assert_eq!(resolved, fake);
    }

    #[test]
    #[cfg(unix)]
    fn spawn_daemon_detached_runs_executable_from_env() {
        use std::os::unix::fs::PermissionsExt;

        let _env_guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock");

        let temp = TempDirGuard::new("cratebay-cli-test");
        let marker = temp.path.join("started");
        let script = temp.path.join("fake-daemon");

        let script_body = format!(
            "#!/bin/sh\nset -eu\nprintf 'ok\\n' > '{}'\n",
            marker.display()
        );
        std::fs::write(&script, script_body).expect("write script");
        let mut perms = std::fs::metadata(&script).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod");

        let _daemon_path = EnvVarGuard::set_path("CRATEBAY_DAEMON_PATH", &script);
        let _pid = spawn_daemon_detached().expect("spawn");

        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            if marker.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        panic!("expected marker file to be created");
    }

    // -----------------------------------------------------------------------
    // CLI argument parsing tests using clap's try_parse_from
    // -----------------------------------------------------------------------

    #[test]
    fn parse_docker_ps() {
        let cli = Cli::try_parse_from(["cratebay", "docker", "ps"]);
        assert!(cli.is_ok(), "docker ps should parse: {:?}", cli.err());
        let cli = cli.unwrap();
        assert!(matches!(
            cli.command,
            Commands::Docker {
                command: DockerCommands::Ps
            }
        ));
    }

    #[test]
    fn parse_vm_list() {
        let cli = Cli::try_parse_from(["cratebay", "vm", "list"]);
        assert!(cli.is_ok(), "vm list should parse: {:?}", cli.err());
        let cli = cli.unwrap();
        assert!(matches!(
            cli.command,
            Commands::Vm {
                command: VmCommands::List
            }
        ));
    }

    #[test]
    fn parse_vm_create_with_defaults() {
        let cli = Cli::try_parse_from(["cratebay", "vm", "create", "my-vm"]).unwrap();
        match cli.command {
            Commands::Vm {
                command:
                    VmCommands::Create {
                        name,
                        cpus,
                        memory,
                        disk,
                        rosetta,
                        os_image,
                    },
            } => {
                assert_eq!(name, "my-vm");
                assert_eq!(cpus, 2);
                assert_eq!(memory, 2048);
                assert_eq!(disk, 20);
                assert!(!rosetta);
                assert!(os_image.is_none());
            }
            _ => panic!("expected Vm Create command"),
        }
    }

    #[test]
    fn parse_vm_create_with_custom_args() {
        let cli = Cli::try_parse_from([
            "cratebay",
            "vm",
            "create",
            "test-vm",
            "--cpus",
            "4",
            "--memory",
            "8192",
            "--disk",
            "100",
            "--rosetta",
            "--os-image",
            "alpine-3.19",
        ])
        .unwrap();
        match cli.command {
            Commands::Vm {
                command:
                    VmCommands::Create {
                        name,
                        cpus,
                        memory,
                        disk,
                        rosetta,
                        os_image,
                    },
            } => {
                assert_eq!(name, "test-vm");
                assert_eq!(cpus, 4);
                assert_eq!(memory, 8192);
                assert_eq!(disk, 100);
                assert!(rosetta);
                assert_eq!(os_image.as_deref(), Some("alpine-3.19"));
            }
            _ => panic!("expected Vm Create command"),
        }
    }

    #[test]
    fn parse_image_search() {
        let cli = Cli::try_parse_from(["cratebay", "image", "search", "nginx"]).unwrap();
        match cli.command {
            Commands::Image {
                command:
                    ImageCommands::Search {
                        query,
                        source,
                        limit,
                    },
            } => {
                assert_eq!(query, "nginx");
                assert_eq!(source, "all");
                assert_eq!(limit, 20);
            }
            _ => panic!("expected Image Search command"),
        }
    }

    #[test]
    fn parse_image_search_with_options() {
        let cli = Cli::try_parse_from([
            "cratebay",
            "image",
            "search",
            "redis",
            "--source",
            "dockerhub",
            "--limit",
            "5",
        ])
        .unwrap();
        match cli.command {
            Commands::Image {
                command:
                    ImageCommands::Search {
                        query,
                        source,
                        limit,
                    },
            } => {
                assert_eq!(query, "redis");
                assert_eq!(source, "dockerhub");
                assert_eq!(limit, 5);
            }
            _ => panic!("expected Image Search command"),
        }
    }

    #[test]
    fn parse_k3s_status() {
        let cli = Cli::try_parse_from(["cratebay", "k3s", "status"]);
        assert!(cli.is_ok(), "k3s status should parse: {:?}", cli.err());
        let cli = cli.unwrap();
        assert!(matches!(
            cli.command,
            Commands::K3s {
                command: K3sCommands::Status
            }
        ));
    }

    #[test]
    fn parse_volume_list() {
        let cli = Cli::try_parse_from(["cratebay", "volume", "list"]);
        assert!(cli.is_ok(), "volume list should parse: {:?}", cli.err());
        let cli = cli.unwrap();
        assert!(matches!(
            cli.command,
            Commands::Volume {
                command: VolumeCommands::List
            }
        ));
    }

    #[test]
    fn parse_volume_create() {
        let cli = Cli::try_parse_from(["cratebay", "volume", "create", "myvolume"]).unwrap();
        match cli.command {
            Commands::Volume {
                command: VolumeCommands::Create { name, driver },
            } => {
                assert_eq!(name, "myvolume");
                assert_eq!(driver, "local");
            }
            _ => panic!("expected Volume Create command"),
        }
    }

    #[test]
    fn parse_docker_run() {
        let cli = Cli::try_parse_from([
            "cratebay",
            "docker",
            "run",
            "nginx:latest",
            "--name",
            "web",
            "--cpus",
            "1",
            "--memory",
            "512",
            "--pull",
            "-e",
            "FOO=bar",
        ])
        .unwrap();
        match cli.command {
            Commands::Docker {
                command:
                    DockerCommands::Run {
                        image,
                        name,
                        cpus,
                        memory,
                        pull,
                        env,
                    },
            } => {
                assert_eq!(image, "nginx:latest");
                assert_eq!(name.as_deref(), Some("web"));
                assert_eq!(cpus, Some(1));
                assert_eq!(memory, Some(512));
                assert!(pull);
                assert_eq!(env, vec!["FOO=bar"]);
            }
            _ => panic!("expected Docker Run command"),
        }
    }

    #[test]
    fn parse_status() {
        let cli = Cli::try_parse_from(["cratebay", "status"]);
        assert!(cli.is_ok(), "status should parse: {:?}", cli.err());
        let cli = cli.unwrap();
        assert!(matches!(cli.command, Commands::Status));
    }

    #[test]
    fn parse_mount_add() {
        let cli = Cli::try_parse_from([
            "cratebay",
            "mount",
            "add",
            "--vm",
            "my-vm",
            "--tag",
            "code",
            "--host-path",
            "/home/user/code",
            "--guest-path",
            "/mnt/code",
            "--readonly",
        ])
        .unwrap();
        match cli.command {
            Commands::Mount {
                command:
                    MountCommands::Add {
                        vm,
                        tag,
                        host_path,
                        guest_path,
                        readonly,
                    },
            } => {
                assert_eq!(vm, "my-vm");
                assert_eq!(tag, "code");
                assert_eq!(host_path, "/home/user/code");
                assert_eq!(guest_path, "/mnt/code");
                assert!(readonly);
            }
            _ => panic!("expected Mount Add command"),
        }
    }

    #[test]
    fn parse_vm_port_add() {
        let cli = Cli::try_parse_from([
            "cratebay",
            "vm",
            "port",
            "add",
            "--vm",
            "myvm",
            "--host",
            "8080",
            "--guest",
            "80",
            "--protocol",
            "tcp",
        ])
        .unwrap();
        match cli.command {
            Commands::Vm {
                command:
                    VmCommands::Port {
                        command:
                            PortCommands::Add {
                                vm,
                                host,
                                guest,
                                protocol,
                            },
                    },
            } => {
                assert_eq!(vm, "myvm");
                assert_eq!(host, 8080);
                assert_eq!(guest, 80);
                assert_eq!(protocol, "tcp");
            }
            _ => panic!("expected Vm Port Add command"),
        }
    }

    // -----------------------------------------------------------------------
    // Invalid argument tests
    // -----------------------------------------------------------------------

    #[test]
    fn no_args_produces_error() {
        let result = Cli::try_parse_from(["cratebay"]);
        assert!(result.is_err(), "no subcommand should be an error");
    }

    #[test]
    fn invalid_subcommand_produces_error() {
        let result = Cli::try_parse_from(["cratebay", "nonexistent"]);
        assert!(result.is_err(), "invalid subcommand should be an error");
    }

    #[test]
    fn vm_create_missing_name_produces_error() {
        let result = Cli::try_parse_from(["cratebay", "vm", "create"]);
        assert!(result.is_err(), "vm create without name should error");
    }

    #[test]
    fn docker_run_missing_image_produces_error() {
        let result = Cli::try_parse_from(["cratebay", "docker", "run"]);
        assert!(result.is_err(), "docker run without image should error");
    }

    #[test]
    fn vm_create_invalid_cpus_type_produces_error() {
        let result = Cli::try_parse_from(["cratebay", "vm", "create", "test", "--cpus", "abc"]);
        assert!(result.is_err(), "non-numeric cpus should error");
    }

    #[test]
    fn k3s_missing_subcommand_produces_error() {
        let result = Cli::try_parse_from(["cratebay", "k3s"]);
        assert!(result.is_err(), "k3s without subcommand should error");
    }

    // -----------------------------------------------------------------------
    // Helper function tests
    // -----------------------------------------------------------------------

    #[test]
    fn split_image_reference_with_tag() {
        let (image, tag) = split_image_reference("nginx:1.25");
        assert_eq!(image, "nginx");
        assert_eq!(tag, "1.25");
    }

    #[test]
    fn split_image_reference_without_tag() {
        let (image, tag) = split_image_reference("nginx");
        assert_eq!(image, "nginx");
        assert_eq!(tag, "latest");
    }

    #[test]
    fn split_image_reference_with_registry() {
        let (image, tag) = split_image_reference("ghcr.io/org/image:v1");
        assert_eq!(image, "ghcr.io/org/image");
        assert_eq!(tag, "v1");
    }

    #[test]
    fn parse_registry_reference_valid() {
        let result = parse_registry_reference("ghcr.io/org/image:v1");
        assert!(result.is_some());
        let (registry, repo) = result.unwrap();
        assert_eq!(registry, "ghcr.io");
        assert_eq!(repo, "org/image");
    }

    #[test]
    fn parse_registry_reference_no_registry() {
        let result = parse_registry_reference("nginx:latest");
        assert!(result.is_none(), "plain image name should return None");
    }

    #[test]
    fn truncate_str_short() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn truncate_str_exact() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn truncate_str_long() {
        let result = truncate_str("hello world", 6);
        assert!(result.len() <= 10); // Unicode ellipsis is multi-byte
        assert!(result.ends_with('\u{2026}'));
    }

    #[test]
    fn parse_docker_host_target_detects_tcp() {
        assert_eq!(
            parse_docker_host_target("tcp://127.0.0.1:2475"),
            DockerHostTarget::Tcp("tcp://127.0.0.1:2475".into())
        );
    }

    #[test]
    fn parse_docker_host_target_detects_unix_socket() {
        assert_eq!(
            parse_docker_host_target("unix:///var/run/docker.sock"),
            DockerHostTarget::Unix("/var/run/docker.sock".into())
        );
        assert_eq!(
            parse_docker_host_target("/var/run/docker.sock"),
            DockerHostTarget::Unix("/var/run/docker.sock".into())
        );
    }

    #[test]
    fn parse_docker_host_target_detects_named_pipe() {
        assert_eq!(
            parse_docker_host_target("npipe:////./pipe/docker_engine"),
            DockerHostTarget::NamedPipe("//./pipe/docker_engine".into())
        );
        assert_eq!(
            parse_docker_host_target("//./pipe/docker_engine"),
            DockerHostTarget::NamedPipe("//./pipe/docker_engine".into())
        );
    }

    #[test]
    fn parse_docker_host_target_rejects_unknown_scheme() {
        assert_eq!(
            parse_docker_host_target("ssh://docker@example"),
            DockerHostTarget::Unsupported("ssh://docker@example".into())
        );
    }

    #[test]
    fn docker_pull_platform_uses_engine_os_and_arch() {
        let version = bollard::system::Version {
            os: Some("linux".into()),
            arch: Some("x86_64".into()),
            ..Default::default()
        };

        assert_eq!(
            docker_pull_platform_for_engine(&version).as_deref(),
            Some("linux/amd64")
        );
    }

    #[test]
    fn docker_pull_platform_rejects_unknown_engine_values() {
        let version = bollard::system::Version {
            os: Some("linux".into()),
            arch: Some("sparc".into()),
            ..Default::default()
        };

        assert_eq!(docker_pull_platform_for_engine(&version), None);
    }

    #[test]
    fn normalize_docker_platform_os_aliases_macos() {
        assert_eq!(normalize_docker_platform_os("macos"), Some("darwin"));
    }

    #[test]
    fn normalize_docker_platform_arch_aliases_x86_64() {
        assert_eq!(normalize_docker_platform_arch("x86_64"), Some("amd64"));
    }

    #[test]
    fn format_bytes_values() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
    }
}
