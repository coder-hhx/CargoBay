fn build_tray_menu(
    app: &tauri::AppHandle,
    running_containers: usize,
    running_vms: usize,
) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let title_item = MenuItemBuilder::with_id("title", "CrateBay")
        .enabled(false)
        .build(app)?;

    let sep1 = PredefinedMenuItem::separator(app)?;

    let dashboard_item = MenuItemBuilder::with_id("dashboard", "Dashboard").build(app)?;

    let containers_label = format!("Containers ({} running)", running_containers);
    let containers_item = MenuItemBuilder::with_id("containers", containers_label)
        .enabled(false)
        .build(app)?;

    let vms_label = format!("VMs ({} running)", running_vms);
    let vms_item = MenuItemBuilder::with_id("vms", vms_label)
        .enabled(false)
        .build(app)?;

    let sep2 = PredefinedMenuItem::separator(app)?;

    let quit_item = MenuItemBuilder::with_id("quit", "Quit CrateBay").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&title_item)
        .item(&sep1)
        .item(&dashboard_item)
        .item(&containers_item)
        .item(&vms_item)
        .item(&sep2)
        .item(&quit_item)
        .build()?;

    Ok(menu)
}

/// Count containers with state == "running" via the Docker API.
async fn count_running_containers() -> usize {
    let Ok(docker) = connect_docker() else {
        return 0;
    };
    let opts = ListContainersOptions::<String> {
        all: true,
        ..Default::default()
    };
    let Ok(containers) = docker.list_containers(Some(opts)).await else {
        return 0;
    };
    containers
        .iter()
        .filter(|c| c.state.as_deref().map(|s| s == "running").unwrap_or(false))
        .count()
}

/// Count VMs with state == "running" via the gRPC daemon (or local hypervisor).
async fn count_running_vms(app_state: &AppState) -> usize {
    if let Ok(mut client) = connect_vm_service(&app_state.grpc_addr).await {
        if let Ok(resp) = client.list_v_ms(proto::ListVMsRequest {}).await {
            return resp
                .into_inner()
                .vms
                .iter()
                .filter(|vm| vm.status == "running")
                .count();
        }
    }
    // Fallback to local hypervisor
    if let Ok(vms) = app_state.hv.list_vms() {
        return vms
            .iter()
            .filter(|vm| matches!(vm.state, cratebay_core::hypervisor::VmState::Running))
            .count();
    }
    0
}

/// Refresh the tray menu with up-to-date container/VM counts.
fn refresh_tray_menu(app: &tauri::AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let running_containers = count_running_containers().await;
        let running_vms = {
            let state = app_handle.state::<AppState>();
            count_running_vms(&state).await
        };

        if let Some(tray) = app_handle.tray_by_id("main-tray") {
            match build_tray_menu(&app_handle, running_containers, running_vms) {
                Ok(menu) => {
                    if let Err(e) = tray.set_menu(Some(menu)) {
                        error!("Failed to update tray menu: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to build tray menu: {}", e);
                }
            }
        }
    });
}

// ── Local model runtime (Ollama) ─────────────────────────────────────

const OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";
