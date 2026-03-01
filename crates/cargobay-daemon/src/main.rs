use std::sync::Arc;
use tracing::info;

use cargobay_core::hypervisor::Hypervisor;
use cargobay_core::proto::vm_service_server::VmServiceServer;

use cargobay_daemon::service::VmServiceImpl;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    cargobay_core::logging::init();

    let addr = std::env::var("CARGOBAY_GRPC_ADDR").unwrap_or_else(|_| "127.0.0.1:50051".into());
    let addr = addr.parse()?;

    let hv: Arc<dyn Hypervisor> = Arc::from(cargobay_core::create_hypervisor());
    let service = VmServiceImpl::new(hv);

    info!("CargoBay daemon v0.1.0");
    info!("Config dir: {}", cargobay_core::config_dir().display());
    info!("Log dir: {}", cargobay_core::log_dir().display());
    info!("gRPC listening on {}", addr);

    tonic::transport::Server::builder()
        .add_service(VmServiceServer::new(service))
        .serve_with_shutdown(addr, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;

    Ok(())
}
