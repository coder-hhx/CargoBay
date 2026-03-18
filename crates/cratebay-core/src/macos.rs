// macOS hypervisor: Apple Virtualization.framework with Rosetta + VirtioFS support.
//
// Rosetta: On Apple Silicon, VZLinuxRosettaDirectoryShare provides x86_64 → arm64
// translation inside Linux VMs. The Rosetta binary is mounted and registered
// via binfmt_misc so x86_64 ELF binaries run transparently.
//
// VirtioFS: VZVirtioFileSystemDeviceConfiguration allows sharing host directories
// with near-native filesystem performance (faster than 9p/NFS).

use crate::hypervisor::{
    Hypervisor, HypervisorError, PortForward, SharedDirectory, VmConfig, VmInfo, VmState,
};
use crate::images;
use crate::store::{data_dir, next_id_for_prefix, VmStore};
use std::collections::{HashMap, HashSet};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};
use tracing::{info, warn};

/// macOS hypervisor backed by Apple Virtualization.framework.
pub struct MacOSHypervisor {
    vms: Mutex<HashMap<String, VmEntry>>,
    next_id: Mutex<u64>,
    store: VmStore,
}

static ATTEMPTED_ORPHAN_RUNNER_VM_IDS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

impl Default for MacOSHypervisor {
    fn default() -> Self {
        Self::new()
    }
}

struct VmEntry {
    info: VmInfo,
    /// VZ configuration parameters stored for lifecycle management.
    _rosetta_mounted: bool,
    runner_pid: Option<u32>,
    runner: Option<Child>,
    /// Paths to kernel/initrd/disk configured at create time.
    kernel_path: Option<String>,
    initrd_path: Option<String>,
    /// Kernel command line (from os_image catalog or env var).
    cmdline: Option<String>,
}

struct RuntimeHttpProxyConfig {
    guest_endpoint: String,
    host_tcp_forward: Option<String>,
    host_http_connect_proxy: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeSocketForwardMode {
    Vsock,
    Tcp,
}

fn runtime_socket_forward_mode() -> RuntimeSocketForwardMode {
    match std::env::var("CRATEBAY_RUNTIME_SOCKET_FORWARD")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
    {
        Some(value) if value == "vsock" => RuntimeSocketForwardMode::Vsock,
        Some(value) if value == "tcp" => RuntimeSocketForwardMode::Tcp,
        _ => RuntimeSocketForwardMode::Tcp,
    }
}

fn vm_dir(id: &str) -> PathBuf {
    data_dir().join("vms").join(id)
}

fn vm_disk_path(id: &str) -> PathBuf {
    vm_dir(id).join("disk.raw")
}

fn vm_console_log_path(id: &str) -> PathBuf {
    vm_dir(id).join("console.log")
}

fn vm_runner_pid_path(id: &str) -> PathBuf {
    vm_dir(id).join("runner.pid")
}

fn vm_runner_ready_path(id: &str) -> PathBuf {
    vm_dir(id).join("runner.ready")
}

fn read_pid_file(path: &Path) -> Option<u32> {
    let content = std::fs::read_to_string(path).ok()?;
    content.trim().parse::<u32>().ok()
}

fn pid_alive(pid: u32) -> bool {
    let rc = unsafe { libc::kill(pid as i32, 0) };
    if rc == 0 {
        return true;
    }
    let err = std::io::Error::last_os_error();
    matches!(err.raw_os_error(), Some(libc::EPERM))
}

fn vm_runner_processes(id: &str) -> Vec<u32> {
    let disk = vm_disk_path(id);
    let ready = vm_runner_ready_path(id);
    let console = vm_console_log_path(id);
    let current_uid = unsafe { libc::geteuid() } as u32;

    let output = match Command::new("ps")
        .args(["-axww", "-o", "pid=,uid=,command="])
        .output()
    {
        Ok(output) if output.status.success() => output,
        Ok(_) | Err(_) => return Vec::new(),
    };

    let disk = disk.to_string_lossy();
    let ready = ready.to_string_lossy();
    let console = console.to_string_lossy();

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse::<u32>().ok()?;
            let uid = parts.next()?.parse::<u32>().ok()?;
            if uid != current_uid {
                return None;
            }

            let command = parts.collect::<Vec<_>>().join(" ");
            if !command.contains("cratebay-vz") {
                return None;
            }
            if command.contains(disk.as_ref())
                || command.contains(ready.as_ref())
                || command.contains(console.as_ref())
            {
                return Some(pid);
            }

            None
        })
        .collect()
}

fn runner_vm_id_from_command(command: &str) -> Option<String> {
    ["/disk.raw", "/runner.ready", "/console.log"]
        .into_iter()
        .find_map(|suffix| {
            let end = command.find(suffix)?;
            let prefix = &command[..end];
            let vms_idx = prefix.rfind("/vms/")?;
            let tail = &prefix[vms_idx + "/vms/".len()..];
            let vm_id = tail.rsplit('/').next()?.trim();
            if vm_id.is_empty() {
                None
            } else {
                Some(vm_id.to_string())
            }
        })
}

fn runner_processes_from_ps_output(output: &str, current_uid: u32) -> HashMap<String, Vec<u32>> {
    let mut processes = HashMap::<String, Vec<u32>>::new();
    for line in output.lines() {
        let mut parts = line.split_whitespace();
        let Some(pid) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let Some(uid) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        if uid != current_uid {
            continue;
        }

        let command = parts.collect::<Vec<_>>().join(" ");
        if !command.contains("cratebay-vz") {
            continue;
        }

        let Some(vm_id) = runner_vm_id_from_command(&command) else {
            continue;
        };
        processes.entry(vm_id).or_default().push(pid);
    }

    processes
}

fn managed_vz_runner_processes() -> HashMap<String, Vec<u32>> {
    let current_uid = unsafe { libc::geteuid() } as u32;
    let output = match Command::new("ps")
        .args(["-axww", "-o", "pid=,uid=,command="])
        .output()
    {
        Ok(output) if output.status.success() => output,
        Ok(_) | Err(_) => return HashMap::new(),
    };

    runner_processes_from_ps_output(&String::from_utf8_lossy(&output.stdout), current_uid)
}

