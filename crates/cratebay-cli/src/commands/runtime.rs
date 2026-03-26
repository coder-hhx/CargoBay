//! Runtime management commands.

use anyhow::Result;

use cratebay_core::runtime::{self, RuntimeState};

/// Show current runtime status.
pub async fn status() -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let state = runtime.get_state().await?;

    let state_str = match &state {
        RuntimeState::None => "not provisioned",
        RuntimeState::Provisioned => "provisioned (stopped)",
        RuntimeState::Starting => "starting",
        RuntimeState::Ready => "ready",
        RuntimeState::Stopping => "stopping",
        RuntimeState::Stopped => "stopped",
        RuntimeState::Error(msg) => {
            println!("Runtime: error — {}", msg);
            return Ok(());
        }
    };

    println!("Runtime: {}", state_str);

    // If ready, also show Docker info
    if state == RuntimeState::Ready {
        let health = runtime.health_check().await?;
        if health.docker_responsive {
            println!("Docker: responsive");
            if let Some(ver) = &health.docker_version {
                println!("Docker version: {}", ver);
            }
        } else {
            println!("Docker: not responsive");
        }
        if let Some(uptime) = health.uptime_seconds {
            let mins = uptime / 60;
            let secs = uptime % 60;
            println!("Uptime: {}m {}s", mins, secs);
        }
    }

    println!(
        "Socket: {}",
        runtime.docker_socket_path().display()
    );

    Ok(())
}

/// Start the built-in runtime (provision if needed).
pub async fn start() -> Result<()> {
    let runtime = runtime::create_runtime_manager();

    let state = runtime.get_state().await?;
    if state == RuntimeState::Ready {
        println!("Runtime is already running.");
        return Ok(());
    }

    println!("Starting CrateBay runtime...");

    // Provision if needed
    if state == RuntimeState::None {
        println!("Provisioning runtime image...");
        runtime
            .provision(Box::new(|progress| {
                if progress.percent > 0.0 {
                    eprint!("\r  {} — {:.0}%", progress.message, progress.percent);
                } else {
                    eprint!("\r  {}", progress.message);
                }
            }))
            .await?;
        eprintln!(); // newline after progress
        println!("Provisioning complete.");
    }

    // Start
    runtime.start().await?;
    println!("Runtime started.");

    // Wait for Docker
    print!("Waiting for Docker...");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
    loop {
        if std::time::Instant::now() >= deadline {
            println!(" timed out.");
            println!("Runtime is running but Docker is not yet responsive.");
            return Ok(());
        }
        let health = runtime.health_check().await?;
        if health.docker_responsive {
            println!(" ready.");
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        print!(".");
    }
}

/// Stop the built-in runtime.
pub async fn stop() -> Result<()> {
    let runtime = runtime::create_runtime_manager();

    let state = runtime.get_state().await?;
    match state {
        RuntimeState::None | RuntimeState::Provisioned | RuntimeState::Stopped => {
            println!("Runtime is not running.");
            return Ok(());
        }
        _ => {}
    }

    println!("Stopping CrateBay runtime...");
    runtime.stop().await?;
    println!("Runtime stopped.");
    Ok(())
}

/// Pre-download runtime image without starting.
pub async fn provision() -> Result<()> {
    let runtime = runtime::create_runtime_manager();

    let state = runtime.get_state().await?;
    if state != RuntimeState::None {
        println!("Runtime is already provisioned.");
        return Ok(());
    }

    println!("Downloading runtime image...");
    runtime
        .provision(Box::new(|progress| {
            if progress.percent > 0.0 {
                eprint!("\r  {} — {:.0}%", progress.message, progress.percent);
            } else {
                eprint!("\r  {}", progress.message);
            }
        }))
        .await?;
    eprintln!(); // newline after progress
    println!("Provisioning complete.");
    Ok(())
}
