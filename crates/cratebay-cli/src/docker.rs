async fn handle_docker(cmd: DockerCommands) -> Result<(), String> {
    let docker = connect_docker()?;
    match cmd {
        DockerCommands::Ps => {
            let mut filters = HashMap::new();
            filters.insert(
                "status",
                vec![
                    "running",
                    "exited",
                    "paused",
                    "created",
                    "restarting",
                    "dead",
                ],
            );
            let opts = ListContainersOptions {
                all: true,
                filters,
                ..Default::default()
            };
            let containers = docker
                .list_containers(Some(opts))
                .await
                .map_err(|e| e.to_string())?;

            println!(
                "{:<16} {:<24} {:<24} {:<16} PORTS",
                "CONTAINER ID", "NAME", "IMAGE", "STATUS"
            );
            for c in containers {
                let id =
                    c.id.as_deref()
                        .unwrap_or("")
                        .chars()
                        .take(12)
                        .collect::<String>();
                let name = c
                    .names
                    .as_ref()
                    .and_then(|n| n.first())
                    .map(|n| n.trim_start_matches('/'))
                    .unwrap_or("")
                    .to_string();
                let image = c.image.as_deref().unwrap_or("");
                let status = c.status.as_deref().unwrap_or("");
                let ports = c
                    .ports
                    .as_ref()
                    .map(|ps| {
                        ps.iter()
                            .map(|p| {
                                let private = p.private_port;
                                let public = p.public_port;
                                let typ = p.typ.map(|t| t.to_string()).unwrap_or_default();
                                match public {
                                    Some(pub_port) => format!(
                                        "{}:{}->{}/{}",
                                        p.ip.as_deref().unwrap_or("0.0.0.0"),
                                        pub_port,
                                        private,
                                        typ
                                    ),
                                    None => format!("{}/{}", private, typ),
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                println!(
                    "{:<16} {:<24} {:<24} {:<16} {}",
                    id, name, image, status, ports
                );
            }
        }
        DockerCommands::Start { id } => {
            docker
                .start_container(&id, None::<StartContainerOptions<String>>)
                .await
                .map_err(|e| e.to_string())?;
            println!("Started container {}", id);
        }
        DockerCommands::Stop { id } => {
            docker
                .stop_container(&id, Some(StopContainerOptions { t: 10 }))
                .await
                .map_err(|e| e.to_string())?;
            println!("Stopped container {}", id);
        }
        DockerCommands::Rm { id } => {
            docker
                .remove_container(
                    &id,
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                )
                .await
                .map_err(|e| e.to_string())?;
            println!("Removed container {}", id);
        }
        DockerCommands::Run {
            image,
            name,
            cpus,
            memory,
            pull,
            env,
        } => {
            if let Err(e) = validation::validate_image_reference(&image) {
                return Err(format!("Invalid image reference '{}': {}", image, e));
            }
            if let Some(ref n) = name {
                if let Err(e) = validation::validate_container_name(n) {
                    return Err(format!("Invalid container name '{}': {}", n, e));
                }
            }
            if pull {
                docker_pull_image(&docker, &image).await?;
            }

            let mut host_config = HostConfig::default();
            if let Some(c) = cpus {
                host_config.nano_cpus = Some((c as i64) * 1_000_000_000);
            }
            if let Some(mb) = memory {
                let bytes = (mb as i64).saturating_mul(1024).saturating_mul(1024);
                host_config.memory = Some(bytes);
            }

            let config = Config::<String> {
                image: Some(image.clone()),
                host_config: Some(host_config),
                env: if env.is_empty() { None } else { Some(env) },
                ..Default::default()
            };

            let create_opts = name.as_deref().map(|n| CreateContainerOptions::<String> {
                name: n.to_string(),
                platform: None,
            });

            let result = docker
                .create_container(create_opts, config)
                .await
                .map_err(|e| e.to_string())?;

            docker
                .start_container(&result.id, None::<StartContainerOptions<String>>)
                .await
                .map_err(|e| e.to_string())?;

            let display = name
                .clone()
                .unwrap_or_else(|| result.id.chars().take(12).collect());
            println!("Created and started container: {}", display);
            println!("Login command:");
            println!("  docker exec -it {} /bin/sh", display);
        }
        DockerCommands::LoginCmd { container, shell } => {
            println!("docker exec -it {} {}", container, shell);
        }
        DockerCommands::Logs {
            container,
            tail,
            timestamps,
        } => {
            let opts = LogsOptions::<String> {
                follow: false,
                stdout: true,
                stderr: true,
                timestamps,
                tail: tail.clone(),
                ..Default::default()
            };

            let mut stream = docker.logs(&container, Some(opts));
            while let Some(chunk) = stream.try_next().await.map_err(format_bollard_error)? {
                print!("{}", chunk);
            }
        }
        DockerCommands::Env { id } => {
            let inspect = docker
                .inspect_container(&id, None::<InspectContainerOptions>)
                .await
                .map_err(|e| format!("Failed to inspect container {}: {}", id, e))?;

            let env_list = inspect.config.and_then(|c| c.env).unwrap_or_default();

            if env_list.is_empty() {
                println!("No environment variables set.");
            } else {
                println!("{:<32} VALUE", "KEY");
                for entry in env_list {
                    if let Some((k, v)) = entry.split_once('=') {
                        println!("{:<32} {}", k, v);
                    } else {
                        println!("{:<32}", entry);
                    }
                }
            }
        }
    }
    Ok(())
}

async fn docker_pull_image(docker: &Docker, reference: &str) -> Result<(), String> {
    let (from_image, tag) = split_image_reference(reference);
    let platform_override = std::env::var("CRATEBAY_DOCKER_PLATFORM")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string());

    #[cfg(windows)]
    if let Some(platform) = platform_override.as_deref() {
        return cratebay_core::runtime::pull_runtime_wsl_image(reference, platform).map_err(
            |error| {
                format!(
                    "Failed to pull image {} inside CrateBay Runtime (WSL2): {}",
                    reference, error
                )
            },
        );
    }

    let platform = if let Some(platform) = platform_override {
        platform
    } else {
        docker
            .version()
            .await
            .ok()
            .and_then(|version| docker_pull_platform_for_engine(&version))
            .unwrap_or_default()
    };
    let opts = CreateImageOptions {
        from_image,
        tag,
        platform,
        ..Default::default()
    };

    let mut stream = docker.create_image(Some(opts), None, None);
    while let Some(_progress) = stream.try_next().await.map_err(format_bollard_error)? {}
    Ok(())
}

fn docker_pull_platform_for_engine(version: &bollard::system::Version) -> Option<String> {
    let os = normalize_docker_platform_os(version.os.as_deref()?)?;
    let arch = normalize_docker_platform_arch(version.arch.as_deref()?)?;
    Some(format!("{os}/{arch}"))
}

fn normalize_docker_platform_os(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => None,
        "linux" => Some("linux"),
        "windows" => Some("windows"),
        "darwin" | "macos" | "macosx" => Some("darwin"),
        _ => None,
    }
}

fn normalize_docker_platform_arch(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => None,
        "amd64" | "x86_64" => Some("amd64"),
        "arm64" | "aarch64" => Some("arm64"),
        "arm" => Some("arm"),
        "386" | "i386" | "i686" => Some("386"),
        _ => None,
    }
}

fn format_bollard_error(error: bollard::errors::Error) -> String {
    match error {
        bollard::errors::Error::DockerStreamError { error } => {
            format!("Docker stream error: {error}")
        }
        bollard::errors::Error::DockerResponseServerError {
            status_code,
            message,
        } => format!("Docker responded with status code {status_code}: {message}"),
        other => other.to_string(),
    }
}

fn split_image_reference(reference: &str) -> (String, String) {
    let no_digest = reference.split('@').next().unwrap_or(reference);
    let last_slash = no_digest.rfind('/').unwrap_or(0);
    let last_colon = no_digest.rfind(':');

    if let Some(colon_idx) = last_colon {
        if colon_idx > last_slash {
            let image = &no_digest[..colon_idx];
            let tag = &no_digest[(colon_idx + 1)..];
            if !image.is_empty() && !tag.is_empty() {
                return (image.to_string(), tag.to_string());
            }
        }
    }

    (no_digest.to_string(), "latest".to_string())
}