fn terminate_runner_pids(id: &str, pids: &[u32], reason: &str) {
    if pids.is_empty() {
        return;
    }

    warn!(
        "Terminating {} VZ runner process(es) for VM {} ({}): {:?}",
        pids.len(),
        id,
        reason,
        pids
    );

    for pid in pids {
        if pid_alive(*pid) {
            let _ = unsafe { libc::kill(*pid as i32, libc::SIGTERM) };
        }
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if pids.iter().all(|pid| !pid_alive(*pid)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    for pid in pids {
        if pid_alive(*pid) {
            warn!(
                "Runner process {} for VM {} did not stop gracefully during {}, sending SIGKILL",
                pid, id, reason
            );
            let _ = unsafe { libc::kill(*pid as i32, libc::SIGKILL) };
        }
    }
}

fn cleanup_stray_runner_processes(id: &str, preserve_pid: Option<u32>, reason: &str) -> Vec<u32> {
    let stray = vm_runner_processes(id)
        .into_iter()
        .filter(|pid| Some(*pid) != preserve_pid)
        .collect::<Vec<_>>();
    terminate_runner_pids(id, &stray, reason);
    stray
}

fn orphan_runner_cleanup_state() -> &'static Mutex<HashSet<String>> {
    ATTEMPTED_ORPHAN_RUNNER_VM_IDS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn orphan_runner_marker_path(vm_id: &str) -> PathBuf {
    data_dir()
        .join("runtime")
        .join("orphan-runner-markers")
        .join(format!("{vm_id}.txt"))
}

fn orphan_runner_signature(pids: &[u32]) -> String {
    let mut sorted = pids.to_vec();
    sorted.sort_unstable();
    sorted
        .into_iter()
        .map(|pid| pid.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn orphan_runner_marker_matches(vm_id: &str, pids: &[u32]) -> bool {
    let path = orphan_runner_marker_path(vm_id);
    let Ok(current) = std::fs::read_to_string(path) else {
        return false;
    };
    current.trim() == orphan_runner_signature(pids)
}

fn write_orphan_runner_marker(vm_id: &str, pids: &[u32]) {
    let path = orphan_runner_marker_path(vm_id);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = crate::store::write_atomic(&path, orphan_runner_signature(pids).as_bytes());
}

fn clear_orphan_runner_marker(vm_id: &str) {
    let _ = std::fs::remove_file(orphan_runner_marker_path(vm_id));
}

fn codesign_output_has_virtualization_entitlements(output: &str) -> bool {
    output.contains("com.apple.security.virtualization")
        && output.contains("com.apple.security.hypervisor")
}

fn path_is_app_bundle_runner(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let Some(contents_dir) = parent.parent() else {
        return false;
    };
    if parent.file_name().and_then(|value| value.to_str()) != Some("MacOS") {
        return false;
    }
    if contents_dir.file_name().and_then(|value| value.to_str()) != Some("Contents") {
        return false;
    }
    contents_dir
        .parent()
        .and_then(|value| value.extension())
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("app"))
}

fn macos_entitlements_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(root) = std::env::var("CRATEBAY_REPO_ROOT") {
        candidates.push(PathBuf::from(root));
    }
    if let Ok(exe) = std::env::current_exe() {
        candidates.extend(exe.ancestors().map(Path::to_path_buf));
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.extend(cwd.ancestors().map(Path::to_path_buf));
    }
    candidates.extend(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .map(Path::to_path_buf),
    );

    for candidate in candidates {
        let entitlements = candidate.join("scripts").join("macos-entitlements.plist");
        if entitlements.is_file() {
            return Some(entitlements);
        }
    }

    None
}

fn runner_has_virtualization_entitlements(path: &Path) -> bool {
    let output = Command::new("codesign")
        .args(["-d", "--entitlements", ":-"])
        .arg(path)
        .output();
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }

    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    codesign_output_has_virtualization_entitlements(&combined)
}

fn ensure_local_vz_runner_entitlements(path: &Path) -> Result<(), HypervisorError> {
    if runner_has_virtualization_entitlements(path) {
        return Ok(());
    }

    if path_is_app_bundle_runner(path) {
        return Err(HypervisorError::CreateFailed(format!(
            "VZ runner is missing required virtualization entitlements: {}. Reinstall or re-sign the app bundle.",
            path.display()
        )));
    }

    let entitlements = macos_entitlements_path().ok_or_else(|| {
        HypervisorError::CreateFailed(format!(
            "VZ runner is missing required virtualization entitlements and scripts/macos-entitlements.plist was not found for automatic signing: {}",
            path.display()
        ))
    })?;

    let output = Command::new("codesign")
        .args([
            "--force",
            "--sign",
            "-",
            "--options",
            "runtime",
            "--entitlements",
        ])
        .arg(&entitlements)
        .arg(path)
        .output()
        .map_err(|error| {
            HypervisorError::CreateFailed(format!(
                "Failed to code-sign VZ runner {}: {}",
                path.display(),
                error
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(HypervisorError::CreateFailed(format!(
            "Failed to code-sign VZ runner {} with {}: {}",
            path.display(),
            entitlements.display(),
            detail
        )));
    }

    if !runner_has_virtualization_entitlements(path) {
        return Err(HypervisorError::CreateFailed(format!(
            "VZ runner still lacks virtualization entitlements after code-signing: {}",
            path.display()
        )));
    }

    warn!(
        "Applied local virtualization entitlements to VZ runner {}",
        path.display()
    );
    Ok(())
}

fn wait_for_child_exit(
    child: &mut Child,
    timeout: Duration,
) -> Result<Option<std::process::ExitStatus>, std::io::Error> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

impl MacOSHypervisor {
    fn allocate_runtime_host_proxy_port() -> Result<u16, HypervisorError> {
        for port in 3128..=3228 {
            if std::net::TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, port)).is_ok() {
                return Ok(port);
            }
        }

        Err(HypervisorError::CreateFailed(
            "Failed to allocate a host proxy bridge port for CrateBay Runtime".into(),
        ))
    }

    fn parse_runtime_http_proxy(raw: &str) -> Option<(String, u16, bool)> {
        let mut value = raw.trim();
        if value.is_empty() {
            return None;
        }

        if let Some(stripped) = value.strip_prefix("http://") {
            value = stripped;
        } else if let Some(stripped) = value.strip_prefix("https://") {
            value = stripped;
        }

        let value = value.split('/').next()?.trim();
        if value.is_empty() {
            return None;
        }

        let (host, port) = if value.starts_with('[') {
            let end = value.find(']')?;
            let host = &value[1..end];
            let port = value[end + 1..].strip_prefix(':')?.parse::<u16>().ok()?;
            (host, port)
        } else {
            let colon = value.rfind(':')?;
            let host = &value[..colon];
            let port = value[colon + 1..].parse::<u16>().ok()?;
            (host, port)
        };

        if port == 0 {
            return None;
        }

        let host = host.trim();
        if host.is_empty() {
            return None;
        }

        let is_loopback = matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]");
        let host = if is_loopback {
            "127.0.0.1".to_string()
        } else {
            host.to_string()
        };

        Some((host, port, is_loopback))
    }

    fn runtime_http_proxy() -> Option<RuntimeHttpProxyConfig> {
        let make_config = |host: String, port: u16, is_loopback: bool| -> Option<_> {
            if is_loopback {
                let bind_port = Self::allocate_runtime_host_proxy_port().ok()?;
                return Some(RuntimeHttpProxyConfig {
                    guest_endpoint: format!("192.168.64.1:{}", bind_port),
                    host_tcp_forward: Some(format!("0.0.0.0:{}={}:{}", bind_port, host, port)),
                    host_http_connect_proxy: None,
                });
            }

            Some(RuntimeHttpProxyConfig {
                guest_endpoint: format!("{}:{}", host, port),
                host_tcp_forward: None,
                host_http_connect_proxy: None,
            })
        };

        if let Ok(raw) = std::env::var("CRATEBAY_RUNTIME_HTTP_PROXY") {
            if let Some((host, port, is_loopback)) = Self::parse_runtime_http_proxy(&raw) {
                return make_config(host, port, is_loopback);
            }
        }

        let output = Command::new("scutil").arg("--proxy").output().ok()?;
        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut values = HashMap::<String, String>::new();
        for line in stdout.lines() {
            let trimmed = line.trim();
            let Some((key, value)) = trimmed.split_once(" : ") else {
                continue;
            };
            values.insert(key.trim().to_string(), value.trim().to_string());
        }

        let candidates = [
            ("HTTPSEnable", "HTTPSProxy", "HTTPSPort"),
            ("HTTPEnable", "HTTPProxy", "HTTPPort"),
        ];
        for (enable_key, host_key, port_key) in candidates {
            if values.get(enable_key).is_some_and(|value| value == "1") {
                let host = values.get(host_key)?;
                let port = values.get(port_key)?;
                if let Some((host, port, is_loopback)) =
                    Self::parse_runtime_http_proxy(&format!("{}:{}", host, port))
                {
                    return make_config(host, port, is_loopback);
                }
            }
        }

        let bind_port = Self::allocate_runtime_host_proxy_port().ok()?;
        Some(RuntimeHttpProxyConfig {
            guest_endpoint: format!("192.168.64.1:{}", bind_port),
            host_tcp_forward: None,
            host_http_connect_proxy: Some(format!("0.0.0.0:{}", bind_port)),
        })
    }

    fn push_runtime_dns_server(servers: &mut Vec<String>, seen: &mut HashSet<String>, value: &str) {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return;
        }

        let Ok(addr) = trimmed.parse::<std::net::Ipv4Addr>() else {
            return;
        };

        if addr.is_unspecified() || addr.is_loopback() || addr.is_link_local() {
            return;
        }

        let octets = addr.octets();
        if octets == [192, 168, 64, 1] {
            return;
        }
        if octets[0] == 198 && matches!(octets[1], 18 | 19) {
            return;
        }

        let server = addr.to_string();
        if seen.insert(server.clone()) {
            servers.push(server);
        }
    }

    fn runtime_dns_servers() -> Vec<String> {
        const DEFAULT_DNS_SERVERS: &[&str] = &["1.1.1.1", "8.8.8.8"];

        let mut servers = Vec::new();
        let mut seen = HashSet::new();

        if let Ok(raw) = std::env::var("CRATEBAY_RUNTIME_DNS") {
            for item in raw.split([',', ' ', '\t', '\n']) {
                Self::push_runtime_dns_server(&mut servers, &mut seen, item);
            }
            if !servers.is_empty() {
                return servers;
            }
        }

        if let Ok(output) = Command::new("scutil").arg("--dns").output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if !line.contains("nameserver") {
                        continue;
                    }
                    if let Some((_, value)) = line.split_once(':') {
                        Self::push_runtime_dns_server(&mut servers, &mut seen, value);
                    }
                }
            }
        }

        for server in DEFAULT_DNS_SERVERS {
            Self::push_runtime_dns_server(&mut servers, &mut seen, server);
        }

        servers
    }

    pub fn new() -> Self {
        let store = VmStore::new();
        let loaded = match store.load_vms() {
            Ok(v) => v,
            Err(e) => {
                warn!(
                    "Failed to load VM store ({}): {}",
                    store.path().display(),
                    e
                );
                vec![]
            }
        };

        let mut map: HashMap<String, VmEntry> = HashMap::new();
        let mut changed = false;
        for mut vm in loaded.iter().cloned() {
            let pid_path = vm_runner_pid_path(&vm.id);
            let ready_path = vm_runner_ready_path(&vm.id);

            let runner_pid = read_pid_file(&pid_path).filter(|pid| pid_alive(*pid));
            if runner_pid.is_some() {
                vm.state = VmState::Running;
            } else {
                if pid_path.exists() {
                    let _ = std::fs::remove_file(&pid_path);
                }
                if ready_path.exists() {
                    let _ = std::fs::remove_file(&ready_path);
                }
                vm.state = VmState::Stopped;
            }

            // Re-derive kernel/initrd paths and cmdline from persisted os_image.
            let (kernel_path, initrd_path, cmdline) = if let Some(ref img_id) = vm.os_image {
                let paths = images::image_paths(img_id);
                let entry = images::find_image(img_id);
                let cl = entry.map(|e| e.default_cmdline);
                (
                    Some(paths.kernel_path.to_string_lossy().into_owned()),
                    Some(paths.initrd_path.to_string_lossy().into_owned()),
                    cl,
                )
            } else {
                (None, None, None)
            };

            map.insert(
                vm.id.clone(),
                VmEntry {
                    info: vm,
                    _rosetta_mounted: false,
                    runner_pid,
                    runner: None,
                    kernel_path,
                    initrd_path,
                    cmdline,
                },
            );
        }

        let known_vm_ids = map.keys().cloned().collect::<HashSet<_>>();
        for vm_id in &known_vm_ids {
            let preserve_pid = map.get(vm_id).and_then(|entry| entry.runner_pid);
            let cleaned =
                cleanup_stray_runner_processes(vm_id, preserve_pid, "startup duplicate cleanup");
            if cleaned.is_empty() {
                continue;
            }

            if let Some(entry) = map.get_mut(vm_id) {
                if entry.runner_pid.is_some_and(|pid| cleaned.contains(&pid)) {
                    entry.runner_pid = None;
                    entry.info.state = VmState::Stopped;
                    let _ = std::fs::remove_file(vm_runner_pid_path(vm_id));
                    let _ = std::fs::remove_file(vm_runner_ready_path(vm_id));
                    changed = true;
                }
            }
        }

        for (vm_id, pids) in managed_vz_runner_processes() {
            if !known_vm_ids.contains(&vm_id) {
                if orphan_runner_marker_matches(&vm_id, &pids) {
                    continue;
                }
                let already_attempted = {
                    let mut attempted = crate::lock_or_recover(orphan_runner_cleanup_state());
                    !attempted.insert(vm_id.clone())
                };
                if already_attempted {
                    continue;
                }
                terminate_runner_pids(&vm_id, &pids, "startup orphan cleanup");
                let remaining = pids
                    .into_iter()
                    .filter(|pid| pid_alive(*pid))
                    .collect::<Vec<_>>();
                if remaining.is_empty() {
                    clear_orphan_runner_marker(&vm_id);
                } else {
                    write_orphan_runner_marker(&vm_id, &remaining);
                }
                continue;
            }

            let Some(entry) = map.get_mut(&vm_id) else {
                continue;
            };
            if entry.runner_pid.is_some() {
                continue;
            }

            if pids.len() == 1 {
                let pid = pids[0];
                entry.runner_pid = Some(pid);
                entry.info.state = VmState::Running;
                let _ = std::fs::write(vm_runner_pid_path(&vm_id), format!("{}\n", pid));
                changed = true;
                continue;
            }

            terminate_runner_pids(&vm_id, &pids, "startup duplicate recovery");
            entry.runner_pid = None;
            entry.info.state = VmState::Stopped;
            let _ = std::fs::remove_file(vm_runner_pid_path(&vm_id));
            let _ = std::fs::remove_file(vm_runner_ready_path(&vm_id));
            changed = true;
        }

        let next_id = next_id_for_prefix(&loaded, "vz-");
        let hypervisor = Self {
            vms: Mutex::new(map),
            next_id: Mutex::new(next_id),
            store,
        };
        if changed {
            let _ = hypervisor.persist();
        }
        hypervisor
    }

    /// Check if Rosetta is available on this Mac.
    /// Rosetta is only available on Apple Silicon (aarch64) running macOS 13+.
    fn check_rosetta_availability() -> bool {
        // Runtime check: arch must be aarch64
        #[cfg(target_arch = "aarch64")]
        {
            // Check if the Rosetta runtime exists
            std::path::Path::new("/Library/Apple/usr/libexec/oah/libRosettaRuntime").exists()
                || std::path::Path::new("/usr/libexec/rosetta").exists()
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            false
        }
    }

    fn persist(&self) -> Result<(), HypervisorError> {
        let vms = crate::lock_or_recover(&self.vms)
            .values()
            .map(|e| e.info.clone())
            .collect::<Vec<_>>();
        self.store.save_vms(&vms)
    }

    fn vz_runner_path() -> PathBuf {
        if let Ok(path) = std::env::var("CRATEBAY_VZ_RUNNER_PATH") {
            return PathBuf::from(path);
        }

        let mut sibling_candidate = None;
        let mut repo_external_candidate = None;
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let candidate = dir.join("cratebay-vz");
                let is_app_bundle_runner = dir
                    .file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|name| name == "MacOS")
                    && dir
                        .parent()
                        .and_then(|p| p.file_name())
                        .and_then(|s| s.to_str())
                        .is_some_and(|name| name == "Contents");

                if is_app_bundle_runner && candidate.is_file() {
                    return candidate;
                }

                sibling_candidate = Some(candidate);
            }
            let host_target = format!("{}-apple-darwin", std::env::consts::ARCH);
            for ancestor in exe.ancestors() {
                let candidate = ancestor
                    .join("crates")
                    .join("cratebay-gui")
                    .join("src-tauri")
                    .join("bin")
                    .join(format!("cratebay-vz-{}", host_target));
                if candidate.is_file() {
                    repo_external_candidate = Some(candidate);
                    break;
                }
            }
        }

        // Allow the CLI to find the runner binary shipped inside the desktop
        // app bundle (when the CLI is installed separately or run from a local
        // build tree where the adjacent runner binary is not entitled).
        if let Ok(home) = std::env::var("HOME") {
            let candidates = [
                PathBuf::from("/Applications/CrateBay.app/Contents/MacOS/cratebay-vz"),
                PathBuf::from(home)
                    .join("Applications")
                    .join("CrateBay.app")
                    .join("Contents")
                    .join("MacOS")
                    .join("cratebay-vz"),
            ];
            for candidate in candidates {
                if candidate.is_file() {
                    return candidate;
                }
            }
        } else {
            let candidate = PathBuf::from("/Applications/CrateBay.app/Contents/MacOS/cratebay-vz");
            if candidate.is_file() {
                return candidate;
            }
        }

        if let Some(candidate) = sibling_candidate {
            if candidate.is_file() {
                return candidate;
            }
        }

        if let Some(candidate) = repo_external_candidate {
            if candidate.is_file() {
                return candidate;
            }
        }

        PathBuf::from("cratebay-vz")
    }

    fn spawn_vz_runner(
        &self,
        vm: &VmInfo,
        kernel_path: Option<&str>,
        initrd_path: Option<&str>,
        vm_cmdline: Option<&str>,
    ) -> Result<Child, HypervisorError> {
        // Use explicitly configured kernel path, then env var as fallback.
        let kernel = kernel_path
            .map(|s| s.to_string())
            .or_else(|| std::env::var("CRATEBAY_VZ_KERNEL").ok())
            .ok_or_else(|| {
                HypervisorError::CreateFailed(
                    "No kernel_path configured and CRATEBAY_VZ_KERNEL is not set".into(),
                )
            })?;

        // Use explicitly configured initrd path, then env var as fallback.
        let initrd = initrd_path
            .map(|s| s.to_string())
            .or_else(|| std::env::var("CRATEBAY_VZ_INITRD").ok());

        // Use VM-specific cmdline (from OS image catalog), then env var, then default.
        let mut cmdline = vm_cmdline
            .map(|s| s.to_string())
            .or_else(|| std::env::var("CRATEBAY_VZ_CMDLINE").ok())
            .unwrap_or_else(|| "console=hvc0".into());
        let runtime_http_proxy = if vm.name == crate::runtime::runtime_vm_name() {
            Self::runtime_http_proxy()
        } else {
            None
        };

        if vm.name == crate::runtime::runtime_vm_name()
            && !cmdline
                .split_whitespace()
                .any(|arg| arg.starts_with("cratebay_dns="))
        {
            let dns_servers = Self::runtime_dns_servers();
            if !dns_servers.is_empty() {
                cmdline.push_str(" cratebay_dns=");
                cmdline.push_str(&dns_servers.join(","));
            }
        }
        if vm.name == crate::runtime::runtime_vm_name()
            && !cmdline
                .split_whitespace()
                .any(|arg| arg.starts_with("cratebay_http_proxy="))
        {
            if let Some(proxy) = runtime_http_proxy
                .as_ref()
                .map(|config| config.guest_endpoint.as_str())
            {
                cmdline.push_str(" cratebay_http_proxy=");
                cmdline.push_str(proxy);
            }
        }
        if vm.name == crate::runtime::runtime_vm_name()
            && !cmdline
                .split_whitespace()
                .any(|arg| arg.starts_with("cratebay_host_epoch="))
        {
            if let Ok(now) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
                cmdline.push_str(" cratebay_host_epoch=");
                cmdline.push_str(&now.as_secs().to_string());
            }
        }

        let disk = vm_disk_path(&vm.id);
        if !disk.exists() {
            return Err(HypervisorError::CreateFailed(format!(
                "VM disk image not found: {}",
                disk.display()
            )));
        }

        let ready_file = vm_runner_ready_path(&vm.id);
        let _ = std::fs::remove_file(&ready_file);

        let console_log = vm_console_log_path(&vm.id);
        let console_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&console_log)?;
        let console_err = console_file.try_clone()?;

        let runner_path = Self::vz_runner_path();
        ensure_local_vz_runner_entitlements(&runner_path)?;

        let mut cmd = Command::new(&runner_path);
        cmd.arg("--kernel")
            .arg(kernel)
            .arg("--disk")
            .arg(disk)
            .arg("--cpus")
            .arg(vm.cpus.to_string())
            .arg("--memory-mb")
            .arg(vm.memory_mb.to_string())
            .arg("--cmdline")
            .arg(cmdline)
            .arg("--ready-file")
            .arg(&ready_file)
            .arg("--console-log")
            .arg(&console_log);

        if vm.name == crate::runtime::runtime_vm_name() {
            let sock_path = crate::runtime::runtime_host_docker_socket_path(&vm.id);
            if let Some(parent) = sock_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let _ = std::fs::remove_file(&sock_path);
            #[cfg(unix)]
            crate::runtime::link_runtime_host_docker_socket(&vm.id)?;
            let spec = format!(
                "{}:{}",
                crate::runtime::docker_vsock_port(),
                sock_path.to_string_lossy()
            );
            match runtime_socket_forward_mode() {
                RuntimeSocketForwardMode::Vsock => {
                    cmd.arg("--vsock-forward").arg(spec);
                }
                RuntimeSocketForwardMode::Tcp => {
                    cmd.arg("--tcp-forward").arg(spec);
                }
            }
            if let Some(forward) = runtime_http_proxy
                .as_ref()
                .and_then(|config| config.host_tcp_forward.as_deref())
            {
                cmd.arg("--host-tcp-forward").arg(forward);
            }
            if let Some(proxy) = runtime_http_proxy
                .as_ref()
                .and_then(|config| config.host_http_connect_proxy.as_deref())
            {
                cmd.arg("--http-connect-proxy").arg(proxy);
            }
        }

        if let Some(initrd) = initrd {
            cmd.arg("--initrd").arg(initrd);
        }

        // Pass Rosetta flag if enabled.
        if vm.rosetta_enabled {
            cmd.arg("--rosetta");
        }

        // Pass shared directories.
        for share in &vm.shared_dirs {
            let spec = if share.read_only {
                format!("{}:{}:ro", share.tag, share.host_path)
            } else {
                format!("{}:{}", share.tag, share.host_path)
            };
            cmd.arg("--share").arg(spec);
        }

        cmd.stdin(Stdio::null())
            .stdout(Stdio::from(console_file))
            .stderr(Stdio::from(console_err));

        // Detach the runner from the parent process/session. This ensures the
        // VM keeps running even if the CLI process exits (e.g. after `cratebay
        // runtime start`) and avoids process-group signals from terminating
        // the runner unexpectedly.
        //
        // The runner's lifecycle is managed via pid files under the VM dir.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }

        let child = cmd.spawn()?;
        Ok(child)
    }
}

