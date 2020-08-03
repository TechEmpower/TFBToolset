use crate::docker::container::stop_containers_because_of_error;
use crate::docker::docker_config::DockerConfig;
use crate::docker::listener::build_network::BuildNetwork;
use crate::docker::listener::simple::Simple;
use crate::error::{ToolsetError, ToolsetResult};
use dockurl::network::NetworkMode;

/// Creates the "TFBNetwork" network.
pub fn create_network(config: &DockerConfig) -> ToolsetResult<String> {
    if let Ok(network) = dockurl::network::inspect_network(
        "TFBNetwork",
        &config.server_docker_host,
        config.use_unix_socket,
        Simple::new(),
    ) {
        Ok(network.id)
    } else {
        match dockurl::network::create_network(
            "TFBNetwork",
            NetworkMode::Bridge,
            &config.server_docker_host,
            config.use_unix_socket,
            BuildNetwork::new(),
        ) {
            Ok(network_id) => Ok(network_id),
            Err(error) => Err(ToolsetError::DockerError(error)),
        }
    }
}

/// Attaches the container given by `container_id` to the network given by
/// `network_id`.
pub fn connect_container_to_network(
    docker_config: &DockerConfig,
    docker_host: &str,
    network_id: &str,
    container_ids: &(String, Option<String>),
) -> ToolsetResult<()> {
    connect_container_to_network_unsafe(docker_config, docker_host, &container_ids.0, network_id)
        .map_err(|e| stop_containers_because_of_error(docker_config, container_ids, e))
}

//
// PRIVATES
//

/// Attaches the container given by `container_id` to the network given by
/// `network_id`.
fn connect_container_to_network_unsafe(
    config: &DockerConfig,
    docker_host: &str,
    container_id: &str,
    network_id: &str,
) -> ToolsetResult<()> {
    dockurl::network::connect_container_to_network(
        container_id,
        network_id,
        vec![],
        docker_host,
        config.use_unix_socket,
        Simple::new(),
    )?;

    Ok(())
}
