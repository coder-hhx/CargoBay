//! macOS runtime — VZ.framework integration.
//!
//! Uses Apple's Virtualization.framework to run a lightweight Linux VM
//! with Docker Engine inside. The VM exposes its Docker socket via vsock,
//! which is bridged to a Unix socket on the host.
//!
//! # Architecture (runtime-spec.md §2.1)
//!
//! ```text
//! macOS Host
//! └── CrateBay.app
//!     └── Rust Backend
//!         └── VZ.framework API calls
//!             └── VZVirtualMachine
//!                 ├── VZLinuxBootLoader (vmlinuz + initrd)
//!                 ├── VZVirtioBlockStorageDevice (rootfs.img)
//!                 ├── VZVirtioFileSystemDevice (shared dirs)
//!                 ├── VZVirtioSocketDevice (vsock)
//!                 └── VZNATNetworkDeviceAttachment
//!                     └── Alpine Linux → Docker Engine
//! ```

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use crate::error::AppError;
use crate::models::ResourceUsage;
use crate::MutexExt;

use super::{
    HealthStatus, ProvisionProgress, RuntimeConfig, RuntimeManager, RuntimeState,
};

/// Minimum macOS version required for VZ.framework (macOS 13 Ventura).
const MIN_MACOS_VERSION: u32 = 13;

// ---------------------------------------------------------------------------
// VZ Virtual Machine Configuration Types
// ---------------------------------------------------------------------------

/// VZ.framework virtual machine configuration.
///
/// This struct describes the desired VM configuration. Actual VZ.framework
/// calls require Objective-C/Swift bridging and are marked TODO.
#[derive(Debug)]
#[allow(dead_code)]
struct VmConfiguration {
    /// Path to the Linux kernel (vmlinuz).
    kernel_path: PathBuf,
    /// Path to the initial ramdisk (initrd).
    initrd_path: PathBuf,
    /// Path to the root filesystem image.
    rootfs_path: PathBuf,
    /// Number of CPU cores for the VM.
    cpu_count: u32,
    /// Memory size in bytes.
    memory_size: u64,
    /// Shared directory configurations (VirtioFS).
    shared_dirs: Vec<super::SharedDir>,
}

// ---------------------------------------------------------------------------
// MacOSRuntime (§11.1)
// ---------------------------------------------------------------------------

/// macOS runtime manager using Apple's VZ.framework.
pub struct MacOSRuntime {
    /// Runtime configuration (CPU, memory, disk, shared dirs).
    config: RuntimeConfig,
    /// Data directory for VM images and socket: `~/.cratebay/runtime/`.
    data_dir: PathBuf,
    /// Current runtime state, shared across async operations.
    state: Arc<Mutex<RuntimeState>>,
}

