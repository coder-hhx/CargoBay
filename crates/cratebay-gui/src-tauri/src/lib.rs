use bollard::auth::DockerCredentials;
use bollard::container::{
    Config, CreateContainerOptions, InspectContainerOptions, ListContainersOptions, LogOutput,
    LogsOptions, RemoveContainerOptions, StartContainerOptions, StatsOptions, StopContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::image::{
    CommitContainerOptions, CreateImageOptions, ImportImageOptions, ListImagesOptions,
    PushImageOptions, RemoveImageOptions, TagImageOptions,
};
use bollard::service::HostConfig;
use bollard::volume::{CreateVolumeOptions, ListVolumesOptions};
#[cfg_attr(mobile, tauri::mobile_entry_point)]
use bollard::Docker;
use futures_util::stream::TryStreamExt;
use futures_util::StreamExt;
use keyring::Entry;
use reqwest::header::WWW_AUTHENTICATE;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tauri::async_runtime::JoinHandle;
use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, RunEvent, State, WindowEvent};
use tokio_util::codec::{BytesCodec, FramedRead};
use tonic::transport::Channel;
use tracing::{error, info, warn};

use cratebay_core::proto;
use cratebay_core::proto::vm_service_client::VmServiceClient;
use cratebay_core::validation;

mod sandbox;
use sandbox::*;

include!("docker.rs");
include!("vm.rs");
include!("kubernetes.rs");
include!("tray.rs");
include!("ai.rs");
include!("update.rs");

struct McpServerRuntime {
    child: Option<Child>,
    logs: Arc<Mutex<VecDeque<String>>>,
    started_at: Option<String>,
    exit_code: Option<i32>,
}

pub struct AppState {
    hv: Box<dyn cratebay_core::hypervisor::Hypervisor>,
    grpc_addr: String,
    daemon: Mutex<Option<Child>>,
    daemon_ready: Mutex<bool>,
    log_stream_handles: Mutex<HashMap<String, JoinHandle<()>>>,
    mcp_runtimes: Mutex<HashMap<String, McpServerRuntime>>,
}

impl AppState {
    /// Ensure the daemon process is running (lazy start on first VM operation).
    /// Subsequent calls are no-ops once the daemon is confirmed ready.
    async fn ensure_daemon(&self) {
        // Fast-path: already initialised.
        {
            let guard = self.daemon_ready.lock().unwrap_or_else(|e| e.into_inner());
            if *guard {
                return;
            }
        }

        // Check if daemon is already running externally.
        if connect_vm_service(&self.grpc_addr).await.is_ok() {
            info!("CrateBay daemon already running at {}", self.grpc_addr);
            let mut guard = self.daemon_ready.lock().unwrap_or_else(|e| e.into_inner());
            *guard = true;
            return;
        }

        // Spawn it.
        info!(
            "CrateBay daemon not detected at {}, starting it",
            self.grpc_addr
        );
        match spawn_daemon(&self.grpc_addr) {
            Ok(child) => {
                if let Ok(mut dg) = self.daemon.lock() {
                    *dg = Some(child);
                }

                let ready = wait_for_daemon(&self.grpc_addr, Duration::from_secs(5)).await;
                if ready {
                    info!("CrateBay daemon is ready at {}", self.grpc_addr);
                    let mut guard = self.daemon_ready.lock().unwrap_or_else(|e| e.into_inner());
                    *guard = true;
                } else {
                    warn!(
                        "CrateBay daemon did not become ready in time ({}), \
                         falling back to local hypervisor",
                        self.grpc_addr
                    );
                }
            }
            Err(e) => {
                error!("Failed to start CrateBay daemon: {}", e);
            }
        }
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        if let Ok(mut runtimes) = self.mcp_runtimes.lock() {
            for runtime in runtimes.values_mut() {
                if let Some(mut child) = runtime.child.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }

        let Ok(mut guard) = self.daemon.lock() else {
            return;
        };
        let Some(mut child) = guard.take() else {
            return;
        };

        let _ = child.kill();
        let _ = child.wait();
    }
}

fn detect_docker_socket() -> Option<String> {
    // Unix socket detection (macOS / Linux)
    #[cfg(unix)]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        let candidates = [
            cratebay_core::runtime::host_docker_socket_path()
                .to_string_lossy()
                .into_owned(),
            format!("{}/.colima/default/docker.sock", home),
            format!("{}/.orbstack/run/docker.sock", home),
            "/var/run/docker.sock".to_string(),
            format!("{}/.docker/run/docker.sock", home),
        ];
        if let Some(sock) = candidates.into_iter().find(|p| Path::new(p).exists()) {
            return Some(sock);
        }
    }

    None
}

#[cfg(unix)]
fn docker_ping_unix_socket(sock: &Path) -> Result<(), String> {
    use std::io::{Read, Write as _};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let mut stream =
        UnixStream::connect(sock).map_err(|e| format!("connect {}: {}", sock.display(), e))?;

    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));

    stream
        .write_all(b"GET /_ping HTTP/1.1\r\nHost: docker\r\n\r\n")
        .map_err(|e| format!("write _ping: {}", e))?;

    // Read more than a single packet: Docker may include enough headers that
    // the "OK" body isn't within the first 256 bytes.
    let mut out: Vec<u8> = Vec::with_capacity(2048);
    let mut buf = [0u8; 1024];
    for _ in 0..16 {
        let n = match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => return Err(format!("read _ping: {}", e)),
        };
        out.extend_from_slice(&buf[..n]);
        if out.len() >= 8192 {
            break;
        }
        // Fast-path: once we see the end of headers, we likely have the body.
        if out.windows(4).any(|w| w == b"\r\n\r\n") && out.windows(2).any(|w| w == b"OK") {
            break;
        }
    }
    let resp = String::from_utf8_lossy(&out);

    if resp.contains("200 OK") && resp.contains("\r\n\r\nOK") {
        return Ok(());
    }

    // Some daemons respond with just OK without headers (proxy layers).
    if resp.contains("\r\n\r\nOK") || resp.trim_end() == "OK" {
        return Ok(());
    }

    Err(format!(
        "unexpected /_ping response: {}",
        resp.lines().next().unwrap_or_default()
    ))
}

#[cfg(unix)]
fn wait_for_docker_socket_ready(sock: &Path, timeout: Duration) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if sock.exists() && docker_ping_unix_socket(sock).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    Err(format!(
        "Docker socket was not ready within {}s: {}",
        timeout.as_secs(),
        sock.display()
    ))
}

#[cfg(target_os = "macos")]
static MACOS_RUNTIME_CONNECT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(target_os = "macos")]
fn macos_runtime_connect_lock() -> &'static Mutex<()> {
    MACOS_RUNTIME_CONNECT_LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(target_os = "macos")]
fn prime_macos_runtime_assets_env() {
    if std::env::var_os("CRATEBAY_RUNTIME_ASSETS_DIR").is_none() {
        if let Some(dir) = cratebay_core::runtime::bundled_runtime_assets_dir() {
            std::env::set_var("CRATEBAY_RUNTIME_ASSETS_DIR", dir);
        }
    }
}

#[cfg(target_os = "macos")]
fn connect_cratebay_runtime_docker() -> Result<Docker, String> {
    let _guard = macos_runtime_connect_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    if std::env::var_os("CRATEBAY_RUNTIME_ASSETS_DIR").is_none() {
        if let Some(dir) = cratebay_core::runtime::bundled_runtime_assets_dir() {
            std::env::set_var("CRATEBAY_RUNTIME_ASSETS_DIR", dir);
        }
    }

    let cratebay_sock = cratebay_core::runtime::host_docker_socket_path().to_path_buf();
    if cratebay_sock.exists() && docker_ping_unix_socket(&cratebay_sock).is_ok() {
        return Docker::connect_with_socket(
            cratebay_sock
                .to_str()
                .ok_or_else(|| "Docker socket path is not valid UTF-8".to_string())?,
            120,
            bollard::API_DEFAULT_VERSION,
        )
        .map_err(|e| {
            format!(
                "Failed to connect to CrateBay Runtime at {}: {}",
                cratebay_sock.display(),
                e
            )
        });
    }

    let hv = cratebay_core::create_hypervisor();
    let start_attempt = |label: &str| -> Result<String, String> {
        let vm_id = cratebay_core::runtime::ensure_runtime_vm_running(hv.as_ref())
            .map_err(|e| format!("{} failed to start CrateBay Runtime VM: {}", label, e))?;
        wait_for_docker_socket_ready(&cratebay_sock, Duration::from_secs(45)).map_err(|e| {
            format!(
                "{} failed waiting for Docker socket (vm: {}, sock: {}): {}",
                label,
                vm_id,
                cratebay_sock.display(),
                e
            )
        })?;
        Ok(vm_id)
    };

    let start_runtime = || -> Result<String, String> {
        if let Ok(vm_id) = start_attempt("start") {
            return Ok(vm_id);
        }

        let _ = cratebay_core::runtime::stop_runtime_vm_if_exists(hv.as_ref());
        let _ = std::fs::remove_file(&cratebay_sock);
        if let Ok(vm_id) = start_attempt("restart") {
            return Ok(vm_id);
        }

        let _ = cratebay_core::runtime::stop_runtime_vm_if_exists(hv.as_ref());
        let _ = std::fs::remove_file(&cratebay_sock);
        cratebay_core::runtime::reset_runtime_vm(hv.as_ref())
            .map_err(|e| format!("reset failed to recreate CrateBay Runtime VM: {}", e))?;
        start_attempt("reset")
    };

    let _vm_id = start_runtime()?;
    Docker::connect_with_socket(
        cratebay_sock
            .to_str()
            .ok_or_else(|| "Docker socket path is not valid UTF-8".to_string())?,
        120,
        bollard::API_DEFAULT_VERSION,
    )
    .map_err(|e| {
        format!(
            "Failed to connect to CrateBay Runtime at {}: {}",
            cratebay_sock.display(),
            e
        )
    })
}

#[cfg(target_os = "linux")]
fn connect_cratebay_runtime_docker() -> Result<Docker, String> {
    let host = cratebay_core::runtime::ensure_runtime_linux_running()
        .map_err(|e| format!("Failed to start CrateBay Runtime (Linux/QEMU): {}", e))?;
    std::env::set_var("DOCKER_HOST", &host);

    Docker::connect_with_http(&host, 120, bollard::API_DEFAULT_VERSION)
        .map_err(|e| format!("Failed to connect to CrateBay Runtime at {}: {}", host, e))
}

