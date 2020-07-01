//! The Docker module supports interfacing with the Docker daemon.
//! This includes actions like building `Test` images, building containers for
//! those images, and running containers in Docker.

use crate::config::{Project, Test};
use crate::docker::container::{
    create_container, get_database_port_bindings, get_port_bindings_for_container, start_container,
    stop_container, stop_containers_because_of_error,
};
use crate::docker::docker_config::DockerConfig;
use crate::docker::image::build_image;
use crate::docker::listener::simple::Simple;
use crate::docker::network::{connect_container_to_network, create_network, NetworkMode};
use crate::error::ToolsetError::NoResponseFromDockerContainerError;
use crate::error::ToolsetResult;
use crate::io::{attach_ctrlc_handler, Logger};
use curl::easy::Easy2;
use std::{thread, time};

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

/// Starts all the underlying docker orchestration required for the given
/// `Test` to be able to respond to requests and, optionally, communicate
/// with a database container.
/// Note: This function blocks the current thread until the test implementation
///  is able to successfully respond to a request.
pub fn start_test_orchestration(
    docker_config: &DockerConfig,
    project: &Project,
    test: &Test,
    logger: &Logger,
) -> ToolsetResult<DockerOrchestration> {
    let network_id = match &docker_config.network_mode {
        NetworkMode::Bridge => create_network(docker_config, test)?,
        NetworkMode::Host => "host".to_string(),
    };

    let database_container_id = start_database_if_necessary(docker_config, test, &network_id)?;
    let database_ports = get_database_port_bindings(docker_config, &database_container_id)?;

    let image_id = build_image(
        docker_config,
        project,
        test,
        &database_container_id,
        &logger,
    )?;

    let container_id = create_container(
        docker_config,
        &image_id,
        &network_id,
        &docker_config.server_host,
        &database_container_id,
    )?;

    let container_ids = (container_id, database_container_id);

    connect_container_to_network(docker_config, &network_id, &container_ids)?;

    attach_ctrlc_handler(docker_config, &container_ids)?;

    start_container(docker_config, &container_ids, &logger)?;

    let host_ports = get_port_bindings_for_container(docker_config, &container_ids)?;

    if let Err(e) =
        wait_until_accepting_requests(docker_config, &container_ids, &host_ports.0, test)
    {
        return Err(stop_containers_because_of_error(
            docker_config,
            &container_ids,
            e,
        ));
    };

    Ok(DockerOrchestration {
        network_id,
        host_container_id: container_ids.0,
        host_port: host_ports.0,
        host_internal_port: host_ports.1,
        database_name: test.database.clone(),
        db_container_id: container_ids.1,
        db_host_port: database_ports.0,
        db_internal_port: database_ports.1,
    })
}

//
// PRIVATES
//

/// Starts the database for the given `Test` if one is specified as being
/// required by the underlying configuration file.
fn start_database_if_necessary(
    docker_config: &DockerConfig,
    test: &Test,
    network_id: &str,
) -> ToolsetResult<Option<String>> {
    if let Some(database) = &test.database {
        // todo - this will be pulled from Dockerhub at some point, but for
        //  local testing, we just have to build it and use it.
        //  Let me try again - you have to:
        //  ```
        //  $ cd FrameworkBenchmarks/toolset/databases/postgres
        //  $ docker build -t tfb.database.postgres -f postgres.dockerfile .
        //  ```
        //  before this local testing will work.
        // println!("Going to pull tfb.database.{}", database);
        // pull_image(
        //     &docker_config,
        //     &format!("tfb.database.{}", database.to_lowercase()),
        // )?;

        let container_id = create_container(
            docker_config,
            &format!("tfb.database.{}", database.to_lowercase()),
            network_id,
            &docker_config.database_host,
            &None,
        )?;

        let container_ids = (container_id, None);

        connect_container_to_network(docker_config, network_id, &container_ids)?;

        let mut logger = Logger::with_prefix(&database);
        logger.quiet = true;

        start_container(docker_config, &container_ids, &logger)?;

        return Ok(Some(container_ids.0));
    }

    Ok(None)
}

/// Blocks the current thread until either the operation times out or `Test`
/// responds successfully (200).
fn wait_until_accepting_requests(
    docker_config: &DockerConfig,
    container_ids: &(String, Option<String>),
    host_port: &str,
    test: &Test,
) -> ToolsetResult<()> {
    let mut slept_for = 0;
    loop {
        if slept_for > 60 {
            stop_container(docker_config, &container_ids.0)?;
            if let Some(database_container_id) = &container_ids.1 {
                stop_container(docker_config, &database_container_id)?;
            }

            return Err(NoResponseFromDockerContainerError);
        }
        let mut easy = Easy2::new(Simple::new());

        let mut endpoint = String::new();
        if let Some(key) = test.urls.keys().next() {
            if let Some(_endpoint) = test.urls.get(key) {
                endpoint = _endpoint.clone();
            }
        }

        match docker_config.server_host.as_str() {
            "tfb-server" => easy.url(&format!("http://localhost:{}{}", host_port, endpoint))?,
            _ => easy.url(&format!(
                "http://{}:{}{}",
                docker_config.server_host, host_port, endpoint
            ))?,
        };
        easy.timeout(time::Duration::from_secs(1))?;
        let _ = easy.perform();

        match easy.response_code() {
            Ok(code) => {
                if code > 0 {
                    return Ok(());
                }
            }
            _ => {
                slept_for += 1;
                thread::sleep(time::Duration::from_secs(1));
            }
        }
    }
}
