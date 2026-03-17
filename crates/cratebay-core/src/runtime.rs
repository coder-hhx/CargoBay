use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::hypervisor::{Hypervisor, HypervisorError, VmConfig, VmState};

pub const DEFAULT_RUNTIME_VM_NAME: &str = "cratebay-runtime";
pub const DEFAULT_DOCKER_VSOCK_PORT: u32 = 6237;
pub const DEFAULT_RUNTIME_ASSETS_SUBDIR: &str = "runtime-images";
#[cfg(target_os = "linux")]
pub const DEFAULT_LINUX_RUNTIME_ASSETS_SUBDIR: &str = "runtime-linux";
#[cfg(target_os = "windows")]
pub const DEFAULT_WSL_ASSETS_SUBDIR: &str = "runtime-wsl";

#[cfg(target_os = "windows")]
pub const DEFAULT_WSL_DOCKER_PORT: u16 = 2375;
#[cfg(target_os = "linux")]
pub const DEFAULT_LINUX_DOCKER_PORT: u16 = 2475;

static RUNTIME_VM_NAME: OnceLock<String> = OnceLock::new();
static DOCKER_VSOCK_PORT: OnceLock<u32> = OnceLock::new();
static DOCKER_SOCKET_PATH: OnceLock<PathBuf> = OnceLock::new();
static RUNTIME_OS_IMAGE_ID: OnceLock<String> = OnceLock::new();

/// The VM name CrateBay uses for its built-in container runtime.
pub fn runtime_vm_name() -> &'static str {
    RUNTIME_VM_NAME
        .get_or_init(|| {
            std::env::var("CRATEBAY_RUNTIME_VM_NAME")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_RUNTIME_VM_NAME.to_string())
        })
        .as_str()
}

/// The guest port for the Docker API proxy inside the runtime VM.
///
/// Historically this was a virtio-vsock port on macOS. Newer runtimes may use
/// TCP forwarding on the guest NAT IP instead, but the port number is shared.
pub fn docker_vsock_port() -> u32 {
    *DOCKER_VSOCK_PORT.get_or_init(|| {
        std::env::var("CRATEBAY_DOCKER_PROXY_PORT")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .or_else(|| {
                std::env::var("CRATEBAY_DOCKER_VSOCK_PORT")
                    .ok()
                    .and_then(|v| v.parse::<u32>().ok())
                    .filter(|v| *v > 0)
            })
            .unwrap_or(DEFAULT_DOCKER_VSOCK_PORT)
    })
}

/// Preferred name for the runtime guest proxy port.
pub fn docker_proxy_port() -> u32 {
    docker_vsock_port()
}

/// The host-side Docker-compatible Unix socket path exposed by CrateBay.
///
/// Defaults to `$HOME/.cratebay/run/docker.sock` to avoid spaces in paths like
/// `~/Library/Application Support/...` (which can break URL parsing in tooling
/// that consumes `DOCKER_HOST=unix://...`).
pub fn host_docker_socket_path() -> &'static Path {
    DOCKER_SOCKET_PATH
        .get_or_init(|| {
            if let Ok(p) = std::env::var("CRATEBAY_DOCKER_SOCKET_PATH") {
                if !p.trim().is_empty() {
                    return PathBuf::from(p);
                }
            }

            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home)
                    .join(".cratebay")
                    .join("run")
                    .join("docker.sock");
            }

            crate::store::data_dir().join("run").join("docker.sock")
        })
        .as_path()
}

pub fn runtime_host_docker_socket_path(vm_id: &str) -> PathBuf {
    let base = host_docker_socket_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| crate::store::data_dir().join("run"));
    base.join(format!("docker-{}.sock", vm_id))
}

#[cfg(unix)]
pub fn link_runtime_host_docker_socket(vm_id: &str) -> Result<(), HypervisorError> {
    use std::os::unix::fs::symlink;

    let alias = host_docker_socket_path();
    let actual = runtime_host_docker_socket_path(vm_id);
    if let Some(parent) = actual.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match std::fs::symlink_metadata(alias) {
        Ok(_) => {
            let _ = std::fs::remove_file(alias);
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }

    symlink(&actual, alias)?;
    Ok(())
}

#[cfg(unix)]
pub fn unlink_runtime_host_docker_socket(vm_id: &str) {
    let alias = host_docker_socket_path();
    let actual = runtime_host_docker_socket_path(vm_id);

    if let Ok(target) = std::fs::read_link(alias) {
        if target == actual {
            let _ = std::fs::remove_file(alias);
        }
    }
    let _ = std::fs::remove_file(&actual);
}

/// OS image id used for the built-in runtime VM.
///
/// Can be overridden via `CRATEBAY_RUNTIME_OS_IMAGE_ID`.
pub fn runtime_os_image_id() -> &'static str {
    RUNTIME_OS_IMAGE_ID
        .get_or_init(|| {
            if let Ok(id) = std::env::var("CRATEBAY_RUNTIME_OS_IMAGE_ID") {
                if !id.trim().is_empty() {
                    return id;
                }
            }

            #[cfg(target_arch = "aarch64")]
            {
                "cratebay-runtime-aarch64".to_string()
            }

            #[cfg(target_arch = "x86_64")]
            {
                "cratebay-runtime-x86_64".to_string()
            }

            #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
            {
                "cratebay-runtime-aarch64".to_string()
            }
        })
        .as_str()
}

pub fn runtime_image_ready() -> bool {
    crate::images::is_image_ready(runtime_os_image_id())
}

fn runtime_images_dir_from_root(root: &Path) -> Option<PathBuf> {
    if root
        .file_name()
        .is_some_and(|n| n == DEFAULT_RUNTIME_ASSETS_SUBDIR)
        && root.is_dir()
    {
        return Some(root.to_path_buf());
    }

    let dir = root.join(DEFAULT_RUNTIME_ASSETS_SUBDIR);
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

fn bundled_runtime_assets_root_from_exe_dir(exe_dir: &Path) -> Option<PathBuf> {
    // macOS app bundle layout: <App>.app/Contents/MacOS/<exe>
    #[cfg(target_os = "macos")]
    {
        if let Some(macos_dir) = exe_dir.file_name().and_then(|s| s.to_str()) {
            if macos_dir == "MacOS" {
                if let Some(contents_dir) = exe_dir.parent() {
                    if contents_dir
                        .file_name()
                        .and_then(|s| s.to_str())
                        .is_some_and(|n| n == "Contents")
                    {
                        let resources = contents_dir.join("Resources");
                        if resources.is_dir() {
                            return Some(resources);
                        }
                    }
                }
            }
        }
    }

    // Tauri Windows/Linux layout (and common app installers): resources are placed
    // under a sibling `resources/` directory next to the executable.
    let direct_resources = exe_dir.join("resources");
    if direct_resources.is_dir() {
        return Some(direct_resources);
    }
    if let Some(parent) = exe_dir.parent() {
        let parent_resources = parent.join("resources");
        if parent_resources.is_dir() {
            return Some(parent_resources);
        }
    }

    None
}

fn workspace_runtime_assets_root_from_exe_dir(exe_dir: &Path) -> Option<PathBuf> {
    for ancestor in exe_dir.ancestors() {
        if !ancestor.join("Cargo.toml").is_file() {
            continue;
        }

        let src_tauri_dir = ancestor
            .join("crates")
            .join("cratebay-gui")
            .join("src-tauri");
        if runtime_images_dir_from_root(&src_tauri_dir).is_some() {
            return Some(src_tauri_dir);
        }
    }

    None
}

#[cfg_attr(not(test), allow(dead_code))]
fn runtime_assets_root_dir_from_exe_dir(exe_dir: &Path) -> Option<PathBuf> {
    if let Some(root) = bundled_runtime_assets_root_from_exe_dir(exe_dir) {
        return Some(root);
    }

    if let Some(root) = workspace_runtime_assets_root_from_exe_dir(exe_dir) {
        return Some(root);
    }

    Some(exe_dir.to_path_buf())
}

fn runtime_assets_root_candidates() -> Vec<PathBuf> {
    fn push_unique(roots: &mut Vec<PathBuf>, path: PathBuf) {
        if !roots.iter().any(|existing| existing == &path) {
            roots.push(path);
        }
    }

    let mut roots: Vec<PathBuf> = Vec::new();

    if let Ok(dir) = std::env::var("CRATEBAY_RUNTIME_ASSETS_DIR") {
        if !dir.trim().is_empty() {
            push_unique(&mut roots, PathBuf::from(dir));
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            if let Some(root) = bundled_runtime_assets_root_from_exe_dir(exe_dir) {
                push_unique(&mut roots, root);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            push_unique(
                &mut roots,
                PathBuf::from(local_app_data)
                    .join("Programs")
                    .join("CrateBay")
                    .join("resources"),
            );
        }
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            push_unique(
                &mut roots,
                PathBuf::from(program_files)
                    .join("CrateBay")
                    .join("resources"),
            );
        }
        if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
            push_unique(
                &mut roots,
                PathBuf::from(program_files_x86)
                    .join("CrateBay")
                    .join("resources"),
            );
        }
    }

    #[cfg(target_os = "linux")]
    {
        push_unique(&mut roots, PathBuf::from("/opt/CrateBay").join("resources"));
        push_unique(
            &mut roots,
            PathBuf::from("/usr/lib/CrateBay").join("resources"),
        );
        push_unique(
            &mut roots,
            PathBuf::from("/usr/lib/cratebay").join("resources"),
        );
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            if let Some(root) = workspace_runtime_assets_root_from_exe_dir(exe_dir) {
                push_unique(&mut roots, root);
            }
            push_unique(&mut roots, exe_dir.to_path_buf());
        }
    }

    // If the CLI is installed separately from the desktop app, the runtime assets
    // won't be located next to the CLI executable. On macOS, fall back to the
    // default app bundle locations so `cratebay runtime start` works out-of-box
    // after installing the desktop app.
    #[cfg(target_os = "macos")]
    {
        push_unique(
            &mut roots,
            PathBuf::from("/Applications/CrateBay.app/Contents/Resources"),
        );
        if let Some(home) = std::env::var_os("HOME") {
            push_unique(
                &mut roots,
                PathBuf::from(home)
                    .join("Applications")
                    .join("CrateBay.app")
                    .join("Contents")
                    .join("Resources"),
            );
        }
    }

    roots
}