fn connect_docker() -> Result<Docker, String> {
    // Check DOCKER_HOST env first
    if std::env::var("DOCKER_HOST").is_ok() {
        return Docker::connect_with_local_defaults()
            .map_err(|e| format!("Failed to connect via DOCKER_HOST: {}", e));
    }

    #[cfg(unix)]
    {
        if let Some(sock) = detect_docker_socket() {
            let sock_path = Path::new(&sock);
            if docker_ping_unix_socket(sock_path).is_ok() {
                return Docker::connect_with_socket(&sock, 120, bollard::API_DEFAULT_VERSION)
                    .map_err(|e| format!("Failed to connect to Docker at {}: {}", sock, e));
            }

            #[cfg(not(target_os = "macos"))]
            {
                return Err(format!(
                    "Docker socket exists but Docker engine is not responding: {}",
                    sock
                ));
            }
        }

        // No runtime detected; start the built-in runtime when available.
        #[cfg(target_os = "macos")]
        {
            return connect_cratebay_runtime_docker();
        }

        #[cfg(target_os = "linux")]
        {
            return connect_cratebay_runtime_docker();
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        Err("No Docker socket found. Set DOCKER_HOST or start a Docker-compatible runtime.".into())
    }

    #[cfg(windows)]
    {
        let candidates = [
            r"//./pipe/docker_engine",
            r"//./pipe/dockerDesktopLinuxEngine",
        ];
        for pipe in &candidates {
            if let Ok(d) = Docker::connect_with_named_pipe(pipe, 120, bollard::API_DEFAULT_VERSION)
            {
                return Ok(d);
            }
        }
        if let Ok(host) = cratebay_core::runtime::ensure_runtime_wsl_running() {
            // Persist for subsequent calls within this GUI process.
            std::env::set_var("DOCKER_HOST", &host);
            return Docker::connect_with_http(&host, 120, bollard::API_DEFAULT_VERSION)
                .map_err(|e| format!("Failed to connect to CrateBay Runtime at {}: {}", host, e));
        }
        Err(
            "No Docker named pipe found. Set DOCKER_HOST or start a Docker-compatible runtime."
                .into(),
        )
    }

    #[cfg(not(any(unix, windows)))]
    {
        Docker::connect_with_local_defaults()
            .map_err(|e| format!("Failed to connect to Docker: {}", e))
    }
}

fn runtime_setup_path() -> String {
    let raw = std::env::var("PATH").unwrap_or_default();
    let mut items = raw
        .split(':')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    // Some launchers (for example tauri-driver) can start the app with an empty PATH.
    // Ensure basic system directories are present so spawned shells can find coreutils.
    if items.is_empty() {
        items = vec![
            "/usr/local/sbin".to_string(),
            "/usr/local/bin".to_string(),
            "/usr/sbin".to_string(),
            "/usr/bin".to_string(),
            "/sbin".to_string(),
            "/bin".to_string(),
        ];
    }

    for extra in [
        "/usr/local/sbin",
        "/usr/local/bin",
        "/usr/sbin",
        "/usr/bin",
        "/sbin",
        "/bin",
        "/opt/homebrew/bin",
    ] {
        if !items.iter().any(|item| item == extra) {
            items.push(extra.to_string());
        }
    }

    items.join(":")
}

fn runtime_setup_command_exists(command: &str) -> bool {
    let output = Command::new("which")
        .arg(command)
        .env("PATH", runtime_setup_path())
        .output();
    matches!(output, Ok(out) if out.status.success())
}

fn runtime_setup_run(command: &str, args: &[&str]) -> Result<std::process::Output, String> {
    Command::new(command)
        .args(args)
        .env("PATH", runtime_setup_path())
        .output()
        .map_err(|e| format!("Failed to run command '{}': {}", command, e))
}

#[cfg(target_os = "macos")]
fn docker_runtime_quick_setup_macos() -> Result<String, String> {
    if connect_docker().is_ok() {
        return Ok("Docker runtime is already available.".to_string());
    }

    let cratebay_sock = cratebay_core::runtime::host_docker_socket_path().to_path_buf();

    if let Some(sock) = detect_docker_socket() {
        // If there's a non-CrateBay socket present but unreachable, don't override it silently.
        if std::path::Path::new(&sock) != cratebay_sock {
            return Err(format!(
                "Found Docker socket at {}, but it is not reachable. Start your runtime, fix permissions, or set DOCKER_HOST, then refresh.",
                sock
            ));
        }
    }

    connect_cratebay_runtime_docker().map(|_| {
        format!(
            "CrateBay Runtime is running. Docker socket: {}",
            cratebay_sock.display()
        )
    })
}

#[cfg(target_os = "windows")]
fn docker_runtime_quick_setup_windows() -> Result<String, String> {
    if connect_docker().is_ok() {
        return Ok("Docker runtime is already available.".to_string());
    }

    let host = cratebay_core::runtime::ensure_runtime_wsl_running()
        .map_err(|e| format!("Failed to start CrateBay Runtime (WSL2): {}", e))?;
    std::env::set_var("DOCKER_HOST", &host);

    let handle = tokio::runtime::Handle::try_current()
        .map_err(|e| format!("No tokio runtime available for Docker check: {}", e))?;
    let ok = handle.block_on(async {
        let docker = Docker::connect_with_http(&host, 120, bollard::API_DEFAULT_VERSION)
            .map_err(|e| format!("Failed to connect to Docker at {}: {}", host, e))?;

        let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
        loop {
            match docker.version().await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    if tokio::time::Instant::now() >= deadline {
                        return Err(format!(
                            "Docker runtime is not responding yet. Last error: {}",
                            e
                        ));
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    });

    ok.map(|_| format!("CrateBay Runtime is running. DOCKER_HOST: {}", host))
}

#[cfg(target_os = "linux")]
fn docker_runtime_quick_setup_linux() -> Result<String, String> {
    let host = cratebay_core::runtime::ensure_runtime_linux_running()
        .map_err(|e| format!("Failed to start CrateBay Runtime (Linux/QEMU): {}", e))?;
    std::env::set_var("DOCKER_HOST", &host);
    Ok(format!(
        "CrateBay Runtime is running. DOCKER_HOST: {}",
        host
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn docker_runtime_quick_setup_generic() -> Result<String, String> {
    connect_docker()
        .map(|_| "Docker runtime is reachable. Containers and Volumes are ready.".into())
}

fn docker_host_for_cli() -> Option<String> {
    if let Ok(v) = std::env::var("DOCKER_HOST") {
        return Some(v);
    }
    #[cfg(unix)]
    {
        #[cfg(target_os = "linux")]
        {
            if cratebay_core::runtime::runtime_linux_is_running() {
                return Some(cratebay_core::runtime::runtime_linux_docker_host());
            }
        }
        detect_docker_socket().map(|sock| format!("unix://{}", sock))
    }
    #[cfg(windows)]
    {
        None
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

fn grpc_addr() -> String {
    std::env::var("CRATEBAY_GRPC_ADDR").unwrap_or_else(|_| "127.0.0.1:50051".into())
}

fn grpc_endpoint(addr: &str) -> String {
    if addr.starts_with("http://") || addr.starts_with("https://") {
        addr.to_string()
    } else {
        format!("http://{}", addr)
    }
}

async fn connect_vm_service(addr: &str) -> Result<VmServiceClient<Channel>, String> {
    let endpoint = grpc_endpoint(addr);
    let connect_fut = VmServiceClient::connect(endpoint.clone());
    tokio::time::timeout(Duration::from_secs(1), connect_fut)
        .await
        .map_err(|_| format!("Timed out connecting to daemon at {}", endpoint))?
        .map_err(|e| format!("Failed to connect to daemon at {}: {}", endpoint, e))
}

fn daemon_bin_name() -> &'static str {
    if cfg!(windows) {
        "cratebay-daemon.exe"
    } else {
        "cratebay-daemon"
    }
}

fn spawn_daemon(grpc_addr: &str) -> Result<Child, String> {
    let mut tried: Vec<String> = Vec::new();

    if let Ok(path) = std::env::var("CRATEBAY_DAEMON_PATH") {
        let mut cmd = Command::new(&path);
        cmd.env("CRATEBAY_GRPC_ADDR", grpc_addr);
        cmd.stdin(Stdio::null());
        if cfg!(debug_assertions) {
            cmd.stdout(Stdio::inherit());
            cmd.stderr(Stdio::inherit());
        } else {
            cmd.stdout(Stdio::null());
            cmd.stderr(Stdio::null());
        }
        return cmd.spawn().map_err(|e| {
            format!(
                "Failed to spawn daemon from CRATEBAY_DAEMON_PATH ({}): {}",
                path, e
            )
        });
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(daemon_bin_name());
            if candidate.is_file() {
                let mut cmd = Command::new(&candidate);
                cmd.env("CRATEBAY_GRPC_ADDR", grpc_addr);
                cmd.stdin(Stdio::null());
                if cfg!(debug_assertions) {
                    cmd.stdout(Stdio::inherit());
                    cmd.stderr(Stdio::inherit());
                } else {
                    cmd.stdout(Stdio::null());
                    cmd.stderr(Stdio::null());
                }
                return cmd.spawn().map_err(|e| {
                    format!(
                        "Failed to spawn daemon next to GUI binary ({}): {}",
                        candidate.display(),
                        e
                    )
                });
            }
            tried.push(candidate.display().to_string());
        }
    }

    tried.push("cratebay-daemon (PATH)".into());
    let mut cmd = Command::new("cratebay-daemon");
    cmd.env("CRATEBAY_GRPC_ADDR", grpc_addr);
    cmd.stdin(Stdio::null());
    if cfg!(debug_assertions) {
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());
    } else {
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
    }

    cmd.spawn().map_err(|e| {
        format!(
            "Failed to spawn daemon (tried: {}): {}",
            tried.join(", "),
            e
        )
    })
}

async fn wait_for_daemon(grpc_addr: &str, timeout: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if connect_vm_service(grpc_addr).await.is_ok() {
            return true;
        }

        if tokio::time::Instant::now() >= deadline {
            return false;
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[derive(Serialize)]
pub struct ContainerInfo {
    id: String,
    name: String,
    image: String,
    state: String,
    status: String,
    ports: String,
}

fn format_published_ports(mut pairs: Vec<(u16, u16)>) -> String {
    pairs.sort_unstable();
    pairs.dedup();
    pairs
        .into_iter()
        .map(|(public, private)| format!("{}:{}", public, private))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::{detect_vm_ssh_port, format_published_ports, vm_login_cmd, PortForwardDto};

    #[test]
    fn format_published_ports_sorts_by_public_then_private() {
        let out = format_published_ports(vec![(443, 443), (80, 8080), (80, 80), (8080, 80)]);
        assert_eq!(out, "80:80, 80:8080, 443:443, 8080:80");
    }

    #[test]
    fn format_published_ports_empty_is_empty() {
        assert_eq!(format_published_ports(vec![]), "");
    }

    #[test]
    fn detect_vm_ssh_port_prefers_guest_port_22_over_tcp() {
        let port = detect_vm_ssh_port(&[
            PortForwardDto {
                host_port: 9090,
                guest_port: 90,
                protocol: "tcp".into(),
            },
            PortForwardDto {
                host_port: 2228,
                guest_port: 22,
                protocol: "tcp".into(),
            },
        ]);
        assert_eq!(port, Some(2228));
    }

    #[test]
    fn vm_login_cmd_falls_back_to_detected_ssh_forward() {
        let cmd = vm_login_cmd(
            "vm-1".into(),
            "root".into(),
            "127.0.0.1".into(),
            None,
            Some(vec![PortForwardDto {
                host_port: 2228,
                guest_port: 22,
                protocol: "tcp".into(),
            }]),
        )
        .expect("login command");
        assert!(cmd.contains("ssh root@127.0.0.1 -p 2228"));
    }
}

// ---------------------------------------------------------------------------
// VM DTOs and commands
// ---------------------------------------------------------------------------

// ── K3s cluster management commands ──────────────────────────────────

#[cfg(test)]
fn infer_assistant_steps(prompt: &str, require_confirm: bool) -> Vec<AssistantPlanStep> {
    infer_assistant_steps_with_runtime(prompt, require_confirm, true)
}

fn command_needs_docker_runtime(command: &str) -> bool {
    matches!(
        command,
        "list_containers"
            | "start_container"
            | "stop_container"
            | "remove_container"
            | "sandbox_list"
            | "sandbox_create"
            | "sandbox_start"
            | "sandbox_stop"
            | "sandbox_delete"
            | "sandbox_exec"
            | "sandbox_cleanup_expired"
    )
}

fn infer_assistant_steps_with_runtime(
    prompt: &str,
    require_confirm: bool,
    docker_runtime_available: bool,
) -> Vec<AssistantPlanStep> {
    let mut steps: Vec<AssistantPlanStep> = Vec::new();
    let lower = prompt.to_ascii_lowercase();

    if lower.contains("container") || prompt.contains("容器") {
        if lower.contains("stop") || prompt.contains("停止") {
            steps.push(AssistantPlanStep {
                id: "step-1".to_string(),
                title: "Stop container".to_string(),
                command: "stop_container".to_string(),
                args: serde_json::json!({ "id": "<container-id>" }),
                risk_level: "write".to_string(),
                requires_confirmation: require_confirm,
                explain: "Stops a running container.".to_string(),
            });
        } else if lower.contains("delete")
            || lower.contains("remove")
            || prompt.contains("删除")
            || prompt.contains("移除")
        {
            steps.push(AssistantPlanStep {
                id: "step-1".to_string(),
                title: "Remove container".to_string(),
                command: "remove_container".to_string(),
                args: serde_json::json!({ "id": "<container-id>" }),
                risk_level: "destructive".to_string(),
                requires_confirmation: true,
                explain: "Removes a container after stopping it.".to_string(),
            });
        } else if lower.contains("start") || prompt.contains("启动") {
            steps.push(AssistantPlanStep {
                id: "step-1".to_string(),
                title: "Start container".to_string(),
                command: "start_container".to_string(),
                args: serde_json::json!({ "id": "<container-id>" }),
                risk_level: "write".to_string(),
                requires_confirmation: require_confirm,
                explain: "Starts a stopped container.".to_string(),
            });
        } else {
            steps.push(AssistantPlanStep {
                id: "step-1".to_string(),
                title: "List containers".to_string(),
                command: "list_containers".to_string(),
                args: serde_json::json!({}),
                risk_level: "read".to_string(),
                requires_confirmation: false,
                explain: "Lists all containers as context before any action.".to_string(),
            });
        }
    }

    if lower.contains("vm") || prompt.contains("虚拟机") {
        if lower.contains("stop") || prompt.contains("停止") {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "Stop VM".to_string(),
                command: "vm_stop".to_string(),
                args: serde_json::json!({ "id": "<vm-id>" }),
                risk_level: "write".to_string(),
                requires_confirmation: require_confirm,
                explain: "Stops a running VM.".to_string(),
            });
        } else if lower.contains("delete")
            || lower.contains("remove")
            || prompt.contains("删除")
            || prompt.contains("移除")
        {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "Delete VM".to_string(),
                command: "vm_delete".to_string(),
                args: serde_json::json!({ "id": "<vm-id>" }),
                risk_level: "destructive".to_string(),
                requires_confirmation: true,
                explain: "Deletes VM metadata and local state.".to_string(),
            });
        } else if lower.contains("start") || prompt.contains("启动") {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "Start VM".to_string(),
                command: "vm_start".to_string(),
                args: serde_json::json!({ "id": "<vm-id>" }),
                risk_level: "write".to_string(),
                requires_confirmation: require_confirm,
                explain: "Starts a VM.".to_string(),
            });
        } else {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "List VMs".to_string(),
                command: "vm_list".to_string(),
                args: serde_json::json!({}),
                risk_level: "read".to_string(),
                requires_confirmation: false,
                explain: "Lists VMs and current state.".to_string(),
            });
        }
    }

    if lower.contains("k8s")
        || lower.contains("kubernetes")
        || prompt.contains("集群")
        || prompt.contains("pod")
    {
        steps.push(AssistantPlanStep {
            id: format!("step-{}", steps.len() + 1),
            title: "List Kubernetes pods".to_string(),
            command: "k8s_list_pods".to_string(),
            args: serde_json::json!({ "namespace": serde_json::Value::Null }),
            risk_level: "read".to_string(),
            requires_confirmation: false,
            explain: "Queries pod status for diagnosis.".to_string(),
        });
    }

    if lower.contains("ollama") || lower.contains("model") || prompt.contains("模型") {
        if lower.contains("pull")
            || lower.contains("download")
            || prompt.contains("拉取")
            || prompt.contains("下载")
        {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "Pull Ollama model".to_string(),
                command: "ollama_pull_model".to_string(),
                args: serde_json::json!({ "name": "<model-name>" }),
                risk_level: "write".to_string(),
                requires_confirmation: require_confirm,
                explain: "Pulls a local model into Ollama.".to_string(),
            });
        } else if lower.contains("delete")
            || lower.contains("remove")
            || prompt.contains("删除")
            || prompt.contains("移除")
        {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "Delete Ollama model".to_string(),
                command: "ollama_delete_model".to_string(),
                args: serde_json::json!({ "name": "<model-name>" }),
                risk_level: "destructive".to_string(),
                requires_confirmation: true,
                explain: "Removes a local Ollama model.".to_string(),
            });
        } else {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "List Ollama models".to_string(),
                command: "ollama_list_models".to_string(),
                args: serde_json::json!({}),
                risk_level: "read".to_string(),
                requires_confirmation: false,
                explain: "Lists available local models.".to_string(),
            });
        }
    }

    if lower.contains("sandbox") || prompt.contains("沙箱") {
        if lower.contains("cleanup")
            || lower.contains("expire")
            || prompt.contains("清理")
            || prompt.contains("过期")
        {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "Cleanup expired sandboxes".to_string(),
                command: "sandbox_cleanup_expired".to_string(),
                args: serde_json::json!({}),
                risk_level: "write".to_string(),
                requires_confirmation: require_confirm,
                explain: "Reclaims expired managed sandboxes.".to_string(),
            });
        } else if lower.contains("delete")
            || lower.contains("remove")
            || prompt.contains("删除")
            || prompt.contains("移除")
        {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "Delete sandbox".to_string(),
                command: "sandbox_delete".to_string(),
                args: serde_json::json!({ "id": "<sandbox-id>" }),
                risk_level: "destructive".to_string(),
                requires_confirmation: true,
                explain: "Deletes a managed sandbox.".to_string(),
            });
        } else if lower.contains("stop") || prompt.contains("停止") {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "Stop sandbox".to_string(),
                command: "sandbox_stop".to_string(),
                args: serde_json::json!({ "id": "<sandbox-id>" }),
                risk_level: "write".to_string(),
                requires_confirmation: require_confirm,
                explain: "Stops a managed sandbox.".to_string(),
            });
        } else if lower.contains("start") || prompt.contains("启动") {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "Start sandbox".to_string(),
                command: "sandbox_start".to_string(),
                args: serde_json::json!({ "id": "<sandbox-id>" }),
                risk_level: "write".to_string(),
                requires_confirmation: require_confirm,
                explain: "Starts a managed sandbox.".to_string(),
            });
        } else if lower.contains("exec") || lower.contains("run") || prompt.contains("执行") {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "Execute command in sandbox".to_string(),
                command: "sandbox_exec".to_string(),
                args: serde_json::json!({ "id": "<sandbox-id>", "command": "<shell-command>" }),
                risk_level: "write".to_string(),
                requires_confirmation: require_confirm,
                explain: "Runs a command inside a managed sandbox.".to_string(),
            });
        } else {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "List sandboxes".to_string(),
                command: "sandbox_list".to_string(),
                args: serde_json::json!({}),
                risk_level: "read".to_string(),
                requires_confirmation: false,
                explain: "Lists managed sandboxes and current state.".to_string(),
            });
        }
    }

    if lower.contains("mcp") {
        if lower.contains("stop") || prompt.contains("停止") {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "Stop MCP server".to_string(),
                command: "mcp_stop_server".to_string(),
                args: serde_json::json!({ "id": "<server-id>" }),
                risk_level: "write".to_string(),
                requires_confirmation: require_confirm,
                explain: "Stops a managed MCP server process.".to_string(),
            });
        } else if lower.contains("start") || prompt.contains("启动") {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "Start MCP server".to_string(),
                command: "mcp_start_server".to_string(),
                args: serde_json::json!({ "id": "<server-id>" }),
                risk_level: "write".to_string(),
                requires_confirmation: require_confirm,
                explain: "Starts a managed MCP server process.".to_string(),
            });
        } else if lower.contains("export") || prompt.contains("导出") {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "Export MCP client config".to_string(),
                command: "mcp_export_client_config".to_string(),
                args: serde_json::json!({ "client": "codex" }),
                risk_level: "read".to_string(),
                requires_confirmation: false,
                explain: "Exports MCP client configuration for the selected client.".to_string(),
            });
        } else {
            steps.push(AssistantPlanStep {
                id: format!("step-{}", steps.len() + 1),
                title: "List MCP servers".to_string(),
                command: "mcp_list_servers".to_string(),
                args: serde_json::json!({}),
                risk_level: "read".to_string(),
                requires_confirmation: false,
                explain: "Lists MCP registry entries and runtime states.".to_string(),
            });
        }
    }

    if steps.is_empty() {
        steps.push(AssistantPlanStep {
            id: "step-1".to_string(),
            title: "List containers".to_string(),
            command: "list_containers".to_string(),
            args: serde_json::json!({}),
            risk_level: "read".to_string(),
            requires_confirmation: false,
            explain: "Fallback baseline step when intent is ambiguous.".to_string(),
        });
        steps.push(AssistantPlanStep {
            id: "step-2".to_string(),
            title: "List VMs".to_string(),
            command: "vm_list".to_string(),
            args: serde_json::json!({}),
            risk_level: "read".to_string(),
            requires_confirmation: false,
            explain: "Collects VM context before deciding next operation.".to_string(),
        });
    }

    if !docker_runtime_available
        && steps
            .iter()
            .any(|step| command_needs_docker_runtime(step.command.as_str()))
    {
        steps.insert(
            0,
            AssistantPlanStep {
                id: "step-1".to_string(),
                title: "Repair container runtime".to_string(),
                command: "docker_runtime_quick_setup".to_string(),
                args: serde_json::json!({}),
                risk_level: "write".to_string(),
                requires_confirmation: false,
                explain:
                    "Auto-analyzes Docker runtime prerequisites and attempts a local repair flow."
                        .to_string(),
            },
        );
    }

    for (index, step) in steps.iter_mut().enumerate() {
        step.id = format!("step-{}", index + 1);
    }

    steps
}