impl Hypervisor for MacOSHypervisor {
    fn create_vm(&self, config: VmConfig) -> Result<String, HypervisorError> {
        // Validate Rosetta request
        if config.rosetta && !Self::check_rosetta_availability() {
            return Err(HypervisorError::RosettaUnavailable(
                "Rosetta is only available on Apple Silicon Macs with macOS 13+".into(),
            ));
        }

        // Validate shared directory paths
        for dir in &config.shared_dirs {
            if !std::path::Path::new(&dir.host_path).exists() {
                return Err(HypervisorError::VirtioFsError(format!(
                    "Host path does not exist: {}",
                    dir.host_path
                )));
            }
        }

        {
            let vms = crate::lock_or_recover(&self.vms);
            if vms.values().any(|e| e.info.name == config.name) {
                return Err(HypervisorError::CreateFailed(format!(
                    "VM name already exists: {}",
                    config.name
                )));
            }
        }

        let mut id_counter = crate::lock_or_recover(&self.next_id);
        let id = format!("vz-{}", *id_counter);
        *id_counter += 1;

        let vm_dir = vm_dir(&id);
        std::fs::create_dir_all(&vm_dir)?;
        let disk_path = vm_disk_path(&id);
        let disk_bytes = config
            .disk_gb
            .checked_mul(1024 * 1024 * 1024)
            .ok_or_else(|| HypervisorError::CreateFailed("disk size overflow".into()))?;

        // If an OS image is specified and its rootfs exists, use it as the disk base.
        // Otherwise create a blank sparse raw disk.
        if let Some(ref img_id) = config.os_image {
            if images::is_image_ready(img_id) {
                images::create_disk_from_image(img_id, &disk_path, disk_bytes).map_err(|e| {
                    HypervisorError::CreateFailed(format!("disk from image: {}", e))
                })?;
            } else {
                // Image not downloaded; create blank disk as fallback.
                let file = std::fs::File::create(&disk_path)?;
                file.set_len(disk_bytes)?;
            }
        } else {
            let file = std::fs::File::create(&disk_path)?;
            file.set_len(disk_bytes)?;
        }

        // Look up the image's default cmdline for later use.
        let cmdline = config
            .os_image
            .as_deref()
            .and_then(images::find_image)
            .map(|e| e.default_cmdline);

        let info = VmInfo {
            id: id.clone(),
            name: config.name,
            state: VmState::Stopped,
            cpus: config.cpus,
            memory_mb: config.memory_mb,
            disk_gb: config.disk_gb,
            rosetta_enabled: config.rosetta,
            shared_dirs: config.shared_dirs,
            port_forwards: config.port_forwards,
            os_image: config.os_image,
        };

        let entry = VmEntry {
            info,
            _rosetta_mounted: false,
            runner_pid: None,
            runner: None,
            kernel_path: config.kernel_path.clone(),
            initrd_path: config.initrd_path.clone(),
            cmdline,
        };

        crate::lock_or_recover(&self.vms).insert(id.clone(), entry);
        if let Err(e) = self.persist() {
            crate::lock_or_recover(&self.vms).remove(&id);
            let _ = std::fs::remove_dir_all(&vm_dir);
            return Err(e);
        }

        // VM configuration (boot loader, network, storage, Rosetta, VirtioFS) is
        // built by the cratebay-vz runner binary at start_vm() time via the Swift
        // Virtualization.framework bridge. At create_vm() time we only allocate
        // the VM directory and disk image.

        Ok(id)
    }

