async fn handle_volume(cmd: VolumeCommands) -> Result<(), String> {
    let docker = connect_docker()?;
    match cmd {
        VolumeCommands::List => {
            let opts = ListVolumesOptions::<String> {
                ..Default::default()
            };
            let resp = docker
                .list_volumes(Some(opts))
                .await
                .map_err(|e| e.to_string())?;
            let volumes = resp.volumes.unwrap_or_default();
            println!("{:<32} {:<12} MOUNTPOINT", "VOLUME NAME", "DRIVER");
            for v in volumes {
                println!("{:<32} {:<12} {}", v.name, v.driver, v.mountpoint);
            }
        }
        VolumeCommands::Create { name, driver } => {
            let opts = CreateVolumeOptions {
                name: name.as_str(),
                driver: driver.as_str(),
                ..Default::default()
            };
            let v = docker
                .create_volume(opts)
                .await
                .map_err(|e| e.to_string())?;
            println!("Created volume '{}'", v.name);
        }
        VolumeCommands::Inspect { name } => {
            let v = docker
                .inspect_volume(&name)
                .await
                .map_err(|e| e.to_string())?;
            let json = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
            println!("{}", json);
        }
        VolumeCommands::Remove { name } => {
            docker
                .remove_volume(&name, None)
                .await
                .map_err(|e| e.to_string())?;
            println!("Removed volume '{}'", name);
        }
    }
    Ok(())
}