#[tauri::command]
async fn ai_generate_plan(
    prompt: String,
    profile_id: Option<String>,
    prefer_model: Option<bool>,
) -> Result<AssistantPlanResult, String> {
    if prompt.trim().is_empty() {
        return Err("Prompt cannot be empty".to_string());
    }

    let request_id = next_ai_request_id();
    let settings = load_ai_settings()?;
    let require_confirm = settings.security_policy.destructive_action_confirmation;
    let docker_runtime_available = connect_docker().is_ok();
    let steps = infer_assistant_steps_with_runtime(
        prompt.trim(),
        require_confirm,
        docker_runtime_available,
    );

    let mut notes = "Plan generated from built-in action rules.".to_string();
    let mut strategy = "heuristic".to_string();
    let mut fallback_used = true;

    if prefer_model.unwrap_or(true) {
        if let Ok(profile) = resolve_ai_profile(&settings, profile_id.as_deref()) {
            let hint_messages = vec![
                AiChatMessage {
                    role: "system".to_string(),
                    content: "You are an infra assistant. Summarize action intent in one sentence."
                        .to_string(),
                },
                AiChatMessage {
                    role: "user".to_string(),
                    content: prompt.clone(),
                },
            ];
            if let Ok(summary) = ai_chat_inner(&profile, &hint_messages, 12_000, &request_id).await
            {
                if !summary.text.trim().is_empty() {
                    notes = summary.text.trim().to_string();
                    strategy = "llm+heuristic".to_string();
                    fallback_used = false;
                }
            }
        }
    }

    ai_audit_log(
        "ai_generate_plan",
        "read",
        &request_id,
        &format!("prompt_len={} steps={}", prompt.len(), steps.len()),
    );

    Ok(AssistantPlanResult {
        request_id,
        strategy,
        notes,
        fallback_used,
        steps,
    })
}

#[tauri::command]
async fn assistant_execute_step(
    state: State<'_, AppState>,
    command: String,
    args: serde_json::Value,
    risk_level: Option<String>,
    requires_confirmation: Option<bool>,
    confirmed: Option<bool>,
) -> Result<AssistantStepExecutionResult, String> {
    let settings = load_ai_settings()?;
    let request_id = next_ai_request_id();
    let command = command.trim().to_string();
    let Some(policy) = assistant_command_policy(&command) else {
        let msg = format!("Assistant command '{}' is not allowed", command);
        ai_audit_log("assistant_execute_step", "deny", &request_id, &msg);
        return Err(msg);
    };

    if let Some(client_risk) = risk_level.as_deref() {
        if client_risk != policy.risk_level {
            let msg = format!(
                "Assistant risk level mismatch for '{}': client='{}' server='{}'",
                command, client_risk, policy.risk_level
            );
            ai_audit_log("assistant_execute_step", "deny", &request_id, &msg);
            return Err(msg);
        }
    }

    let needs_confirmation = policy.always_confirm
        || (settings.security_policy.destructive_action_confirmation
            && requires_confirmation.unwrap_or(false));
    if needs_confirmation && !confirmed.unwrap_or(false) {
        let msg = format!(
            "Assistant command '{}' requires explicit confirmation",
            command
        );
        ai_audit_log("assistant_execute_step", "deny", &request_id, &msg);
        return Err(msg);
    }

    let arg_keys = args
        .as_object()
        .map(|obj| obj.keys().cloned().collect::<Vec<_>>().join(","))
        .unwrap_or_default();
    let args_map = assistant_arg_map(&args)?;

    let output = match command.as_str() {
        "list_containers" => {
            let items = list_containers().await?;
            serde_json::to_value(items).map_err(|e| e.to_string())?
        }
        "ollama_list_models" => {
            let items = ollama_list_models().await?;
            serde_json::to_value(items).map_err(|e| e.to_string())?
        }
        "mcp_list_servers" => {
            let items = mcp_list_servers(state.clone())?;
            serde_json::to_value(items).map_err(|e| e.to_string())?
        }
        "mcp_export_client_config" => {
            let client = assistant_arg_optional_string(args_map, "client")?
                .unwrap_or_else(|| "codex".to_string());
            serde_json::to_value(mcp_export_client_config(client)?).map_err(|e| e.to_string())?
        }
        "sandbox_list" => {
            let items = sandbox_list().await?;
            serde_json::to_value(items).map_err(|e| e.to_string())?
        }
        "start_container" => {
            let id = assistant_arg_string(args_map, "id")?;
            start_container(id).await?;
            serde_json::json!({ "ok": true })
        }
        "ollama_pull_model" => {
            let name = assistant_arg_string(args_map, "name")?;
            serde_json::to_value(ollama_pull_model(name).await?).map_err(|e| e.to_string())?
        }
        "mcp_start_server" => {
            let id = assistant_arg_string(args_map, "id")?;
            serde_json::to_value(mcp_start_server(state.clone(), id).await?)
                .map_err(|e| e.to_string())?
        }
        "mcp_stop_server" => {
            let id = assistant_arg_string(args_map, "id")?;
            serde_json::to_value(mcp_stop_server(state.clone(), id).await?)
                .map_err(|e| e.to_string())?
        }
        "sandbox_start" => {
            let id = assistant_arg_string(args_map, "id")?;
            sandbox_start(id).await?;
            serde_json::json!({ "ok": true })
        }
        "sandbox_stop" => {
            let id = assistant_arg_string(args_map, "id")?;
            sandbox_stop(id).await?;
            serde_json::json!({ "ok": true })
        }
        "sandbox_cleanup_expired" => {
            serde_json::to_value(sandbox_cleanup_expired().await?).map_err(|e| e.to_string())?
        }
        "sandbox_exec" => {
            let id = assistant_arg_string(args_map, "id")?;
            let command = assistant_arg_string(args_map, "command")?;
            let timeout_sec = args_map.get("timeout_sec").and_then(|v| v.as_u64());
            serde_json::to_value(sandbox_exec(id, command, timeout_sec).await?)
                .map_err(|e| e.to_string())?
        }
        "stop_container" => {
            let id = assistant_arg_string(args_map, "id")?;
            stop_container(id).await?;
            serde_json::json!({ "ok": true })
        }
        "remove_container" => {
            let id = assistant_arg_string(args_map, "id")?;
            remove_container(id).await?;
            serde_json::json!({ "ok": true })
        }
        "ollama_delete_model" => {
            let name = assistant_arg_string(args_map, "name")?;
            serde_json::to_value(ollama_delete_model(name).await?).map_err(|e| e.to_string())?
        }
        "vm_list" => {
            let items = vm_list_inner(state.inner()).await?;
            serde_json::to_value(items).map_err(|e| e.to_string())?
        }
        "vm_start" => {
            let id = assistant_arg_string(args_map, "id")?;
            vm_start_inner(state.inner(), id).await?;
            serde_json::json!({ "ok": true })
        }
        "vm_stop" => {
            let id = assistant_arg_string(args_map, "id")?;
            vm_stop_inner(state.inner(), id).await?;
            serde_json::json!({ "ok": true })
        }
        "vm_delete" => {
            let id = assistant_arg_string(args_map, "id")?;
            vm_delete_inner(state.inner(), id).await?;
            serde_json::json!({ "ok": true })
        }
        "sandbox_delete" => {
            let id = assistant_arg_string(args_map, "id")?;
            sandbox_delete(id).await?;
            serde_json::json!({ "ok": true })
        }
        "k8s_list_pods" => {
            let namespace = assistant_arg_optional_string(args_map, "namespace")?
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            let pods = k8s_list_pods(namespace).await?;
            serde_json::to_value(pods).map_err(|e| e.to_string())?
        }
        "docker_runtime_quick_setup" => {
            let result = docker_runtime_quick_setup().await?;
            serde_json::to_value(result).map_err(|e| e.to_string())?
        }
        _ => unreachable!("assistant command policy should reject unknown commands"),
    };

    ai_audit_log(
        "assistant_execute_step",
        "write",
        &request_id,
        &format!(
            "command={} risk={} args_keys=[{}] confirm={}",
            command,
            policy.risk_level,
            arg_keys,
            confirmed.unwrap_or(false)
        ),
    );

    Ok(AssistantStepExecutionResult {
        ok: true,
        request_id,
        command,
        risk_level: policy.risk_level.to_string(),
        output,
    })
}