    fn start_vm(&self, id: &str) -> Result<(), HypervisorError> {
        let preserve_pid = {
            let vms = crate::lock_or_recover(&self.vms);
            vms.get(id)
                .ok_or(HypervisorError::NotFound(id.into()))?
                .runner_pid
        };
        let cleaned_pids = cleanup_stray_runner_processes(id, preserve_pid, "start preflight");
        let remaining_conflicts = vm_runner_processes(id)
            .into_iter()
            .filter(|pid| Some(*pid) != preserve_pid)
            .collect::<Vec<_>>();

        let (already_running, need_persist, vm_info, kernel_path, initrd_path, cmdline) = {
            let mut vms = crate::lock_or_recover(&self.vms);
            let entry = vms
                .get_mut(id)
                .ok_or(HypervisorError::NotFound(id.into()))?;

            let mut already_running = false;
            let mut need_persist = false;

            if !cleaned_pids.is_empty() {
                if entry
                    .runner_pid
                    .is_some_and(|pid| cleaned_pids.contains(&pid))
                {
                    entry.runner = None;
                    entry.runner_pid = None;
                }
                let _ = std::fs::remove_file(vm_runner_pid_path(id));
                let _ = std::fs::remove_file(vm_runner_ready_path(id));
                entry.info.state = VmState::Stopped;
                need_persist = true;
            }

            if let Some(pid) = entry.runner_pid {
                if pid_alive(pid) {
                    already_running = true;
                    need_persist = entry.info.state != VmState::Running;
                    entry.info.state = VmState::Running;
                } else {
                    entry.runner_pid = None;
                    let _ = std::fs::remove_file(vm_runner_pid_path(id));
                    let _ = std::fs::remove_file(vm_runner_ready_path(id));
                }
            }

            if !already_running && entry.runner.is_some() {
                already_running = true;
                need_persist = entry.info.state != VmState::Running;
                entry.info.state = VmState::Running;
            }

            (
                already_running,
                need_persist,
                entry.info.clone(),
                entry.kernel_path.clone(),
                entry.initrd_path.clone(),
                entry.cmdline.clone(),
            )
        };

        if !remaining_conflicts.is_empty() {
            if need_persist {
                let _ = self.persist();
            }
            return Err(HypervisorError::CreateFailed(format!(
                "Stale VZ runner processes are still active for VM {}: {:?}",
                id, remaining_conflicts
            )));
        }

        if already_running {
            if need_persist {
                let _ = self.persist();
            }
            return Ok(());
        }

        let mut child = self.spawn_vz_runner(
            &vm_info,
            kernel_path.as_deref(),
            initrd_path.as_deref(),
            cmdline.as_deref(),
        )?;

        let ready_file = vm_runner_ready_path(&vm_info.id);
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if ready_file.exists() {
                break;
            }

            if let Ok(Some(status)) = child.try_wait() {
                return Err(HypervisorError::CreateFailed(format!(
                    "cratebay-vz exited early: {}",
                    status
                )));
            }

            if Instant::now() >= deadline {
                let _ = child.kill();
                match wait_for_child_exit(&mut child, Duration::from_secs(5)) {
                    Ok(Some(_)) => {}
                    Ok(None) => warn!(
                        "Timed out waiting for runner process {} to exit after start timeout",
                        child.id()
                    ),
                    Err(error) => warn!(
                        "Failed waiting for runner process {} after start timeout: {}",
                        child.id(),
                        error
                    ),
                }
                return Err(HypervisorError::CreateFailed(
                    "Timed out waiting for VM to start".into(),
                ));
            }

            std::thread::sleep(Duration::from_millis(200));
        }

