//! The Docker module supports interfacing with the Docker daemon.
//! This includes actions like building `Test` images, building containers for
//! those images, and running containers in Docker.

use crate::docker::listener::verifier::Error;
use crate::docker::listener::verifier::Warning;

pub mod container;
pub mod docker_config;
pub mod image;
pub mod listener;
pub mod network;

#[derive(Debug)]
pub struct DockerOrchestration {
    pub network_id: String,
    pub host_container_id: String,
    pub host_port: String,
    pub host_internal_port: String,
    pub database_name: Option<String>,
    pub db_container_id: Option<String>,
    pub db_host_port: Option<String>,
    pub db_internal_port: Option<String>,
}

#[derive(Clone)]
pub struct Verification {
    pub framework_name: String,
    pub test_name: String,
    pub type_name: String,
    pub warnings: Vec<Warning>,
    pub errors: Vec<Error>,
}
