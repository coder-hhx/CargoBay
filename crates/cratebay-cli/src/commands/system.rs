use anyhow::Result;

/// Show system information.
pub fn info() -> Result<()> {
    println!("CrateBay v{}", env!("CARGO_PKG_VERSION"));
    println!("Platform: {}", std::env::consts::OS);
    println!("Arch: {}", std::env::consts::ARCH);
    Ok(())
}

/// Show Docker connection status without starting the built-in runtime.
pub async fn docker_status() -> Result<()> {
    let Some(docker) = cratebay_core::docker::try_connect().await else {
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
