use anyhow::Result;
use std::time::Duration;

const DOCKER_STATUS_RETRIES: usize = 3;
const DOCKER_STATUS_RETRY_DELAY_MS: u64 = 250;

/// Show system information.
pub fn info() -> Result<()> {
    println!("CrateBay v{}", env!("CARGO_PKG_VERSION"));
    println!("Platform: {}", std::env::consts::OS);
    println!("Arch: {}", std::env::consts::ARCH);
    Ok(())
}

/// Show Docker connection status without starting the built-in runtime.
pub async fn docker_status() -> Result<()> {
    let mut docker = None;
    for attempt in 0..DOCKER_STATUS_RETRIES {
        docker = cratebay_core::docker::try_connect().await;
        if docker.is_some() {
            break;
        }
        if attempt + 1 < DOCKER_STATUS_RETRIES {
            tokio::time::sleep(Duration::from_millis(DOCKER_STATUS_RETRY_DELAY_MS)).await;
        }
    }

    let Some(docker) = docker else {
        println!("Docker: not connected");
        return Ok(());
    };

    let version = cratebay_core::docker::version(&docker).await.ok();
    println!("Docker: connected");
    if let Some(v) = version {
        if let Some(ver) = v.version {
            println!("Version: {}", ver);
        }
        if let Some(api) = v.api_version {
            println!("API: {}", api);
        }
        if let Some(os) = v.os {
            println!("OS: {}", os);
        }
        if let Some(arch) = v.arch {
            println!("Arch: {}", arch);
        }
    }

    Ok(())
}
