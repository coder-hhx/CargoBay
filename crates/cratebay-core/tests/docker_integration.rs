//! Docker integration tests.
//!
//! These tests require a running Docker daemon and are marked `#[ignore]`
//! by default. Run with `cargo test --test docker_integration -- --ignored`
//! when Docker is available.

use cratebay_core::docker;

#[tokio::test]
#[ignore = "Requires running Docker daemon"]
async fn docker_connect_succeeds() {
    let result = docker::connect().await;
    assert!(
        result.is_ok(),
        "docker::connect() should succeed when Docker is running: {:?}",
        result.err()
    );
}

#[tokio::test]
#[ignore = "Requires running Docker daemon"]
async fn docker_try_connect_returns_some() {
    let docker = docker::try_connect().await;
    assert!(
        docker.is_some(),
        "docker::try_connect() should return Some when Docker is running"
    );
}

#[tokio::test]
#[ignore = "Requires running Docker daemon"]
async fn docker_is_available_after_connect() {
    let docker = docker::connect().await.expect("Docker must be running");
    assert!(
        docker::is_available(&docker).await,
        "is_available() should be true after successful connect"
    );
}

#[tokio::test]
#[ignore = "Requires running Docker daemon"]
async fn docker_version_returns_info() {
    let docker = docker::connect().await.expect("Docker must be running");
    let version = docker::version(&docker).await;
    assert!(
        version.is_ok(),
        "version() should succeed: {:?}",
        version.err()
    );
    let v = version.unwrap();
    assert!(
        v.version.is_some(),
        "Docker version string should be present"
    );
}

#[tokio::test]
#[ignore = "Requires running Docker daemon"]
async fn container_list_returns_vec() {
    use cratebay_core::container;

    let docker = docker::connect().await.expect("Docker must be running");
    let containers = container::list(&docker, true, None).await;
    assert!(
        containers.is_ok(),
        "container::list() should succeed: {:?}",
        containers.err()
    );
    // We can't assert the count, but we can verify the return type
    let _list = containers.unwrap();
}