impl MacOSRuntime {
    /// Create a new macOS runtime manager with default configuration.
    pub fn new() -> Self {
        let data_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".cratebay/runtime");
        Self {
            config: RuntimeConfig::default(),
            data_dir,
            state: Arc::new(Mutex::new(RuntimeState::None)),
        }
    }

    /// Create a new macOS runtime manager with custom configuration.
    #[allow(dead_code)]
    pub fn with_config(config: RuntimeConfig, data_dir: PathBuf) -> Self {
        Self {
            config,
            data_dir,
            state: Arc::new(Mutex::new(RuntimeState::None)),
        }
    }

    /// Build the VZ virtual machine configuration from current settings.
    ///
    /// Describes the desired VM layout:
    /// - VZLinuxBootLoader with vmlinuz + initrd
    /// - VZVirtioBlockStorageDevice with rootfs.img
    /// - VZVirtioFileSystemDevice for each shared directory
    /// - VZNATNetworkDeviceAttachment for networking
    /// - VZVirtioSocketDevice for Docker socket exposure
    #[allow(dead_code)]
    fn create_vm_config(&self) -> Result<VmConfiguration, AppError> {
        let kernel_path = self.data_dir.join("vmlinuz");
        let initrd_path = self.data_dir.join("initrd");
        let rootfs_path = self.data_dir.join("rootfs.img");

        // Verify required files exist
        if !kernel_path.exists() {
            return Err(AppError::Runtime(format!(
                "Kernel image not found: {}",
                kernel_path.display()
            )));
        }
        if !initrd_path.exists() {
            return Err(AppError::Runtime(format!(
                "Initrd not found: {}",
                initrd_path.display()
            )));
        }
        if !rootfs_path.exists() {
            return Err(AppError::Runtime(format!(
                "Root filesystem not found: {}",
                rootfs_path.display()
            )));
        }

        Ok(VmConfiguration {
            kernel_path,
            initrd_path,
            rootfs_path,
            cpu_count: self.config.cpu_cores,
            memory_size: self.config.memory_mb * 1024 * 1024,
            shared_dirs: self.config.shared_dirs.clone(),
        })
    }

    /// Check if the host macOS version meets the minimum requirement.
    ///
    /// VZ.framework requires macOS 13 (Ventura) or later.
    fn check_macos_version() -> Result<bool, AppError> {
        let output = Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .map_err(|e| {
                AppError::Runtime(format!("Failed to check macOS version: {}", e))
            })?;

        let version_str = String::from_utf8_lossy(&output.stdout);
        let version_str = version_str.trim();

        // Parse major version from "13.x.x" or "14.x.x" format
        let major: u32 = version_str
            .split('.')
            .next()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        Ok(major >= MIN_MACOS_VERSION)
    }

    /// Check if VM image files (vmlinuz, initrd, rootfs.img) are present.
    fn has_vm_images(&self) -> bool {
        self.data_dir.join("vmlinuz").exists()
            && self.data_dir.join("initrd").exists()
            && self.data_dir.join("rootfs.img").exists()
    }

    /// Check if the VM process is alive.
    ///
    /// TODO: Once VZ.framework bridge is implemented, this should check
    /// the actual VZVirtualMachine state.
    fn is_vm_process_alive(&self) -> bool {
        // TODO: Check VZVirtualMachine.state == .running
        // For now, check if the docker socket exists as a proxy signal
        self.docker_socket_path().exists()
    }

    /// Wait for Docker inside the VM to become responsive.
    ///
    /// Polls the Docker socket with a ping every second until Docker
    /// responds or the timeout is reached.
    #[allow(dead_code)]
    async fn wait_for_docker(&self, timeout: Duration) -> Result<(), AppError> {
        let socket_path = self.docker_socket_path();
        let start = std::time::Instant::now();

        tracing::info!(
            "Waiting for Docker at {} (timeout: {:?})",
            socket_path.display(),
            timeout
        );

        while start.elapsed() < timeout {
            if socket_path.exists() {
                match bollard::Docker::connect_with_unix(
                    socket_path.to_str().unwrap_or_default(),
                    5,
                    bollard::API_DEFAULT_VERSION,
                ) {
                    Ok(docker) => {
                        if docker.ping().await.is_ok() {
                            tracing::info!(
                                "Docker is responsive at {}",
                                socket_path.display()
                            );
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        tracing::trace!("Docker not yet ready: {}", e);
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        Err(AppError::Runtime(format!(
            "Docker did not become responsive within {:?}",
            timeout
        )))
    }

    /// Set up the vsock → Unix socket bridge for Docker access.
    ///
    /// The VM exposes Docker's socket via VZVirtioSocketDevice (vsock).
    /// This method creates a Unix socket on the host that proxies
    /// connections to the VM's vsock.
    ///
    /// TODO: Implement actual vsock bridging once VZ.framework
    /// Objective-C/Swift bridge is in place.
    #[allow(dead_code)]
    async fn setup_socket_bridge(&self) -> Result<(), AppError> {
        let socket_path = self.docker_socket_path();
        tracing::info!(
            "Setting up vsock → Unix socket bridge at {}",
            socket_path.display()
        );

        // TODO: Implement vsock (AF_VSOCK) to Unix socket proxy
        // 1. Listen on VZVirtioSocketDevice for incoming connections
        // 2. Create a Unix socket at docker_socket_path()
        // 3. Forward data between vsock and Unix socket

        // Ensure parent directory exists
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::Runtime(format!(
                    "Failed to create socket directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        Err(AppError::Runtime(
            "vsock to Unix socket bridge not yet implemented (requires VZ.framework bridge)"
                .into(),
        ))
    }

    /// Update the internal runtime state.
    fn set_state(&self, new_state: RuntimeState) -> Result<(), AppError> {
        let mut state = self.state.lock_or_recover()?;
        tracing::info!("Runtime state: {:?} → {:?}", *state, new_state);
        *state = new_state;
        Ok(())
    }

    /// Read the current internal runtime state.
    fn get_state(&self) -> Result<RuntimeState, AppError> {
        let state = self.state.lock_or_recover()?;
        Ok(state.clone())
    }
}

impl Default for MacOSRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RuntimeManager for MacOSRuntime {
    /// Detect the current runtime state.
    ///
    /// Checks:
    /// 1. macOS version >= 13 (Ventura) for VZ.framework support
    /// 2. Whether VM images (vmlinuz, initrd, rootfs.img) exist
    /// 3. Whether the Docker socket is active (VM is running)
    async fn detect(&self) -> Result<RuntimeState, AppError> {
        // Check macOS version
        match Self::check_macos_version() {
            Ok(true) => {}
            Ok(false) => {
                let state = RuntimeState::Error(format!(
                    "macOS {} or later is required for VZ.framework",
                    MIN_MACOS_VERSION
                ));
                self.set_state(state.clone())?;
                return Ok(state);
            }
            Err(e) => {
                tracing::warn!("Could not determine macOS version: {}", e);
                // Continue — may still work
            }
        }

        // Check if VM images are present
        if !self.has_vm_images() {
            self.set_state(RuntimeState::None)?;
            return Ok(RuntimeState::None);
        }

        // Check if the VM is currently running (Docker socket active)
        if self.is_vm_process_alive() {
            // Verify Docker is actually responsive
            let socket_path = self.docker_socket_path();
            if let Ok(docker) = bollard::Docker::connect_with_unix(
                socket_path.to_str().unwrap_or_default(),
                5,
                bollard::API_DEFAULT_VERSION,
            ) {
                if docker.ping().await.is_ok() {
                    self.set_state(RuntimeState::Ready)?;
                    return Ok(RuntimeState::Ready);
                }
            }
            // Socket exists but Docker not responsive — could be starting
            self.set_state(RuntimeState::Starting)?;
            return Ok(RuntimeState::Starting);
        }

        // Images exist but VM is not running
        self.set_state(RuntimeState::Provisioned)?;
        Ok(RuntimeState::Provisioned)
    }

    /// Download and prepare the VM image (first-run provisioning).
    ///
    /// Stages (§8.1):
    /// 1. `checking` — Verify macOS version and VZ.framework
    /// 2. `downloading` — Download Alpine Linux VM image (~400 MB)
    /// 3. `extracting` — Extract rootfs, kernel, initrd
    /// 4. `configuring` — Configure Docker Engine inside VM image
    /// 5. `complete` — Provisioning finished
    async fn provision(
        &self,
        on_progress: Box<dyn Fn(ProvisionProgress) + Send>,
    ) -> Result<(), AppError> {
        self.set_state(RuntimeState::Provisioning)?;

        // Stage 1: Check prerequisites
        on_progress(ProvisionProgress {
            stage: "checking".into(),
            percent: 5.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Checking macOS version and VZ.framework availability...".into(),
        });

        let version_ok = Self::check_macos_version()?;
        if !version_ok {
            self.set_state(RuntimeState::Error(
                "macOS version too old for VZ.framework".into(),
            ))?;
            return Err(AppError::Runtime(format!(
                "macOS {} or later is required for Virtualization.framework",
                MIN_MACOS_VERSION
            )));
        }

        // Create data directory
        std::fs::create_dir_all(&self.data_dir).map_err(|e| {
            AppError::Runtime(format!(
                "Failed to create runtime directory {}: {}",
                self.data_dir.display(),
                e
            ))
        })?;

        on_progress(ProvisionProgress {
            stage: "checking".into(),
            percent: 10.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Prerequisites verified.".into(),
        });

        // Stage 2: Download VM image
        on_progress(ProvisionProgress {
            stage: "downloading".into(),
            percent: 15.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Downloading CrateBay runtime image...".into(),
        });

        // TODO: Implement actual download from GitHub Release / CDN
        // The download should:
        // 1. Determine arch (aarch64 vs x86_64) via std::env::consts::ARCH
        // 2. Construct download URL for the compressed VM image
        // 3. Stream download with progress reporting via on_progress
        // 4. Support resume for interrupted downloads
        // 5. Verify checksum after download
        //
        // Expected files after download + extraction:
        //   ~/.cratebay/runtime/vmlinuz
        //   ~/.cratebay/runtime/initrd
        //   ~/.cratebay/runtime/rootfs.img
        //
        // For now, report error since download is not yet implemented.

        self.set_state(RuntimeState::Error(
            "VM image download not yet implemented".into(),
        ))?;
        return Err(AppError::Runtime(
            "VM image download not yet implemented — \
             CDN URL and image packaging are required before provisioning can work"
                .into(),
        ));

        // The following stages will execute once download is implemented:

        // Stage 3: Extract image
        // on_progress(ProvisionProgress {
        //     stage: "extracting".into(),
        //     percent: 75.0,
        //     bytes_downloaded: bytes_total,
        //     bytes_total,
        //     message: "Extracting runtime image...".into(),
        // });
        // TODO: Decompress zstd archive → vmlinuz + initrd + rootfs.img

        // Stage 4: Configure
        // on_progress(ProvisionProgress {
        //     stage: "configuring".into(),
        //     percent: 90.0,
        //     bytes_downloaded: bytes_total,
        //     bytes_total,
        //     message: "Configuring Docker Engine...".into(),
        // });
        // TODO: Patch rootfs.img if needed (e.g., inject ssh keys, network config)

        // Stage 5: Complete
        // on_progress(ProvisionProgress {
        //     stage: "complete".into(),
        //     percent: 100.0,
        //     bytes_downloaded: bytes_total,
        //     bytes_total,
        //     message: "Runtime provisioned successfully.".into(),
        // });
        // self.set_state(RuntimeState::Provisioned)?;
        // Ok(())
    }

    /// Start the VZ.framework virtual machine.
    ///
    /// Flow (§11.1):
    /// 1. Verify VM images exist
    /// 2. Build VM configuration (create_vm_config)
    /// 3. Boot the VM (VZVirtualMachine.start)
    /// 4. Wait for Docker inside the VM to become responsive
    /// 5. Set up vsock → Unix socket bridge
    /// 6. Transition state to Ready
    async fn start(&self) -> Result<(), AppError> {
        // Verify images are provisioned
        if !self.has_vm_images() {
            return Err(AppError::Runtime(
                "Runtime not provisioned. Call provision() first.".into(),
            ));
        }

        self.set_state(RuntimeState::Starting)?;

        // Build VM configuration
        let _vm_config = self.create_vm_config()?;

        // TODO: Create VZVirtualMachine with the configuration
        // This requires Objective-C/Swift bridging to VZ.framework:
        //
        // let boot_loader = VZLinuxBootLoader::new(&vm_config.kernel_path, &vm_config.initrd_path)?;
        // let disk = VZVirtioBlockStorageDevice::new(&vm_config.rootfs_path)?;
        // let shared = VZVirtioFileSystemDevice::new("shared", &vm_config.shared_dirs)?;
        // let network = VZNATNetworkDeviceAttachment::new();
        // let vsock = VZVirtioSocketDevice::new();
        //
        // let vz_config = VZVirtualMachineConfiguration {
        //     boot_loader,
        //     cpu_count: vm_config.cpu_count,
        //     memory_size: vm_config.memory_size,
        //     storage_devices: vec![disk],
        //     filesystem_devices: vec![shared],
        //     network_devices: vec![network],
        //     socket_devices: vec![vsock],
        // };
        //
        // let vm = VZVirtualMachine::new(vz_config)?;
        // vm.start().await?;

        tracing::warn!(
            "VZ.framework VM start is not yet implemented \
             (requires Objective-C/Swift bridge)"
        );

        // Wait for Docker inside the VM to be ready (30s timeout)
        // self.wait_for_docker(Duration::from_secs(30)).await?;

        // Set up vsock → Unix socket bridge for Docker access
        // self.setup_socket_bridge().await?;

        // self.set_state(RuntimeState::Ready)?;
        // tracing::info!("macOS VZ runtime started successfully");
        // Ok(())

        self.set_state(RuntimeState::Error(
            "VZ.framework bridge not yet implemented".into(),
        ))?;
        Err(AppError::Runtime(
            "VZ.framework VM start requires Objective-C/Swift bridge (not yet implemented)"
                .into(),
        ))
    }

    /// Stop the VZ.framework virtual machine gracefully.
    ///
    /// Flow:
    /// 1. Transition state to Stopping
    /// 2. Request VM to stop (VZVirtualMachine.stop)
    /// 3. Clean up Docker socket
    /// 4. Transition state to Stopped
    async fn stop(&self) -> Result<(), AppError> {
        let current = self.get_state()?;
        if current == RuntimeState::Stopped || current == RuntimeState::None {
            tracing::info!("Runtime is already stopped");
            return Ok(());
        }

        self.set_state(RuntimeState::Stopping)?;

        // TODO: Stop the VZ virtual machine
        // vm.stop().await?;
        // or vm.requestStop() for graceful shutdown

        // Clean up the Docker socket file
        let socket_path = self.docker_socket_path();
        if socket_path.exists() {
            if let Err(e) = std::fs::remove_file(&socket_path) {
                tracing::warn!(
                    "Failed to remove Docker socket {}: {}",
                    socket_path.display(),
                    e
                );
            }
        }

        self.set_state(RuntimeState::Stopped)?;
        tracing::info!("macOS VZ runtime stopped");
        Ok(())
    }

    /// Check runtime health and Docker responsiveness.
    ///
    /// Health check protocol (§9.1):
    /// 1. Check if VM process is running
    /// 2. Check if Docker socket exists
    /// 3. Try Docker ping (5 second timeout)
    /// 4. Get Docker version if responsive
    async fn health_check(&self) -> Result<HealthStatus, AppError> {
        let vm_running = self.is_vm_process_alive();
        let socket_path = self.docker_socket_path();
        let socket_exists = socket_path.exists();

        let mut docker_responsive = false;
        let mut docker_version = None;

        if socket_exists {
            if let Ok(docker) = bollard::Docker::connect_with_unix(
                socket_path.to_str().unwrap_or_default(),
                5,
                bollard::API_DEFAULT_VERSION,
            ) {
                if docker.ping().await.is_ok() {
                    docker_responsive = true;

                    // Try to get version info
                    if let Ok(version) = docker.version().await {
                        docker_version = version.version;
                    }
                }
            }
        }

        let runtime_state = if docker_responsive {
            RuntimeState::Ready
        } else if vm_running {
            RuntimeState::Starting
        } else if self.has_vm_images() {
            // Images provisioned but not running
            match self.get_state()? {
                RuntimeState::Stopping => RuntimeState::Stopped,
                other => other,
            }
        } else {
            RuntimeState::None
        };

        self.set_state(runtime_state.clone())?;

        // TODO: Calculate actual VM uptime once VZ.framework bridge is implemented
        let uptime_seconds = if docker_responsive {
            Some(0_u64) // Placeholder
        } else {
            Option::None
        };

        Ok(HealthStatus {
            runtime_state,
            docker_responsive,
            docker_version,
            uptime_seconds,
            last_check: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Get the Docker socket path: `~/.cratebay/runtime/docker.sock`.
    fn docker_socket_path(&self) -> PathBuf {
        self.data_dir.join("docker.sock")
    }

    /// Get current resource usage of the VM.
    ///
    /// TODO: Query actual VM resource usage once VZ.framework bridge is
    /// implemented. Metrics include CPU, memory, disk, and container count.
    async fn resource_usage(&self) -> Result<ResourceUsage, AppError> {
        // TODO: Once VZ.framework bridge is in place:
        // 1. Query VM CPU usage via VZVirtualMachine APIs
        // 2. Query memory usage from VM's /proc/meminfo via vsock
        // 3. Query disk usage from VM's df output via vsock
        // 4. Query container count via Docker API

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_runtime(data_dir: PathBuf) -> MacOSRuntime {
        MacOSRuntime {
            config: RuntimeConfig::default(),
            data_dir,
            state: Arc::new(Mutex::new(RuntimeState::None)),
        }
    }

    #[test]
    fn new_creates_with_defaults() {
        let rt = MacOSRuntime::new();
        assert_eq!(rt.config.cpu_cores, 2);
        assert_eq!(rt.config.memory_mb, 2048);
        assert!(rt.data_dir.ends_with(".cratebay/runtime"));
    }

    #[test]
    fn docker_socket_path_is_correct() {
        let rt = test_runtime(PathBuf::from("/tmp/test-runtime"));
        assert_eq!(
            rt.docker_socket_path(),
            PathBuf::from("/tmp/test-runtime/docker.sock")
        );
    }

    #[test]
    fn has_vm_images_false_when_missing() {
        let rt = test_runtime(PathBuf::from("/tmp/nonexistent-runtime-dir"));
        assert!(!rt.has_vm_images());
    }

    #[test]
    fn check_macos_version_does_not_panic() {
        // This test just verifies the function doesn't panic.
        // The actual result depends on the host OS.
        let _ = MacOSRuntime::check_macos_version();
    }

    #[test]
    fn state_management_works() {
        let rt = test_runtime(PathBuf::from("/tmp/test-runtime"));
        assert_eq!(rt.get_state().unwrap(), RuntimeState::None);

        rt.set_state(RuntimeState::Provisioning).unwrap();
        assert_eq!(rt.get_state().unwrap(), RuntimeState::Provisioning);

        rt.set_state(RuntimeState::Ready).unwrap();
        assert_eq!(rt.get_state().unwrap(), RuntimeState::Ready);
    }

    #[test]
    fn is_vm_process_alive_false_when_no_socket() {
        let rt = test_runtime(PathBuf::from("/tmp/nonexistent-runtime-dir"));
        assert!(!rt.is_vm_process_alive());
    }

    #[tokio::test]
    async fn detect_returns_none_when_no_images() {
        let rt = test_runtime(PathBuf::from("/tmp/nonexistent-runtime-dir"));
        let state = rt.detect().await.unwrap();
        assert_eq!(state, RuntimeState::None);
    }

    #[tokio::test]
    async fn stop_succeeds_when_already_stopped() {
        let rt = test_runtime(PathBuf::from("/tmp/nonexistent-runtime-dir"));
        // State is None, stop should succeed without error
        let result = rt.stop().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn start_fails_without_images() {
        let rt = test_runtime(PathBuf::from("/tmp/nonexistent-runtime-dir"));
        let result = rt.start().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not provisioned"));
    }

    #[tokio::test]
    async fn health_check_returns_none_when_no_images() {
        let rt = test_runtime(PathBuf::from("/tmp/nonexistent-runtime-dir"));
        let status = rt.health_check().await.unwrap();
        assert_eq!(status.runtime_state, RuntimeState::None);
        assert!(!status.docker_responsive);
        assert!(status.docker_version.is_none());
    }

    #[tokio::test]
    async fn resource_usage_returns_defaults() {
        let rt = test_runtime(PathBuf::from("/tmp/nonexistent-runtime-dir"));
        let usage = rt.resource_usage().await.unwrap();
        assert_eq!(usage.cpu_percent, 0.0);
        assert_eq!(usage.memory_total_mb, 2048);
        assert_eq!(usage.disk_total_gb, 20.0);
        assert_eq!(usage.container_count, 0);
    }

    #[test]
    fn default_trait_creates_same_as_new() {
        let from_new = MacOSRuntime::new();
        let from_default = MacOSRuntime::default();
        assert_eq!(from_new.config.cpu_cores, from_default.config.cpu_cores);
        assert_eq!(from_new.config.memory_mb, from_default.config.memory_mb);
        assert_eq!(from_new.config.disk_gb, from_default.config.disk_gb);
        assert_eq!(from_new.data_dir, from_default.data_dir);
    }

    #[test]
    fn with_config_uses_provided_values() {
        let config = RuntimeConfig {
            cpu_cores: 8,
            memory_mb: 8192,
            disk_gb: 100,
            auto_start: false,
            shared_dirs: vec![super::super::SharedDir {
                host_path: "/Users/test".into(),
                tag: "test".into(),
            }],
        };
        let rt = MacOSRuntime::with_config(config, PathBuf::from("/custom/path"));
        assert_eq!(rt.config.cpu_cores, 8);
        assert_eq!(rt.config.memory_mb, 8192);
        assert_eq!(rt.config.disk_gb, 100);
        assert!(!rt.config.auto_start);
        assert_eq!(rt.config.shared_dirs.len(), 1);
        assert_eq!(rt.data_dir, PathBuf::from("/custom/path"));
    }

    #[test]
    fn state_transitions_through_full_lifecycle() {
        let rt = test_runtime(PathBuf::from("/tmp/test-runtime"));
        // None → Provisioning → Provisioned → Starting → Ready → Stopping → Stopped
        let transitions = vec![
            RuntimeState::None,
            RuntimeState::Provisioning,
            RuntimeState::Provisioned,
            RuntimeState::Starting,
            RuntimeState::Ready,
            RuntimeState::Stopping,
            RuntimeState::Stopped,
        ];
        for expected in transitions {
            rt.set_state(expected.clone()).unwrap();
            assert_eq!(rt.get_state().unwrap(), expected);
        }
    }

    #[test]
    fn state_can_transition_to_error() {
        let rt = test_runtime(PathBuf::from("/tmp/test-runtime"));
        rt.set_state(RuntimeState::Starting).unwrap();
        rt.set_state(RuntimeState::Error("VZ.framework failed".into()))
            .unwrap();
        let state = rt.get_state().unwrap();
        match state {
            RuntimeState::Error(msg) => assert_eq!(msg, "VZ.framework failed"),
            other => panic!("Expected Error state, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn resource_usage_with_custom_config() {
        let config = RuntimeConfig {
            cpu_cores: 4,
            memory_mb: 4096,
            disk_gb: 50,
            auto_start: true,
            shared_dirs: vec![],
        };
        let rt = MacOSRuntime::with_config(
            config,
            PathBuf::from("/tmp/nonexistent-runtime-dir"),
        );
        let usage = rt.resource_usage().await.unwrap();
        assert_eq!(usage.memory_total_mb, 4096);
        assert_eq!(usage.disk_total_gb, 50.0);
        assert_eq!(usage.memory_used_mb, 0);
        assert_eq!(usage.cpu_percent, 0.0);
    }

    #[tokio::test]
    async fn detect_updates_internal_state() {
        let rt = test_runtime(PathBuf::from("/tmp/nonexistent-runtime-dir"));
        // Initially state is None
        assert_eq!(rt.get_state().unwrap(), RuntimeState::None);
        // After detect, state should be set to None (no images)
        let detected = rt.detect().await.unwrap();
        assert_eq!(detected, RuntimeState::None);
        assert_eq!(rt.get_state().unwrap(), RuntimeState::None);
    }

    #[tokio::test]
    async fn health_check_last_check_is_rfc3339() {
        let rt = test_runtime(PathBuf::from("/tmp/nonexistent-runtime-dir"));
        let status = rt.health_check().await.unwrap();
        // Verify last_check is a valid RFC 3339 timestamp
        assert!(
            chrono::DateTime::parse_from_rfc3339(&status.last_check).is_ok(),
            "last_check should be valid RFC 3339: {}",
            status.last_check
        );
    }
}