fn runtime_images_dir_candidates() -> Vec<PathBuf> {
    runtime_assets_root_candidates()
        .into_iter()
        .filter_map(|root| runtime_images_dir_from_root(&root))
        .collect()
}

pub fn bundled_runtime_assets_dir() -> Option<PathBuf> {
    runtime_images_dir_candidates().into_iter().next()
}

#[cfg(target_os = "linux")]
fn runtime_linux_assets_dir_from_root(root: &Path) -> Option<PathBuf> {
    if root
        .file_name()
        .is_some_and(|name| name == DEFAULT_LINUX_RUNTIME_ASSETS_SUBDIR)
        && root.is_dir()
    {
        return Some(root.to_path_buf());
    }

    let dir = root.join(DEFAULT_LINUX_RUNTIME_ASSETS_SUBDIR);
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn runtime_linux_assets_dir() -> Option<PathBuf> {
    let mut incomplete_dir: Option<PathBuf> = None;

    for root in runtime_assets_root_candidates() {
        let Some(dir) = runtime_linux_assets_dir_from_root(&root) else {
            continue;
        };

        let bundle_dir = dir.join(runtime_linux_bundle_id());
        let qemu_path = bundle_dir.join(runtime_linux_qemu_binary_name());
        if qemu_path.is_file() {
            return Some(dir);
        }

        if incomplete_dir.is_none() {
            incomplete_dir = Some(dir);
        }
    }

    incomplete_dir
}

fn file_contains_placeholder_marker(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if meta.len() >= 1024 {
        return false;
    }

    std::fs::read_to_string(path)
        .map(|txt| txt.contains("PLACEHOLDER"))
        .unwrap_or(false)
}

fn runtime_image_assets_dir(image_id: &str) -> Option<PathBuf> {
    let mut placeholder_dir: Option<PathBuf> = None;

    for images_dir in runtime_images_dir_candidates() {
        let dir = images_dir.join(image_id);
        if !dir.is_dir() {
            continue;
        }

        match bundled_assets_ready(image_id, &dir) {
            Some(true) => return Some(dir),
            Some(false) => {
                if placeholder_dir.is_none() {
                    placeholder_dir = Some(dir);
                }
            }
            None => {}
        }
    }

    placeholder_dir
}

fn required_image_files(image_id: &str) -> Vec<&'static str> {
    let rootfs_required = crate::images::find_image(image_id)
        .map(|e| !e.rootfs_url.trim().is_empty())
        .unwrap_or(true);

    let mut files = vec!["vmlinuz", "initramfs"];
    if rootfs_required {
        files.push("rootfs.img");
    }
    files
}

fn bundled_assets_ready(image_id: &str, image_dir: &Path) -> Option<bool> {
    let mut has_placeholder = false;
    for name in required_image_files(image_id) {
        let path = image_dir.join(name);
        if !path.is_file() {
            return None;
        }
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() < 1024 && file_contains_placeholder_marker(&path) {
                has_placeholder = true;
            }
        }
    }

    Some(!has_placeholder)
}

fn image_files_present(image_id: &str) -> bool {
    let dest_dir = crate::images::image_dir(image_id);
    required_image_files(image_id)
        .into_iter()
        .all(|name| dest_dir.join(name).is_file())
}

fn files_equal(src: &Path, dest: &Path) -> Result<bool, HypervisorError> {
    use std::io::Read;

    let mut src_file = std::fs::File::open(src)?;
    let mut dest_file = std::fs::File::open(dest)?;
    let mut src_buf = [0u8; 128 * 1024];
    let mut dest_buf = [0u8; 128 * 1024];

    loop {
        let src_read = src_file.read(&mut src_buf)?;
        let dest_read = dest_file.read(&mut dest_buf)?;

        if src_read != dest_read {
            return Ok(false);
        }
        if src_read == 0 {
            return Ok(true);
        }

        if src_buf[..src_read] != dest_buf[..dest_read] {
            return Ok(false);
        }
    }
}

fn file_matches(src: &Path, dest: &Path) -> Result<bool, HypervisorError> {
    if !src.is_file() || !dest.is_file() {
        return Ok(false);
    }
    let src_meta = std::fs::metadata(src)?;
    let dest_meta = std::fs::metadata(dest)?;
    if src_meta.len() != dest_meta.len() {
        return Ok(false);
    }

    files_equal(src, dest)
}

fn runtime_image_installed_up_to_date(image_id: &str) -> Result<bool, HypervisorError> {
    if !crate::images::is_image_ready(image_id) {
        return Ok(false);
    }
    if !image_files_present(image_id) {
        return Ok(false);
    }

    // If we can't locate bundled assets (for example, only the CLI is installed),
    // keep the already-installed runtime image usable.
    let Some(assets_dir) = runtime_image_assets_dir(image_id) else {
        return Ok(true);
    };

    let dest_dir = crate::images::image_dir(image_id);
    for name in required_image_files(image_id) {
        let src = assets_dir.join(name);
        let dest = dest_dir.join(name);

        // If bundled assets are missing, we can't compare; don't fail an existing install.
        if !src.is_file() {
            continue;
        }

        if !file_matches(&src, &dest)? {
            return Ok(false);
        }
    }

    Ok(true)
}

fn write_ready_metadata(image_id: &str) -> Result<(), HypervisorError> {
    let dir = crate::images::image_dir(image_id);
    std::fs::create_dir_all(&dir)?;

    let path = dir.join("metadata.json");
    let json = serde_json::to_vec_pretty(&serde_json::json!({ "status": "ready" }))
        .map_err(|e| HypervisorError::CreateFailed(e.to_string()))?;
    crate::store::write_atomic(&path, &json)?;
    Ok(())
}

