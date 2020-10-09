use crate::docker::docker_config::DockerConfig;
use crate::docker::listener::build_network::BuildNetwork;
use crate::docker::listener::simple::Simple;
use crate::error::ToolsetError::DockerError;
use crate::error::ToolsetResult;
use dockurl::network::NetworkMode;

/// Gets the network id for the given `docker_host` and `network_name`.
pub fn get_network_id(
    use_unix_socket: bool,
    docker_host: &str,
    network_name: &str,
) -> ToolsetResult<String> {
    match dockurl::network::inspect_network(
        network_name,
        docker_host,
        use_unix_socket,
        Simple::new(),
    ) {
        Ok(network) => Ok(network.id),
        Err(error) => Err(DockerError(error)),
    }
}

/// Gets the network id for the "TFBNetwork" on the given `docker_host`.
/// Will create the network if it does not already exist.
pub fn get_tfb_network_id(use_unix_socket: bool, docker_host: &str) -> ToolsetResult<String> {
    if let Ok(network) =
        dockurl::network::inspect_network("TFBNetwork", docker_host, use_unix_socket, Simple::new())
    {
        Ok(network.id)
    } else {
        match dockurl::network::create_network(
            "TFBNetwork",
            NetworkMode::Bridge,
            docker_host,
            use_unix_socket,
            BuildNetwork::new(),
        ) {
            Ok(network_id) => Ok(network_id),
            Err(error) => Err(DockerError(error)),
        }
    }
}

/// Attaches the container given by `container_id` to the network given by
/// `network_id` on the given `docker_host`.
pub fn connect_container_to_network(
    docker_config: &DockerConfig,
    docker_host: &str,
    network_id: &str,
    container_id: &str,
) -> ToolsetResult<()> {
    match dockurl::network::connect_container_to_network(
        container_id,
        network_id,
        vec![],
        docker_host,
        docker_config.use_unix_socket,
        Simple::new(),
    ) {
        Ok(()) => Ok(()),
        Err(error) => Err(DockerError(error)),
    }
}
