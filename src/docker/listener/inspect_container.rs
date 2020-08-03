use crate::docker::docker_config::DockerConfig;
use crate::docker::listener::accumulate;
use crate::error::ToolsetError::FailedToInspectDockerContainerError;
use crate::error::ToolsetResult;
use curl::easy::{Handler, WriteError};
use dockurl::network::NetworkMode;
use serde_json::Value;

pub struct InspectContainer {
    pub string_buffer: String,
    network_mode: NetworkMode,
}
impl InspectContainer {
    pub fn new(docker_config: &DockerConfig) -> Self {
        Self {
            string_buffer: String::new(),
            network_mode: docker_config.network_mode.clone(),
        }
    }
    pub fn get_host_ports(&self) -> ToolsetResult<(String, String)> {
        if let Ok(json) = serde_json::from_str::<Value>(&self.string_buffer) {
            if let Some(exposed_ports) = json["Config"]["ExposedPorts"].as_object() {
                let mut exposed_port_protocol = String::new();
                for key in exposed_ports.keys() {
                    if exposed_port_protocol.is_empty() {
                        exposed_port_protocol = key.clone();
                    }
                }
                if let Some(exposed_port) = exposed_port_protocol.split('/').next() {
                    if let Some(host_port) = json["NetworkSettings"]["Ports"]
                        [&exposed_port_protocol][0]["HostPort"]
                        .as_str()
                    {
                        return Ok((host_port.to_string(), exposed_port.to_string()));
                    } else if let NetworkMode::Host = self.network_mode {
                        return Ok((exposed_port.to_string(), exposed_port.to_string()));
                    }
                }
            }
        }

        Err(FailedToInspectDockerContainerError(
            "Could not determine host port for running container.".to_string(),
        ))
    }
}
impl Handler for InspectContainer {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        accumulate(&mut self.string_buffer, data)
    }
}
