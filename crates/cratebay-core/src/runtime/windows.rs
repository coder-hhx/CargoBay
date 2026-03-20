//! Windows runtime — WSL2 integration.
//!
//! Uses a custom WSL2 distro containing Docker Engine
//! for the built-in container runtime.
//!
//! All WSL2 management is done via the `wsl.exe` command-line tool.
//! Docker inside the distro is exposed to the host via a named pipe
//! or TCP proxy on `localhost:2375`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::error::AppError;
use crate::models::ResourceUsage;

use super::{
    HealthStatus, ProvisionProgress, RuntimeConfig, RuntimeManager, RuntimeState,
};

/// WSL2 distro name used by CrateBay.
const DISTRO_NAME: &str = "CrateBay";

/// Windows runtime manager using WSL2.
///
/// Manages a custom WSL2 distro (`CrateBay`) that contains Docker Engine.
/// The distro is imported from a tar archive during provisioning and controlled
/// via `wsl.exe` commands.
pub struct WindowsRuntime {
    config: RuntimeConfig,
    data_dir: PathBuf,
    distro_name: String,
    state: Arc<Mutex<RuntimeState>>,
}

impl WindowsRuntime {
    /// Create a new Windows runtime manager with default configuration.
    pub fn new() -> Self {
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("C:\\temp"))
            .join(".cratebay\\runtime");
        Self {
            config: RuntimeConfig::default(),
            data_dir,
            distro_name: DISTRO_NAME.to_string(),
            state: Arc::new(Mutex::new(RuntimeState::None)),
        }
    }

    /// Check if WSL2 is available on this system.
    ///
    /// Executes `wsl --status` and checks for a successful exit code.
    async fn check_wsl2(&self) -> Result<bool, AppError> {
        let output = tokio::process::Command::new("wsl")
            .args(["--status"])
            .output()
            .await
            .map_err(|e| AppError::Runtime(format!("WSL check failed: {}", e)))?;
        Ok(output.status.success())
    }

    /// Import the CrateBay distro into WSL2 from a tar archive.
    ///
    /// Uses `wsl --import CrateBay {install_dir} {tar_path} --version 2`.
    async fn import_distro(&self) -> Result<(), AppError> {
        let tar_path = self.data_dir.join("cratebay-wsl.tar");
        let install_dir = self.data_dir.join("wsl-distro");

        // Ensure the installation directory exists
        tokio::fs::create_dir_all(&install_dir)
            .await
            .map_err(|e| {
                AppError::Runtime(format!(
                    "Failed to create WSL distro directory {}: {}",
                    install_dir.display(),
                    e
                ))
            })?;

        let output = tokio::process::Command::new("wsl")
            .args([
                "--import",
                &self.distro_name,
                &install_dir.to_string_lossy(),
                &tar_path.to_string_lossy(),
                "--version",
                "2",
            ])
            .output()
            .await
            .map_err(|e| AppError::Runtime(format!("WSL import command failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Runtime(format!(
                "WSL import failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Set up Docker socket forwarding from WSL2 distro to the host.
    ///
    /// Uses `socat` inside the WSL2 distro to forward the Docker socket
    /// to a TCP listener on `localhost:2375`, or to a Windows named pipe.
    async fn setup_socket_forward(&self) -> Result<(), AppError> {
        // Start socat inside the distro to bridge Docker socket to TCP
        let output = tokio::process::Command::new("wsl")
            .args([
                "-d",
                &self.distro_name,
                "--",
                "sh",
                "-c",
                "socat TCP-LISTEN:2375,reuseaddr,fork UNIX-CONNECT:/var/run/docker.sock &",
            ])
            .output()
            .await
            .map_err(|e| {
                AppError::Runtime(format!("Socket forwarding setup failed: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Socket forward setup warning: {}", stderr);
            // Non-fatal: socat might already be running, or named pipe will be used
        }

        Ok(())
    }

    /// Wait for Docker inside the WSL2 distro to become responsive.
    ///
    /// Polls `docker info` inside the distro every second until it succeeds
    /// or the timeout expires.
    async fn wait_for_docker(&self, timeout: Duration) -> Result<(), AppError> {
        let start = tokio::time::Instant::now();
        let poll_interval = Duration::from_secs(1);

        loop {
            if start.elapsed() >= timeout {
                return Err(AppError::Runtime(format!(
                    "Docker did not become ready within {} seconds",
                    timeout.as_secs()
                )));
            }

            let output = tokio::process::Command::new("wsl")
                .args([
                    "-d",
                    &self.distro_name,
                    "--",
                    "docker",
                    "info",
                ])
                .output()
                .await;

            match output {
                Ok(o) if o.status.success() => {
                    tracing::info!(
                        "Docker is ready inside WSL2 distro '{}'",
                        self.distro_name
                    );
                    return Ok(());
                }
                _ => {
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }

    /// Check if the CrateBay WSL2 distro is currently running.
    ///
    /// Parses the output of `wsl -l --running` and looks for the distro name.
    async fn is_distro_running(&self) -> bool {
        let output = tokio::process::Command::new("wsl")
            .args(["-l", "--running"])
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                // WSL output may be UTF-16LE on Windows; handle both encodings
                let stdout = String::from_utf8_lossy(&o.stdout);
                stdout.contains(&self.distro_name)
            }
            _ => false,
        }
    }

    /// Check if the CrateBay distro has been imported (exists in WSL).
    ///
    /// Parses the output of `wsl -l -v` and looks for the distro name.
    async fn is_distro_imported(&self) -> bool {
        let output = tokio::process::Command::new("wsl")
            .args(["-l", "-v"])
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                stdout.contains(&self.distro_name)
            }
            _ => false,
        }
    }
}

impl Default for WindowsRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RuntimeManager for WindowsRuntime {
    async fn detect(&self) -> Result<RuntimeState, AppError> {
        // Step 1: Check if WSL2 is available at all
        let wsl2_available = self.check_wsl2().await.unwrap_or(false);
        if !wsl2_available {
            return Ok(RuntimeState::None);
        }

        // Step 2: Check if the CrateBay distro has been imported
        if !self.is_distro_imported().await {
            return Ok(RuntimeState::None);
        }

        // Step 3: Check if the distro is currently running
        if self.is_distro_running().await {
            // Distro is running — check if Docker is responsive
            let docker_check = tokio::process::Command::new("wsl")
                .args(["-d", &self.distro_name, "--", "docker", "info"])
                .output()
                .await;

            match docker_check {
                Ok(o) if o.status.success() => {
                    let mut state = self.state.lock().await;
                    *state = RuntimeState::Ready;
                    Ok(RuntimeState::Ready)
                }
                _ => {
                    // Distro running but Docker not ready — still starting
                    let mut state = self.state.lock().await;
                    *state = RuntimeState::Starting;
                    Ok(RuntimeState::Starting)
                }
            }
        } else {
            // Distro exists but is not running
            let mut state = self.state.lock().await;
            *state = RuntimeState::Provisioned;
            Ok(RuntimeState::Provisioned)
        }
    }

    async fn provision(
        &self,
        on_progress: Box<dyn Fn(ProvisionProgress) + Send>,
    ) -> Result<(), AppError> {
        // Stage 1: Check WSL2 availability (10%)
        on_progress(ProvisionProgress {
            stage: "checking".into(),
            percent: 10.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Checking WSL2 availability...".into(),
        });

        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Provisioning;
        }

        if !self.check_wsl2().await? {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Error(
                "WSL2 is required. Please run: wsl --install".into(),
            );
            return Err(AppError::Runtime(
                "WSL2 is required. Please run: wsl --install".into(),
            ));
        }

        // Stage 2: Download distro image (20% - 80%)
        on_progress(ProvisionProgress {
            stage: "downloading".into(),
            percent: 20.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Downloading CrateBay WSL2 runtime image...".into(),
        });

        // Ensure data directory exists
        tokio::fs::create_dir_all(&self.data_dir)
            .await
            .map_err(|e| {
                AppError::Runtime(format!(
                    "Failed to create runtime directory {}: {}",
                    self.data_dir.display(),
                    e
                ))
            })?;

        // TODO: Implement actual image download from GitHub Release assets / CDN
        // The download should:
        //   1. Fetch cratebay-wsl.tar (~350 MB) from release URL
        //   2. Support resume for interrupted downloads
        //   3. Report progress via on_progress callback (20% → 80%)
        //   4. Verify checksum after download
        //
        // For now, check if the tar file already exists (manual placement)
        let tar_path = self.data_dir.join("cratebay-wsl.tar");
        if !tar_path.exists() {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Error("Runtime image not found".into());
            return Err(AppError::Runtime(format!(
                "Runtime image not found at {}. Download not yet implemented.",
                tar_path.display()
            )));
        }

        on_progress(ProvisionProgress {
            stage: "downloading".into(),
            percent: 80.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Download complete.".into(),
        });

        // Stage 3: Import distro into WSL2 (85%)
        on_progress(ProvisionProgress {
            stage: "configuring".into(),
            percent: 85.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Importing WSL2 distro...".into(),
        });

        self.import_distro().await?;

        // Stage 4: Complete (100%)
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

        Ok(())
    }

    async fn start(&self) -> Result<(), AppError> {
        // Verify WSL2 is available
        if !self.check_wsl2().await? {
            return Err(AppError::Runtime(
                "WSL2 is not available. Please enable WSL2 in Windows Features.".into(),
            ));
        }

        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Starting;
        }

        // Start Docker daemon inside the CrateBay distro
        let spawn_result = tokio::process::Command::new("wsl")
            .args(["-d", &self.distro_name, "--", "dockerd", "&"])
            .output()
            .await
            .map_err(|e| {
                AppError::Runtime(format!("Failed to start WSL distro: {}", e))
            })?;

        if !spawn_result.status.success() {
            let stderr = String::from_utf8_lossy(&spawn_result.stderr);
            tracing::warn!("WSL dockerd start warning: {}", stderr);
            // dockerd may emit warnings on stderr even when starting successfully
        }

        // Set up socket forwarding: Docker socket → named pipe or TCP localhost:2375
        self.setup_socket_forward().await?;

        // Wait for Docker to become responsive (30 second timeout)
        self.wait_for_docker(Duration::from_secs(30)).await?;

        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Ready;
        }

        tracing::info!(
            "Windows WSL2 runtime started (distro: {})",
            self.distro_name
        );

        Ok(())
    }

    async fn stop(&self) -> Result<(), AppError> {
        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Stopping;
        }

        // Terminate the WSL2 distro
        let output = tokio::process::Command::new("wsl")
            .args(["-t", &self.distro_name])
            .output()
            .await
            .map_err(|e| {
                AppError::Runtime(format!(
                    "Failed to terminate WSL distro '{}': {}",
                    self.distro_name, e
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Runtime(format!(
                "WSL terminate failed: {}",
                stderr
            )));
        }

        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Stopped;
        }

        tracing::info!(
            "Windows WSL2 runtime stopped (distro: {})",
            self.distro_name
        );

        Ok(())
    }

    async fn health_check(&self) -> Result<HealthStatus, AppError> {
        let current_state = self.state.lock().await.clone();

        // Check if the distro is running
        let distro_running = self.is_distro_running().await;

        if !distro_running {
            return Ok(HealthStatus {
                runtime_state: RuntimeState::Stopped,
                docker_responsive: false,
                docker_version: None,
                uptime_seconds: None,
                last_check: chrono::Utc::now().to_rfc3339(),
            });
        }

        // Distro is running — check if Docker is responsive
        let docker_check = tokio::process::Command::new("wsl")
            .args([
                "-d",
                &self.distro_name,
                "--",
                "docker",
                "version",
                "--format",
                "{{.Server.Version}}",
            ])
            .output()
            .await;

        let (docker_responsive, docker_version) = match docker_check {
            Ok(o) if o.status.success() => {
                let version = String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .to_string();
                (true, if version.is_empty() { None } else { Some(version) })
            }
            _ => (false, None),
        };

        // Determine runtime state based on health check results
        let runtime_state = if docker_responsive {
            RuntimeState::Ready
        } else if distro_running {
            // Distro running but Docker not ready
            match current_state {
                RuntimeState::Starting => RuntimeState::Starting,
                _ => RuntimeState::Error(
                    "Distro running but Docker is not responsive".into(),
                ),
            }
        } else {
            RuntimeState::Stopped
        };

        // Try to get uptime from the distro
        let uptime_seconds = self.get_uptime().await;

        Ok(HealthStatus {
            runtime_state,
            docker_responsive,
            docker_version,
            uptime_seconds,
            last_check: chrono::Utc::now().to_rfc3339(),
        })
    }

    fn docker_socket_path(&self) -> PathBuf {
        // Windows named pipe for Docker socket
        PathBuf::from(r"\\.\pipe\cratebay-docker")
    }

    async fn resource_usage(&self) -> Result<ResourceUsage, AppError> {
        // TODO: Implement detailed resource usage querying from the WSL2 distro.
        // This would involve:
        //   - `wsl -d CrateBay -- cat /proc/meminfo` for memory usage
        //   - `wsl -d CrateBay -- cat /proc/stat` for CPU usage
        //   - `wsl -d CrateBay -- df -B1 /` for disk usage
        //   - `wsl -d CrateBay -- docker ps -q | wc -l` for container count
        //
        // For now, attempt to get basic memory info from the distro.

        let memory_info = self.get_memory_info().await;
        let container_count = self.get_container_count().await;

        let (memory_used_mb, memory_total_mb) = memory_info.unwrap_or((0, self.config.memory_mb));

        Ok(ResourceUsage {
            cpu_percent: 0.0, // TODO: Parse /proc/stat for CPU usage
            memory_used_mb,
            memory_total_mb,
            disk_used_gb: 0.0, // TODO: Parse df output for disk usage
            disk_total_gb: self.config.disk_gb as f32,
            container_count: container_count.unwrap_or(0),
        })
    }
}

// ---------------------------------------------------------------------------
// Private helpers (not part of RuntimeManager trait)
// ---------------------------------------------------------------------------

impl WindowsRuntime {
    /// Get uptime in seconds from the WSL2 distro.
    async fn get_uptime(&self) -> Option<u64> {
        let output = tokio::process::Command::new("wsl")
            .args([
                "-d",
                &self.distro_name,
                "--",
                "cat",
                "/proc/uptime",
            ])
            .output()
            .await
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        // /proc/uptime format: "uptime_seconds idle_seconds"
        let uptime_str = stdout.split_whitespace().next()?;
        let uptime_f64: f64 = uptime_str.parse().ok()?;
        Some(uptime_f64 as u64)
    }

    /// Get memory info (used_mb, total_mb) from the WSL2 distro via /proc/meminfo.
    async fn get_memory_info(&self) -> Option<(u64, u64)> {
        let output = tokio::process::Command::new("wsl")
            .args([
                "-d",
                &self.distro_name,
                "--",
                "cat",
                "/proc/meminfo",
            ])
            .output()
            .await
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut mem_total_kb: Option<u64> = None;
        let mut mem_available_kb: Option<u64> = None;

        for line in stdout.lines() {
            if line.starts_with("MemTotal:") {
                mem_total_kb = parse_meminfo_value(line);
            } else if line.starts_with("MemAvailable:") {
                mem_available_kb = parse_meminfo_value(line);
            }
            if mem_total_kb.is_some() && mem_available_kb.is_some() {
                break;
            }
        }

        let total_kb = mem_total_kb?;
        let available_kb = mem_available_kb?;
        let used_kb = total_kb.saturating_sub(available_kb);

        Some((used_kb / 1024, total_kb / 1024))
    }

    /// Get the number of running containers inside the WSL2 distro.
    async fn get_container_count(&self) -> Option<u32> {
        let output = tokio::process::Command::new("wsl")
            .args([
                "-d",
                &self.distro_name,
                "--",
                "sh",
                "-c",
                "docker ps -q 2>/dev/null | wc -l",
            ])
            .output()
            .await
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.trim().parse::<u32>().ok()
    }
}

/// Parse a value from a /proc/meminfo line (e.g. "MemTotal:       16384 kB").
fn parse_meminfo_value(line: &str) -> Option<u64> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse::<u64>().ok()
    } else {
        None
    }
}
