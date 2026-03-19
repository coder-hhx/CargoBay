#[derive(Debug, Serialize)]
pub struct VmInfoDto {
    id: String,
    name: String,
    state: String,
    cpus: u32,
    memory_mb: u64,
    disk_gb: u64,
    rosetta_enabled: bool,
    mounts: Vec<SharedDirectoryDto>,
    port_forwards: Vec<PortForwardDto>,
    os_image: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PortForwardDto {
    host_port: u16,
    guest_port: u16,
    protocol: String,
}

#[derive(Debug, Serialize)]
pub struct SharedDirectoryDto {
    tag: String,
    host_path: String,
    guest_path: String,
    read_only: bool,
}

impl From<cratebay_core::hypervisor::SharedDirectory> for SharedDirectoryDto {
    fn from(value: cratebay_core::hypervisor::SharedDirectory) -> Self {
        Self {
            tag: value.tag,
            host_path: value.host_path,
            guest_path: value.guest_path,
            read_only: value.read_only,
        }
    }
}

impl From<proto::SharedDirectory> for SharedDirectoryDto {
    fn from(value: proto::SharedDirectory) -> Self {
        Self {
            tag: value.tag,
            host_path: value.host_path,
            guest_path: value.guest_path,
            read_only: value.read_only,
        }
    }
}

fn vm_state_to_string(state: cratebay_core::hypervisor::VmState) -> String {
    match state {
        cratebay_core::hypervisor::VmState::Running => "running".into(),
        cratebay_core::hypervisor::VmState::Stopped => "stopped".into(),
        cratebay_core::hypervisor::VmState::Creating => "creating".into(),
    }
}

async fn vm_list_inner(state: &AppState) -> Result<Vec<VmInfoDto>, String> {
    state.ensure_daemon().await;
    if let Ok(mut client) = connect_vm_service(&state.grpc_addr).await {
        if let Ok(resp) = client.list_v_ms(proto::ListVMsRequest {}).await {
            let resp = resp.into_inner();
            return Ok(resp
                .vms
                .into_iter()
                .map(|vm| VmInfoDto {
                    id: vm.vm_id,
                    name: vm.name,
                    state: vm.status,
                    cpus: vm.cpus,
                    memory_mb: vm.memory_mb,
                    disk_gb: vm.disk_gb,
                    rosetta_enabled: vm.rosetta_enabled,
                    mounts: vm
                        .shared_dirs
                        .into_iter()
                        .map(SharedDirectoryDto::from)
                        .collect(),
                    port_forwards: vm
                        .port_forwards
                        .into_iter()
                        .map(|pf| PortForwardDto {
                            host_port: pf.host_port as u16,
                            guest_port: pf.guest_port as u16,
                            protocol: pf.protocol,
                        })
                        .collect(),
                    os_image: None, // gRPC path does not expose os_image yet
                })
                .collect());
        }
    }

    let vms = state.hv.list_vms().map_err(|e| e.to_string())?;
    Ok(vms
        .into_iter()
        .map(|vm| {
            let os_img = vm.os_image.clone();
            VmInfoDto {
                id: vm.id,
                name: vm.name,
                state: vm_state_to_string(vm.state),
                cpus: vm.cpus,
                memory_mb: vm.memory_mb,
                disk_gb: vm.disk_gb,
                rosetta_enabled: vm.rosetta_enabled,
                mounts: vm
                    .shared_dirs
                    .into_iter()
                    .map(SharedDirectoryDto::from)
                    .collect(),
                port_forwards: vm
                    .port_forwards
                    .into_iter()
                    .map(|pf| PortForwardDto {
                        host_port: pf.host_port,
                        guest_port: pf.guest_port,
                        protocol: pf.protocol,
                    })
                    .collect(),
                os_image: os_img,
            }
        })
        .collect())
}

async fn vm_start_inner(state: &AppState, id: String) -> Result<(), String> {
    state.ensure_daemon().await;
    if let Ok(mut client) = connect_vm_service(&state.grpc_addr).await {
        client
            .start_vm(proto::StartVmRequest { vm_id: id })
            .await
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    state.hv.start_vm(&id).map_err(|e| e.to_string())
}

async fn vm_stop_inner(state: &AppState, id: String) -> Result<(), String> {
    state.ensure_daemon().await;
    if let Ok(mut client) = connect_vm_service(&state.grpc_addr).await {
        client
            .stop_vm(proto::StopVmRequest { vm_id: id })
            .await
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    state.hv.stop_vm(&id).map_err(|e| e.to_string())
}

async fn vm_delete_inner(state: &AppState, id: String) -> Result<(), String> {
    state.ensure_daemon().await;
    if let Ok(mut client) = connect_vm_service(&state.grpc_addr).await {
        client
            .delete_vm(proto::DeleteVmRequest { vm_id: id })
            .await
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    state.hv.delete_vm(&id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn vm_list(state: State<'_, AppState>) -> Result<Vec<VmInfoDto>, String> {
    vm_list_inner(state.inner()).await
}

#[tauri::command]
async fn vm_create(
    state: State<'_, AppState>,
    name: String,
    cpus: u32,
    memory_mb: u64,
    disk_gb: u64,
    rosetta: bool,
    os_image: Option<String>,
) -> Result<String, String> {
    state.ensure_daemon().await;
    validation::validate_vm_name(&name)
        .map_err(|e| format!("Invalid VM name '{}': {}", name, e))?;

    // Resolve image paths from the selected OS image.
    let (kernel_path, initrd_path, disk_path) = if let Some(ref img_id) = os_image {
        if !cratebay_core::images::is_image_ready(img_id) {
            return Err(format!("OS image '{}' is not downloaded yet", img_id));
        }
        let paths = cratebay_core::images::image_paths(img_id);
        (
            Some(paths.kernel_path.to_string_lossy().into_owned()),
            Some(paths.initrd_path.to_string_lossy().into_owned()),
            Some(paths.rootfs_path.to_string_lossy().into_owned()),
        )
    } else {
        (None, None, None)
    };

    if let Ok(mut client) = connect_vm_service(&state.grpc_addr).await {
        let resp = client
            .create_vm(proto::CreateVmRequest {
                name,
                cpus,
                memory_mb,
                disk_gb,
                rosetta,
                shared_dirs: vec![],
            })
            .await
            .map_err(|e| e.to_string())?
            .into_inner();
        return Ok(resp.vm_id);
    }

    use cratebay_core::hypervisor::VmConfig;
    let config = VmConfig {
        name,
        cpus,
        memory_mb,
        disk_gb,
        rosetta,
        shared_dirs: vec![],
        os_image,
        kernel_path,
        initrd_path,
        disk_path,
        port_forwards: vec![],
    };
    state.hv.create_vm(config).map_err(|e| e.to_string())
}

#[tauri::command]
async fn vm_start(state: State<'_, AppState>, id: String) -> Result<(), String> {
    vm_start_inner(state.inner(), id).await
}

#[tauri::command]
async fn vm_stop(state: State<'_, AppState>, id: String) -> Result<(), String> {
    vm_stop_inner(state.inner(), id).await
}

#[tauri::command]
async fn vm_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    vm_delete_inner(state.inner(), id).await
}

fn detect_vm_ssh_port(port_forwards: &[PortForwardDto]) -> Option<u16> {
    port_forwards
        .iter()
        .find(|pf| pf.guest_port == 22 && pf.protocol.eq_ignore_ascii_case("tcp"))
        .map(|pf| pf.host_port)
}

#[tauri::command]
fn vm_login_cmd(
    name: String,
    user: String,
    host: String,
    port: Option<u16>,
    port_forwards: Option<Vec<PortForwardDto>>,
) -> Result<String, String> {
    let detected_port = port_forwards.as_deref().and_then(detect_vm_ssh_port);
    let Some(port) = port.or(detected_port) else {
        return Err(
            "VM login is not available yet. Add a guest port 22 forward or specify an SSH port."
                .into(),
        );
    };
    Ok(format!(
        "ssh {}@{} -p {}
# VM: {}",
        user, host, port, name
    ))
}

#[tauri::command]
async fn vm_console(
    state: State<'_, AppState>,
    id: String,
    offset: Option<u64>,
) -> Result<(String, u64), String> {
    state.ensure_daemon().await;
    let off = offset.unwrap_or(0);

    if let Ok(mut client) = connect_vm_service(&state.grpc_addr).await {
        let resp = client
            .get_vm_console(proto::GetVmConsoleRequest {
                vm_id: id.clone(),
                offset: off,
            })
            .await
            .map_err(|e| e.to_string())?
            .into_inner();
        return Ok((resp.data, resp.new_offset));
    }

    state
        .hv
        .read_vm_console(&id, off)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn vm_mount_add(
    state: State<'_, AppState>,
    vm: String,
    tag: String,
    host_path: String,
    guest_path: String,
    readonly: bool,
) -> Result<(), String> {
    state.ensure_daemon().await;
    validation::validate_mount_path(&host_path)
        .map_err(|e| format!("Invalid host path '{}': {}", host_path, e))?;
    validation::validate_mount_path(&guest_path)
        .map_err(|e| format!("Invalid guest path '{}': {}", guest_path, e))?;

    if let Ok(mut client) = connect_vm_service(&state.grpc_addr).await {
        client
            .mount_virtio_fs(proto::MountVirtioFsRequest {
                vm_id: vm,
                share: Some(proto::SharedDirectory {
                    tag,
                    host_path,
                    guest_path,
                    read_only: readonly,
                }),
            })
            .await
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    use cratebay_core::hypervisor::SharedDirectory;
    let share = SharedDirectory {
        tag,
        host_path,
        guest_path,
        read_only: readonly,
    };
    state
        .hv
        .mount_virtiofs(&vm, &share)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn vm_mount_remove(
    state: State<'_, AppState>,
    vm: String,
    tag: String,
) -> Result<(), String> {
    state.ensure_daemon().await;
    if let Ok(mut client) = connect_vm_service(&state.grpc_addr).await {
        client
            .unmount_virtio_fs(proto::UnmountVirtioFsRequest { vm_id: vm, tag })
            .await
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    state
        .hv
        .unmount_virtiofs(&vm, &tag)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn vm_mount_list(
    state: State<'_, AppState>,
    vm: String,
) -> Result<Vec<SharedDirectoryDto>, String> {
    state.ensure_daemon().await;
    if let Ok(mut client) = connect_vm_service(&state.grpc_addr).await {
        let resp = client
            .list_virtio_fs_mounts(proto::ListVirtioFsMountsRequest { vm_id: vm })
            .await
            .map_err(|e| e.to_string())?
            .into_inner();
        return Ok(resp
            .mounts
            .into_iter()
            .map(SharedDirectoryDto::from)
            .collect());
    }

    let mounts = state
        .hv
        .list_virtiofs_mounts(&vm)
        .map_err(|e| e.to_string())?;
    Ok(mounts.into_iter().map(SharedDirectoryDto::from).collect())
}

#[tauri::command]
async fn vm_port_forward_add(
    state: State<'_, AppState>,
    vm_id: String,
    host_port: u16,
    guest_port: u16,
    protocol: String,
) -> Result<(), String> {
    state.ensure_daemon().await;
    let proto_str = if protocol.is_empty() {
        "tcp".to_string()
    } else {
        protocol.clone()
    };

    if let Ok(mut client) = connect_vm_service(&state.grpc_addr).await {
        client
            .add_port_forward(proto::AddPortForwardRequest {
                vm_id,
                host_port: host_port as u32,
                guest_port: guest_port as u32,
                protocol: proto_str,
            })
            .await
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    let pf = cratebay_core::hypervisor::PortForward {
        host_port,
        guest_port,
        protocol: proto_str,
    };
    state
        .hv
        .add_port_forward(&vm_id, &pf)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn vm_port_forward_remove(
    state: State<'_, AppState>,
    vm_id: String,
    host_port: u16,
) -> Result<(), String> {
    state.ensure_daemon().await;
    if let Ok(mut client) = connect_vm_service(&state.grpc_addr).await {
        client
            .remove_port_forward(proto::RemovePortForwardRequest {
                vm_id,
                host_port: host_port as u32,
            })
            .await
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    state
        .hv
        .remove_port_forward(&vm_id, host_port)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn vm_port_forward_list(
    state: State<'_, AppState>,
    vm_id: String,
) -> Result<Vec<PortForwardDto>, String> {
    state.ensure_daemon().await;
    if let Ok(mut client) = connect_vm_service(&state.grpc_addr).await {
        let resp = client
            .list_port_forwards(proto::ListPortForwardsRequest { vm_id })
            .await
            .map_err(|e| e.to_string())?
            .into_inner();
        return Ok(resp
            .forwards
            .into_iter()
            .map(|pf| PortForwardDto {
                host_port: pf.host_port as u16,
                guest_port: pf.guest_port as u16,
                protocol: pf.protocol,
            })
            .collect());
    }

    let forwards = state
        .hv
        .list_port_forwards(&vm_id)
        .map_err(|e| e.to_string())?;
    Ok(forwards
        .into_iter()
        .map(|pf| PortForwardDto {
            host_port: pf.host_port,
            guest_port: pf.guest_port,
            protocol: pf.protocol,
        })
        .collect())
}

#[tauri::command]
async fn volume_list() -> Result<Vec<VolumeInfo>, String> {
    let docker = connect_docker()?;
    let opts = ListVolumesOptions::<String> {
        ..Default::default()
    };
    let resp = docker
        .list_volumes(Some(opts))
        .await
        .map_err(|e| e.to_string())?;

    let volumes = resp.volumes.unwrap_or_default();
    let mut out: Vec<VolumeInfo> = volumes
        .into_iter()
        .map(|v| VolumeInfo {
            name: v.name,
            driver: v.driver,
            mountpoint: v.mountpoint,
            created_at: v.created_at.unwrap_or_default(),
            labels: v.labels,
            options: v.options,
            scope: v.scope.map(|s| format!("{:?}", s)).unwrap_or_default(),
        })
        .collect();
    // Docker doesn't guarantee ordering; keep it stable to avoid UI jitter on refresh.
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[derive(Debug, Serialize)]
pub struct VmStatsDto {
    cpu_percent: f64,
    memory_usage_mb: u64,
    disk_usage_gb: u64,
}

#[tauri::command]
async fn vm_stats(state: State<'_, AppState>, id: String) -> Result<VmStatsDto, String> {
    state.ensure_daemon().await;
    if let Ok(mut client) = connect_vm_service(&state.grpc_addr).await {
        let resp = client
            .get_vm_stats(proto::GetVmStatsRequest { vm_id: id })
            .await
            .map_err(|e| e.to_string())?
            .into_inner();

        return Ok(VmStatsDto {
            cpu_percent: resp.cpu_percent,
            memory_usage_mb: resp.memory_usage_mb,
            disk_usage_gb: resp.disk_usage_gb,
        });
    }

    // Fallback: stub stats for local hypervisor
    let vms = state.hv.list_vms().map_err(|e| e.to_string())?;
    let vm = vms
        .into_iter()
        .find(|v| v.id == id || v.name == id)
        .ok_or_else(|| format!("VM not found: {}", id))?;

    Ok(VmStatsDto {
        cpu_percent: 0.0,
        memory_usage_mb: 0,
        disk_usage_gb: vm.disk_gb,
    })
}

#[tauri::command]
async fn volume_create(name: String, driver: Option<String>) -> Result<VolumeInfo, String> {
    let docker = connect_docker()?;
    let opts = CreateVolumeOptions {
        name: name.as_str(),
        driver: driver.as_deref().unwrap_or("local"),
        ..Default::default()
    };
    let v = docker
        .create_volume(opts)
        .await
        .map_err(|e| e.to_string())?;
    Ok(VolumeInfo {
        name: v.name,
        driver: v.driver,
        mountpoint: v.mountpoint,
        created_at: v.created_at.unwrap_or_default(),
        labels: v.labels,
        options: v.options,
        scope: v.scope.map(|s| format!("{:?}", s)).unwrap_or_default(),
    })
}

#[tauri::command]
async fn volume_inspect(name: String) -> Result<VolumeInfo, String> {
    let docker = connect_docker()?;
    let v = docker
        .inspect_volume(&name)
        .await
        .map_err(|e| e.to_string())?;
    Ok(VolumeInfo {
        name: v.name,
        driver: v.driver,
        mountpoint: v.mountpoint,
        created_at: v.created_at.unwrap_or_default(),
        labels: v.labels,
        options: v.options,
        scope: v.scope.map(|s| format!("{:?}", s)).unwrap_or_default(),
    })
}

#[tauri::command]
async fn volume_remove(name: String) -> Result<(), String> {
    let docker = connect_docker()?;
    docker
        .remove_volume(&name, None)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn image_list() -> Result<Vec<LocalImageInfo>, String> {
    let docker = connect_docker()?;
    let opts = ListImagesOptions::<String> {
        all: false,
        ..Default::default()
    };
    let images = docker
        .list_images(Some(opts))
        .await
        .map_err(|e| e.to_string())?;

    Ok(images
        .into_iter()
        .map(|img| {
            let full_id = img.id.clone();
            let short_id = if let Some(stripped) = full_id.strip_prefix("sha256:") {
                stripped.chars().take(12).collect::<String>()
            } else {
                full_id.chars().take(12).collect::<String>()
            };
            let size = img.size.max(0) as u64;
            LocalImageInfo {
                id: short_id,
                repo_tags: img.repo_tags,
                size_bytes: size,
                size_human: format_bytes_human(size),
                created: img.created,
            }
        })
        .collect())
}

#[tauri::command]
async fn image_remove(id: String) -> Result<(), String> {
    let docker = connect_docker()?;
    let opts = RemoveImageOptions {
        force: false,
        noprune: false,
    };
    docker
        .remove_image(&id, Some(opts), None)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn image_tag(source: String, repo: String, tag: String) -> Result<(), String> {
    let docker = connect_docker()?;
    let opts = TagImageOptions {
        repo: &repo,
        tag: &tag,
    };
    docker
        .tag_image(&source, Some(opts))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn image_inspect(id: String) -> Result<ImageInspectInfo, String> {
    let docker = connect_docker()?;
    let detail = docker.inspect_image(&id).await.map_err(|e| e.to_string())?;

    let full_id = detail.id.clone().unwrap_or_default();
    let short_id = if let Some(stripped) = full_id.strip_prefix("sha256:") {
        stripped.chars().take(12).collect::<String>()
    } else {
        full_id.chars().take(12).collect::<String>()
    };
    let repo_tags = detail.repo_tags.clone().unwrap_or_default();
    let size = detail.size.unwrap_or(0).max(0) as u64;
    let created = detail.created.clone().unwrap_or_default();
    let architecture = detail.architecture.clone().unwrap_or_default();
    let os = detail.os.clone().unwrap_or_default();
    let docker_version = detail.docker_version.clone().unwrap_or_default();
    let layers = detail
        .root_fs
        .as_ref()
        .and_then(|r| r.layers.as_ref())
        .map(|l| l.len())
        .unwrap_or(0);

    Ok(ImageInspectInfo {
        id: short_id,
        repo_tags,
        size_bytes: size,
        created,
        architecture,
        os,
        docker_version,
        layers,
    })
}
