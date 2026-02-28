#[cfg_attr(mobile, tauri::mobile_entry_point)]

use bollard::Docker;
use bollard::container::{ListContainersOptions, StopContainerOptions, RemoveContainerOptions, StartContainerOptions};
use serde::Serialize;
use std::path::PathBuf;

fn detect_docker() -> Result<Docker, bollard::errors::Error> {
    // Check DOCKER_HOST env first
    if std::env::var("DOCKER_HOST").is_ok() {
        return Docker::connect_with_local_defaults();
    }

    // Try common socket paths
    let home = std::env::var("HOME").unwrap_or_default();
    let candidates = [
        format!("{}/.colima/default/docker.sock", home),
        format!("{}/.orbstack/run/docker.sock", home),
        "/var/run/docker.sock".to_string(),
        format!("{}/.docker/run/docker.sock", home),
    ];

    for path in &candidates {
        if PathBuf::from(path).exists() {
            return Docker::connect_with_unix(path, 120, bollard::API_DEFAULT_VERSION);
        }
    }

    Docker::connect_with_local_defaults()
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

#[tauri::command]
async fn list_containers() -> Result<Vec<ContainerInfo>, String> {
    let docker = detect_docker().map_err(|e| e.to_string())?;

    let opts = ListContainersOptions::<String> {
        all: true,
        ..Default::default()
    };

    let containers = docker.list_containers(Some(opts)).await.map_err(|e| e.to_string())?;

    Ok(containers.into_iter().map(|c| {
        let ports = c.ports.unwrap_or_default().iter().filter_map(|p| {
            p.public_port.map(|pub_p| format!("{}:{}", pub_p, p.private_port))
        }).collect::<Vec<_>>().join(", ");

        ContainerInfo {
            id: c.id.unwrap_or_default()[..12].to_string(),
            name: c.names.unwrap_or_default().first()
                .unwrap_or(&String::new()).trim_start_matches('/').to_string(),
            image: c.image.unwrap_or_default(),
            state: c.state.unwrap_or_default(),
            status: c.status.unwrap_or_default(),
            ports,
        }
    }).collect())
}

#[tauri::command]
async fn stop_container(id: String) -> Result<(), String> {
    let docker = detect_docker().map_err(|e| e.to_string())?;
    docker.stop_container(&id, Some(StopContainerOptions { t: 10 })).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_container(id: String) -> Result<(), String> {
    let docker = detect_docker().map_err(|e| e.to_string())?;
    docker.start_container(&id, None::<StartContainerOptions<String>>).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_container(id: String) -> Result<(), String> {
    let docker = detect_docker().map_err(|e| e.to_string())?;
    let _ = docker.stop_container(&id, Some(StopContainerOptions { t: 10 })).await;
    docker.remove_container(&id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await.map_err(|e| e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_containers, stop_container, start_container, remove_container])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
