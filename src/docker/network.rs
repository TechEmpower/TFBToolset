use crate::config::Test;
use crate::docker::container::stop_containers_because_of_error;
use crate::docker::docker_config::DockerConfig;
use crate::docker::listener::build_network::BuildNetwork;
use crate::docker::listener::simple::Simple;
use crate::error::ToolsetError::{
    DockerAttachContainerToNetworkError, DockerNetworkCreateError, DockerNetworkDeleteError,
    FailedToAttachDockerContainerToNetworkError, FailedToCreateDockerNetworkError,
    FailedToDeleteNetworkError,
};
use crate::error::ToolsetResult;
use crate::options;
use curl::easy::{Easy2, List};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum NetworkMode {
    Bridge,
    Host,
}
impl FromStr for NetworkMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == options::network_modes::HOST {
            return Ok(NetworkMode::Host);
        }
        Ok(NetworkMode::Bridge)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NetworkCreationOptions {
    pub name: String,
    pub driver: String,
    pub internal: bool,
    pub check_duplicate: bool,
}
impl NetworkCreationOptions {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NetworkConnectOptions {
    pub container: String,
    pub endpoint_config: EndpointConfig,
}
impl NetworkConnectOptions {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct EndpointConfig {
    pub i_p_a_m_config: IPAMConfig,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IPAMConfig {
    pub aliases: Vec<String>,
}

/// Creates the container for the given `Test`.
pub fn create_network(config: &DockerConfig, test: &Test) -> ToolsetResult<String> {
    let mut easy = Easy2::new(BuildNetwork::new());
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    let mut headers = List::new();
    headers.append("Content-Type: application/json")?;

    let name = "TFBNetwork";
    let options = NetworkCreationOptions {
        name: name.to_string(),
        driver: "bridge".to_string(),
        internal: false,
        check_duplicate: true,
    };
    let json = options.to_json();
    let len = json.len();

    easy.post(true)?;
    easy.url(&format!("http://{}/networks/create", config.docker_host))?;
    easy.http_headers(headers)?;
    easy.in_filesize(len as u64)?;
    easy.post_field_size(len as u64)?;
    easy.post_fields_copy(json.as_bytes())?;
    easy.perform()?;

    match easy.response_code() {
        Ok(201) => {
            let network_id = &easy.get_mut().network_id;
            if network_id.is_some() {
                return Ok(network_id.clone().unwrap());
            } else {
                let error_message = &easy.get_ref().error_message;
                if error_message.is_some() {
                    return Err(FailedToCreateDockerNetworkError(
                        error_message.clone().unwrap(),
                    ));
                }
            }
            Err(DockerNetworkCreateError)
        }
        Ok(409) => {
            // Network already exists; delete and try again.
            delete_network(config, name)?;
            create_network(config, test)
        }
        Ok(_) => Err(DockerNetworkCreateError),
        Err(e) => Err(FailedToCreateDockerNetworkError(e.to_string())),
    }
}

/// Attaches the container given by `container_id` to the network given by
/// `network_id`.
pub fn connect_container_to_network(
    docker_config: &DockerConfig,
    network_id: &str,
    container_ids: &(String, Option<String>),
) -> ToolsetResult<()> {
    connect_container_to_network_unsafe(docker_config, &container_ids.0, network_id)
        .map_err(|e| stop_containers_because_of_error(docker_config, container_ids, e))
}

//
// PRIVATES
//

/// Attaches the container given by `container_id` to the network given by
/// `network_id`.
fn connect_container_to_network_unsafe(
    config: &DockerConfig,
    container_id: &str,
    network_id: &str,
) -> ToolsetResult<()> {
    let mut easy = Easy2::new(Simple::new());
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    let mut headers = List::new();
    headers.append("Content-Type: application/json")?;

    let options = NetworkConnectOptions {
        container: container_id.to_string(),
        endpoint_config: EndpointConfig {
            i_p_a_m_config: IPAMConfig {
                aliases: vec![config.server_host.clone()],
            },
        },
    };
    let json = options.to_json();
    let len = json.len();

    easy.post(true)?;
    easy.url(&format!(
        "http://{}/networks/{}/connect",
        config.docker_host, network_id
    ))?;
    easy.http_headers(headers)?;
    easy.in_filesize(len as u64)?;
    easy.post_field_size(len as u64)?;
    easy.post_fields_copy(json.as_bytes())?;
    easy.perform()?;

    match easy.response_code() {
        Ok(200) => Ok(()),
        Ok(_) => Err(DockerAttachContainerToNetworkError),
        Err(e) => Err(FailedToAttachDockerContainerToNetworkError(e.to_string())),
    }
}

/// Deletes the Docker network given by `network_name`.
fn delete_network(config: &DockerConfig, network_name: &str) -> ToolsetResult<()> {
    let mut easy = Easy2::new(Simple::new());
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    easy.custom_request("DELETE")?;
    easy.url(&format!(
        "http://{}/networks/{}",
        config.docker_host, network_name
    ))?;
    easy.perform()?;

    match easy.response_code() {
        Ok(204) => Ok(()),
        Ok(_) => Err(DockerNetworkDeleteError),
        Err(e) => Err(FailedToDeleteNetworkError(e.to_string())),
    }
}