fn install_runtime_image_from_assets(image_id: &str) -> Result<(), HypervisorError> {
    let Some(assets_dir) = runtime_image_assets_dir(image_id) else {
        return Err(HypervisorError::CreateFailed(format!(
            "CrateBay Runtime assets not found for image '{}'. Ensure the desktop app is installed correctly or set CRATEBAY_RUNTIME_ASSETS_DIR.",
            image_id
        )));
    };

    let dest_dir = crate::images::image_dir(image_id);
    std::fs::create_dir_all(&dest_dir)?;

    let rootfs_required = crate::images::find_image(image_id)
        .map(|e| !e.rootfs_url.trim().is_empty())
        .unwrap_or(true);

    let copy_required = |name: &str| -> Result<(), HypervisorError> {
        let src = assets_dir.join(name);
        if !src.is_file() {
            return Err(HypervisorError::CreateFailed(format!(
                "Missing runtime asset '{}': {}",
                name,
                src.display()
            )));
        }
        if file_contains_placeholder_marker(&src) {
            return Err(HypervisorError::CreateFailed(format!(
                "Runtime asset '{}' is a placeholder. Fetch real assets before using CrateBay Runtime.",
                src.display()
            )));
        }
        let dest = dest_dir.join(name);
        crate::fsutil::copy_file_fast(&src, &dest)?;
        Ok(())
    };

    copy_required("vmlinuz")?;
    copy_required("initramfs")?;
    if rootfs_required {
        copy_required("rootfs.img")?;
    }

    write_ready_metadata(image_id)?;
    Ok(())
}

fn ensure_runtime_image_ready(image_id: &str) -> Result<(), HypervisorError> {
    if runtime_image_installed_up_to_date(image_id)? {
        return Ok(());
    }

    install_runtime_image_from_assets(image_id)?;
    if !crate::images::is_image_ready(image_id) {
        return Err(HypervisorError::CreateFailed(format!(
            "Runtime OS image '{}' was installed but is still not marked ready",
            image_id
        )));
    }
    Ok(())
}

fn existing_runtime_vm(
    hv: &dyn Hypervisor,
) -> Result<Option<crate::hypervisor::VmInfo>, HypervisorError> {
    let name = runtime_vm_name();
    Ok(hv.list_vms()?.into_iter().find(|vm| vm.name == name))
}

fn create_runtime_vm(hv: &dyn Hypervisor, image_id: &str) -> Result<String, HypervisorError> {
    if crate::images::find_image(image_id).is_none() {
        return Err(HypervisorError::CreateFailed(format!(
            "Runtime OS image '{}' not found in catalog",
            image_id
        )));
    }

    ensure_runtime_image_ready(image_id)?;

    let paths = crate::images::image_paths(image_id);
    let config = VmConfig {
        name: runtime_vm_name().to_string(),
        cpus: 2,
        memory_mb: 2048,
        disk_gb: 20,
        rosetta: hv.rosetta_available(),
        shared_dirs: vec![],
        os_image: Some(image_id.to_string()),
        kernel_path: Some(paths.kernel_path.to_string_lossy().into_owned()),
        initrd_path: Some(paths.initrd_path.to_string_lossy().into_owned()),
        disk_path: None,
        port_forwards: vec![],
    };

    hv.create_vm(config)
}

pub fn stop_runtime_vm_if_exists(hv: &dyn Hypervisor) -> Result<(), HypervisorError> {
    if let Some(vm) = existing_runtime_vm(hv)? {
        let _ = hv.stop_vm(&vm.id);
    }
    Ok(())
}

pub fn reset_runtime_vm(hv: &dyn Hypervisor) -> Result<String, HypervisorError> {
    let image_id = runtime_os_image_id().to_string();
    stop_runtime_vm_if_exists(hv)?;

    if let Some(vm) = existing_runtime_vm(hv)? {
        hv.delete_vm(&vm.id)?;
    }

    create_runtime_vm(hv, image_id.as_str())
}

/// Ensure the built-in runtime VM exists, returning its VM id.
pub fn ensure_runtime_vm_exists(hv: &dyn Hypervisor) -> Result<String, HypervisorError> {
    let default_image_id = runtime_os_image_id().to_string();
    if let Some(vm) = existing_runtime_vm(hv)? {
        let image_id = vm.os_image.as_deref().unwrap_or(default_image_id.as_str());
        ensure_runtime_image_ready(image_id)?;
        return Ok(vm.id);
    }

    create_runtime_vm(hv, default_image_id.as_str())
}

/// Ensure the built-in runtime VM is running and the host Docker socket exists.
pub fn ensure_runtime_vm_running(hv: &dyn Hypervisor) -> Result<String, HypervisorError> {
    let vm_id = ensure_runtime_vm_exists(hv)?;

    let vms = hv.list_vms()?;
    let state = vms
        .into_iter()
        .find(|vm| vm.id == vm_id)
        .map(|vm| vm.state)
        .unwrap_or(VmState::Stopped);

    if state != VmState::Running {
        hv.start_vm(&vm_id)?;
    }

    Ok(vm_id)
}

// ---------------------------------------------------------------------------
// Linux runtime: bundled QEMU/KVM runner + Docker guest
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
static LINUX_RUNTIME_LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();

#[cfg(target_os = "linux")]
fn linux_runtime_lock() -> &'static std::sync::Mutex<()> {
    LINUX_RUNTIME_LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

#[cfg(target_os = "linux")]
fn linux_runtime_docker_port() -> u16 {
    std::env::var("CRATEBAY_LINUX_DOCKER_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port > 0)
        .unwrap_or(DEFAULT_LINUX_DOCKER_PORT)
}

#[cfg(target_os = "linux")]
fn runtime_linux_bundle_id() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        "cratebay-runtime-linux-aarch64"
    }

    #[cfg(target_arch = "x86_64")]
    {
        "cratebay-runtime-linux-x86_64"
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        "cratebay-runtime-linux-x86_64"
    }
}

#[cfg(target_os = "linux")]
fn runtime_linux_qemu_binary_name() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        "qemu-system-aarch64"
    }

    #[cfg(target_arch = "x86_64")]
    {
        "qemu-system-x86_64"
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        "qemu-system-x86_64"
    }
}

#[cfg(target_os = "linux")]
fn runtime_linux_dir() -> PathBuf {
    crate::store::data_dir().join("runtime-linux")
}

#[cfg(target_os = "linux")]
fn runtime_linux_pidfile_path() -> PathBuf {
    runtime_linux_dir().join("qemu.pid")
}

#[cfg(target_os = "linux")]
fn runtime_linux_disk_path() -> PathBuf {
    runtime_linux_dir().join("disk.raw")
}

#[cfg(target_os = "linux")]
pub fn runtime_linux_console_log_path() -> PathBuf {
    runtime_linux_dir().join("console.log")
}

#[cfg(target_os = "linux")]
pub fn runtime_linux_docker_host() -> String {
    format!("tcp://127.0.0.1:{}", linux_runtime_docker_port())
}

#[cfg(target_os = "linux")]
fn runtime_linux_bundle_dir() -> Result<PathBuf, HypervisorError> {
    let Some(root) = runtime_linux_assets_dir() else {
        return Err(HypervisorError::CreateFailed(
            "CrateBay Linux runtime assets not found. Ensure the desktop app is installed correctly or set CRATEBAY_RUNTIME_ASSETS_DIR.".into(),
        ));
    };

    let dir = root.join(runtime_linux_bundle_id());
    if dir.is_dir() {
        Ok(dir)
    } else {
        Err(HypervisorError::CreateFailed(format!(
            "CrateBay Linux runtime bundle not found: {}",
            dir.display()
        )))
    }
}

#[cfg(target_os = "linux")]
fn find_executable_on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|raw| {
        std::env::split_paths(&raw)
            .map(|dir| dir.join(name))
            .find(|path| path.is_file())
    })
}

