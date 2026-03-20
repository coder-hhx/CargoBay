# Built-in Container Runtime Specification

> Version: 1.0.0 | Last Updated: 2026-03-20 | Author: architect

---

## Table of Contents

1. [Design Goal](#1-design-goal)
2. [Platform Strategies](#2-platform-strategies)
3. [Runtime Lifecycle](#3-runtime-lifecycle)
4. [Docker Socket Exposure](#4-docker-socket-exposure)
5. [VirtioFS / Shared Directory](#5-virtiofs--shared-directory)
6. [Port Forwarding](#6-port-forwarding)
7. [Resource Management](#7-resource-management)
8. [First-Run Experience](#8-first-run-experience)
9. [Runtime Health Check](#9-runtime-health-check)
10. [Fallback: External Docker Detection](#10-fallback-external-docker-detection)
11. [Platform-Specific Implementation](#11-platform-specific-implementation)

---

## 1. Design Goal

### Zero-Dependency Container Runtime

CrateBay ships a **built-in container runtime** so that users can install the application and immediately start creating and managing containers — without installing Docker Desktop, Colima, Podman, or any other external tool.

### Key Requirements

| Requirement | Description |
|-------------|-------------|
| **Zero external dependencies** | No Docker, no Colima, no manual configuration |
| **Automatic provisioning** | First launch downloads and configures the runtime automatically |
| **Transparent to users** | Users interact with containers through the Chat UI; the runtime layer is invisible |
| **Native performance** | Use platform-native virtualization (VZ.framework, KVM, WSL2) for near-native speed |
| **Small footprint** | VM image < 500 MB download, < 1 GB disk after provisioning |
| **Graceful coexistence** | If external Docker is already installed, use it instead of the built-in runtime |

### Architecture Overview

```
CrateBay App
     │
     ├── Detect existing Docker? ──→ Yes ──→ Use external Docker
     │                                        (no runtime needed)
     │
     └── No external Docker ──→ Start built-in runtime
                                  │
                    ┌─────────────┼─────────────┐
                    │             │             │
                macOS          Linux        Windows
                    │             │             │
             VZ.framework     KVM/QEMU       WSL2
                    │             │             │
              Linux VM       Linux VM     WSL2 Distro
                    │             │             │
             Docker Engine   Docker Engine  Docker Engine
                    │             │             │
              Unix Socket    Unix Socket    Socket/Pipe
                    │             │             │
                    └─────────────┼─────────────┘
                                  │
                         bollard connects
                         via socket
```

---

## 2. Platform Strategies

### 2.1 macOS: Virtualization.framework

| Aspect | Detail |
|--------|--------|
| **Hypervisor** | Apple Virtualization.framework (VZ) |
| **VM Type** | `VZVirtualMachine` with `VZLinuxBootLoader` |
| **Guest OS** | Alpine Linux (minimal, ~150 MB) |
| **Docker** | Docker Engine CE installed inside VM |
| **File Sharing** | VirtioFS (`VZVirtioFileSystemDeviceConfiguration`) |
| **Networking** | NAT via `VZNATNetworkDeviceAttachment` |
| **Requirements** | macOS 13+ (Ventura), Apple Silicon or Intel |

```
macOS Host
├── CrateBay.app
│   └── Rust Backend
│       └── VZ.framework API calls
│           └── VZVirtualMachine
│               ├── VZLinuxBootLoader (vmlinuz + initrd)
│               ├── VZVirtioBlockStorageDevice (rootfs.img)
│               ├── VZVirtioFileSystemDevice (shared dirs)
│               ├── VZVirtioSocketDevice (vsock)
│               └── VZNATNetworkDeviceAttachment
│                   └── Alpine Linux
│                       └── Docker Engine
│                           └── /var/run/docker.sock
│                               └── Exposed via vsock → host socket
```

### 2.2 Linux: KVM/QEMU

| Aspect | Detail |
|--------|--------|
| **Hypervisor** | KVM (hardware) + QEMU (userspace) |
| **VM Type** | Lightweight QEMU VM with KVM acceleration |
| **Guest OS** | Alpine Linux (same image as macOS) |
| **Docker** | Docker Engine CE installed inside VM |
| **File Sharing** | VirtioFS via virtiofsd |
| **Networking** | User-mode networking (SLIRP) or TAP |
| **Requirements** | Linux kernel 5.10+, KVM support (`/dev/kvm`) |

```
Linux Host
├── CrateBay binary
│   └── Rust Backend
│       └── QEMU process management
│           └── qemu-system-x86_64 (or aarch64)
│               ├── -enable-kvm
│               ├── -kernel vmlinuz -initrd initrd
│               ├── -drive file=rootfs.img
│               ├── -chardev socket for Docker
│               └── -virtfs for shared directories
│                   └── Alpine Linux
│                       └── Docker Engine
│                           └── Exposed via socket
```

### 2.3 Windows: WSL2

| Aspect | Detail |
|--------|--------|
| **Hypervisor** | Hyper-V (via WSL2) |
| **VM Type** | WSL2 lightweight utility VM |
| **Distro** | Custom WSL2 distro (Alpine-based) |
| **Docker** | Docker Engine CE inside WSL2 |
| **File Sharing** | Plan 9 (9P) protocol (built into WSL2) |
| **Networking** | WSL2 NAT networking |
| **Requirements** | Windows 10 21H2+ or Windows 11, WSL2 enabled |

```
Windows Host
├── CrateBay.exe
│   └── Rust Backend
│       └── WSL2 management (wsl.exe commands)
│           └── CrateBay WSL2 Distro
│               ├── Alpine Linux
│               ├── Docker Engine
│               └── /var/run/docker.sock
│                   └── Exposed via:
│                       ├── socat → TCP localhost:2375
│                       └── or WSL2 interop socket
```

---

## 3. Runtime Lifecycle

### 3.1 State Machine

```
                    ┌─────────┐
                    │  NONE   │ (no runtime detected)
                    └────┬────┘
                         │ provision()
                    ┌────▼────┐
                    │PROVISION│ (downloading VM image)
                    │  ING    │
                    └────┬────┘
                         │ complete
                    ┌────▼────┐
                    │PROVISION│ (image ready, not started)
                    │   ED    │
                    └────┬────┘
                         │ start()
                    ┌────▼────┐
                    │STARTING │ (VM booting)
                    └────┬────┘
                         │ health_check() passes
                    ┌────▼────┐
              ┌────→│  READY  │ (Docker available)
              │     └────┬────┘
              │          │ stop()
              │     ┌────▼────┐
              │     │STOPPING │
              │     └────┬────┘
              │          │ stopped
              │     ┌────▼────┐
              └─────│ STOPPED │
                    └─────────┘
```

### 3.2 Lifecycle Operations

```rust
/// Runtime lifecycle trait (platform-agnostic)
pub trait RuntimeManager: Send + Sync {
    /// Detect current runtime state
    async fn detect(&self) -> Result<RuntimeState, AppError>;

    /// Download and prepare VM image (first run)
    async fn provision(
        &self,
        on_progress: impl Fn(ProvisionProgress) + Send + 'static,
    ) -> Result<(), AppError>;

    /// Start the runtime VM
    async fn start(&self) -> Result<(), AppError>;

    /// Stop the runtime VM gracefully
    async fn stop(&self) -> Result<(), AppError>;

    /// Check if runtime is healthy and Docker is responsive
    async fn health_check(&self) -> Result<HealthStatus, AppError>;

    /// Get the Docker socket path for bollard connection
    fn docker_socket_path(&self) -> PathBuf;

    /// Get current resource usage
    async fn resource_usage(&self) -> Result<ResourceUsage, AppError>;
}

#[derive(Debug, Clone, Serialize)]
pub enum RuntimeState {
    None,           // No runtime, needs provisioning
    Provisioning,   // Downloading VM image
    Provisioned,    // Image ready, not running
    Starting,       // VM is booting
    Ready,          // Docker is available
    Stopping,       // Shutting down
    Stopped,        // VM stopped
    Error(String),  // Runtime error
}

#[derive(Debug, Clone, Serialize)]
pub struct ProvisionProgress {
    pub stage: String,         // "downloading", "extracting", "configuring"
    pub percent: f32,          // 0.0 - 100.0
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
    pub message: String,
}
```

### 3.3 Automatic Start Flow

```
App Launch
    │
    ├── docker::connect() attempts socket detection
    │   ├── Found external Docker → use it (no runtime start)
    │   └── No Docker found → check runtime state
    │
    ├── runtime.detect()
    │   ├── RuntimeState::None → runtime.provision() → runtime.start()
    │   ├── RuntimeState::Provisioned → runtime.start()
    │   ├── RuntimeState::Stopped → runtime.start()
    │   └── RuntimeState::Ready → already good
    │
    └── health_check() loop (max 30s timeout)
        ├── Docker socket responsive → READY
        └── Timeout → show error to user
```

---

## 4. Docker Socket Exposure

### 4.1 Socket Path Convention

| Platform | Socket Location |
|----------|----------------|
| macOS | `~/.cratebay/runtime/docker.sock` |
| Linux | `~/.cratebay/runtime/docker.sock` |
| Windows | `\\.\pipe\cratebay-docker` or `localhost:2375` |

### 4.2 macOS: vsock to Unix Socket

```
VM (guest)                          Host (macOS)
Docker Engine                       CrateBay Backend
/var/run/docker.sock                ~/.cratebay/runtime/docker.sock
       │                                      │
       └── socat UNIX-LISTEN:   ←── vsock ──→ socat creates
           /var/run/docker.sock     (AF_VSOCK) Unix socket
           (inside VM)                         on host
```

The VM exposes its Docker socket via `VZVirtioSocketDevice` (vsock). A lightweight proxy on the host side creates a Unix socket that bollard can connect to.

### 4.3 Linux: QEMU Socket Forwarding

```
VM (guest)                          Host (Linux)
Docker Engine                       CrateBay Backend
/var/run/docker.sock                ~/.cratebay/runtime/docker.sock
       │                                      │
       └── QEMU -chardev socket  ──────────→  Unix socket
           forwarding                          on host
```

### 4.4 Windows: TCP or Named Pipe

```
WSL2 Distro                         Host (Windows)
Docker Engine                        CrateBay Backend
/var/run/docker.sock                 \\.\pipe\cratebay-docker
       │                                      │
       └── socat TCP-LISTEN:2375  ──────────→  localhost:2375
           or named pipe proxy                  or named pipe
```

---

## 5. VirtioFS / Shared Directory

### 5.1 Purpose

Users may want to mount host directories into containers (e.g., project source code). The runtime provides transparent file sharing between host and VM.

### 5.2 Shared Directory Structure

```
Host: ~/.cratebay/shared/         → VM: /mnt/host/
Host: ~/Projects/                 → VM: /mnt/projects/ (user-configurable)
```

### 5.3 Platform Implementation

| Platform | Technology | Performance |
|----------|-----------|-------------|
| macOS | VirtioFS (`VZVirtioFileSystemDeviceConfiguration`) | Near-native |
| Linux | virtiofsd + VirtioFS | Near-native |
| Windows | WSL2 Plan 9 (9P) mount | Good (built into WSL2) |

### 5.4 Container Bind Mount Flow

```
User Request: "Create a container with ~/Projects/myapp mounted"
    │
    ├── Host path: ~/Projects/myapp
    │
    ├── VM shared dir: /mnt/projects/myapp (via VirtioFS)
    │
    └── Container bind mount: /mnt/projects/myapp → /workspace
        (Docker -v flag inside the VM)
```

---

## 6. Port Forwarding

### 6.1 Mechanism

Containers running inside the VM need their ports accessible from the host.

```
Host Browser                        VM                  Container
localhost:8080  ──→  VM NAT  ──→  0.0.0.0:8080  ──→  container:8080
```

### 6.2 Implementation

| Platform | Approach |
|----------|----------|
| macOS | VZ.framework NAT device automatically forwards ports |
| Linux | QEMU user-mode networking with `-hostfwd` flags, or dynamic port forwarding via socat |
| Windows | WSL2 localhost forwarding (automatic in Windows 11) |

### 6.3 Dynamic Port Forwarding

When a container exposes a port, the runtime detects it and sets up forwarding:

```rust
pub struct PortForward {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: Protocol, // TCP or UDP
}

/// Detect exposed ports from container config and set up forwarding
pub async fn setup_port_forwards(
    container_id: &str,
    ports: Vec<PortForward>,
) -> Result<(), AppError> { /* ... */ }
```

---

## 7. Resource Management

### 7.1 Default Resource Allocation

| Resource | Default | Minimum | Maximum |
|----------|---------|---------|---------|
| CPU cores | 2 | 1 | Host cores - 1 |
| Memory | 2 GB | 1 GB | Host RAM / 2 |
| Disk | 20 GB (thin provisioned) | 10 GB | 100 GB |

### 7.2 Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Number of CPU cores allocated to the VM
    pub cpu_cores: u32,

    /// Memory allocated to the VM in MB
    pub memory_mb: u64,

    /// Maximum disk size in GB (thin provisioned)
    pub disk_gb: u32,

    /// Whether to auto-start runtime on app launch
    pub auto_start: bool,

    /// Shared directories (host_path → guest_mount_point)
    pub shared_dirs: Vec<SharedDir>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            cpu_cores: 2,
            memory_mb: 2048,
            disk_gb: 20,
            auto_start: true,
            shared_dirs: vec![],
        }
    }
}
```

### 7.3 Resource Monitoring

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ResourceUsage {
    pub cpu_percent: f32,       // VM CPU usage (0-100)
    pub memory_used_mb: u64,    // VM memory in use
    pub memory_total_mb: u64,   // VM memory allocated
    pub disk_used_gb: f32,      // VM disk usage
    pub disk_total_gb: f32,     // VM disk capacity
    pub container_count: u32,   // Running containers inside VM
}
```

---

## 8. First-Run Experience

### 8.1 Flow

```
First App Launch
    │
    ├── detect() → RuntimeState::None
    │
    ├── Show "Setting up CrateBay Runtime" UI
    │   └── Progress bar with stages
    │
    ├── Stage 1: Download VM Image (~500 MB)
    │   ├── Source: GitHub Release assets / CDN
    │   ├── Show: "Downloading runtime image... 45%"
    │   └── Resume support for interrupted downloads
    │
    ├── Stage 2: Extract and Configure
    │   ├── Extract rootfs image
    │   ├── Configure Docker Engine
    │   └── Show: "Configuring runtime..."
    │
    ├── Stage 3: Start Runtime
    │   ├── Boot VM
    │   ├── Start Docker Engine inside VM
    │   └── Show: "Starting container engine..."
    │
    ├── Stage 4: Health Check
    │   ├── Verify Docker socket is responsive
    │   └── Show: "Verifying..."
    │
    └── Complete: "CrateBay is ready!"
```

### 8.2 VM Image Distribution

| Platform | Image Format | Approximate Size |
|----------|-------------|-----------------|
| macOS (arm64) | vmlinuz + initrd + rootfs.img | ~400 MB |
| macOS (x86_64) | vmlinuz + initrd + rootfs.img | ~400 MB |
| Linux (x86_64) | vmlinuz + initrd + rootfs.qcow2 | ~400 MB |
| Linux (arm64) | vmlinuz + initrd + rootfs.qcow2 | ~400 MB |
| Windows | WSL2 tar export | ~350 MB |

### 8.3 Image Contents

The VM image is a minimal Alpine Linux with:
- Docker Engine CE
- containerd
- socat (for socket forwarding)
- Standard container networking (iptables, bridge-utils)
- Minimal shell utilities

No GUI, no unnecessary packages — optimized for size and boot speed.

---

## 9. Runtime Health Check

### 9.1 Health Check Protocol

```rust
#[derive(Debug, Clone, Serialize)]
pub struct HealthStatus {
    pub runtime_state: RuntimeState,
    pub docker_responsive: bool,
    pub docker_version: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub last_check: String, // RFC3339 timestamp
}

/// Perform a health check on the runtime
pub async fn health_check(&self) -> Result<HealthStatus, AppError> {
    // 1. Check if VM process is running
    let vm_running = self.is_vm_process_alive();

    // 2. Check if Docker socket exists
    let socket_exists = self.docker_socket_path().exists();

    // 3. Try Docker ping
    let docker_responsive = if socket_exists {
        let docker = Docker::connect_with_unix(
            self.docker_socket_path().to_str().unwrap(),
            5, // 5 second timeout
            API_DEFAULT_VERSION,
        )?;
        docker.ping().await.is_ok()
    } else {
        false
    };

    Ok(HealthStatus {
        runtime_state: if docker_responsive {
            RuntimeState::Ready
        } else if vm_running {
            RuntimeState::Starting
        } else {
            RuntimeState::Stopped
        },
        docker_responsive,
        docker_version: None, // filled if responsive
        uptime_seconds: None,
        last_check: Utc::now().to_rfc3339(),
    })
}
```

### 9.2 Periodic Health Monitoring

The backend runs a background health check every 30 seconds:

```rust
pub fn start_health_monitor(
    runtime: Arc<dyn RuntimeManager>,
    app: AppHandle,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            match runtime.health_check().await {
                Ok(status) => {
                    let _ = app.emit("runtime:health", &status);
                }
                Err(e) => {
                    tracing::warn!("Health check failed: {}", e);
                    let _ = app.emit("runtime:health", &HealthStatus {
                        runtime_state: RuntimeState::Error(e.to_string()),
                        docker_responsive: false,
                        docker_version: None,
                        uptime_seconds: None,
                        last_check: Utc::now().to_rfc3339(),
                    });
                }
            }
        }
    });
}
```

---

## 10. Fallback: External Docker Detection

### 10.1 Detection Priority

Before starting the built-in runtime, CrateBay checks for existing Docker installations:

```
Priority 1: DOCKER_HOST environment variable
Priority 2: Platform-specific known socket paths
Priority 3: Built-in runtime socket
Priority 4: Start built-in runtime
```

### 10.2 Known Socket Paths

```rust
fn detect_external_docker() -> Option<PathBuf> {
    // Check DOCKER_HOST first
    if let Ok(host) = std::env::var("DOCKER_HOST") {
        if let Some(path) = parse_docker_host(&host) {
            if path.exists() { return Some(path); }
        }
    }

    let home = dirs::home_dir()?;

    #[cfg(target_os = "macos")]
    {
        let paths = [
            home.join(".colima/default/docker.sock"),
            home.join(".orbstack/run/docker.sock"),
            PathBuf::from("/var/run/docker.sock"),
            home.join(".docker/run/docker.sock"),
        ];
        for path in paths {
            if path.exists() { return Some(path); }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let paths = [
            PathBuf::from("/var/run/docker.sock"),
            home.join(".docker/run/docker.sock"),
        ];
        for path in paths {
            if path.exists() { return Some(path); }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Check named pipe
        if PathBuf::from(r"\\.\pipe\docker_engine").exists() {
            return Some(PathBuf::from(r"\\.\pipe\docker_engine"));
        }
    }

    None
}
```

### 10.3 Coexistence Strategy

| Scenario | Behavior |
|----------|----------|
| Docker Desktop running | Use Docker Desktop, skip runtime |
| Colima running (macOS) | Use Colima socket, skip runtime |
| OrbStack running (macOS) | Use OrbStack socket, skip runtime |
| No Docker found | Start built-in runtime |
| External Docker + built-in runtime | Prefer external, offer switch in settings |
| External Docker stopped | Detect change, offer to start built-in runtime |

---

## 11. Platform-Specific Implementation

### 11.1 macOS: VZ.framework Implementation

```rust
// cratebay-core/src/runtime/macos.rs
#[cfg(target_os = "macos")]
pub struct MacOSRuntime {
    config: RuntimeConfig,
    data_dir: PathBuf,       // ~/.cratebay/runtime/
    vm_handle: Option<VmHandle>,
}

impl MacOSRuntime {
    pub fn new() -> Self {
        Self {
            config: RuntimeConfig::default(),
            data_dir: dirs::home_dir().unwrap().join(".cratebay/runtime"),
            vm_handle: None,
        }
    }

    /// Create VZ virtual machine configuration
    fn create_vm_config(&self) -> Result<VmConfiguration, AppError> {
        // Boot loader
        let kernel = self.data_dir.join("vmlinuz");
        let initrd = self.data_dir.join("initrd");
        let boot_loader = VZLinuxBootLoader::new(&kernel, &initrd)?;

        // Storage
        let rootfs = self.data_dir.join("rootfs.img");
        let disk = VZVirtioBlockStorageDevice::new(&rootfs)?;

        // Shared directories (VirtioFS)
        let shared = VZVirtioFileSystemDevice::new("shared", &self.config.shared_dirs)?;

        // Network (NAT)
        let network = VZNATNetworkDeviceAttachment::new();

        // Socket (vsock for Docker)
        let vsock = VZVirtioSocketDevice::new();

        Ok(VmConfiguration {
            boot_loader,
            cpu_count: self.config.cpu_cores,
            memory_size: self.config.memory_mb * 1024 * 1024,
            devices: vec![disk, shared, network, vsock],
        })
    }
}

impl RuntimeManager for MacOSRuntime {
    async fn start(&self) -> Result<(), AppError> {
        let config = self.create_vm_config()?;
        let vm = VZVirtualMachine::new(config)?;
        vm.start().await?;

        // Wait for Docker inside VM to be ready
        self.wait_for_docker(Duration::from_secs(30)).await?;

        // Set up vsock → Unix socket bridge
        self.setup_socket_bridge().await?;

        Ok(())
    }

    // ... other trait methods
}
```

### 11.2 Linux: KVM/QEMU Implementation

```rust
// cratebay-core/src/runtime/linux.rs
#[cfg(target_os = "linux")]
pub struct LinuxRuntime {
    config: RuntimeConfig,
    data_dir: PathBuf,
    qemu_process: Option<Child>,
}

impl LinuxRuntime {
    /// Build QEMU command line arguments
    fn build_qemu_args(&self) -> Vec<String> {
        let mut args = vec![
            "-enable-kvm".to_string(),
            "-m".to_string(), format!("{}M", self.config.memory_mb),
            "-smp".to_string(), format!("{}", self.config.cpu_cores),
            "-kernel".to_string(), self.data_dir.join("vmlinuz").to_string_lossy().to_string(),
            "-initrd".to_string(), self.data_dir.join("initrd").to_string_lossy().to_string(),
            "-drive".to_string(), format!(
                "file={},format=qcow2,if=virtio",
                self.data_dir.join("rootfs.qcow2").display()
            ),
            "-nographic".to_string(),
            "-nodefaults".to_string(),
        ];

        // Docker socket forwarding
        let socket_path = self.docker_socket_path();
        args.extend_from_slice(&[
            "-chardev".to_string(),
            format!("socket,id=docker,path={},server=on,wait=off", socket_path.display()),
        ]);

        // Shared directories (VirtioFS)
        for dir in &self.config.shared_dirs {
            args.extend_from_slice(&[
                "-virtfs".to_string(),
                format!(
                    "local,path={},mount_tag={},security_model=mapped-xattr",
                    dir.host_path, dir.tag
                ),
            ]);
        }

        // Network
        args.extend_from_slice(&[
            "-netdev".to_string(), "user,id=net0".to_string(),
            "-device".to_string(), "virtio-net-pci,netdev=net0".to_string(),
        ]);

        args
    }
}

impl RuntimeManager for LinuxRuntime {
    async fn start(&self) -> Result<(), AppError> {
        // Check KVM availability
        if !Path::new("/dev/kvm").exists() {
            return Err(AppError::Runtime(
                "KVM not available. Ensure virtualization is enabled in BIOS.".into()
            ));
        }

        let args = self.build_qemu_args();
        let child = Command::new("qemu-system-x86_64")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AppError::Runtime(format!("Failed to start QEMU: {}", e)))?;

        self.qemu_process = Some(child);
        self.wait_for_docker(Duration::from_secs(30)).await?;
        Ok(())
    }

    // ... other trait methods
}
```

### 11.3 Windows: WSL2 Implementation

```rust
// cratebay-core/src/runtime/windows.rs
#[cfg(target_os = "windows")]
pub struct WindowsRuntime {
    config: RuntimeConfig,
    data_dir: PathBuf,
    distro_name: String, // "CrateBay"
}

impl WindowsRuntime {
    const DISTRO_NAME: &'static str = "CrateBay";

    /// Check if WSL2 is available
    async fn check_wsl2(&self) -> Result<bool, AppError> {
        let output = Command::new("wsl")
            .args(["--status"])
            .output()
            .await
            .map_err(|e| AppError::Runtime(format!("WSL check failed: {}", e)))?;
        Ok(output.status.success())
    }

    /// Import CrateBay distro into WSL2
    async fn import_distro(&self) -> Result<(), AppError> {
        let tar_path = self.data_dir.join("cratebay-wsl.tar");
        let install_dir = self.data_dir.join("wsl-distro");

        let output = Command::new("wsl")
            .args([
                "--import",
                Self::DISTRO_NAME,
                &install_dir.to_string_lossy(),
                &tar_path.to_string_lossy(),
                "--version", "2",
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(AppError::Runtime(
                format!("WSL import failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        Ok(())
    }
}

impl RuntimeManager for WindowsRuntime {
    async fn start(&self) -> Result<(), AppError> {
        // Ensure WSL2 is available
        if !self.check_wsl2().await? {
            return Err(AppError::Runtime(
                "WSL2 is not available. Please enable WSL2 in Windows Features.".into()
            ));
        }

        // Start the distro
        Command::new("wsl")
            .args(["-d", Self::DISTRO_NAME, "--", "dockerd", "&"])
            .spawn()
            .map_err(|e| AppError::Runtime(format!("Failed to start WSL distro: {}", e)))?;

        // Set up socket forwarding (Docker socket → named pipe)
        self.setup_socket_forward().await?;

        self.wait_for_docker(Duration::from_secs(30)).await?;
        Ok(())
    }

    async fn provision(
        &self,
        on_progress: impl Fn(ProvisionProgress) + Send + 'static,
    ) -> Result<(), AppError> {
        // 1. Check WSL2 availability
        on_progress(ProvisionProgress {
            stage: "checking".into(),
            percent: 10.0,
            message: "Checking WSL2 availability...".into(),
            ..Default::default()
        });

        if !self.check_wsl2().await? {
            return Err(AppError::Runtime(
                "WSL2 is required. Please run: wsl --install".into()
            ));
        }

        // 2. Download distro image
        on_progress(ProvisionProgress {
            stage: "downloading".into(),
            percent: 20.0,
            message: "Downloading CrateBay runtime image...".into(),
            ..Default::default()
        });
        self.download_image(|progress| {
            on_progress(ProvisionProgress {
                stage: "downloading".into(),
                percent: 20.0 + progress * 0.6,
                ..Default::default()
            });
        }).await?;

        // 3. Import into WSL2
        on_progress(ProvisionProgress {
            stage: "configuring".into(),
            percent: 85.0,
            message: "Importing WSL2 distro...".into(),
            ..Default::default()
        });
        self.import_distro().await?;

        on_progress(ProvisionProgress {
            stage: "complete".into(),
            percent: 100.0,
            message: "Runtime provisioned successfully".into(),
            ..Default::default()
        });

        Ok(())
    }

    // ... other trait methods
}
```

### 11.4 Platform Comparison Summary

| Aspect | macOS (VZ) | Linux (KVM) | Windows (WSL2) |
|--------|-----------|-------------|----------------|
| Boot time | ~3-5 s | ~3-5 s | ~2-4 s |
| Memory overhead | ~200 MB | ~200 MB | ~150 MB (shared) |
| File sharing perf | Excellent (VirtioFS) | Excellent (VirtioFS) | Good (9P) |
| Port forwarding | Automatic (NAT) | Manual (hostfwd) | Automatic (Win11) |
| Docker socket | vsock → Unix | chardev → Unix | socat → pipe |
| First-run download | ~400 MB | ~400 MB | ~350 MB |
| Min OS version | macOS 13 | Kernel 5.10 | Win10 21H2 |

---

## Appendix A: VM Image Build Process

The VM images are built in CI and published as GitHub Release assets:

```
1. Start with Alpine Linux minimal rootfs
2. Install Docker Engine CE + containerd
3. Install socat, iptables, bridge-utils
4. Configure Docker to start on boot
5. Configure socket forwarding service
6. Strip unnecessary files
7. Package:
   - macOS/Linux: vmlinuz + initrd + rootfs.img/qcow2
   - Windows: tar export for WSL import
8. Compress with zstd
9. Upload to GitHub Release
```

## Appendix B: Troubleshooting

| Issue | Platform | Resolution |
|-------|----------|------------|
| "KVM not available" | Linux | Enable virtualization in BIOS/UEFI |
| "VZ.framework error" | macOS | Ensure macOS 13+, check System Preferences > Security |
| "WSL2 not available" | Windows | Run `wsl --install` in admin PowerShell |
| Docker socket timeout | All | Check `~/.cratebay/runtime/` for socket file; restart runtime |
| VM won't start | All | Delete `~/.cratebay/runtime/` and re-provision |
| Port forwarding fails | Linux | Check iptables rules inside VM |
