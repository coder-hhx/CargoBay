//! Linux runtime — KVM/QEMU integration.
//!
//! Uses KVM-accelerated QEMU to run a lightweight Linux VM
//! with Docker Engine inside.
//!
//! ## Architecture (runtime-spec.md §2.2)
//!
//! ```text
//! Linux Host
//! ├── CrateBay binary
//! │   └── Rust Backend
//! │       └── QEMU process management
//! │           └── qemu-system-{x86_64|aarch64}
//! │               ├── -enable-kvm
//! │               ├── -kernel vmlinuz -initrd initrd
//! │               ├── -drive file=rootfs.qcow2
//! │               ├── -chardev socket for Docker
//! │               └── -virtfs for shared directories
//! │                   └── Alpine Linux
//! │                       └── Docker Engine
//! │                           └── Exposed via socket
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::error::AppError;
use crate::models::ResourceUsage;

use super::{
    HealthStatus, ProvisionProgress, RuntimeConfig, RuntimeManager, RuntimeState,
};

// ---------------------------------------------------------------------------
// LinuxRuntime (§11.2)
// ---------------------------------------------------------------------------

/// Linux runtime manager using KVM/QEMU.
///
/// Manages a lightweight QEMU virtual machine with KVM hardware acceleration
/// running Alpine Linux + Docker Engine. The Docker socket is exposed to the
/// host via QEMU chardev socket forwarding.
pub struct LinuxRuntime {
    config: RuntimeConfig,
    data_dir: PathBuf,
    state: Arc<Mutex<RuntimeState>>,
    /// PID of the running QEMU process (if any).
    qemu_pid: Arc<Mutex<Option<u32>>>,
    /// Timestamp (seconds since epoch) when the QEMU process was started.
    started_at: Arc<Mutex<Option<i64>>>,
}

impl LinuxRuntime {
    /// Create a new Linux runtime manager with default configuration.
    pub fn new() -> Self {
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".cratebay/runtime");
        Self {
            config: RuntimeConfig::default(),
            data_dir,
            state: Arc::new(Mutex::new(RuntimeState::None)),
            qemu_pid: Arc::new(Mutex::new(None)),
            started_at: Arc::new(Mutex::new(None)),
        }
    }

    /// Build QEMU command-line arguments according to runtime-spec.md §11.2.
    ///
    /// The argument list includes:
    /// - KVM acceleration
    /// - CPU and memory allocation from config
    /// - Kernel, initrd, and rootfs disk paths
    /// - Headless operation flags
    /// - Docker socket chardev forwarding
    /// - Shared directories via virtfs (9P)
    /// - User-mode networking with virtio-net
    fn build_qemu_args(&self) -> Vec<String> {
        let mut args = vec![
            "-enable-kvm".to_string(),
            "-m".to_string(),
            format!("{}M", self.config.memory_mb),
            "-smp".to_string(),
            format!("{}", self.config.cpu_cores),
            "-kernel".to_string(),
            self.data_dir
                .join("vmlinuz")
                .to_string_lossy()
                .to_string(),
            "-initrd".to_string(),
            self.data_dir
                .join("initrd")
                .to_string_lossy()
                .to_string(),
            "-drive".to_string(),
            format!(
                "file={},format=qcow2,if=virtio",
                self.data_dir.join("rootfs.qcow2").display()
            ),
            "-nographic".to_string(),
            "-nodefaults".to_string(),
        ];

        // Docker socket forwarding via QEMU chardev.
        // The guest connects to a virtio-serial device backed by this host socket.
        let socket_path = self.docker_socket_path();
        args.extend_from_slice(&[
            "-chardev".to_string(),
            format!(
                "socket,id=docker,path={},server=on,wait=off",
                socket_path.display()
            ),
        ]);

        // Shared directories via VirtioFS / 9P (§5).
        // Each shared dir gets a separate -virtfs entry with a unique mount_tag.
        for dir in &self.config.shared_dirs {
            args.extend_from_slice(&[
                "-virtfs".to_string(),
                format!(
                    "local,path={},mount_tag={},security_model=mapped-xattr",
                    dir.host_path, dir.tag
                ),
            ]);
        }

        // User-mode networking with virtio-net-pci device.
        args.extend_from_slice(&[
            "-netdev".to_string(),
            "user,id=net0".to_string(),
            "-device".to_string(),
            "virtio-net-pci,netdev=net0".to_string(),
        ]);

        args
    }

    /// Wait for the Docker socket inside the VM to become responsive.
    ///
    /// Polls the socket path with exponential back-off until Docker responds
    /// to a ping, or `timeout` elapses.
    async fn wait_for_docker(&self, timeout: Duration) -> Result<(), AppError> {
        let socket_path = self.docker_socket_path();
        let start = tokio::time::Instant::now();
        let mut interval = Duration::from_millis(500);

        while start.elapsed() < timeout {
            // Step 1: Check if the socket file exists on disk.
            if socket_path.exists() {
                // Step 2: Attempt a Docker ping via bollard.
                let socket_str = socket_path
                    .to_str()
                    .ok_or_else(|| {
                        AppError::Runtime("Docker socket path is not valid UTF-8".into())
                    })?;

                match bollard::Docker::connect_with_unix(socket_str, 5, bollard::API_DEFAULT_VERSION)
                {
                    Ok(docker) => {
                        if docker.ping().await.is_ok() {
                            return Ok(());
                        }
                    }
                    Err(_) => {
                        // Connection failed, will retry.
                    }
                }
            }

            tokio::time::sleep(interval).await;
            // Exponential back-off capped at 2 seconds.
            interval = std::cmp::min(interval * 2, Duration::from_secs(2));
        }

        Err(AppError::Runtime(format!(
            "Docker did not become responsive within {} seconds",
            timeout.as_secs()
        )))
    }

    /// Check whether the QEMU process (identified by stored PID) is still alive.
    fn is_qemu_process_alive(pid: Option<u32>) -> bool {
        match pid {
            Some(pid) => {
                // On Linux, sending signal 0 checks process existence without
                // actually delivering a signal.
                let path = format!("/proc/{}", pid);
                Path::new(&path).exists()
            }
            None => false,
        }
    }

    /// Detect the appropriate QEMU binary for the current architecture.
    ///
    /// Returns the binary name if found in PATH:
    /// - `qemu-system-x86_64` on x86_64
    /// - `qemu-system-aarch64` on aarch64/arm64
    fn detect_qemu_binary() -> Option<String> {
        let candidates = if cfg!(target_arch = "x86_64") {
            vec!["qemu-system-x86_64"]
        } else if cfg!(target_arch = "aarch64") {
            vec!["qemu-system-aarch64"]
        } else {
            vec!["qemu-system-x86_64", "qemu-system-aarch64"]
        };

        for candidate in candidates {
            if which_binary(candidate) {
                return Some(candidate.to_string());
            }
        }
        None
    }
}