#[cfg(target_os = "linux")]
fn runtime_linux_qemu_path() -> Result<PathBuf, HypervisorError> {
    if let Ok(path) = std::env::var("CRATEBAY_RUNTIME_QEMU_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(HypervisorError::CreateFailed(format!(
            "CRATEBAY_RUNTIME_QEMU_PATH does not point to a file: {}",
            path.display()
        )));
    }

    if let Ok(bundle_dir) = runtime_linux_bundle_dir() {
        let bundled = bundle_dir.join(runtime_linux_qemu_binary_name());
        if bundled.is_file() {
            return Ok(bundled);
        }
    }

    if let Some(path) = find_executable_on_path(runtime_linux_qemu_binary_name()) {
        return Ok(path);
    }

    Err(HypervisorError::CreateFailed(format!(
        "Bundled Linux runtime helper '{}' was not found. Reinstall CrateBay or set CRATEBAY_RUNTIME_QEMU_PATH.",
        runtime_linux_qemu_binary_name()
    )))
}

#[cfg(target_os = "linux")]
fn runtime_linux_qemu_lib_dir(qemu_path: &Path) -> Option<PathBuf> {
    let dir = qemu_path.parent()?.join("lib");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn runtime_linux_qemu_share_dir(qemu_path: &Path) -> Option<PathBuf> {
    let dir = qemu_path.parent()?.join("share").join("qemu");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn runtime_linux_pid() -> Option<u32> {
    let raw = std::fs::read_to_string(runtime_linux_pidfile_path()).ok()?;
    raw.trim().parse::<u32>().ok().filter(|pid| *pid > 0)
}

#[cfg(target_os = "linux")]
fn linux_process_is_alive(pid: u32) -> bool {
    let rc = unsafe { libc::kill(pid as i32, 0) };
    if rc == 0 {
        return true;
    }

    matches!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EPERM)
    )
}

#[cfg(target_os = "linux")]
fn linux_runtime_wait_ready(timeout: std::time::Duration) -> Result<(), String> {
    let host = runtime_linux_docker_host();
    let deadline = std::time::Instant::now() + timeout;
    let mut last_error = "Docker runtime is still starting".to_string();

    while std::time::Instant::now() < deadline {
        match docker_http_ping_host(&host) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    Err(last_error)
}

#[cfg(target_os = "linux")]
fn linux_runtime_default_cmdline() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        "console=ttyAMA0 panic=1"
    }

    #[cfg(target_arch = "x86_64")]
    {
        "console=ttyS0 panic=1"
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        "console=ttyS0 panic=1"
    }
}

#[cfg(target_os = "linux")]
fn linux_runtime_cmdline() -> String {
    let mut cmdline = std::env::var("CRATEBAY_LINUX_RUNTIME_CMDLINE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| linux_runtime_default_cmdline().to_string());

    if !cmdline
        .split_whitespace()
        .any(|arg| arg.starts_with("cratebay_http_proxy="))
    {
        if let Ok(proxy) = std::env::var("CRATEBAY_RUNTIME_HTTP_PROXY") {
            let proxy = proxy
                .trim()
                .trim_start_matches("http://")
                .trim_start_matches("https://");
            if !proxy.is_empty() {
                cmdline.push_str(" cratebay_http_proxy=");
                cmdline.push_str(proxy);
            }
        }
    }

    cmdline
}

#[cfg(target_os = "linux")]
fn linux_kvm_available() -> bool {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/kvm")
        .is_ok()
}

#[cfg(target_os = "linux")]
fn tail_linux_runtime_console_log() -> String {
    let path = runtime_linux_console_log_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return String::new();
    };

    let lines = text.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return String::new();
    }

    let tail = lines
        .iter()
        .rev()
        .take(25)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");

    format!("\nConsole tail ({}):\n{}", path.display(), tail)
}

#[cfg(target_os = "linux")]
fn ensure_linux_runtime_disk() -> Result<PathBuf, HypervisorError> {
    let path = runtime_linux_disk_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if !path.exists() {
        let file = std::fs::File::create(&path)?;
        file.set_len(20_u64 * 1024 * 1024 * 1024)?;
    }

    Ok(path)
}

#[cfg(target_os = "linux")]
fn stop_runtime_linux_impl() -> Result<(), HypervisorError> {
    if let Some(pid) = runtime_linux_pid() {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if !linux_process_is_alive(pid) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        if linux_process_is_alive(pid) {
            unsafe {
                libc::kill(pid as i32, libc::SIGKILL);
            }
        }
    }

    let _ = std::fs::remove_file(runtime_linux_pidfile_path());
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn runtime_linux_is_running() -> bool {
    runtime_linux_pid().is_some_and(linux_process_is_alive)
}

#[cfg(target_os = "linux")]
pub fn stop_runtime_linux() -> Result<(), HypervisorError> {
    let _guard = crate::lock_or_recover(linux_runtime_lock());
    stop_runtime_linux_impl()
}

#[cfg(target_os = "linux")]
pub fn ensure_runtime_linux_running() -> Result<String, HypervisorError> {
    let _guard = crate::lock_or_recover(linux_runtime_lock());
    let host = runtime_linux_docker_host();

    if docker_http_ping_host(&host).is_ok() {
        return Ok(host);
    }

    if runtime_linux_is_running() {
        if linux_runtime_wait_ready(std::time::Duration::from_secs(5)).is_ok() {
            return Ok(host);
        }
        stop_runtime_linux_impl()?;
    } else {
        let _ = std::fs::remove_file(runtime_linux_pidfile_path());
    }

    ensure_runtime_image_ready(runtime_os_image_id())?;

    let runtime_dir = runtime_linux_dir();
    std::fs::create_dir_all(&runtime_dir)?;
    let _ = std::fs::remove_file(runtime_linux_pidfile_path());

    let console_log = runtime_linux_console_log_path();
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&console_log)?;

    let qemu_path = runtime_linux_qemu_path()?;
    let disk_path = ensure_linux_runtime_disk()?;
    let image_paths = crate::images::image_paths(runtime_os_image_id());
    let pidfile = runtime_linux_pidfile_path();
    let host_port = linux_runtime_docker_port();
    let guest_port = docker_proxy_port();
    let use_kvm = linux_kvm_available();
    let cmdline = linux_runtime_cmdline();

    let machine = if cfg!(target_arch = "aarch64") {
        if use_kvm {
            "virt,accel=kvm"
        } else {
            "virt,accel=tcg"
        }
    } else if use_kvm {
        "q35,accel=kvm"
    } else {
        "q35,accel=tcg"
    };

    let cpu = if use_kvm { "host" } else { "max" };

    let mut cmd = std::process::Command::new(&qemu_path);
    cmd.arg("-name")
        .arg(runtime_vm_name())
        .arg("-machine")
        .arg(machine)
        .arg("-cpu")
        .arg(cpu)
        .arg("-smp")
        .arg("2")
        .arg("-m")
        .arg("2048")
        .arg("-kernel")
        .arg(&image_paths.kernel_path)
        .arg("-initrd")
        .arg(&image_paths.initrd_path)
        .arg("-append")
        .arg(&cmdline)
        .arg("-drive")
        .arg(format!("if=virtio,format=raw,file={}", disk_path.display()))
        .arg("-netdev")
        .arg(format!(
            "user,id=net0,hostfwd=tcp:127.0.0.1:{host_port}-:{guest_port}"
        ))
        .arg("-device")
        .arg("virtio-net-pci,netdev=net0")
        .arg("-device")
        .arg("virtio-rng-pci")
        .arg("-serial")
        .arg(format!("file:{}", console_log.display()))
        .arg("-display")
        .arg("none")
        .arg("-monitor")
        .arg("none")
        .arg("-daemonize")
        .arg("-pidfile")
        .arg(&pidfile)
        .arg("-no-reboot");

    if let Some(share_dir) = runtime_linux_qemu_share_dir(&qemu_path) {
        cmd.arg("-L").arg(share_dir);
    }

    if let Some(lib_dir) = runtime_linux_qemu_lib_dir(&qemu_path) {
        let current = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
        let joined = if current.trim().is_empty() {
            lib_dir.to_string_lossy().into_owned()
        } else {
            format!("{}:{}", lib_dir.display(), current)
        };
        cmd.env("LD_LIBRARY_PATH", joined);
    }

    let out = cmd.output().map_err(|error| {
        HypervisorError::CreateFailed(format!(
            "Failed to launch CrateBay Linux runtime helper '{}': {}",
            qemu_path.display(),
            error
        ))
    })?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("exit {}", out.status)
        };
        return Err(HypervisorError::CreateFailed(format!(
            "Failed to start CrateBay Runtime (Linux/QEMU): {}",
            detail
        )));
    }

    linux_runtime_wait_ready(std::time::Duration::from_secs(45)).map_err(|error| {
        let _ = stop_runtime_linux_impl();
        HypervisorError::CreateFailed(format!(
            "CrateBay Runtime (Linux/QEMU) did not become ready within 45 seconds: {}{}",
            error,
            tail_linux_runtime_console_log()
        ))
    })?;

    Ok(host)
}