        let pid = child.id();

        let previous_state = {
            let mut vms = crate::lock_or_recover(&self.vms);
            let entry = vms
                .get_mut(id)
                .ok_or(HypervisorError::NotFound(id.into()))?;
            let prev = entry.info.state.clone();
            entry.info.state = VmState::Running;
            entry.runner_pid = Some(pid);
            entry.runner = Some(child);
            prev
        };

        if let Err(e) = self.persist() {
            let mut vms = crate::lock_or_recover(&self.vms);
            if let Some(entry) = vms.get_mut(id) {
                entry.info.state = previous_state;
                if let Some(mut child) = entry.runner.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                entry.runner_pid = None;
            }
            return Err(e);
        }

        let _ = std::fs::write(vm_runner_pid_path(&vm_info.id), format!("{}\n", pid));
        info!("Started VZ VM {} (pid {})", vm_info.id, pid);
        Ok(())
    }

    fn stop_vm(&self, id: &str) -> Result<(), HypervisorError> {
        let (child, pid_opt, previous_state, rosetta_prev, is_runtime_vm) = {
            let mut vms = crate::lock_or_recover(&self.vms);
            let entry = vms
                .get_mut(id)
                .ok_or(HypervisorError::NotFound(id.into()))?;
            let prev = entry.info.state.clone();
            let rosetta_prev = entry._rosetta_mounted;
            let is_runtime_vm = entry.info.name == crate::runtime::runtime_vm_name();
            let child = entry.runner.take();
            let pid_opt = entry.runner_pid;
            entry.info.state = VmState::Stopped;
            entry._rosetta_mounted = false;
            entry.runner_pid = None;
            (child, pid_opt, prev, rosetta_prev, is_runtime_vm)
        };

        // Phase 1: Send SIGTERM for graceful ACPI shutdown (the runner
        // process handles SIGTERM by calling vz_stop_vm with requestStop).
        let runner_pid = if let Some(ref child) = child {
            Some(child.id())
        } else {
            pid_opt
        };

        if let Some(pid) = runner_pid {
            let _ = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        }

        // Phase 2: Wait up to 15 seconds for graceful shutdown.
        if let Some(pid) = runner_pid {
            let deadline = Instant::now() + Duration::from_secs(15);
            while Instant::now() < deadline {
                if !pid_alive(pid) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(250));
            }

            // Phase 3: Force kill if still alive.
            if pid_alive(pid) {
                warn!(
                    "VM {} runner (pid {}) did not stop gracefully, sending SIGKILL",
                    id, pid
                );
                let _ = unsafe { libc::kill(pid as i32, libc::SIGKILL) };
            }
        }

        // Wait for the child process to be reaped.
        if let Some(mut child) = child {
            match wait_for_child_exit(&mut child, Duration::from_secs(5)) {
                Ok(Some(_)) => {}
                Ok(None) => warn!(
                    "Timed out waiting for VM {} runner {} to exit after stop",
                    id,
                    child.id()
                ),
                Err(error) => warn!(
                    "Failed waiting for VM {} runner {} after stop: {}",
                    id,
                    child.id(),
                    error
                ),
            }
        }

        let _ = std::fs::remove_file(vm_runner_pid_path(id));
        let _ = std::fs::remove_file(vm_runner_ready_path(id));
        cleanup_stray_runner_processes(id, None, "stop cleanup");
        if is_runtime_vm {
            #[cfg(unix)]
            crate::runtime::unlink_runtime_host_docker_socket(id);
        }

        if let Err(e) = self.persist() {
            let mut vms = crate::lock_or_recover(&self.vms);
            if let Some(entry) = vms.get_mut(id) {
                entry.info.state = previous_state;
                entry._rosetta_mounted = rosetta_prev;
                entry.runner_pid = pid_opt;
            }
            return Err(e);
        }

        info!("Stopped VZ VM {}", id);
        Ok(())
    }

    fn delete_vm(&self, id: &str) -> Result<(), HypervisorError> {
        // Best-effort stop before deletion.
        let _ = self.stop_vm(id);

        let removed = self
            .vms
            .lock()
            .unwrap()
            .remove(id)
            .ok_or(HypervisorError::NotFound(id.into()))?;
        if let Err(e) = self.persist() {
            crate::lock_or_recover(&self.vms).insert(id.to_string(), removed);
            return Err(e);
        }

        let _ = std::fs::remove_dir_all(vm_dir(id));
        Ok(())
    }

    fn list_vms(&self) -> Result<Vec<VmInfo>, HypervisorError> {
        let mut changed = false;
        {
            let mut vms = crate::lock_or_recover(&self.vms);
            for entry in vms.values_mut() {
                if entry
                    .runner
                    .as_mut()
                    .and_then(|c| c.try_wait().ok())
                    .flatten()
                    .is_some()
                {
                    entry.runner = None;
                    entry.runner_pid = None;
                    entry.info.state = VmState::Stopped;
                    let _ = std::fs::remove_file(vm_runner_pid_path(&entry.info.id));
                    let _ = std::fs::remove_file(vm_runner_ready_path(&entry.info.id));
                    changed = true;
                    continue;
                }

                if let Some(pid) = entry.runner_pid {
                    if !pid_alive(pid) {
                        entry.runner_pid = None;
                        entry.info.state = VmState::Stopped;
                        let _ = std::fs::remove_file(vm_runner_pid_path(&entry.info.id));
                        let _ = std::fs::remove_file(vm_runner_ready_path(&entry.info.id));
                        changed = true;
                        continue;
                    }
                    if entry.info.state != VmState::Running {
                        entry.info.state = VmState::Running;
                        changed = true;
                    }
                }
            }
        }
        if changed {
            let _ = self.persist();
        }

        Ok(self
            .vms
            .lock()
            .unwrap()
            .values()
            .map(|e| e.info.clone())
            .collect())
    }

    fn rosetta_available(&self) -> bool {
        Self::check_rosetta_availability()
    }

    fn mount_virtiofs(&self, vm_id: &str, share: &SharedDirectory) -> Result<(), HypervisorError> {
        // Validate tag: must be non-empty, no slashes, no colons, reasonable length.
        if share.tag.is_empty() {
            return Err(HypervisorError::VirtioFsError(
                "Mount tag must not be empty".into(),
            ));
        }
        if share.tag.len() > 255 {
            return Err(HypervisorError::VirtioFsError(
                "Mount tag must not exceed 255 characters".into(),
            ));
        }
        if share.tag.contains('/') || share.tag.contains(':') || share.tag.contains('\0') {
            return Err(HypervisorError::VirtioFsError(format!(
                "Mount tag contains invalid characters: {}",
                share.tag
            )));
        }
        // "rosetta" is reserved for Rosetta directory share.
        if share.tag == "rosetta" {
            return Err(HypervisorError::VirtioFsError(
                "Mount tag 'rosetta' is reserved for Rosetta support".into(),
            ));
        }

        if !std::path::Path::new(&share.host_path).exists() {
            return Err(HypervisorError::VirtioFsError(format!(
                "Host path does not exist: {}",
                share.host_path
            )));
        }
        if !std::path::Path::new(&share.host_path).is_dir() {
            return Err(HypervisorError::VirtioFsError(format!(
                "Host path is not a directory: {}",
                share.host_path
            )));
        }

        let is_running;
        {
            let mut vms = crate::lock_or_recover(&self.vms);
            let entry = vms
                .get_mut(vm_id)
                .ok_or(HypervisorError::NotFound(vm_id.into()))?;

            // Check for duplicate tag
            if entry.info.shared_dirs.iter().any(|d| d.tag == share.tag) {
                return Err(HypervisorError::VirtioFsError(format!(
                    "Mount tag already exists: {}",
                    share.tag
                )));
            }

            is_running = entry.info.state == VmState::Running;
            entry.info.shared_dirs.push(share.clone());
        }

        if let Err(e) = self.persist() {
            let mut vms = crate::lock_or_recover(&self.vms);
            if let Some(entry) = vms.get_mut(vm_id) {
                entry.info.shared_dirs.retain(|d| d.tag != share.tag);
            }
            return Err(e);
        }

        if is_running {
            // VirtioFS devices are configured at VM creation time in VZBridge.swift
            // and cannot be hot-attached to a running VM. The mount is persisted and
            // will take effect on the next VM restart.
            info!(
                "VirtioFS mount '{}' added to running VM {} — will take effect after restart",
                share.tag, vm_id
            );
        } else {
            info!(
                "VirtioFS mount '{}' added to VM {} — will be active on next start",
                share.tag, vm_id
            );
        }

        Ok(())
    }

    fn unmount_virtiofs(&self, vm_id: &str, tag: &str) -> Result<(), HypervisorError> {
        let (previous, is_running, found) = {
            let mut vms = crate::lock_or_recover(&self.vms);
            let entry = vms
                .get_mut(vm_id)
                .ok_or(HypervisorError::NotFound(vm_id.into()))?;
            let found = entry.info.shared_dirs.iter().any(|d| d.tag == tag);
            let prev = entry.info.shared_dirs.clone();
            let is_running = entry.info.state == VmState::Running;
            entry.info.shared_dirs.retain(|d| d.tag != tag);
            (prev, is_running, found)
        };

        if !found {
            return Err(HypervisorError::VirtioFsError(format!(
                "Mount tag not found: {}",
                tag
            )));
        }

        if let Err(e) = self.persist() {
            let mut vms = crate::lock_or_recover(&self.vms);
            if let Some(entry) = vms.get_mut(vm_id) {
                entry.info.shared_dirs = previous;
            }
            return Err(e);
        }

        if is_running {
            // VirtioFS devices cannot be hot-detached from a running VM.
            // The mount is removed from the persisted config and will not be
            // present on the next VM restart.
            info!(
                "VirtioFS mount '{}' removed from running VM {} — removal takes effect after restart",
                tag, vm_id
            );
        } else {
            info!("VirtioFS mount '{}' removed from VM {}", tag, vm_id);
        }

        Ok(())
    }

    fn list_virtiofs_mounts(&self, vm_id: &str) -> Result<Vec<SharedDirectory>, HypervisorError> {
        let vms = crate::lock_or_recover(&self.vms);
        let entry = vms
            .get(vm_id)
            .ok_or(HypervisorError::NotFound(vm_id.into()))?;
        Ok(entry.info.shared_dirs.clone())
    }

    fn add_port_forward(&self, vm_id: &str, pf: &PortForward) -> Result<(), HypervisorError> {
        {
            let mut vms = crate::lock_or_recover(&self.vms);
            let entry = vms
                .get_mut(vm_id)
                .ok_or(HypervisorError::NotFound(vm_id.into()))?;
            if entry
                .info
                .port_forwards
                .iter()
                .any(|p| p.host_port == pf.host_port)
            {
                return Err(HypervisorError::CreateFailed(format!(
                    "Host port already forwarded: {}",
                    pf.host_port
                )));
            }
            entry.info.port_forwards.push(pf.clone());
        }
        if let Err(e) = self.persist() {
            let mut vms = crate::lock_or_recover(&self.vms);
            if let Some(entry) = vms.get_mut(vm_id) {
                entry
                    .info
                    .port_forwards
                    .retain(|p| p.host_port != pf.host_port);
            }
            return Err(e);
        }
        Ok(())
    }

    fn remove_port_forward(&self, vm_id: &str, host_port: u16) -> Result<(), HypervisorError> {
        let previous = {
            let mut vms = crate::lock_or_recover(&self.vms);
            let entry = vms
                .get_mut(vm_id)
                .ok_or(HypervisorError::NotFound(vm_id.into()))?;
            let prev = entry.info.port_forwards.clone();
            entry
                .info
                .port_forwards
                .retain(|p| p.host_port != host_port);
            prev
        };
        if let Err(e) = self.persist() {
            let mut vms = crate::lock_or_recover(&self.vms);
            if let Some(entry) = vms.get_mut(vm_id) {
                entry.info.port_forwards = previous;
            }
            return Err(e);
        }
        Ok(())
    }

    fn list_port_forwards(&self, vm_id: &str) -> Result<Vec<PortForward>, HypervisorError> {
        let vms = crate::lock_or_recover(&self.vms);
        let entry = vms
            .get(vm_id)
            .ok_or(HypervisorError::NotFound(vm_id.into()))?;
        Ok(entry.info.port_forwards.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        codesign_output_has_virtualization_entitlements, path_is_app_bundle_runner,
        runner_processes_from_ps_output, runner_vm_id_from_command,
    };
    use std::path::Path;

    #[test]
    fn runner_vm_id_from_command_extracts_vm_id() {
        let command = "/Applications/CrateBay.app/Contents/MacOS/cratebay-vz --kernel /Users/test/Library/Application Support/com.cratebay.app/images/cratebay-runtime-x86_64/vmlinuz --disk /Users/test/Library/Application Support/com.cratebay.app/vms/vz-3/disk.raw --console-log /Users/test/Library/Application Support/com.cratebay.app/vms/vz-3/console.log";
        assert_eq!(runner_vm_id_from_command(command).as_deref(), Some("vz-3"));
    }

    #[test]
    fn runner_vm_id_from_command_ignores_non_vm_processes() {
        assert_eq!(
            runner_vm_id_from_command("/tmp/cratebay-vz-signed-runtime --help"),
            None
        );
        assert_eq!(runner_vm_id_from_command("rg cratebay-vz"), None);
    }

    #[test]
    fn managed_vz_runner_processes_groups_by_vm_id() {
        let output = "\
123 501 /Applications/CrateBay.app/Contents/MacOS/cratebay-vz --disk /Users/test/Library/Application Support/com.cratebay.app/vms/vz-1/disk.raw
456 501 /Applications/CrateBay.app/Contents/MacOS/cratebay-vz --console-log /Users/test/Library/Application Support/com.cratebay.app/vms/vz-1/console.log
789 501 /Applications/CrateBay.app/Contents/MacOS/cratebay-vz --disk /Users/test/Library/Application Support/com.cratebay.app/vms/vz-2/disk.raw
";
        let grouped = runner_processes_from_ps_output(output, 501);

        assert_eq!(grouped.get("vz-1"), Some(&vec![123, 456]));
        assert_eq!(grouped.get("vz-2"), Some(&vec![789]));
        assert!(!grouped.contains_key("vz-3"));
    }

    #[test]
    fn codesign_output_detects_virtualization_entitlements() {
        let output = r#"
<plist version="1.0">
<dict>
  <key>com.apple.security.hypervisor</key>
  <true/>
  <key>com.apple.security.virtualization</key>
  <true/>
</dict>
</plist>
"#;
        assert!(codesign_output_has_virtualization_entitlements(output));
        assert!(!codesign_output_has_virtualization_entitlements(
            "Executable=/tmp/cratebay-vz"
        ));
    }

    #[test]
    fn path_is_app_bundle_runner_detects_bundle_layout() {
        assert!(path_is_app_bundle_runner(Path::new(
            "/Applications/CrateBay.app/Contents/MacOS/cratebay-vz"
        )));
        assert!(!path_is_app_bundle_runner(Path::new(
            "/tmp/CrateBay/target/debug/cratebay-vz"
        )));
    }
}