impl Default for LinuxRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Check whether a binary exists in PATH by looking up /usr/bin, /usr/local/bin,
/// and the directories listed in the PATH environment variable.
fn which_binary(name: &str) -> bool {
    if let Ok(path_env) = std::env::var("PATH") {
        for dir in path_env.split(':') {
            if Path::new(dir).join(name).exists() {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// RuntimeManager implementation (§3.2, §11.2)
// ---------------------------------------------------------------------------

#[async_trait]
impl RuntimeManager for LinuxRuntime {
    /// Detect the current runtime state (§3.2).
    ///
    /// Checks:
    /// 1. `/dev/kvm` existence — KVM hardware acceleration available.
    /// 2. QEMU binary in PATH (`qemu-system-x86_64` or `qemu-system-aarch64`).
    /// 3. VM image files present in `data_dir` (vmlinuz, initrd, rootfs.qcow2).
    /// 4. Whether a QEMU process is currently running.
    async fn detect(&self) -> Result<RuntimeState, AppError> {
        // If we have a tracked QEMU process, check if it's still alive.
        let pid = *self.qemu_pid.lock().await;
        if Self::is_qemu_process_alive(pid) {
            // QEMU is running — check if Docker is responsive.
            let socket_path = self.docker_socket_path();
            if socket_path.exists() {
                if let Ok(docker) = bollard::Docker::connect_with_unix(
                    socket_path.to_str().unwrap_or_default(),
                    5,
                    bollard::API_DEFAULT_VERSION,
                ) {
                    if docker.ping().await.is_ok() {
                        let mut state = self.state.lock().await;
                        *state = RuntimeState::Ready;
                        return Ok(RuntimeState::Ready);
                    }
                }
            }
            // QEMU running but Docker not yet responsive — still starting.
            let mut state = self.state.lock().await;
            *state = RuntimeState::Starting;
            return Ok(RuntimeState::Starting);
        }

        // No running QEMU process. Check if the VM image is provisioned.
        let has_vmlinuz = self.data_dir.join("vmlinuz").exists();
        let has_initrd = self.data_dir.join("initrd").exists();
        let has_rootfs = self.data_dir.join("rootfs.qcow2").exists();

        if has_vmlinuz && has_initrd && has_rootfs {
            // Check prerequisites for starting.
            if !Path::new("/dev/kvm").exists() {
                let msg = "KVM not available (/dev/kvm not found). \
                           Ensure hardware virtualization is enabled in BIOS/UEFI."
                    .to_string();
                let mut state = self.state.lock().await;
                *state = RuntimeState::Error(msg.clone());
                return Ok(RuntimeState::Error(msg));
            }

            if LinuxRuntime::detect_qemu_binary().is_none() {
                let msg = "QEMU not found in PATH. \
                           Install qemu-system-x86_64 or qemu-system-aarch64."
                    .to_string();
                let mut state = self.state.lock().await;
                *state = RuntimeState::Error(msg.clone());
                return Ok(RuntimeState::Error(msg));
            }

            let mut state = self.state.lock().await;
            *state = RuntimeState::Provisioned;
            Ok(RuntimeState::Provisioned)
        } else {
            let mut state = self.state.lock().await;
            *state = RuntimeState::None;
            Ok(RuntimeState::None)
        }
    }

    /// Provision the runtime — download and prepare the VM image (§8).
    ///
    /// Progress stages: checking → downloading → extracting → configuring → complete.
    async fn provision(
        &self,
        on_progress: Box<dyn Fn(ProvisionProgress) + Send>,
    ) -> Result<(), AppError> {
        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Provisioning;
        }

        // Stage 1: Check prerequisites.
        on_progress(ProvisionProgress {
            stage: "checking".into(),
            percent: 5.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Checking KVM and QEMU availability...".into(),
        });

        if !Path::new("/dev/kvm").exists() {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Error("KVM not available".into());
            return Err(AppError::Runtime(
                "KVM not available. Ensure hardware virtualization is enabled in BIOS/UEFI."
                    .into(),
            ));
        }

        if LinuxRuntime::detect_qemu_binary().is_none() {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Error("QEMU not found".into());
            return Err(AppError::Runtime(
                "QEMU binary not found in PATH. \
                 Install qemu-system-x86_64 or qemu-system-aarch64."
                    .into(),
            ));
        }

        // Create data directory if it does not exist.
        tokio::fs::create_dir_all(&self.data_dir)
            .await
            .map_err(|e| {
                AppError::Runtime(format!("Failed to create data directory: {}", e))
            })?;

        // Stage 2: Download VM image.
        on_progress(ProvisionProgress {
            stage: "downloading".into(),
            percent: 10.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Downloading CrateBay Linux KVM runtime image...".into(),
        });

        // TODO: Implement actual image download from GitHub Release assets / CDN.
        // The download should fetch:
        //   - vmlinuz   (Alpine Linux kernel)
        //   - initrd    (initial ramdisk)
        //   - rootfs.qcow2 (QCOW2 disk image with Docker Engine pre-installed)
        //
        // Expected total download size: ~400 MB (compressed with zstd).
        // Resume support should be implemented for interrupted downloads.
        //
        // For now, return an error indicating this is not yet implemented.
        // When implemented, this section should:
        //   1. Determine the download URL based on architecture (x86_64 / aarch64)
        //   2. Stream the download with progress callbacks
        //   3. Verify checksum (SHA-256) of the downloaded archive
        //   4. Proceed to extraction stage

        on_progress(ProvisionProgress {
            stage: "downloading".into(),
            percent: 50.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Download not yet implemented — placeholder".into(),
        });

        // Stage 3: Extract.
        on_progress(ProvisionProgress {
            stage: "extracting".into(),
            percent: 70.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Extracting runtime image...".into(),
        });

        // TODO: Extract the downloaded zstd-compressed archive into data_dir:
        //   - data_dir/vmlinuz
        //   - data_dir/initrd
        //   - data_dir/rootfs.qcow2

        // Stage 4: Configure.
        on_progress(ProvisionProgress {
            stage: "configuring".into(),
            percent: 90.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Configuring runtime environment...".into(),
        });

        // TODO: Any post-extraction configuration:
        //   - Set file permissions on images
        //   - Generate runtime metadata file
        //   - Pre-validate QEMU can load the kernel

        // Stage 5: Complete.
        on_progress(ProvisionProgress {
            stage: "complete".into(),
            percent: 100.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Runtime provisioned successfully.".into(),
        });

        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Provisioned;
        }

        // NOTE: Returning an error until download/extract are implemented.
        // Remove this once the TODO sections above are filled in.
        Err(AppError::Runtime(
            "Linux KVM/QEMU provisioning: download and extraction not yet implemented".into(),
        ))
    }

    /// Start the QEMU VM (§11.2).
    ///
    /// 1. Verify KVM availability (`/dev/kvm`).
    /// 2. Verify VM images are provisioned.
    /// 3. Detect QEMU binary.
    /// 4. Build QEMU argument list via [`build_qemu_args`].
    /// 5. Spawn the QEMU process.
    /// 6. Wait for Docker to become responsive (30 s timeout).
    /// 7. Transition state to `Ready`.
    async fn start(&self) -> Result<(), AppError> {
        // Guard: Check current state.
        {
            let current = self.state.lock().await;
            match &*current {
                RuntimeState::Ready => {
                    return Ok(()); // Already running.
                }
                RuntimeState::Starting => {
                    return Err(AppError::Runtime("Runtime is already starting".into()));
                }
                RuntimeState::Provisioning => {
                    return Err(AppError::Runtime(
                        "Runtime is currently being provisioned".into(),
                    ));
                }
                _ => {} // Proceed with start.
            }
        }

        // Step 1: Check KVM (§11.2).
        if !Path::new("/dev/kvm").exists() {
            return Err(AppError::Runtime(
                "KVM not available. Ensure virtualization is enabled in BIOS/UEFI.".into(),
            ));
        }

        // Step 2: Check that VM images exist.
        let vmlinuz = self.data_dir.join("vmlinuz");
        let initrd = self.data_dir.join("initrd");
        let rootfs = self.data_dir.join("rootfs.qcow2");

        if !vmlinuz.exists() || !initrd.exists() || !rootfs.exists() {
            return Err(AppError::Runtime(
                "VM images not found. Run provision() first to download the runtime image.".into(),
            ));
        }

        // Step 3: Detect QEMU binary.
        let qemu_bin = LinuxRuntime::detect_qemu_binary().ok_or_else(|| {
            AppError::Runtime(
                "QEMU binary not found in PATH. \
                 Install qemu-system-x86_64 or qemu-system-aarch64."
                    .into(),
            )
        })?;

        // Transition to Starting state.
        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Starting;
        }

        // Step 4: Clean up any stale socket file from a previous run.
        let socket_path = self.docker_socket_path();
        if socket_path.exists() {
            let _ = tokio::fs::remove_file(&socket_path).await;
        }

        // Step 5: Build QEMU arguments and spawn the process.
        let args = self.build_qemu_args();

        tracing::info!(
            "Starting QEMU: {} {}",
            qemu_bin,
            args.join(" ")
        );

        let child = tokio::process::Command::new(&qemu_bin)
            .args(&args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null())
            .spawn()
            .map_err(|e| AppError::Runtime(format!("Failed to start QEMU: {}", e)))?;

        // Store the child PID.
        let pid = child.id().ok_or_else(|| {
            AppError::Runtime("Failed to obtain QEMU process ID".into())
        })?;

        {
            let mut qemu_pid = self.qemu_pid.lock().await;
            *qemu_pid = Some(pid);
        }
        {
            let mut started = self.started_at.lock().await;
            *started = Some(chrono::Utc::now().timestamp());
        }

        tracing::info!("QEMU process started with PID {}", pid);

        // Step 6: Wait for Docker to become responsive (30 s timeout).
        match self.wait_for_docker(Duration::from_secs(30)).await {
            Ok(()) => {
                let mut state = self.state.lock().await;
                *state = RuntimeState::Ready;
                tracing::info!("Docker is responsive — runtime is Ready");
                Ok(())
            }
            Err(e) => {
                // Docker did not come up in time. The QEMU process may still be
                // booting. Transition to Error state but leave the process running
                // so the health monitor can detect it later.
                let mut state = self.state.lock().await;
                *state = RuntimeState::Error(format!(
                    "QEMU started but Docker is not responsive: {}",
                    e
                ));
                Err(e)
            }
        }
    }

    /// Stop the QEMU VM gracefully (§3.1).
    ///
    /// 1. Send SIGTERM to the QEMU process.
    /// 2. Wait up to 10 seconds for graceful exit.
    /// 3. If still alive, send SIGKILL.
    /// 4. Clean up the Docker socket file.
    async fn stop(&self) -> Result<(), AppError> {
        let pid = {
            let guard = self.qemu_pid.lock().await;
            *guard
        };

        let pid = match pid {
            Some(p) => p,
            None => {
                // No tracked process — check if state is already stopped.
                let mut state = self.state.lock().await;
                *state = RuntimeState::Stopped;
                return Ok(());
            }
        };

        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Stopping;
        }

        tracing::info!("Stopping QEMU process (PID {})", pid);

        // Step 1: Send SIGTERM via the `kill` command.
        let term_result = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .output();
        if let Err(e) = term_result {
            tracing::warn!(
                "SIGTERM to PID {} failed: {}, process may have already exited",
                pid,
                e
            );
        }

        // Step 2: Wait up to 10 seconds for the process to exit.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            if !Self::is_qemu_process_alive(Some(pid)) {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                // Step 3: Force kill.
                tracing::warn!("QEMU PID {} did not exit after SIGTERM, sending SIGKILL", pid);
                let _ = std::process::Command::new("kill")
                    .args(["-KILL", &pid.to_string()])
                    .output();
                // Give it a moment to die.
                tokio::time::sleep(Duration::from_millis(500)).await;
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        // Step 4: Clean up.
        {
            let mut qemu_pid = self.qemu_pid.lock().await;
            *qemu_pid = None;
        }
        {
            let mut started = self.started_at.lock().await;
            *started = None;
        }

        // Remove the Docker socket file if it exists.
        let socket_path = self.docker_socket_path();
        if socket_path.exists() {
            let _ = tokio::fs::remove_file(&socket_path).await;
        }

        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Stopped;
        }

        tracing::info!("QEMU process stopped and cleaned up");
        Ok(())
    }

    /// Health check (§9.1).
    ///
    /// 1. Check if the QEMU process is alive.
    /// 2. Check if the Docker socket file exists.
    /// 3. Try Docker ping.
    async fn health_check(&self) -> Result<HealthStatus, AppError> {
        let pid = *self.qemu_pid.lock().await;
        let vm_running = Self::is_qemu_process_alive(pid);

        let socket_path = self.docker_socket_path();
        let socket_exists = socket_path.exists();

        let (docker_responsive, docker_version) = if socket_exists {
            let socket_str = socket_path.to_str().unwrap_or_default();
            match bollard::Docker::connect_with_unix(socket_str, 5, bollard::API_DEFAULT_VERSION) {
                Ok(docker) => {
                    let responsive = docker.ping().await.is_ok();
                    let version = if responsive {
                        docker
                            .version()
                            .await
                            .ok()
                            .and_then(|v| v.version)
                    } else {
                        None
                    };
                    (responsive, version)
                }
                Err(_) => (false, None),
            }
        } else {
            (false, None)
        };

        // Calculate uptime if we have a start timestamp.
        let uptime_seconds = if vm_running {
            let started = self.started_at.lock().await;
            started.map(|ts| {
                let now = chrono::Utc::now().timestamp();
                (now - ts).max(0) as u64
            })
        } else {
            None
        };

        // Determine runtime state from health signals.
        let runtime_state = if docker_responsive {
            RuntimeState::Ready
        } else if vm_running {
            RuntimeState::Starting
        } else {
            // Check if we had a process previously (unexpected exit).
            if pid.is_some() {
                RuntimeState::Error("QEMU process exited unexpectedly".into())
            } else {
                // No process tracked — could be stopped or never started.
                let current = self.state.lock().await;
                current.clone()
            }
        };

        // Update internal state to match health check result.
        {
            let mut state = self.state.lock().await;
            *state = runtime_state.clone();
        }

        Ok(HealthStatus {
            runtime_state,
            docker_responsive,
            docker_version,
            uptime_seconds,
            last_check: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Get the Docker socket path (§4.1).
    ///
    /// On Linux: `~/.cratebay/runtime/docker.sock`
    fn docker_socket_path(&self) -> PathBuf {
        self.data_dir.join("docker.sock")
    }

    /// Get current resource usage of the QEMU VM (§7.3).
    ///
    /// Reads from `/proc/{pid}/stat` and `/proc/{pid}/statm` to obtain
    /// CPU and memory usage of the QEMU process.
    async fn resource_usage(&self) -> Result<ResourceUsage, AppError> {
        let pid = *self.qemu_pid.lock().await;

        match pid {
            Some(pid) => {
                let cpu_percent = read_proc_cpu_percent(pid).await;
                let memory_used_mb = read_proc_memory_mb(pid).await;

                // Container count: query Docker if responsive.
                let container_count = {
                    let socket_path = self.docker_socket_path();
                    if socket_path.exists() {
                        if let Ok(docker) = bollard::Docker::connect_with_unix(
                            socket_path.to_str().unwrap_or_default(),
                            5,
                            bollard::API_DEFAULT_VERSION,
                        ) {
                            use bollard::container::ListContainersOptions;
                            let opts = ListContainersOptions::<String> {
                                all: false, // Only running containers.
                                ..Default::default()
                            };
                            docker
                                .list_containers(Some(opts))
                                .await
                                .map(|c| c.len() as u32)
                                .unwrap_or(0)
                        } else {
                            0
                        }
                    } else {
                        0
                    }
                };

                // Disk usage: check the rootfs.qcow2 apparent size.
                let disk_used_gb = {
                    let rootfs = self.data_dir.join("rootfs.qcow2");
                    tokio::fs::metadata(&rootfs)
                        .await
                        .map(|m| m.len() as f32 / (1024.0 * 1024.0 * 1024.0))
                        .unwrap_or(0.0)
                };

                Ok(ResourceUsage {
                    cpu_percent,
                    memory_used_mb,
                    memory_total_mb: self.config.memory_mb,
                    disk_used_gb,
                    disk_total_gb: self.config.disk_gb as f32,
                    container_count,
                })
            }
            None => {
                // No QEMU process running — return zeroed usage.
                Ok(ResourceUsage {
                    cpu_percent: 0.0,
                    memory_used_mb: 0,
                    memory_total_mb: self.config.memory_mb,
                    disk_used_gb: 0.0,
                    disk_total_gb: self.config.disk_gb as f32,
                    container_count: 0,
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// /proc helpers — read QEMU process resource usage on Linux
// ---------------------------------------------------------------------------

/// Read approximate CPU usage percentage from `/proc/{pid}/stat`.
///
/// This is a simplified single-sample approach: it reads total CPU ticks
/// consumed by the process and compares against system uptime. For more
/// accurate instantaneous CPU%, a two-sample delta approach would be needed.
async fn read_proc_cpu_percent(pid: u32) -> f32 {
    let stat_path = format!("/proc/{}/stat", pid);
    let stat_content = match tokio::fs::read_to_string(&stat_path).await {
        Ok(c) => c,
        Err(_) => return 0.0,
    };

    // /proc/{pid}/stat fields (space-separated):
    // Index 13 = utime (user ticks), Index 14 = stime (kernel ticks)
    // We skip the comm field (field 1) which may contain spaces by finding
    // the closing ')' first.
    let after_comm = match stat_content.rfind(')') {
        Some(pos) => &stat_content[pos + 2..], // skip ") "
        None => return 0.0,
    };

    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    // After comm: field 0 = state, field 1..N = remaining stat fields
    // utime is at original index 13 → after removing pid and comm (2 fields),
    // and state (1 field) → index 11 in this slice.
    if fields.len() < 13 {
        return 0.0;
    }

    let utime: u64 = fields[11].parse().unwrap_or(0);
    let stime: u64 = fields[12].parse().unwrap_or(0);
    let total_ticks = utime + stime;

    // Read system uptime from /proc/uptime.
    let uptime = match tokio::fs::read_to_string("/proc/uptime").await {
        Ok(c) => c
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(1.0),
        Err(_) => return 0.0,
    };

    // clock ticks per second — typically 100 on Linux.
    let clk_tck: f64 = 100.0;
    let process_seconds = total_ticks as f64 / clk_tck;

    if uptime > 0.0 {
        ((process_seconds / uptime) * 100.0) as f32
    } else {
        0.0
    }
}

/// Read memory usage in MB from `/proc/{pid}/statm`.
///
/// Field 1 (RSS — resident set size) is in pages. Page size is typically 4096.
async fn read_proc_memory_mb(pid: u32) -> u64 {
    let statm_path = format!("/proc/{}/statm", pid);
    let statm_content = match tokio::fs::read_to_string(&statm_path).await {
        Ok(c) => c,
        Err(_) => return 0,
    };

    let fields: Vec<&str> = statm_content.split_whitespace().collect();
    if fields.len() < 2 {
        return 0;
    }

    // Field 1 = RSS in pages.
    let rss_pages: u64 = fields[1].parse().unwrap_or(0);
    let page_size: u64 = 4096; // Standard Linux page size.

    (rss_pages * page_size) / (1024 * 1024)
}