// ---------------------------------------------------------------------------
// Windows runtime: WSL2 distro + dockerd on TCP
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn wsl_docker_port() -> u16 {
    std::env::var("CRATEBAY_WSL_DOCKER_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|p| *p > 0)
        .unwrap_or(DEFAULT_WSL_DOCKER_PORT)
}

#[cfg(target_os = "windows")]
fn wsl_list_distros() -> Result<Vec<String>, HypervisorError> {
    use std::process::Command;

    let mut command = Command::new("wsl.exe");
    command.args(["-l", "-q"]);
    let out = run_windows_command_with_timeout(
        &mut command,
        std::time::Duration::from_secs(20),
        "wsl.exe -l -q",
    )?;

    if !out.status.success() {
        return Err(HypervisorError::CreateFailed(format!(
            "wsl.exe -l failed (exit {}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        )));
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

#[cfg(target_os = "windows")]
fn run_windows_command_with_timeout(
    command: &mut std::process::Command,
    timeout: std::time::Duration,
    description: &str,
) -> Result<std::process::Output, HypervisorError> {
    use std::process::Stdio;

    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            HypervisorError::CreateFailed(format!("Failed to start {}: {}", description, error))
        })?;

    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child.wait_with_output().map_err(|error| {
                    HypervisorError::CreateFailed(format!(
                        "Failed to collect output for {}: {}",
                        description, error
                    ))
                });
            }
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(HypervisorError::CreateFailed(format!(
                    "{} timed out after {} seconds",
                    description,
                    timeout.as_secs()
                )));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(HypervisorError::CreateFailed(format!(
                    "Failed while waiting for {}: {}",
                    description, error
                )));
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn wsl_distro_exists(name: &str) -> Result<bool, HypervisorError> {
    let distros = wsl_list_distros()?;
    Ok(distros.iter().any(|d| d == name))
}

#[cfg(target_os = "windows")]
fn runtime_wsl_assets_dir_from_root(root: &Path) -> Option<PathBuf> {
    if root
        .file_name()
        .is_some_and(|n| n == DEFAULT_WSL_ASSETS_SUBDIR)
        && root.is_dir()
    {
        return Some(root.to_path_buf());
    }
    let dir = root.join(DEFAULT_WSL_ASSETS_SUBDIR);
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
fn runtime_wsl_assets_dir() -> Option<PathBuf> {
    let mut placeholder_dir: Option<PathBuf> = None;

    for root in runtime_assets_root_candidates() {
        let Some(dir) = runtime_wsl_assets_dir_from_root(&root) else {
            continue;
        };

        let path = dir.join(runtime_wsl_image_id()).join("rootfs.tar");
        if path.is_file() && !file_contains_placeholder_marker(&path) {
            return Some(dir);
        }

        if placeholder_dir.is_none() {
            placeholder_dir = Some(dir);
        }
    }

    placeholder_dir
}

#[cfg(target_os = "windows")]
fn runtime_wsl_image_id() -> String {
    #[cfg(target_arch = "aarch64")]
    {
        "cratebay-runtime-wsl-aarch64".to_string()
    }
    #[cfg(target_arch = "x86_64")]
    {
        "cratebay-runtime-wsl-x86_64".to_string()
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        "cratebay-runtime-wsl-x86_64".to_string()
    }
}

#[cfg(target_os = "windows")]
fn runtime_wsl_rootfs_tar_path() -> Result<PathBuf, HypervisorError> {
    if let Ok(p) = std::env::var("CRATEBAY_WSL_ROOTFS_TAR") {
        if !p.trim().is_empty() {
            return Ok(PathBuf::from(p));
        }
    }

    let Some(dir) = runtime_wsl_assets_dir() else {
        return Err(HypervisorError::CreateFailed(
            "CrateBay WSL runtime assets not found. Ensure the desktop app is installed correctly or set CRATEBAY_RUNTIME_ASSETS_DIR / CRATEBAY_WSL_ROOTFS_TAR.".into(),
        ));
    };

    let image_dir = dir.join(runtime_wsl_image_id());
    let path = image_dir.join("rootfs.tar");
    if !path.is_file() {
        return Err(HypervisorError::CreateFailed(format!(
            "Missing WSL runtime asset rootfs.tar: {}",
            path.display()
        )));
    }

    if file_contains_placeholder_marker(&path) {
        return Err(HypervisorError::CreateFailed(format!(
            "WSL runtime asset '{}' is a placeholder. Fetch real assets before using CrateBay Runtime.",
            path.display()
        )));
    }

    Ok(path)
}

#[cfg(target_os = "windows")]
fn wsl_install_dir(distro: &str) -> PathBuf {
    crate::store::data_dir().join("wsl").join(distro)
}

#[cfg(target_os = "windows")]
fn prepare_wsl_install_dir(distro: &str) -> Result<PathBuf, HypervisorError> {
    let install_dir = wsl_install_dir(distro);

    if install_dir.exists() {
        let mut entries = std::fs::read_dir(&install_dir)?;
        if entries.next().is_some() {
            std::fs::remove_dir_all(&install_dir)?;
        }
    }

    std::fs::create_dir_all(&install_dir)?;
    Ok(install_dir)
}

#[cfg(target_os = "windows")]
fn wsl_import_runtime_distro(distro: &str) -> Result<(), HypervisorError> {
    use std::process::Command;

    let rootfs = runtime_wsl_rootfs_tar_path()?;
    let install_dir = prepare_wsl_install_dir(distro)?;

    let mut command = Command::new("wsl.exe");
    command.args([
        "--import",
        distro,
        &install_dir.to_string_lossy(),
        &rootfs.to_string_lossy(),
        "--version",
        "2",
    ]);
    let description = format!("wsl.exe --import {}", distro);
    let out = run_windows_command_with_timeout(
        &mut command,
        std::time::Duration::from_secs(300),
        &description,
    )?;

    if !out.status.success() {
        return Err(HypervisorError::CreateFailed(format!(
            "wsl.exe --import failed (exit {}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        )));
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn wsl_exec(distro: &str, shell_cmd: &str) -> Result<String, HypervisorError> {
    use std::process::Command;

    // Run via `sh -lc` to keep quoting predictable.
    let mut command = Command::new("wsl.exe");
    command.args(["-d", distro, "--", "sh", "-lc", shell_cmd]);
    let description = format!("wsl.exe exec in '{}'", distro);
    let out = run_windows_command_with_timeout(
        &mut command,
        std::time::Duration::from_secs(30),
        &description,
    )?;

    if !out.status.success() {
        return Err(HypervisorError::CreateFailed(format!(
            "wsl.exe exec failed (exit {}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        )));
    }

    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(any(test, target_os = "windows"))]
fn extract_first_non_loopback_ipv4(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() {
            return None;
        }

        if trimmed_line.contains(" lo") && trimmed_line.contains("inet ") {
            return None;
        }

        trimmed_line.split_whitespace().find_map(|token| {
            let candidate = token
                .trim_matches(|c: char| c == '"' || c == '\'' || c == ',' || c == ';')
                .split('/')
                .next()
                .unwrap_or(token)
                .trim();

            if candidate.is_empty() || candidate.starts_with("127.") || candidate == "0.0.0.0" {
                return None;
            }

            let octets = candidate
                .split('.')
                .map(str::parse::<u8>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;

            if octets.len() == 4 {
                Some(candidate.to_string())
            } else {
                None
            }
        })
    })
}

#[cfg(any(test, target_os = "windows"))]
fn is_usable_wsl_guest_ipv4(candidate: &str) -> bool {
    !(candidate.starts_with("127.")
        || candidate == "0.0.0.0"
        || candidate == "10.255.255.254"
        || candidate.starts_with("169.254."))
}

#[cfg(any(test, target_os = "windows"))]
fn extract_usable_wsl_guest_ipv4(output: &str) -> Option<String> {
    extract_first_non_loopback_ipv4(output).filter(|ip| is_usable_wsl_guest_ipv4(ip))
}

#[cfg(any(test, target_os = "windows"))]
fn extract_hosts_ipv4_for_hostname(output: &str, hostname: &str) -> Option<String> {
    let hostname = hostname.trim();
    if hostname.is_empty() {
        return None;
    }

    output.lines().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }

        let mut parts = trimmed.split_whitespace();
        let ip = parts.next()?;
        if !is_usable_wsl_guest_ipv4(ip) {
            return None;
        }

        if parts.any(|alias| alias == hostname) {
            Some(ip.to_string())
        } else {
            None
        }
    })
}

#[cfg(any(test, target_os = "windows"))]
fn extract_first_non_loopback_ipv4_from_fib_trie(output: &str) -> Option<String> {
    let mut last_ipv4 = None::<String>;

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(ip) = extract_usable_wsl_guest_ipv4(trimmed) {
            last_ipv4 = Some(ip);
        }

        if trimmed.contains("/32 host LOCAL") {
            if let Some(ip) = last_ipv4.take() {
                return Some(ip);
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn wsl_guest_ip(distro: &str) -> Result<String, HypervisorError> {
    let out = wsl_exec(
        distro,
        "ip -4 -o addr show 2>/dev/null | awk '$2 != \"lo\" {print $4}' | cut -d/ -f1 | head -n1",
    )
    .unwrap_or_default();
    if let Some(ip) = extract_usable_wsl_guest_ipv4(&out) {
        return Ok(ip);
    }

    let out = wsl_exec(
        distro,
        "hostname -I 2>/dev/null | awk '{print $1}' | tr -d '\\r'",
    )
    .unwrap_or_default();
    if let Some(ip) = extract_usable_wsl_guest_ipv4(&out) {
        return Ok(ip);
    }

    let out = wsl_exec(distro, "hostname -i 2>/dev/null | tr -d '\\r'").unwrap_or_default();
    if let Some(ip) = extract_usable_wsl_guest_ipv4(&out) {
        return Ok(ip);
    }

    let hostname = wsl_exec(distro, "hostname 2>/dev/null").unwrap_or_default();
    let hosts = wsl_exec(distro, "cat /etc/hosts 2>/dev/null").unwrap_or_default();
    if let Some(ip) = extract_hosts_ipv4_for_hostname(&hosts, &hostname) {
        return Ok(ip);
    }

    let out = wsl_exec(distro, "cat /proc/net/fib_trie 2>/dev/null").unwrap_or_default();
    if let Some(ip) = extract_first_non_loopback_ipv4_from_fib_trie(&out) {
        return Ok(ip);
    }

    Err(HypervisorError::CreateFailed(
        "Failed to determine WSL guest IP from iproute2, hostname, /etc/hosts, or /proc/net/fib_trie".into(),
    ))
}

#[cfg(any(test, target_os = "windows", target_os = "linux"))]
fn docker_host_tcp_endpoint(host: &str) -> Option<(String, u16)> {
    let endpoint = host.strip_prefix("tcp://")?;

    if endpoint.starts_with('[') {
        let end = endpoint.find(']')?;
        let host_part = endpoint.get(1..end)?.to_string();
        let port = endpoint.get(end + 1..)?.strip_prefix(':')?.parse().ok()?;
        return Some((host_part, port));
    }

    let (host_part, port_part) = endpoint.rsplit_once(':')?;
    let port = port_part.parse().ok()?;
    if host_part.trim().is_empty() {
        return None;
    }
    Some((host_part.to_string(), port))
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn docker_http_ping_host(host: &str) -> Result<(), String> {
    use std::io::{Read, Write};
    use std::net::ToSocketAddrs;
    use std::time::Duration;

    let (tcp_host, port) =
        docker_host_tcp_endpoint(host).ok_or_else(|| format!("invalid Docker host '{}'", host))?;

    let mut addresses = (tcp_host.as_str(), port)
        .to_socket_addrs()
        .map_err(|error| format!("resolve {}:{}: {}", tcp_host, port, error))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(format!(
            "resolve {}:{}: no addresses returned",
            tcp_host, port
        ));
    }
    addresses.sort_by_key(|address| if address.is_ipv4() { 0 } else { 1 });

    let mut last_error = None;
    for address in addresses {
        match std::net::TcpStream::connect_timeout(&address, Duration::from_millis(500)) {
            Ok(mut stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

                stream
                    .write_all(b"GET /_ping HTTP/1.1\r\nHost: docker\r\nConnection: close\r\n\r\n")
                    .map_err(|error| format!("write {}: {}", address, error))?;

                let mut buf = [0_u8; 512];
                let n = stream
                    .read(&mut buf)
                    .map_err(|error| format!("read {}: {}", address, error))?;
                let response = String::from_utf8_lossy(&buf[..n]);
                if response.contains("200 OK") || response.ends_with("OK") {
                    return Ok(());
                }

                last_error = Some(format!("{} returned unexpected response", address));
            }
            Err(error) => {
                last_error = Some(format!("connect {}: {}", address, error));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "unknown error".to_string()))
}

#[cfg(target_os = "windows")]
fn wsl_start_dockerd(distro: &str, port: u16) -> Result<(), HypervisorError> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let prep_cmd = "mkdir -p /var/lib/docker /var/run /var/log; \
         if [ -f /var/run/dockerd.pid ] && kill -0 \"$(cat /var/run/dockerd.pid)\" 2>/dev/null; then \
           echo running; \
           exit 0; \
         fi; \
         rm -f /var/run/dockerd.pid; \
         : > /var/log/dockerd.log; \
         echo start";
    let prep_output = wsl_exec(distro, prep_cmd)?;
    if prep_output.lines().any(|line| line.trim() == "running") {
        return Ok(());
    }

    let dockerd_cmd = format!(
        "ulimit -n 65536 >/dev/null 2>&1 || true; \
         exec dockerd --pidfile /var/run/dockerd.pid \
           -H unix:///var/run/docker.sock \
           -H tcp://0.0.0.0:{port} \
           >> /var/log/dockerd.log 2>&1"
    );

    Command::new("wsl.exe")
        .args(["-d", distro, "--", "sh", "-lc", &dockerd_cmd])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            HypervisorError::CreateFailed(format!(
                "Failed to start dockerd via wsl.exe for '{}': {}",
                distro, error
            ))
        })
}

#[cfg(target_os = "windows")]
fn wsl_runtime_diagnostics(distro: &str) -> String {
    let probes = [
        (
            "ip -4 -o addr show",
            "ip -4 -o addr show 2>/dev/null || true",
        ),
        ("hostname -I", "hostname -I 2>/dev/null || true"),
        ("hostname -i", "hostname -i 2>/dev/null || true"),
        ("fib_trie", "cat /proc/net/fib_trie 2>/dev/null || true"),
        ("which dockerd", "command -v dockerd 2>/dev/null || true"),
        ("dockerd --version", "dockerd --version 2>/dev/null || true"),
        (
            "ps -ef | grep dockerd",
            "ps -ef | grep '[d]ockerd' 2>/dev/null || true",
        ),
        (
            "dockerd.log",
            "tail -n 80 /var/log/dockerd.log 2>/dev/null || true",
        ),
    ];

    let mut diagnostics = Vec::new();
    for (label, command) in probes {
        match wsl_exec(distro, command) {
            Ok(output) if !output.trim().is_empty() => {
                diagnostics.push(format!("{label}:\n{}", output.trim()));
            }
            Ok(_) => {}
            Err(error) => diagnostics.push(format!("{label}: <probe failed: {error}>")),
        }
    }

    diagnostics.join("\n\n")
}

#[cfg(target_os = "windows")]
struct ProcessLocalWslHostRelay {
    target: std::sync::Arc<std::sync::Mutex<String>>,
}

#[cfg(target_os = "windows")]
fn process_local_wsl_host_relay() -> &'static std::sync::Mutex<Option<ProcessLocalWslHostRelay>> {
    static RELAY: OnceLock<std::sync::Mutex<Option<ProcessLocalWslHostRelay>>> = OnceLock::new();
    RELAY.get_or_init(|| std::sync::Mutex::new(None))
}

#[cfg(target_os = "windows")]
fn proxy_wsl_host_connection(
    inbound: std::net::TcpStream,
    target_addr: String,
) -> Result<(), HypervisorError> {
    use std::io;
    use std::net::Shutdown;

    let outbound = std::net::TcpStream::connect(&target_addr).map_err(|error| {
        HypervisorError::CreateFailed(format!(
            "Failed to connect to WSL Docker relay target {}: {}",
            target_addr, error
        ))
    })?;

    let _ = inbound.set_nodelay(true);
    let _ = outbound.set_nodelay(true);

    let mut inbound_reader = inbound.try_clone().map_err(|error| {
        HypervisorError::CreateFailed(format!("Failed to clone inbound relay stream: {}", error))
    })?;
    let mut inbound_writer = inbound;
    let mut outbound_reader = outbound.try_clone().map_err(|error| {
        HypervisorError::CreateFailed(format!("Failed to clone outbound relay stream: {}", error))
    })?;
    let mut outbound_writer = outbound;

    let client_to_server = std::thread::spawn(move || {
        let _ = io::copy(&mut inbound_reader, &mut outbound_writer);
        let _ = outbound_writer.shutdown(Shutdown::Write);
    });
    let server_to_client = std::thread::spawn(move || {
        let _ = io::copy(&mut outbound_reader, &mut inbound_writer);
        let _ = inbound_writer.shutdown(Shutdown::Write);
    });

    let _ = client_to_server.join();
    let _ = server_to_client.join();
    Ok(())
}

#[cfg(target_os = "windows")]
fn run_wsl_host_relay_listener(
    listener: std::net::TcpListener,
    target: std::sync::Arc<std::sync::Mutex<String>>,
) {
    while let Ok((inbound, _peer)) = listener.accept() {
        let target_addr = target
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        std::thread::spawn(move || {
            let _ = proxy_wsl_host_connection(inbound, target_addr);
        });
    }
}

#[cfg(target_os = "windows")]
fn ensure_process_local_wsl_host_relay(
    guest_ip: &str,
    port: u16,
) -> Result<String, HypervisorError> {
    let localhost = format!("tcp://127.0.0.1:{port}");
    if docker_http_ping_host(&localhost).is_ok() {
        return Ok(localhost);
    }

    let target_addr = format!("{guest_ip}:{port}");
    let relay_state = process_local_wsl_host_relay();
    let mut relay = relay_state
        .lock()
        .unwrap_or_else(|error| error.into_inner());

    if let Some(existing) = relay.as_ref() {
        *existing
            .target
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = target_addr;
        return Ok(localhost);
    }

    let listener =
        std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).map_err(|error| {
            HypervisorError::CreateFailed(format!(
                "Failed to bind local Docker relay on 127.0.0.1:{}: {}",
                port, error
            ))
        })?;

    let target = std::sync::Arc::new(std::sync::Mutex::new(target_addr));
    let thread_target = std::sync::Arc::clone(&target);
    std::thread::Builder::new()
        .name("cratebay-wsl-docker-relay".into())
        .spawn(move || run_wsl_host_relay_listener(listener, thread_target))
        .map_err(|error| {
            HypervisorError::CreateFailed(format!(
                "Failed to spawn local Docker relay thread: {}",
                error
            ))
        })?;

    *relay = Some(ProcessLocalWslHostRelay { target });
    Ok(localhost)
}

#[cfg(target_os = "windows")]
pub fn run_wsl_host_relay_server(
    listen_port: u16,
    target_host: &str,
    target_port: u16,
) -> Result<(), HypervisorError> {
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, listen_port))
        .map_err(|error| {
            HypervisorError::CreateFailed(format!(
                "Failed to bind WSL Docker relay on 127.0.0.1:{}: {}",
                listen_port, error
            ))
        })?;
    let target = std::sync::Arc::new(std::sync::Mutex::new(format!(
        "{}:{}",
        target_host, target_port
    )));
    run_wsl_host_relay_listener(listener, target);
    Ok(())
}

#[cfg(target_os = "windows")]
fn wait_for_wsl_dockerd_ready_in_guest(distro: &str, port: u16) -> Result<String, HypervisorError> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let mut last_error = "Docker runtime is still starting".to_string();
    let readiness_probe = "if [ -S /var/run/docker.sock ] && \
         grep -Eq 'Daemon has completed initialization|API listen on \\[::\\]:2375|API listen on /var/run/docker.sock' /var/log/dockerd.log 2>/dev/null; then \
           echo ready; \
         fi";

    while std::time::Instant::now() < deadline {
        match wsl_exec(distro, readiness_probe) {
            Ok(output) if output.lines().any(|line| line.trim() == "ready") => {
                let guest_ip = wsl_guest_ip(distro)?;
                return Ok(format!("tcp://{guest_ip}:{port}"));
            }
            Ok(_) => {
                last_error = "dockerd is still starting inside WSL".to_string();
            }
            Err(error) => last_error = error.to_string(),
        }

        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    let diagnostics = wsl_runtime_diagnostics(distro);

    let message = if diagnostics.trim().is_empty() {
        format!(
            "CrateBay Runtime (WSL2) did not become ready within 120 seconds: {}",
            last_error
        )
    } else {
        format!(
            "CrateBay Runtime (WSL2) did not become ready within 120 seconds: {}\n{}",
            last_error,
            diagnostics.trim()
        )
    };

    Err(HypervisorError::CreateFailed(message))
}

/// Ensure CrateBay Runtime is running on Windows via a WSL2 distro.
///
/// Returns the guest Docker endpoint inside WSL (e.g. `tcp://172.21.x.x:2375`).
#[cfg(target_os = "windows")]
pub fn ensure_runtime_wsl_guest_host() -> Result<String, HypervisorError> {
    let distro = runtime_vm_name();
    let port = wsl_docker_port();

    // 1) Ensure the distro exists (import if missing).
    if !wsl_distro_exists(distro)? {
        wsl_import_runtime_distro(distro)?;
    }

    // 2) Ensure dockerd is running inside WSL and only return once the Docker
    // API is actually responding inside the WSL guest.
    wsl_start_dockerd(distro, port)?;
    wait_for_wsl_dockerd_ready_in_guest(distro, port)
}

/// Ensure CrateBay Runtime is running on Windows via a WSL2 distro.
///
/// Returns a docker-compatible `DOCKER_HOST` value that is reachable from the
/// current process (preferably `tcp://127.0.0.1:2375`, falling back to the
/// WSL guest IP when that is reachable directly).
#[cfg(target_os = "windows")]
pub fn ensure_runtime_wsl_running() -> Result<String, HypervisorError> {
    let guest = ensure_runtime_wsl_guest_host()?;
    let (guest_ip, port) = docker_host_tcp_endpoint(&guest).ok_or_else(|| {
        HypervisorError::CreateFailed(format!("Invalid WSL guest Docker host '{}'", guest))
    })?;
    let localhost = format!("tcp://127.0.0.1:{port}");

    if docker_http_ping_host(&localhost).is_ok() {
        return Ok(localhost);
    }

    if let Ok(host) = ensure_process_local_wsl_host_relay(&guest_ip, port) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut relay_error = "local Docker relay is still starting".to_string();
        while std::time::Instant::now() < deadline {
            match docker_http_ping_host(&host) {
                Ok(()) => return Ok(host),
                Err(error) => relay_error = error,
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        if docker_http_ping_host(&guest).is_ok() {
            return Ok(guest);
        }

        return Err(HypervisorError::CreateFailed(format!(
            "CrateBay Runtime (WSL2) is running in the guest but the local Docker relay did not become reachable: {}",
            relay_error
        )));
    }

    if docker_http_ping_host(&guest).is_ok() {
        return Ok(guest);
    }

    Err(HypervisorError::CreateFailed(format!(
        "CrateBay Runtime (WSL2) is running in the guest but is not reachable from Windows at {} or {}",
        localhost, guest
    )))
}

/// Stop CrateBay Runtime on Windows (terminates the WSL distro).
#[cfg(target_os = "windows")]
pub fn stop_runtime_wsl() -> Result<(), HypervisorError> {
    use std::process::Command;

    let distro = runtime_vm_name();
    if !wsl_distro_exists(distro)? {
        return Ok(());
    }

    let mut command = Command::new("wsl.exe");
    command.args(["--terminate", distro]);
    let description = format!("wsl.exe --terminate {}", distro);
    let out = run_windows_command_with_timeout(
        &mut command,
        std::time::Duration::from_secs(30),
        &description,
    )?;

    if !out.status.success() {
        return Err(HypervisorError::CreateFailed(format!(
            "wsl.exe --terminate failed (exit {}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_assets_root_prefers_resources_next_to_exe_dir() {
        let dir = tempfile::tempdir().unwrap();
        let exe_dir = dir.path().join("bin");
        std::fs::create_dir_all(&exe_dir).unwrap();
        std::fs::create_dir_all(exe_dir.join("resources")).unwrap();

        let root = runtime_assets_root_dir_from_exe_dir(&exe_dir).unwrap();
        assert_eq!(root, exe_dir.join("resources"));
    }

    #[test]
    fn runtime_assets_root_falls_back_to_parent_resources_dir() {
        let dir = tempfile::tempdir().unwrap();
        let exe_dir = dir.path().join("bin");
        std::fs::create_dir_all(&exe_dir).unwrap();
        std::fs::create_dir_all(dir.path().join("resources")).unwrap();

        let root = runtime_assets_root_dir_from_exe_dir(&exe_dir).unwrap();
        assert_eq!(root, dir.path().join("resources"));
    }

    #[test]
    fn runtime_assets_root_falls_back_to_exe_dir_when_no_resources() {
        let dir = tempfile::tempdir().unwrap();
        let exe_dir = dir.path().join("bin");
        std::fs::create_dir_all(&exe_dir).unwrap();

        let root = runtime_assets_root_dir_from_exe_dir(&exe_dir).unwrap();
        assert_eq!(root, exe_dir);
    }

    #[test]
    fn runtime_assets_root_prefers_workspace_assets_over_target_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n").unwrap();

        let src_tauri_dir = dir
            .path()
            .join("crates")
            .join("cratebay-gui")
            .join("src-tauri");
        std::fs::create_dir_all(src_tauri_dir.join(DEFAULT_RUNTIME_ASSETS_SUBDIR)).unwrap();

        let exe_dir = dir.path().join("target").join("debug");
        std::fs::create_dir_all(exe_dir.join(DEFAULT_RUNTIME_ASSETS_SUBDIR)).unwrap();

        let root = runtime_assets_root_dir_from_exe_dir(&exe_dir).unwrap();
        assert_eq!(root, src_tauri_dir);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn runtime_linux_assets_dir_detects_bundle_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join(DEFAULT_LINUX_RUNTIME_ASSETS_SUBDIR);
        std::fs::create_dir_all(&root).unwrap();

        let detected = runtime_linux_assets_dir_from_root(&root).unwrap();
        assert_eq!(detected, root);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn runtime_assets_root_detects_macos_app_bundle_resources() {
        let dir = tempfile::tempdir().unwrap();
        let contents_dir = dir.path().join("CrateBay.app").join("Contents");
        let macos_dir = contents_dir.join("MacOS");
        let resources_dir = contents_dir.join("Resources");
        std::fs::create_dir_all(&macos_dir).unwrap();
        std::fs::create_dir_all(&resources_dir).unwrap();

        let root = runtime_assets_root_dir_from_exe_dir(&macos_dir).unwrap();
        assert_eq!(root, resources_dir);
    }

    #[test]
    fn docker_host_tcp_endpoint_parses_ipv4_hosts() {
        assert_eq!(
            docker_host_tcp_endpoint("tcp://127.0.0.1:2375"),
            Some(("127.0.0.1".to_string(), 2375))
        );
    }

    #[test]
    fn docker_host_tcp_endpoint_parses_ipv6_hosts() {
        assert_eq!(
            docker_host_tcp_endpoint("tcp://[::1]:2375"),
            Some(("::1".to_string(), 2375))
        );
    }

    #[test]
    fn docker_host_tcp_endpoint_rejects_invalid_hosts() {
        assert_eq!(
            docker_host_tcp_endpoint("unix:///var/run/docker.sock"),
            None
        );
        assert_eq!(docker_host_tcp_endpoint("tcp://:2375"), None);
        assert_eq!(docker_host_tcp_endpoint("tcp://127.0.0.1"), None);
    }

    #[test]
    fn extract_first_non_loopback_ipv4_skips_loopback_lines() {
        assert_eq!(
            extract_first_non_loopback_ipv4("1: lo    inet 10.255.255.254/32 scope global lo"),
            None
        );
        assert_eq!(
            extract_first_non_loopback_ipv4("inet 127.0.0.1/8 scope host lo"),
            None
        );
    }

    #[test]
    fn extract_first_non_loopback_ipv4_reads_hostname_output() {
        assert_eq!(
            extract_first_non_loopback_ipv4("172.28.245.112  fd00::1"),
            Some("172.28.245.112".to_string())
        );
        assert_eq!(
            extract_first_non_loopback_ipv4(
                "2: eth0    inet 172.28.245.112/20 brd 172.28.255.255 scope global eth0"
            ),
            Some("172.28.245.112".to_string())
        );
    }

    #[test]
    fn extract_first_non_loopback_ipv4_from_fib_trie_reads_local_host_entry() {
        let fib_trie = r#"
Main:
  +-- 172.28.240.0/20 2 0 2
     +-- 172.28.240.0/24 2 0 2
        |-- 172.28.240.0
           /20 link UNICAST
        |-- 172.28.245.112
           /32 host LOCAL
"#;

        assert_eq!(
            extract_first_non_loopback_ipv4_from_fib_trie(fib_trie),
            Some("172.28.245.112".to_string())
        );
    }

    #[test]
    fn extract_first_non_loopback_ipv4_from_fib_trie_skips_loopback_local_entry() {
        let fib_trie = r#"
Local:
  +-- 127.0.0.0/8 2 0 2
     |-- 127.0.0.1
        /32 host LOCAL
"#;

        assert_eq!(
            extract_first_non_loopback_ipv4_from_fib_trie(fib_trie),
            None
        );
    }

    #[test]
    fn extract_hosts_ipv4_for_hostname_reads_matching_host_entry() {
        let hosts = r#"
127.0.0.1 localhost
172.28.245.112 cratebay-wsl
"#;

        assert_eq!(
            extract_hosts_ipv4_for_hostname(hosts, "cratebay-wsl"),
            Some("172.28.245.112".to_string())
        );
    }

    #[test]
    fn extract_hosts_ipv4_for_hostname_skips_unusable_host_entry() {
        let hosts = r#"
10.255.255.254 cratebay-wsl
172.28.245.112 other-host
"#;

        assert_eq!(extract_hosts_ipv4_for_hostname(hosts, "cratebay-wsl"), None);
        assert!(!is_usable_wsl_guest_ipv4("10.255.255.254"));
    }
}
