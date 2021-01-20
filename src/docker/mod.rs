//! The Docker module supports interfacing with the Docker daemon.
//! This includes actions like building `Test` images, building containers for
//! those images, and running containers in Docker.

use crate::docker::listener::verifier::Error;
use crate::docker::listener::verifier::Warning;
use serde::Deserialize;
use std::task::Poll;

pub mod container;
pub mod docker_config;
pub mod image;
pub mod listener;
pub mod network;

#[derive(Debug)]
pub struct DockerOrchestration {
    pub host_container_id: String,
    pub host_port: String,
    pub host_internal_port: String,
    pub database_name: Option<String>,
    pub db_container_id: Option<String>,
    pub db_host_port: Option<String>,
    pub db_internal_port: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Verification {
    pub framework_name: String,
    pub test_name: String,
    pub type_name: String,
    pub warnings: Vec<Warning>,
    pub errors: Vec<Error>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct BenchmarkCommands {
    pub primer_command: Vec<String>,
    pub warmup_command: Vec<String>,
    pub benchmark_commands: Vec<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct DockerContainerIdFuture {
    requires_wait_to_stop: bool,
    container_id: Option<String>,
    image_id: Option<String>,
    docker_host: String,
}
impl DockerContainerIdFuture {
    pub fn new(docker_host: &str) -> Self {
        DockerContainerIdFuture {
            requires_wait_to_stop: false,
            container_id: None,
            image_id: None,
            docker_host: docker_host.to_string(),
        }
    }

    pub fn image_id(&mut self, image_id: &str) {
        self.image_id = Some(image_id.to_string());
    }

    pub fn register(&mut self, container_id: &str) {
        self.requires_wait_to_stop = true;
        self.container_id = Some(container_id.to_string());
    }

    pub fn unregister(&mut self) {
        self.requires_wait_to_stop = false;
        self.container_id = None;
    }

    fn poll(&self) -> Poll<()> {
        if self.requires_wait_to_stop {
            if self.container_id.is_some() {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        } else {
            Poll::Ready(())
        }
    }
}