fn resolve_ai_skill(settings: &AiSettings, skill_id: &str) -> Result<AiSkillDefinition, String> {
    settings
        .skills
        .iter()
        .find(|skill| skill.id == skill_id)
        .cloned()
        .ok_or_else(|| format!("Skill not found: {}", skill_id))
}

fn skill_prompt_input(input: &serde_json::Value) -> Option<String> {
    if let Some(prompt) = input.as_str() {
        let prompt = prompt.trim();
        if !prompt.is_empty() {
            return Some(prompt.to_string());
        }
    }

    input
        .as_object()
        .and_then(|obj| obj.get("prompt"))
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn skill_input_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn normalize_skill_input(skill: &AiSkillDefinition, input: serde_json::Value) -> serde_json::Value {
    if skill.executor != "agent_cli_preset" {
        return input;
    }

    match input {
        serde_json::Value::String(prompt) => {
            serde_json::json!({ "prompt": prompt.trim() })
        }
        serde_json::Value::Object(mut map) => {
            if let Some(prompt) = map.get("prompt").and_then(|value| value.as_str()) {
                map.insert(
                    "prompt".to_string(),
                    serde_json::Value::String(prompt.trim().to_string()),
                );
            }
            serde_json::Value::Object(map)
        }
        other => other,
    }
}

fn validate_skill_input(
    path: &str,
    value: &serde_json::Value,
    schema: &serde_json::Value,
) -> Result<(), String> {
    let Some(schema_obj) = schema.as_object() else {
        return Ok(());
    };
    if schema_obj.is_empty() {
        return Ok(());
    }

    if let Some(expected_type) = schema_obj.get("type").and_then(|value| value.as_str()) {
        match expected_type {
            "object" => {
                let object = value.as_object().ok_or_else(|| {
                    format!(
                        "{} must be an object, got {}",
                        path,
                        skill_input_kind(value)
                    )
                })?;
                let properties = schema_obj
                    .get("properties")
                    .and_then(|value| value.as_object());

                if let Some(required) = schema_obj
                    .get("required")
                    .and_then(|value| value.as_array())
                {
                    for item in required {
                        let Some(key) = item.as_str() else {
                            continue;
                        };
                        if !object.contains_key(key) {
                            return Err(format!("{}.{} is required", path, key));
                        }
                    }
                }

                let allow_additional = schema_obj
                    .get("additionalProperties")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true);
                if !allow_additional {
                    for key in object.keys() {
                        if !properties
                            .map(|items| items.contains_key(key))
                            .unwrap_or(false)
                        {
                            return Err(format!("{}.{} is not allowed", path, key));
                        }
                    }
                }

                if let Some(properties) = properties {
                    for (key, property_schema) in properties {
                        if let Some(property_value) = object.get(key) {
                            validate_skill_input(
                                &format!("{}.{}", path, key),
                                property_value,
                                property_schema,
                            )?;
                        }
                    }
                }
            }
            "string" => {
                let text = value.as_str().ok_or_else(|| {
                    format!("{} must be a string, got {}", path, skill_input_kind(value))
                })?;
                if let Some(min_length) =
                    schema_obj.get("minLength").and_then(|value| value.as_u64())
                {
                    if text.chars().count() < min_length as usize {
                        return Err(format!(
                            "{} must be at least {} characters",
                            path, min_length
                        ));
                    }
                }
            }
            "boolean" => {
                if !value.is_boolean() {
                    return Err(format!(
                        "{} must be a boolean, got {}",
                        path,
                        skill_input_kind(value)
                    ));
                }
            }
            "number" => {
                if !value.is_number() {
                    return Err(format!(
                        "{} must be a number, got {}",
                        path,
                        skill_input_kind(value)
                    ));
                }
            }
            "integer" => match value {
                serde_json::Value::Number(number) if number.is_i64() || number.is_u64() => {}
                _ => {
                    return Err(format!(
                        "{} must be an integer, got {}",
                        path,
                        skill_input_kind(value)
                    ));
                }
            },
            "array" => {
                let items = value.as_array().ok_or_else(|| {
                    format!("{} must be an array, got {}", path, skill_input_kind(value))
                })?;
                if let Some(min_items) = schema_obj.get("minItems").and_then(|value| value.as_u64())
                {
                    if items.len() < min_items as usize {
                        return Err(format!(
                            "{} must contain at least {} items",
                            path, min_items
                        ));
                    }
                }
                if let Some(item_schema) = schema_obj.get("items") {
                    for (index, item) in items.iter().enumerate() {
                        validate_skill_input(&format!("{}[{}]", path, index), item, item_schema)?;
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(allowed_values) = schema_obj.get("enum").and_then(|value| value.as_array()) {
        if !allowed_values.iter().any(|candidate| candidate == value) {
            return Err(format!("{} must match one of the allowed values", path));
        }
    }

    Ok(())
}

#[tauri::command]
async fn ai_skill_execute(
    state: State<'_, AppState>,
    skill_id: String,
    input: Option<serde_json::Value>,
    dry_run: Option<bool>,
    confirmed: Option<bool>,
) -> Result<AiSkillExecutionResult, String> {
    let settings = load_ai_settings()?;
    let skill = resolve_ai_skill(&settings, skill_id.trim())?;
    if !skill.enabled {
        return Err(format!("Skill '{}' is disabled", skill.id));
    }

    let input_value = normalize_skill_input(&skill, input.unwrap_or_else(|| serde_json::json!({})));
    validate_skill_input("input", &input_value, &skill.input_schema)?;

    let (request_id, output) = match skill.executor.as_str() {
        "assistant_step" | "sandbox_action" => {
            let confirmation_hint = assistant_command_policy(&skill.target)
                .map(|policy| policy.risk_level != "read")
                .unwrap_or(false);
            let result = assistant_execute_step(
                state.clone(),
                skill.target.clone(),
                input_value,
                None,
                Some(confirmation_hint),
                confirmed,
            )
            .await?;
            let request_id = result.request_id.clone();
            (
                request_id,
                serde_json::to_value(result).map_err(|e| e.to_string())?,
            )
        }
        "mcp_action" => {
            let access = mcp_check_access(
                skill.target.clone(),
                None,
                Some(mcp_action_policy(&skill.target).requires_confirmation),
                confirmed,
            )?;
            if !access.allowed {
                return Err(access.message);
            }
            let result = assistant_execute_step(
                state.clone(),
                skill.target.clone(),
                input_value,
                Some(access.risk_level),
                Some(access.requires_confirmation),
                confirmed,
            )
            .await?;
            let request_id = result.request_id.clone();
            (
                request_id,
                serde_json::to_value(result).map_err(|e| e.to_string())?,
            )
        }
        "agent_cli_preset" => {
            let result = agent_cli_run(
                Some(skill.target.clone()),
                None,
                None,
                skill_prompt_input(&input_value),
                dry_run.unwrap_or(false),
                None,
            )
            .await?;
            let request_id = result.request_id.clone();
            (
                request_id,
                serde_json::to_value(result).map_err(|e| e.to_string())?,
            )
        }
        other => return Err(format!("Unsupported skill executor '{}'", other)),
    };

    ai_audit_log(
        "ai_skill_execute",
        "write",
        &request_id,
        &format!(
            "skill_id={} executor={} target={} dry_run={}",
            skill.id,
            skill.executor,
            skill.target,
            dry_run.unwrap_or(false)
        ),
    );

    Ok(AiSkillExecutionResult {
        ok: true,
        skill_id: skill.id,
        executor: skill.executor,
        target: skill.target,
        request_id,
        output,
    })
}

#[tauri::command]
fn mcp_check_access(
    action: String,
    token: Option<String>,
    requires_confirmation: Option<bool>,
    confirmed: Option<bool>,
) -> Result<McpAccessCheckResult, String> {
    let settings = load_ai_settings()?;
    let policy = settings.security_policy;
    let request_id = next_ai_request_id();
    let action_policy = mcp_action_policy(&action);
    let confirmation_required =
        action_policy.requires_confirmation && policy.destructive_action_confirmation;

    if !policy.mcp_remote_enabled {
        let msg = "MCP remote is disabled by policy".to_string();
        if policy.mcp_audit_enabled {
            ai_audit_log("mcp_check_access", "deny", &request_id, &msg);
        }
        return Ok(McpAccessCheckResult {
            allowed: false,
            request_id,
            message: msg,
            risk_level: action_policy.risk_level.to_string(),
            requires_confirmation: confirmation_required,
        });
    }

    if !policy.mcp_allowed_actions.is_empty()
        && !policy
            .mcp_allowed_actions
            .iter()
            .any(|item| item == &action)
    {
        let msg = format!("Action '{}' is not in MCP whitelist", action);
        if policy.mcp_audit_enabled {
            ai_audit_log("mcp_check_access", "deny", &request_id, &msg);
        }
        return Ok(McpAccessCheckResult {
            allowed: false,
            request_id,
            message: msg,
            risk_level: action_policy.risk_level.to_string(),
            requires_confirmation: confirmation_required,
        });
    }

    if !policy.mcp_auth_token_ref.trim().is_empty() {
        let expected = secret_get(policy.mcp_auth_token_ref.trim())?;
        if let Some(expected_token) = expected {
            if token.as_deref().unwrap_or("") != expected_token {
                let msg = "MCP token verification failed".to_string();
                if policy.mcp_audit_enabled {
                    ai_audit_log("mcp_check_access", "deny", &request_id, &msg);
                }
                return Ok(McpAccessCheckResult {
                    allowed: false,
                    request_id,
                    message: msg,
                    risk_level: action_policy.risk_level.to_string(),
                    requires_confirmation: confirmation_required,
                });
            }
        }
    }

    if !mcp_confirmation_satisfied(
        action_policy,
        policy.destructive_action_confirmation,
        requires_confirmation,
        confirmed,
    ) {
        let msg = format!(
            "MCP action '{}' (risk={}) requires explicit confirmation",
            action, action_policy.risk_level
        );
        if policy.mcp_audit_enabled {
            ai_audit_log("mcp_check_access", "deny", &request_id, &msg);
        }
        return Ok(McpAccessCheckResult {
            allowed: false,
            request_id,
            message: msg,
            risk_level: action_policy.risk_level.to_string(),
            requires_confirmation: confirmation_required,
        });
    }

    if policy.mcp_audit_enabled {
        ai_audit_log(
            "mcp_check_access",
            "allow",
            &request_id,
            &format!(
                "action={} allowed=true risk={} confirm_required={} confirmed={}",
                action,
                action_policy.risk_level,
                confirmation_required,
                confirmed.unwrap_or(false)
            ),
        );
    }

    Ok(McpAccessCheckResult {
        allowed: true,
        request_id,
        message: "MCP access granted".to_string(),
        risk_level: action_policy.risk_level.to_string(),
        requires_confirmation: confirmation_required,
    })
}

#[tauri::command]
async fn docker_runtime_quick_setup() -> Result<DockerRuntimeSetupResult, String> {
    let request_id = next_ai_request_id();
    if connect_docker().is_ok() {
        return Ok(DockerRuntimeSetupResult {
            ok: true,
            request_id,
            message: "Docker runtime is already available.".to_string(),
        });
    }

    #[cfg(target_os = "macos")]
    let setup_result = tokio::task::spawn_blocking(docker_runtime_quick_setup_macos)
        .await
        .map_err(|e| format!("Runtime setup task failed: {}", e))?;

    #[cfg(target_os = "windows")]
    let setup_result = tokio::task::spawn_blocking(docker_runtime_quick_setup_windows)
        .await
        .map_err(|e| format!("Runtime setup task failed: {}", e))?;

    #[cfg(target_os = "linux")]
    let setup_result = tokio::task::spawn_blocking(docker_runtime_quick_setup_linux)
        .await
        .map_err(|e| format!("Runtime setup task failed: {}", e))?;

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    let setup_result = tokio::task::spawn_blocking(docker_runtime_quick_setup_generic)
        .await
        .map_err(|e| format!("Runtime setup task failed: {}", e))?;

    let (ok, message) = match setup_result {
        Ok(msg) => (true, msg),
        Err(msg) => (false, msg),
    };

    ai_audit_log(
        "docker_runtime_quick_setup",
        if ok { "write" } else { "error" },
        &request_id,
        &format!("ok={} message={}", ok, message),
    );

    Ok(DockerRuntimeSetupResult {
        ok,
        request_id,
        message,
    })
}

fn default_agent_cli_presets() -> Vec<AgentCliPreset> {
    vec![
        AgentCliPreset {
            id: "codex".to_string(),
            name: "OpenAI Codex CLI".to_string(),
            description: "Run codex in non-interactive mode".to_string(),
            command: "codex".to_string(),
            args_template: vec!["exec".to_string(), "{{prompt}}".to_string()],
            timeout_sec: 180,
            dangerous: false,
        },
        AgentCliPreset {
            id: "claude".to_string(),
            name: "Claude Code CLI".to_string(),
            description: "Invoke claude with prompt text".to_string(),
            command: "claude".to_string(),
            args_template: vec!["--print".to_string(), "{{prompt}}".to_string()],
            timeout_sec: 180,
            dangerous: false,
        },
        AgentCliPreset {
            id: "openclaw".to_string(),
            name: "OpenClaw CLI".to_string(),
            description: "Invoke openclaw cli prompt mode".to_string(),
            command: "openclaw".to_string(),
            args_template: vec![
                "run".to_string(),
                "--prompt".to_string(),
                "{{prompt}}".to_string(),
            ],
            timeout_sec: 180,
            dangerous: false,
        },
        AgentCliPreset {
            id: "gemini".to_string(),
            name: "Gemini CLI".to_string(),
            description: "Invoke gemini cli prompt mode".to_string(),
            command: "gemini".to_string(),
            args_template: vec!["--prompt".to_string(), "{{prompt}}".to_string()],
            timeout_sec: 180,
            dangerous: false,
        },
        AgentCliPreset {
            id: "qwen".to_string(),
            name: "Qwen CLI".to_string(),
            description: "Invoke qwen command line client".to_string(),
            command: "qwen".to_string(),
            args_template: vec!["--prompt".to_string(), "{{prompt}}".to_string()],
            timeout_sec: 180,
            dangerous: false,
        },
    ]
}

#[tauri::command]
fn agent_cli_list_presets() -> Vec<AgentCliPreset> {
    default_agent_cli_presets()
}

fn build_command_line(command: &str, args: &[String]) -> String {
    if args.is_empty() {
        command.to_string()
    } else {
        format!("{} {}", command, args.join(" "))
    }
}

fn is_command_allowed(allowlist: &[String], command: &str) -> bool {
    if allowlist.is_empty() {
        return true;
    }
    let command_name = std::path::Path::new(command)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(command);
    allowlist.iter().any(|item| item == command_name)
}

#[tauri::command]
async fn agent_cli_run(
    preset_id: Option<String>,
    command: Option<String>,
    args: Option<Vec<String>>,
    prompt: Option<String>,
    dry_run: bool,
    timeout_sec: Option<u64>,
) -> Result<AgentCliRunResult, String> {
    let settings = load_ai_settings()?;
    let request_id = next_ai_request_id();

    let presets = default_agent_cli_presets();
    let (resolved_command, resolved_args, preset_timeout, dangerous) =
        if let Some(preset_id) = preset_id {
            let preset = presets
                .into_iter()
                .find(|item| item.id == preset_id)
                .ok_or_else(|| format!("Unknown preset: {}", preset_id))?;
            let prompt_text = prompt.unwrap_or_default();
            let args = preset
                .args_template
                .iter()
                .map(|arg| arg.replace("{{prompt}}", &prompt_text))
                .collect::<Vec<_>>();
            (preset.command, args, preset.timeout_sec, preset.dangerous)
        } else {
            let cmd =
                command.ok_or_else(|| "command is required when preset_id is empty".to_string())?;
            (cmd, args.unwrap_or_default(), 180, false)
        };

    if !is_command_allowed(
        &settings.security_policy.cli_command_allowlist,
        &resolved_command,
    ) {
        let detail = format!("command '{}' blocked by CLI allowlist", resolved_command);
        ai_audit_log("agent_cli_run", "deny", &request_id, &detail);
        return Err(detail);
    }

    if dangerous && settings.security_policy.destructive_action_confirmation {
        ai_audit_log(
            "agent_cli_run",
            "deny",
            &request_id,
            "dangerous preset blocked by destructive_action_confirmation",
        );
        return Err(
            "Dangerous preset blocked by policy (disable confirmation policy to proceed)"
                .to_string(),
        );
    }

    let command_line = build_command_line(&resolved_command, &resolved_args);
    if dry_run {
        ai_audit_log(
            "agent_cli_run",
            "read",
            &request_id,
            &format!(
                "dry_run=true command={} args_count={}",
                resolved_command,
                resolved_args.len()
            ),
        );
        return Ok(AgentCliRunResult {
            ok: true,
            request_id,
            command_line,
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 0,
        });
    }

    let timeout = timeout_sec.unwrap_or(preset_timeout).max(1);
    let start = std::time::Instant::now();

    let mut command_builder = tokio::process::Command::new(&resolved_command);
    command_builder
        .args(&resolved_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = command_builder
        .spawn()
        .map_err(|e| format!("Failed to spawn command '{}': {}", resolved_command, e))?;

    let output = match tokio::time::timeout(Duration::from_secs(timeout), child.wait_with_output())
        .await
    {
        Ok(v) => v.map_err(|e| format!("Failed to wait command '{}': {}", resolved_command, e))?,
        Err(_) => {
            ai_audit_log(
                "agent_cli_run",
                "error",
                &request_id,
                &format!(
                    "command timeout={}s command={} args_count={}",
                    timeout,
                    resolved_command,
                    resolved_args.len()
                ),
            );
            return Err(format!("Command timed out after {} seconds", timeout));
        }
    };

    let duration_ms = start.elapsed().as_millis();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);
    let ok = output.status.success();
    ai_audit_log(
        "agent_cli_run",
        if ok { "write" } else { "error" },
        &request_id,
        &format!(
            "command={} args_count={} exit_code={} duration_ms={}",
            resolved_command,
            resolved_args.len(),
            exit_code,
            duration_ms
        ),
    );

    Ok(AgentCliRunResult {
        ok,
        request_id,
        command_line,
        exit_code,
        stdout,
        stderr,
        duration_ms,
    })
}

#[cfg(test)]
mod ai_tests {
    use super::{
        assistant_arg_optional_string, assistant_arg_string, assistant_command_policy,
        default_ai_settings, infer_assistant_steps, infer_assistant_steps_with_runtime,
        is_command_allowed, mcp_action_policy, mcp_confirmation_satisfied, normalize_ai_settings,
        normalize_skill_input, redact_sensitive, resolve_ai_skill, skill_prompt_input,
        validate_skill_input,
    };

    #[test]
    fn infer_plan_marks_destructive_actions() {
        let steps = infer_assistant_steps("delete container web", true);
        assert!(!steps.is_empty());
        assert_eq!(steps[0].command, "remove_container");
        assert_eq!(steps[0].risk_level, "destructive");
        assert!(steps[0].requires_confirmation);
    }

    #[test]
    fn infer_plan_fallback_has_two_read_steps() {
        let steps = infer_assistant_steps("show me infra context", true);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].risk_level, "read");
        assert_eq!(steps[1].risk_level, "read");
    }

    #[test]
    fn redact_sensitive_rewrites_auth_markers() {
        let input = "Authorization: Bearer sk-test-abc";
        let output = redact_sensitive(input.to_string());
        assert!(output.contains("Authorization[redacted]"));
        assert!(output.contains("Bearer [redacted]"));
    }

    #[test]
    fn assistant_policy_limits_command_surface() {
        let destructive = assistant_command_policy("vm_delete").expect("policy should exist");
        assert_eq!(destructive.risk_level, "destructive");
        assert!(destructive.always_confirm);

        let repair =
            assistant_command_policy("docker_runtime_quick_setup").expect("policy should exist");
        assert_eq!(repair.risk_level, "write");
        assert!(!repair.always_confirm);

        let read = assistant_command_policy("list_containers").expect("policy should exist");
        assert_eq!(read.risk_level, "read");
        assert!(!read.always_confirm);

        assert!(assistant_command_policy("docker_run").is_none());
    }

    #[test]
    fn assistant_arg_helpers_parse_required_and_optional_values() {
        let args = serde_json::json!({
            "id": "vm-1",
            "namespace": "default",
            "nullable": null
        });
        let map = args.as_object().expect("object");
        assert_eq!(
            assistant_arg_string(map, "id").expect("id"),
            "vm-1".to_string()
        );
        assert_eq!(
            assistant_arg_optional_string(map, "namespace").expect("namespace"),
            Some("default".to_string())
        );
        assert_eq!(
            assistant_arg_optional_string(map, "nullable").expect("nullable"),
            None
        );
    }

    #[test]
    fn cli_allowlist_matches_binary_file_name() {
        let allowlist = vec!["openclaw".to_string(), "codex".to_string()];
        assert!(is_command_allowed(&allowlist, "/usr/local/bin/openclaw"));
        assert!(!is_command_allowed(&allowlist, "/usr/local/bin/bash"));
    }

    #[test]
    fn default_ai_settings_exposes_skill_scaffold_entries() {
        let settings = default_ai_settings();
        assert!(!settings.skills.is_empty());
        assert!(settings
            .skills
            .iter()
            .any(|skill| skill.id == "managed-sandbox-list"));
        assert!(settings
            .skills
            .iter()
            .any(|skill| skill.id == "managed-sandbox-command"));
        assert!(settings
            .skills
            .iter()
            .any(|skill| skill.id == "agent-cli-codex-prompt"));
        assert!(settings
            .skills
            .iter()
            .any(|skill| skill.id == "agent-cli-claude-prompt"));
        assert!(settings
            .skills
            .iter()
            .any(|skill| skill.id == "agent-cli-openclaw-plan"));
    }

    #[test]
    fn normalize_ai_settings_rebuilds_skills_when_empty() {
        let mut settings = default_ai_settings();
        settings.skills.clear();
        let normalized = normalize_ai_settings(settings);
        assert!(!normalized.skills.is_empty());
    }

    #[test]
    fn normalize_ai_settings_appends_new_default_skills() {
        let mut settings = default_ai_settings();
        settings
            .skills
            .retain(|skill| skill.id == "assistant-container-diagnose");
        let normalized = normalize_ai_settings(settings);
        assert!(normalized
            .skills
            .iter()
            .any(|skill| skill.id == "managed-sandbox-list"));
        assert!(normalized
            .skills
            .iter()
            .any(|skill| skill.id == "managed-sandbox-command"));
        assert!(normalized
            .skills
            .iter()
            .any(|skill| skill.id == "agent-cli-codex-prompt"));
        assert!(normalized
            .skills
            .iter()
            .any(|skill| skill.id == "agent-cli-claude-prompt"));
    }

    #[test]
    fn skill_prompt_input_supports_string_and_object_forms() {
        assert_eq!(
            skill_prompt_input(&serde_json::json!("plan infra")),
            Some("plan infra".to_string())
        );
        assert_eq!(
            skill_prompt_input(&serde_json::json!({ "prompt": "run tests" })),
            Some("run tests".to_string())
        );
        assert_eq!(skill_prompt_input(&serde_json::json!({})), None);
    }

    #[test]
    fn skill_input_schema_normalizes_prompt_string_input() {
        let settings = default_ai_settings();
        let skill = resolve_ai_skill(&settings, "agent-cli-codex-prompt").expect("codex skill");
        let normalized = normalize_skill_input(&skill, serde_json::json!("  summarize repo  "));
        assert_eq!(
            normalized,
            serde_json::json!({ "prompt": "summarize repo" })
        );
        validate_skill_input("input", &normalized, &skill.input_schema).expect("prompt schema");
    }

    #[test]
    fn skill_input_schema_rejects_missing_and_unknown_fields() {
        let settings = default_ai_settings();
        let skill = resolve_ai_skill(&settings, "managed-sandbox-command").expect("sandbox skill");
        let missing = validate_skill_input(
            "input",
            &serde_json::json!({ "id": "sandbox-1" }),
            &skill.input_schema,
        )
        .expect_err("missing command should fail");
        assert!(missing.contains("input.command is required"));

        let unexpected = validate_skill_input(
            "input",
            &serde_json::json!({
                "id": "sandbox-1",
                "command": "echo hi",
                "extra": true
            }),
            &skill.input_schema,
        )
        .expect_err("unexpected field should fail");
        assert!(unexpected.contains("input.extra is not allowed"));
    }

    #[test]
    fn skill_input_schema_rejects_empty_prompt_values() {
        let settings = default_ai_settings();
        let skill = resolve_ai_skill(&settings, "agent-cli-claude-prompt").expect("claude skill");
        let normalized = normalize_skill_input(&skill, serde_json::json!("   "));
        let err = validate_skill_input("input", &normalized, &skill.input_schema)
            .expect_err("blank prompt should fail");
        assert!(err.contains("input.prompt must be at least 1 characters"));
    }

    #[derive(Clone, Copy)]
    struct CoreScenario {
        name: &'static str,
        prompt: &'static str,
        expected_command: &'static str,
        expected_risk: &'static str,
        expected_confirm: bool,
    }

    #[test]
    fn assistant_core_scenarios_success_rate() {
        let scenarios = vec![
            CoreScenario {
                name: "container_delete",
                prompt: "delete container web",
                expected_command: "remove_container",
                expected_risk: "destructive",
                expected_confirm: true,
            },
            CoreScenario {
                name: "container_remove",
                prompt: "remove container api",
                expected_command: "remove_container",
                expected_risk: "destructive",
                expected_confirm: true,
            },
            CoreScenario {
                name: "container_stop",
                prompt: "stop container gateway",
                expected_command: "stop_container",
                expected_risk: "write",
                expected_confirm: true,
            },
            CoreScenario {
                name: "container_start",
                prompt: "start container worker",
                expected_command: "start_container",
                expected_risk: "write",
                expected_confirm: true,
            },
            CoreScenario {
                name: "container_list",
                prompt: "show container status overview",
                expected_command: "list_containers",
                expected_risk: "read",
                expected_confirm: false,
            },
            CoreScenario {
                name: "container_k8s_dual",
                prompt: "container and kubernetes pod diagnosis",
                expected_command: "k8s_list_pods",
                expected_risk: "read",
                expected_confirm: false,
            },
            CoreScenario {
                name: "vm_delete",
                prompt: "delete vm dev",
                expected_command: "vm_delete",
                expected_risk: "destructive",
                expected_confirm: true,
            },
            CoreScenario {
                name: "vm_remove",
                prompt: "remove vm qa",
                expected_command: "vm_delete",
                expected_risk: "destructive",
                expected_confirm: true,
            },
            CoreScenario {
                name: "vm_stop",
                prompt: "stop vm alpha",
                expected_command: "vm_stop",
                expected_risk: "write",
                expected_confirm: true,
            },
            CoreScenario {
                name: "vm_start",
                prompt: "start vm alpha",
                expected_command: "vm_start",
                expected_risk: "write",
                expected_confirm: true,
            },
            CoreScenario {
                name: "vm_list",
                prompt: "show vm status overview",
                expected_command: "vm_list",
                expected_risk: "read",
                expected_confirm: false,
            },
            CoreScenario {
                name: "vm_k8s_dual",
                prompt: "vm and pod health check",
                expected_command: "k8s_list_pods",
                expected_risk: "read",
                expected_confirm: false,
            },
            CoreScenario {
                name: "k8s_pods_status",
                prompt: "kubernetes pods status",
                expected_command: "k8s_list_pods",
                expected_risk: "read",
                expected_confirm: false,
            },
            CoreScenario {
                name: "k8s_pods_crashloop",
                prompt: "k8s pod crashloop check",
                expected_command: "k8s_list_pods",
                expected_risk: "read",
                expected_confirm: false,
            },
            CoreScenario {
                name: "fallback_infra_context",
                prompt: "show me infra context",
                expected_command: "list_containers",
                expected_risk: "read",
                expected_confirm: false,
            },
            CoreScenario {
                name: "fallback_general_diagnostics",
                prompt: "need full diagnostics",
                expected_command: "list_containers",
                expected_risk: "read",
                expected_confirm: false,
            },
            CoreScenario {
                name: "delete_container_and_vm",
                prompt: "delete container and vm now",
                expected_command: "vm_delete",
                expected_risk: "destructive",
                expected_confirm: true,
            },
            CoreScenario {
                name: "start_container_and_vm",
                prompt: "start container and vm together",
                expected_command: "vm_start",
                expected_risk: "write",
                expected_confirm: true,
            },
            CoreScenario {
                name: "container_logs_and_pod",
                prompt: "container logs and pod status",
                expected_command: "list_containers",
                expected_risk: "read",
                expected_confirm: false,
            },
            CoreScenario {
                name: "pod_investigation",
                prompt: "pod issue investigation",
                expected_command: "k8s_list_pods",
                expected_risk: "read",
                expected_confirm: false,
            },
        ];

        let mut passed = 0usize;
        for scenario in &scenarios {
            let steps = infer_assistant_steps(scenario.prompt, true);
            let matched = steps
                .iter()
                .find(|s| s.command == scenario.expected_command)
                .map(|s| {
                    s.risk_level == scenario.expected_risk
                        && s.requires_confirmation == scenario.expected_confirm
                })
                .unwrap_or(false);
            if matched {
                passed += 1;
            } else {
                eprintln!(
                    "scenario failed: {} prompt='{}' expected command={} risk={} confirm={} actual={:?}",
                    scenario.name,
                    scenario.prompt,
                    scenario.expected_command,
                    scenario.expected_risk,
                    scenario.expected_confirm,
                    steps
                );
            }
        }

        let total = scenarios.len();
        let success_rate = passed as f64 / total as f64;
        assert!(
            success_rate >= 0.95,
            "assistant core scenarios below threshold: passed={} total={} rate={:.2}",
            passed,
            total,
            success_rate
        );
    }

    #[test]
    fn destructive_steps_always_require_confirmation() {
        let prompts = vec![
            "delete container web",
            "remove container api",
            "delete vm dev",
            "remove vm qa",
            "delete container and vm now",
        ];
        for prompt in prompts {
            let steps = infer_assistant_steps(prompt, true);
            for step in steps {
                if step.risk_level == "destructive" {
                    assert!(
                        step.requires_confirmation,
                        "destructive step without confirmation: prompt='{}' command='{}'",
                        prompt, step.command
                    );
                }
            }
        }
    }

    #[test]
    fn infer_plan_injects_runtime_repair_when_docker_is_unavailable() {
        let steps = infer_assistant_steps_with_runtime("show container status", true, false);
        assert!(!steps.is_empty());
        assert_eq!(steps[0].command, "docker_runtime_quick_setup");
        assert_eq!(steps[0].risk_level, "write");
        assert!(!steps[0].requires_confirmation);
        assert!(
            steps.iter().any(|step| step.command == "list_containers"),
            "container read step should be preserved after prepending repair step"
        );
    }

    #[test]
    fn infer_plan_skips_runtime_repair_when_runtime_is_available() {
        let steps = infer_assistant_steps_with_runtime("show container status", true, true);
        assert!(!steps.is_empty());
        assert_ne!(steps[0].command, "docker_runtime_quick_setup");
    }

    #[test]
    fn mcp_policy_classifies_risk_levels() {
        let destructive = mcp_action_policy("k8s.delete_pod");
        assert_eq!(destructive.risk_level, "destructive");
        assert!(destructive.requires_confirmation);

        let write = mcp_action_policy("vm.restart");
        assert_eq!(write.risk_level, "write");
        assert!(!write.requires_confirmation);

        let read = mcp_action_policy("k8s.list_pods");
        assert_eq!(read.risk_level, "read");
        assert!(!read.requires_confirmation);
    }

    #[test]
    fn mcp_destructive_confirmation_needs_explicit_ack() {
        let destructive = mcp_action_policy("container.remove");
        assert!(!mcp_confirmation_satisfied(destructive, true, None, None));
        assert!(!mcp_confirmation_satisfied(
            destructive,
            true,
            Some(true),
            Some(false)
        ));
        assert!(mcp_confirmation_satisfied(
            destructive,
            true,
            Some(true),
            Some(true)
        ));

        let read = mcp_action_policy("k8s.list_pods");
        assert!(mcp_confirmation_satisfied(read, true, None, None));
    }
}

#[cfg(test)]
#[allow(clippy::await_holding_lock)]
mod ai_runtime_tests {
    use super::{
        agent_cli_run, ai_profile, ai_test_connection, default_ai_settings, load_ai_settings,
        mcp_export_client_config, mcp_list_servers_inner, mcp_server_logs_inner,
        mcp_start_server_inner, mcp_stop_server_inner, ollama_delete_model, ollama_list_models,
        ollama_pull_model, ollama_status, ollama_storage_info, sandbox_audit_list, sandbox_create,
        sandbox_delete, sandbox_exec, sandbox_inspect, sandbox_list, sandbox_start, sandbox_stop,
        save_ai_settings, secret_delete, secret_set, AppState, McpServerEntry,
        SandboxCreateRequest,
    };
    use serde_json::json;
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    struct EnvGuard {
        key: String,
        prev: Option<OsString>,
    }

    impl EnvGuard {
        fn set(key: &str, value: impl AsRef<str>) -> Self {
            let prev = std::env::var_os(key);
            std::env::set_var(key, value.as_ref());
            Self {
                key: key.to_string(),
                prev,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.prev.take() {
                Some(value) => std::env::set_var(&self.key, value),
                None => std::env::remove_var(&self.key),
            }
        }
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), nanos));
            std::fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn canary_timeout(env_key: &str, default_secs: u64) -> u64 {
        std::env::var(env_key)
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(default_secs)
            .max(1)
    }

    fn require_env(key: &str) -> String {
        std::env::var(key).unwrap_or_else(|_| panic!("required env var missing: {}", key))
    }

    #[cfg(unix)]
    fn is_executable(path: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    fn is_executable(path: &Path) -> bool {
        path.is_file()
    }

    fn resolve_executable_path(target: &str) -> Option<PathBuf> {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            return None;
        }

        let candidate = PathBuf::from(trimmed);
        if candidate.is_file() && is_executable(&candidate) {
            return Some(candidate);
        }

        if trimmed.contains('/') || trimmed.contains('\\') {
            return None;
        }

        let path_env = std::env::var_os("PATH").unwrap_or_default();
        for dir in std::env::split_paths(&path_env) {
            let candidate = dir.join(trimmed);
            if candidate.is_file() && is_executable(&candidate) {
                return Some(candidate);
            }
        }

        None
    }

    fn normalize_canary_bin_env(env_key: &str) -> EnvGuard {
        let raw = require_env(env_key);
        let resolved = resolve_executable_path(&raw).unwrap_or_else(|| {
            panic!(
                "{} must be an executable path or available in PATH; got: {}",
                env_key, raw
            )
        });
        EnvGuard::set(env_key, resolved.to_string_lossy())
    }

    fn configure_canary_dirs(tmp: &TempDir) -> (EnvGuard, EnvGuard, EnvGuard, EnvGuard) {
        let config_dir = tmp.path.join("config");
        let data_dir = tmp.path.join("data");
        let log_dir = tmp.path.join("logs");
        let secret_dir = tmp.path.join("secrets");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        std::fs::create_dir_all(&data_dir).expect("create data dir");
        std::fs::create_dir_all(&log_dir).expect("create log dir");
        std::fs::create_dir_all(&secret_dir).expect("create secret dir");
        (
            EnvGuard::set(
                "CRATEBAY_CONFIG_DIR",
                config_dir.to_str().expect("config dir"),
            ),
            EnvGuard::set("CRATEBAY_DATA_DIR", data_dir.to_str().expect("data dir")),
            EnvGuard::set("CRATEBAY_LOG_DIR", log_dir.to_str().expect("log dir")),
            EnvGuard::set(
                "CRATEBAY_TEST_SECRET_DIR",
                secret_dir.to_str().expect("secret dir"),
            ),
        )
    }

    fn prepend_path(dir: &Path) -> String {
        let current = std::env::var("PATH").unwrap_or_default();
        if current.is_empty() {
            dir.display().to_string()
        } else {
            format!("{}:{}", dir.display(), current)
        }
    }

    fn write_forwarder_binary(bin_dir: &Path, name: &str, target_env: &str) {
        std::fs::create_dir_all(bin_dir).expect("create canary bin dir");
        let script_path = bin_dir.join(name);
        let script = format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
target="${{{target_env}:-}}"
if [[ -z "$target" ]]; then
  echo "missing {target_env}" >&2
  exit 97
fi
exec "$target" "$@"
"#
        );
        std::fs::write(&script_path, script).expect("write forwarder script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script_path)
                .expect("forwarder metadata")
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script_path, perms).expect("forwarder perms");
        }
    }

    struct DockerCleanup {
        container_name: Option<String>,
        image_tag: Option<String>,
    }

    impl Drop for DockerCleanup {
        fn drop(&mut self) {
            if let Some(name) = &self.container_name {
                let _ = Command::new("docker").args(["rm", "-f", name]).output();
            }
            if let Some(tag) = &self.image_tag {
                let _ = Command::new("docker")
                    .args(["image", "rm", "-f", tag])
                    .output();
            }
        }
    }

    fn docker_ready() -> bool {
        matches!(
            Command::new("docker").arg("info").output(),
            Ok(output) if output.status.success()
        )
    }

    fn test_app_state() -> AppState {
        AppState {
            hv: Box::new(cratebay_core::vm::StubHypervisor::new()),
            grpc_addr: "http://127.0.0.1:65531".to_string(),
            daemon: Mutex::new(None),
            daemon_ready: Mutex::new(false),
            log_stream_handles: Mutex::new(HashMap::new()),
            mcp_runtimes: Mutex::new(HashMap::new()),
        }
    }

    fn fake_ollama_models(state_path: &Path) -> Vec<String> {
        std::fs::read_to_string(state_path)
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToString::to_string)
            .collect()
    }

    fn fake_ollama_tags_payload(state_path: &Path) -> String {
        let models = fake_ollama_models(state_path)
            .into_iter()
            .enumerate()
            .map(|(index, name)| {
                let size = 64_u64 * 1024 * 1024 * (index as u64 + 1);
                json!({
                    "name": name,
                    "modified_at": format!("2026-03-07T00:00:{:02}Z", index),
                    "size": size,
                    "digest": format!("sha256:test{:02}", index),
                    "details": {
                        "family": "qwen2.5",
                        "parameter_size": "7B",
                        "quantization_level": "Q4_K_M"
                    }
                })
            })
            .collect::<Vec<_>>();
        json!({ "models": models }).to_string()
    }

    struct FakeOllamaServer {
        stop: Arc<AtomicBool>,
        handle: Option<thread::JoinHandle<()>>,
        base_url: String,
    }

    impl FakeOllamaServer {
        fn start(state_path: PathBuf) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake ollama server");
            listener
                .set_nonblocking(true)
                .expect("set fake ollama listener nonblocking");
            let port = listener.local_addr().expect("listener addr").port();
            let stop = Arc::new(AtomicBool::new(false));
            let stop_flag = stop.clone();
            let handle = thread::spawn(move || {
                while !stop_flag.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let mut buffer = [0_u8; 4096];
                            let read = stream.read(&mut buffer).unwrap_or(0);
                            let request = String::from_utf8_lossy(&buffer[..read]);
                            let path = request
                                .lines()
                                .next()
                                .and_then(|line| line.split_whitespace().nth(1))
                                .unwrap_or("/");
                            let (status, body) =
                                if path == "/api/version" || path.ends_with("/api/version") {
                                    (
                                        "HTTP/1.1 200 OK",
                                        json!({ "version": "0.5.7-test" }).to_string(),
                                    )
                                } else if path == "/api/tags" || path.ends_with("/api/tags") {
                                    ("HTTP/1.1 200 OK", fake_ollama_tags_payload(&state_path))
                                } else {
                                    (
                                        "HTTP/1.1 404 Not Found",
                                        json!({ "error": "not found", "path": path }).to_string(),
                                    )
                                };
                            let response = format!(
                                "{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                status,
                                body.len(),
                                body
                            );
                            let _ = stream.write_all(response.as_bytes());
                            let _ = stream.flush();
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(25));
                        }
                        Err(_) => break,
                    }
                }
            });
            let server = Self {
                stop,
                handle: Some(handle),
                base_url: format!("http://127.0.0.1:{}", port),
            };
            server.wait_until_ready();
            server
        }

        fn wait_until_ready(&self) {
            let addr = self
                .base_url
                .trim_start_matches("http://")
                .trim_start_matches("https://")
                .to_string();
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while std::time::Instant::now() < deadline {
                if let Ok(mut stream) = TcpStream::connect(&addr) {
                    let _ = stream.write_all(
                        b"GET /api/version HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
                    );
                    let _ = stream.flush();
                    let mut response = Vec::new();
                    let _ = stream.read_to_end(&mut response);
                    if String::from_utf8_lossy(&response).contains("200 OK") {
                        return;
                    }
                }
                thread::sleep(Duration::from_millis(25));
            }
            panic!(
                "fake ollama server did not become ready at {}",
                self.base_url
            );
        }
    }

    impl Drop for FakeOllamaServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(port) = self.base_url.rsplit(':').next() {
                let _ = TcpStream::connect(format!("127.0.0.1:{}", port));
            }
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn write_fake_ollama_binary(bin_dir: &Path, state_path: &Path) {
        let script = format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nstate=\"{}\"\nmkdir -p \"$(dirname \"$state\")\"\ntouch \"$state\"\ncase \"${{1:-}}\" in\n  --version)\n    echo \"ollama version 0.5.7-test\"\n    ;;\n  pull)\n    model=\"${{2:?model required}}\"\n    if ! grep -Fxq \"$model\" \"$state\"; then\n      echo \"$model\" >> \"$state\"\n    fi\n    echo \"pulled $model\"\n    ;;\n  rm)\n    model=\"${{2:?model required}}\"\n    tmp=\"$state.tmp\"\n    grep -Fxv \"$model\" \"$state\" > \"$tmp\" || true\n    mv \"$tmp\" \"$state\"\n    echo \"removed $model\"\n    ;;\n  *)\n    echo \"unsupported fake ollama args: $*\" >&2\n    exit 1\n    ;;\nesac\n",
            state_path.display()
        );
        let script_path = bin_dir.join("ollama");
        std::fs::write(&script_path, script).expect("write fake ollama binary");
        let chmod = Command::new("chmod")
            .args(["+x", script_path.to_str().expect("script path")])
            .status()
            .expect("chmod fake ollama");
        assert!(chmod.success(), "fake ollama binary should be executable");
    }

    #[tokio::test]
    #[ignore = "requires Docker runtime"]
    async fn sandbox_runtime_smoke_lifecycle() {
        let _lock = env_lock();
        assert!(
            docker_ready(),
            "Docker daemon must be available for sandbox runtime smoke"
        );

        let tmp = TempDir::new("cratebay-ai-sandbox-smoke");
        let config_dir = tmp.path.join("config");
        let _config = EnvGuard::set(
            "CRATEBAY_CONFIG_DIR",
            config_dir.to_str().expect("config dir"),
        );

        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let sandbox_name = format!("cbx-ai-sandbox-{}-{}", std::process::id(), suffix);
        let mut cleanup = DockerCleanup {
            container_name: Some(sandbox_name.clone()),
            image_tag: None,
        };

        let created = sandbox_create(SandboxCreateRequest {
            template_id: "python-dev".to_string(),
            name: Some(sandbox_name.clone()),
            image: Some("alpine:3.20".to_string()),
            command: Some("sleep 300".to_string()),
            env: Some(vec!["CRATEBAY_E2E=1".to_string()]),
            cpu_cores: Some(1),
            memory_mb: Some(256),
            ttl_hours: Some(1),
            owner: Some("ci".to_string()),
        })
        .await
        .expect("create sandbox");

        assert_eq!(created.name, sandbox_name);
        assert!(created.login_cmd.contains(&sandbox_name));

        let list = sandbox_list().await.expect("list sandboxes");
        let item = list
            .iter()
            .find(|entry| entry.name == sandbox_name)
            .expect("created sandbox should be listed");
        assert_eq!(item.template_id, "python-dev");
        assert_eq!(item.owner, "ci");
        assert_eq!(item.cpu_cores, 1);
        assert_eq!(item.memory_mb, 256);
        assert!(!item.is_expired);

        let inspect = sandbox_inspect(created.id.clone())
            .await
            .expect("inspect sandbox");
        assert!(inspect.running);
        assert!(inspect
            .env
            .iter()
            .any(|entry| entry == "CRATEBAY_SANDBOX=1"));
        assert!(inspect.env.iter().any(|entry| entry == "CRATEBAY_E2E=1"));

        let exec = sandbox_exec(
            created.id.clone(),
            "echo CRATEBAY_SANDBOX_OK".to_string(),
            None,
        )
        .await
        .expect("exec sandbox command");
        assert!(exec.ok);
        assert_eq!(exec.exit_code, Some(0));
        assert!(exec.stdout.contains("CRATEBAY_SANDBOX_OK"));
        assert!(exec.output.contains("CRATEBAY_SANDBOX_OK"));

        sandbox_stop(created.id.clone())
            .await
            .expect("stop sandbox");
        let stopped = sandbox_inspect(created.id.clone())
            .await
            .expect("inspect stopped sandbox");
        assert!(!stopped.running);

        sandbox_start(created.id.clone())
            .await
            .expect("restart sandbox");
        let restarted = sandbox_inspect(created.id.clone())
            .await
            .expect("inspect restarted sandbox");
        assert!(restarted.running);

        sandbox_delete(created.id.clone())
            .await
            .expect("delete sandbox");
        cleanup.container_name = None;

        let after = sandbox_list().await.expect("list sandboxes after delete");
        assert!(!after.iter().any(|entry| entry.name == sandbox_name));

        let audit = sandbox_audit_list(Some(20)).expect("sandbox audit list");
        assert!(audit
            .iter()
            .any(|event| event.action == "create" && event.sandbox_name == sandbox_name));
        assert!(audit
            .iter()
            .any(|event| event.action == "delete" && event.sandbox_name == sandbox_name));
    }

    #[tokio::test]
    #[ignore = "requires local runtime canary server"]
    async fn ollama_runtime_canary_smoke() {
        let _lock = env_lock();

        let tmp = TempDir::new("cratebay-ollama-smoke");
        let config_dir = tmp.path.join("config");
        let bin_dir = tmp.path.join("bin");
        let models_dir = tmp.path.join("models");
        let state_path = tmp.path.join("fake-ollama-models.txt");
        std::fs::create_dir_all(&bin_dir).expect("create fake bin dir");
        std::fs::create_dir_all(&models_dir).expect("create fake models dir");
        std::fs::write(&state_path, "qwen2.5:7b\n").expect("seed fake models");
        write_fake_ollama_binary(&bin_dir, &state_path);
        let server = FakeOllamaServer::start(state_path.clone());

        let current_path = std::env::var("PATH").unwrap_or_default();
        let joined_path = if current_path.is_empty() {
            bin_dir.display().to_string()
        } else {
            format!("{}:{}", bin_dir.display(), current_path)
        };
        let _path = EnvGuard::set("PATH", joined_path);
        let _config = EnvGuard::set(
            "CRATEBAY_CONFIG_DIR",
            config_dir.to_str().expect("config dir"),
        );
        let _models = EnvGuard::set("OLLAMA_MODELS", models_dir.to_str().expect("models dir"));
        let _base_url = EnvGuard::set("CRATEBAY_OLLAMA_BASE_URL", &server.base_url);

        let status = ollama_status().await.expect("ollama status");
        assert!(status.installed);
        assert!(status.running);
        assert_eq!(status.version, "0.5.7-test");
        assert_eq!(status.base_url, server.base_url);

        let models = ollama_list_models().await.expect("initial ollama models");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].name, "qwen2.5:7b");

        let storage = ollama_storage_info().await.expect("ollama storage info");
        assert!(storage.exists);
        assert_eq!(storage.model_count, 1);
        assert_eq!(PathBuf::from(storage.path), models_dir);

        let pull = ollama_pull_model("smoke:test".to_string())
            .await
            .expect("pull fake model");
        assert!(pull.ok);
        assert!(pull.message.contains("pulled smoke:test"));

        let pulled_models = ollama_list_models().await.expect("models after pull");
        assert!(pulled_models.iter().any(|item| item.name == "smoke:test"));

        let storage_after_pull = ollama_storage_info().await.expect("storage after pull");
        assert_eq!(storage_after_pull.model_count, 2);

        let delete = ollama_delete_model("smoke:test".to_string())
            .await
            .expect("delete fake model");
        assert!(delete.ok);
        assert!(delete.message.contains("removed smoke:test"));

        let models_after_delete = ollama_list_models().await.expect("models after delete");
        assert_eq!(models_after_delete.len(), 1);
        assert_eq!(models_after_delete[0].name, "qwen2.5:7b");
    }

    #[tokio::test]
    #[ignore = "requires real OpenAI canary credentials"]
    async fn openai_provider_canary_real_connection() {
        let _lock = env_lock();
        let tmp = TempDir::new("cratebay-openai-provider-canary");
        let (_config, _data, _log, _secret_dir) = configure_canary_dirs(&tmp);

        let profile_id = "openai-canary";
        let api_key_ref = "OPENAI_CANARY_API_KEY";
        let api_key = require_env("CRATEBAY_CANARY_OPENAI_API_KEY");
        let base_url = std::env::var("CRATEBAY_CANARY_OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let model = std::env::var("CRATEBAY_CANARY_OPENAI_MODEL")
            .unwrap_or_else(|_| "gpt-4.1-mini".to_string());

        let mut settings = default_ai_settings();
        settings.profiles = vec![ai_profile(
            profile_id,
            "openai",
            "OpenAI Canary",
            &model,
            &base_url,
            api_key_ref,
        )];
        settings.active_profile_id = profile_id.to_string();
        save_ai_settings(settings).expect("save OpenAI canary settings");
        secret_set(api_key_ref, &api_key).expect("store OpenAI canary API key");

        let result = ai_test_connection(
            Some(profile_id.to_string()),
            Some(canary_timeout("CRATEBAY_CANARY_OPENAI_TIMEOUT_SEC", 25) * 1000),
        )
        .await
        .expect("run OpenAI canary connection test");
        let _ = secret_delete(api_key_ref);

        assert!(result.ok, "OpenAI canary failed: {}", result.message);
        assert!(
            result.message.to_ascii_uppercase().contains("PONG"),
            "OpenAI canary response should contain PONG: {}",
            result.message
        );
    }

    #[tokio::test]
    #[ignore = "requires real Anthropic canary credentials"]
    async fn anthropic_provider_canary_real_connection() {
        let _lock = env_lock();
        let tmp = TempDir::new("cratebay-anthropic-provider-canary");
        let (_config, _data, _log, _secret_dir) = configure_canary_dirs(&tmp);

        let profile_id = "anthropic-canary";
        let api_key_ref = "ANTHROPIC_CANARY_API_KEY";
        let api_key = require_env("CRATEBAY_CANARY_ANTHROPIC_API_KEY");
        let base_url = std::env::var("CRATEBAY_CANARY_ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com/v1".to_string());
        let model = std::env::var("CRATEBAY_CANARY_ANTHROPIC_MODEL")
            .unwrap_or_else(|_| "claude-3-7-sonnet-latest".to_string());

        let mut settings = default_ai_settings();
        settings.profiles = vec![ai_profile(
            profile_id,
            "anthropic",
            "Anthropic Canary",
            &model,
            &base_url,
            api_key_ref,
        )];
        settings.active_profile_id = profile_id.to_string();
        save_ai_settings(settings).expect("save Anthropic canary settings");
        secret_set(api_key_ref, &api_key).expect("store Anthropic canary API key");

        let result = ai_test_connection(
            Some(profile_id.to_string()),
            Some(canary_timeout("CRATEBAY_CANARY_ANTHROPIC_TIMEOUT_SEC", 25) * 1000),
        )
        .await
        .expect("run Anthropic canary connection test");
        let _ = secret_delete(api_key_ref);

        assert!(result.ok, "Anthropic canary failed: {}", result.message);
        assert!(
            result.message.to_ascii_uppercase().contains("PONG"),
            "Anthropic canary response should contain PONG: {}",
            result.message
        );
    }

    #[tokio::test]
    #[ignore = "requires real Codex CLI bridge"]
    async fn codex_cli_bridge_canary() {
        let _lock = env_lock();
        let _target = normalize_canary_bin_env("CRATEBAY_CANARY_CODEX_BIN");
        let tmp = TempDir::new("cratebay-codex-cli-canary");
        let (_config, _data, _log, _secret_dir) = configure_canary_dirs(&tmp);
        let bin_dir = tmp.path.join("bin");
        write_forwarder_binary(&bin_dir, "codex", "CRATEBAY_CANARY_CODEX_BIN");
        let _path = EnvGuard::set("PATH", prepend_path(&bin_dir));
        let prompt = std::env::var("CRATEBAY_CANARY_CODEX_PROMPT")
            .unwrap_or_else(|_| "Reply with PONG and exit.".to_string());

        let result = agent_cli_run(
            Some("codex".to_string()),
            None,
            None,
            Some(prompt),
            false,
            Some(canary_timeout("CRATEBAY_CANARY_CODEX_TIMEOUT_SEC", 60)),
        )
        .await
        .expect("run Codex CLI canary");

        let combined = format!(
            "{}
{}",
            result.stdout, result.stderr
        )
        .to_ascii_uppercase();
        assert!(result.ok, "Codex CLI canary failed: {:?}", result);
        assert!(
            result.command_line.starts_with("codex exec "),
            "Codex preset command should use preset form: {}",
            result.command_line
        );
        assert!(
            combined.contains("PONG"),
            "Codex CLI output should contain PONG: {}",
            combined
        );
    }

    #[tokio::test]
    #[ignore = "requires real Claude CLI bridge"]
    async fn claude_cli_bridge_canary() {
        let _lock = env_lock();
        let _target = normalize_canary_bin_env("CRATEBAY_CANARY_CLAUDE_BIN");
        let tmp = TempDir::new("cratebay-claude-cli-canary");
        let (_config, _data, _log, _secret_dir) = configure_canary_dirs(&tmp);
        let bin_dir = tmp.path.join("bin");
        write_forwarder_binary(&bin_dir, "claude", "CRATEBAY_CANARY_CLAUDE_BIN");
        let _path = EnvGuard::set("PATH", prepend_path(&bin_dir));
        let prompt = std::env::var("CRATEBAY_CANARY_CLAUDE_PROMPT")
            .unwrap_or_else(|_| "Reply with PONG and exit.".to_string());

        let result = agent_cli_run(
            Some("claude".to_string()),
            None,
            None,
            Some(prompt),
            false,
            Some(canary_timeout("CRATEBAY_CANARY_CLAUDE_TIMEOUT_SEC", 60)),
        )
        .await
        .expect("run Claude CLI canary");

        let combined = format!(
            "{}
{}",
            result.stdout, result.stderr
        )
        .to_ascii_uppercase();
        assert!(result.ok, "Claude CLI canary failed: {:?}", result);
        assert!(
            result.command_line.starts_with("claude --print "),
            "Claude preset command should use preset form: {}",
            result.command_line
        );
        assert!(
            combined.contains("PONG"),
            "Claude CLI output should contain PONG: {}",
            combined
        );
    }

    #[tokio::test]
    #[ignore = "requires real Ollama daemon"]
    async fn ollama_real_daemon_canary_smoke() {
        let _lock = env_lock();
        let tmp = TempDir::new("cratebay-ollama-daemon-canary");
        let (_config, _data, _log, _secret_dir) = configure_canary_dirs(&tmp);

        let _ollama_bin = if std::env::var_os("CRATEBAY_CANARY_OLLAMA_BIN").is_some() {
            Some(normalize_canary_bin_env("CRATEBAY_CANARY_OLLAMA_BIN"))
        } else {
            None
        };
        let _ollama_path = if _ollama_bin.is_some() {
            let bin_dir = tmp.path.join("bin");
            write_forwarder_binary(&bin_dir, "ollama", "CRATEBAY_CANARY_OLLAMA_BIN");
            Some(EnvGuard::set("PATH", prepend_path(&bin_dir)))
        } else {
            None
        };

        let _base = std::env::var("CRATEBAY_CANARY_OLLAMA_BASE_URL")
            .ok()
            .map(|value| EnvGuard::set("CRATEBAY_OLLAMA_BASE_URL", value));
        let _models = std::env::var("CRATEBAY_CANARY_OLLAMA_MODELS_DIR")
            .ok()
            .map(|value| EnvGuard::set("OLLAMA_MODELS", value));

        let status = ollama_status().await.expect("ollama daemon status");
        assert!(
            status.installed,
            "Ollama daemon canary expects installed=true"
        );
        assert!(status.running, "Ollama daemon canary expects running=true");

        let models = ollama_list_models().await.expect("ollama daemon models");
        if let Ok(expected_model) = std::env::var("CRATEBAY_CANARY_OLLAMA_EXPECT_MODEL") {
            assert!(
                models.iter().any(|item| item.name == expected_model),
                "expected Ollama model '{}' to be present; got {:?}",
                expected_model,
                models
                    .iter()
                    .map(|item| item.name.clone())
                    .collect::<Vec<_>>()
            );
        } else {
            assert!(
                !models.is_empty(),
                "Ollama daemon canary expects at least one model"
            );
        }

        let storage = ollama_storage_info()
            .await
            .expect("ollama daemon storage info");
        assert!(storage.exists, "Ollama storage should exist");
        assert!(
            storage.model_count >= 1,
            "Ollama storage should report at least one model"
        );

        if let Ok(model) = std::env::var("CRATEBAY_CANARY_OLLAMA_PULL_MODEL") {
            let pull = ollama_pull_model(model.clone())
                .await
                .expect("pull Ollama canary model");
            assert!(pull.ok, "Ollama pull should succeed: {}", pull.message);
            let delete = ollama_delete_model(model.clone())
                .await
                .expect("delete Ollama canary model");
            assert!(
                delete.ok,
                "Ollama delete should succeed: {}",
                delete.message
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires local process runtime"]
    async fn mcp_runtime_smoke_lifecycle() {
        let _lock = env_lock();

        let tmp = TempDir::new("cratebay-mcp-smoke");
        let config_dir = tmp.path.join("config");
        let _config = EnvGuard::set(
            "CRATEBAY_CONFIG_DIR",
            config_dir.to_str().expect("config dir"),
        );

        let mut settings = default_ai_settings();
        settings.mcp_servers = vec![McpServerEntry {
            id: "local-smoke".to_string(),
            name: "Local Smoke MCP".to_string(),
            command: "/bin/sh".to_string(),
            args: vec![
                "-lc".to_string(),
                "echo MCP_READY; while true; do sleep 1; done".to_string(),
            ],
            env: vec!["CRATEBAY_MCP=1".to_string()],
            working_dir: tmp.path.display().to_string(),
            enabled: true,
            notes: "runtime smoke".to_string(),
        }];
        save_ai_settings(settings).expect("save MCP test settings");

        let state = test_app_state();
        let started = mcp_start_server_inner(&state, "local-smoke".to_string())
            .await
            .expect("start MCP runtime");
        assert!(started.ok);
        assert!(started.message.contains("Started Local Smoke MCP"));

        tokio::time::sleep(Duration::from_millis(250)).await;

        let servers = mcp_list_servers_inner(&state).expect("list MCP servers");
        let server = servers
            .iter()
            .find(|entry| entry.id == "local-smoke")
            .expect("started MCP server should be listed");
        assert!(server.running);
        assert_eq!(server.status, "running");
        assert!(server.pid.is_some());

        let logs = mcp_server_logs_inner(&state, "local-smoke".to_string(), Some(20))
            .expect("read MCP runtime logs");
        assert!(
            logs.iter().any(|line| line.contains("MCP_READY"))
                || logs.iter().any(|line| line.contains("started pid="))
        );

        let exported =
            mcp_export_client_config("codex".to_string()).expect("export codex MCP config");
        assert!(
            exported.contains("[mcp_servers.local-smoke]")
                || exported.contains("[mcp_servers.\"local-smoke\"]")
        );
        assert!(exported.contains("command = \"/bin/sh\""));

        let loaded = load_ai_settings().expect("reload AI settings");
        assert_eq!(loaded.mcp_servers.len(), 1);
        assert_eq!(loaded.mcp_servers[0].id, "local-smoke");

        let stopped = mcp_stop_server_inner(&state, "local-smoke".to_string())
            .await
            .expect("stop MCP runtime");
        assert!(stopped.ok);

        let after_stop = mcp_list_servers_inner(&state).expect("list MCP servers after stop");
        let stopped_server = after_stop
            .iter()
            .find(|entry| entry.id == "local-smoke")
            .expect("stopped MCP server should still be listed");
        assert!(!stopped_server.running);
        assert!(matches!(
            stopped_server.status.as_str(),
            "exited" | "stopped"
        ));
    }
}

pub fn run() {
    #[cfg(target_os = "macos")]
    prime_macos_runtime_assets_env();
    cratebay_core::logging::init();
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init());

    // Enable MCP debug plugin only in debug builds
    #[cfg(debug_assertions)]
    {
        let mcp_bind = std::env::var("CRATEBAY_MCP_BIND").unwrap_or_else(|_| "127.0.0.1".into());
        let mcp_port = std::env::var("CRATEBAY_MCP_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(9223);

        info!(
            "Debug build detected: enabling MCP bridge at {}:{}",
            mcp_bind, mcp_port
        );
        builder = builder.plugin(
            tauri_plugin_mcp_bridge::Builder::new()
                .bind_address(&mcp_bind)
                .base_port(mcp_port)
                .build(),
        );
    }

    let app = builder
        .manage(AppState {
            hv: cratebay_core::create_hypervisor(),
            grpc_addr: grpc_addr(),
            daemon: Mutex::new(None),
            daemon_ready: Mutex::new(false),
            log_stream_handles: Mutex::new(HashMap::new()),
            mcp_runtimes: Mutex::new(HashMap::new()),
        })
        .setup(|app| {
            // ── System tray ─────────────────────────────────────────────
            let app_handle = app.handle().clone();
            let menu = build_tray_menu(&app_handle, 0, 0)?;

            TrayIconBuilder::with_id("main-tray")
                .icon(tauri::include_image!("icons/tray-icon.png"))
                .icon_as_template(true)
                .tooltip("CrateBay")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "dashboard" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            // Initial tray menu refresh (to get real counts)
            refresh_tray_menu(&app_handle);

            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(900));
                loop {
                    interval.tick().await;
                    if let Err(err) = sandbox_cleanup_expired_internal().await {
                        warn!("sandbox cleanup worker failed: {}", err);
                    }
                }
            });

            // Set webview background color to match dark theme (prevents white flash on resize)
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_background_color(Some(tauri::window::Color(15, 17, 26, 255)));
                // Hide native title bar text (macOS/Linux with decorations: true)
                let _ = window.set_title("");
                // Enforce minimum window size (decorations:false may not honor config minWidth/minHeight)
                let _ = window.set_min_size(Some(tauri::LogicalSize::new(1100.0, 650.0)));
            }

            Ok(())
        })
        // ── Hide window on close instead of quitting ────────────────
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();

                // Refresh the tray menu so counts are up-to-date when the
                // user re-opens via the tray.
                refresh_tray_menu(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_containers,
            stop_container,
            start_container,
            remove_container,
            docker_run,
            container_login_cmd,
            container_logs,
            container_logs_stream,
            container_logs_stream_stop,
            container_exec,
            container_exec_interactive_cmd,
            container_env,
            container_stats,
            image_search,
            image_tags,
            image_load,
            image_push,
            image_pack_container,
            image_catalog,
            image_download_os,
            image_download_status,
            image_delete_os,
            vm_list,
            vm_create,
            vm_start,
            vm_stop,
            vm_delete,
            vm_login_cmd,
            vm_console,
            vm_mount_add,
            vm_mount_remove,
            vm_mount_list,
            vm_port_forward_add,
            vm_port_forward_remove,
            vm_port_forward_list,
            vm_stats,
            volume_list,
            volume_create,
            volume_inspect,
            volume_remove,
            image_list,
            image_remove,
            image_tag,
            image_inspect,
            k3s_status,
            k3s_install,
            k3s_start,
            k3s_stop,
            k3s_uninstall,
            k8s_list_namespaces,
            k8s_list_pods,
            k8s_list_services,
            k8s_list_deployments,
            k8s_pod_logs,
            ollama_status,
            gpu_status,
            ollama_list_models,
            ollama_storage_info,
            ollama_pull_model,
            ollama_delete_model,
            sandbox_templates,
            sandbox_list,
            sandbox_create,
            sandbox_start,
            sandbox_stop,
            sandbox_delete,
            sandbox_inspect,
            sandbox_runtime_usage,
            sandbox_audit_list,
            sandbox_cleanup_expired,
            sandbox_exec,
            load_ai_settings,
            save_ai_settings,
            mcp_list_servers,
            mcp_save_servers,
            mcp_start_server,
            mcp_stop_server,
            mcp_server_logs,
            mcp_export_client_config,
            opensandbox_status,
            ai_skill_execute,
            validate_ai_profile,
            ai_secret_set,
            ai_secret_delete,
            ai_secret_exists,
            ai_chat,
            ai_test_connection,
            ai_generate_plan,
            assistant_execute_step,
            mcp_check_access,
            docker_runtime_quick_setup,
            agent_cli_list_presets,
            agent_cli_run,
            check_update,
            open_release_page,
            set_window_theme
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen {
            has_visible_windows: false,
            ..
        } = event
        {
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    });
}
