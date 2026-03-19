async fn handle_k3s(cmd: K3sCommands) -> Result<(), String> {
    match cmd {
        K3sCommands::Status => {
            let status =
                cratebay_core::k3s::K3sManager::cluster_status().map_err(|e| e.to_string())?;
            println!("K3s Status");
            println!(
                "  Installed: {}",
                if status.installed { "yes" } else { "no" }
            );
            println!("  Running:   {}", if status.running { "yes" } else { "no" });
            if !status.version.is_empty() {
                println!("  Version:   {}", status.version);
            }
            if status.running {
                println!("  Nodes:     {}", status.node_count);
            }
            println!(
                "  Kubeconfig: {}",
                cratebay_core::k3s::K3sManager::kubeconfig_path().display()
            );
        }
        K3sCommands::Install => {
            println!("Downloading K3s...");
            cratebay_core::k3s::K3sManager::install(None)
                .await
                .map_err(|e| e.to_string())?;
            println!("K3s installed successfully.");
        }
        K3sCommands::Start => {
            let config = cratebay_core::k3s::K3sConfig::default();
            cratebay_core::k3s::K3sManager::start_cluster(&config).map_err(|e| e.to_string())?;
            println!("K3s cluster started.");
        }
        K3sCommands::Stop => {
            cratebay_core::k3s::K3sManager::stop_cluster().map_err(|e| e.to_string())?;
            println!("K3s cluster stopped.");
        }
        K3sCommands::Uninstall => {
            cratebay_core::k3s::K3sManager::uninstall().map_err(|e| e.to_string())?;
            println!("K3s uninstalled.");
        }
    }
    Ok(())
}
