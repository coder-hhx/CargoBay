use anyhow::Result;

/// Show system information.
pub fn info() -> Result<()> {
    println!("CrateBay v{}", env!("CARGO_PKG_VERSION"));
    println!("Platform: {}", std::env::consts::OS);
    println!("Arch: {}", std::env::consts::ARCH);
    Ok(())
}
