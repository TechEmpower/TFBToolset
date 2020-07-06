//! The Docker module supports interfacing with the Docker daemon.
//! This includes actions like building `Test` images, building containers for
//! those images, and running containers in Docker.

pub mod container;
pub mod docker_config;
pub mod image;
pub mod listener;
pub mod network;
pub mod verification;

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
