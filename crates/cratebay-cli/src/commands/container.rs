use anyhow::Result;

/// List all containers.
pub async fn list() -> Result<()> {
    println!("Containers:");
    println!("  (none yet — runtime not initialized)");
    Ok(())
}
