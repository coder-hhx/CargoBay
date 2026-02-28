use tracing_subscriber;

fn main() {
    tracing_subscriber::fmt::init();
    println!("CargoBay daemon v0.1.0");
    println!("gRPC server not yet implemented");
}
