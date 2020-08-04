use crate::config::{Project, Test};
use crate::docker::docker_config::DockerConfig;
use crate::docker::listener::application::Application;
use crate::docker::listener::build_container::BuildContainer;
use crate::docker::listener::simple::Simple;
use crate::docker::listener::verifier::Verifier;
use crate::docker::{DockerContainerIdFuture, DockerOrchestration, Verification};
use crate::error::ToolsetError::ContainerPortMappingInspectionError;
use crate::error::{ToolsetError, ToolsetResult};
use crate::io::Logger;
use dockurl::container::create::host_config::HostConfig;
use dockurl::container::create::networking_config::{
    EndpointSettings, EndpointsConfig, NetworkingConfig,
};
use dockurl::container::create::options::Options;
use dockurl::container::{attach_to_container, inspect_container, kill_container, stop_container};
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::thread;
use std::time::Duration;

/// Creates the container for the given `Test`.
/// Note: this function makes the assumption that the image is already
/// built and that the Docker daemon is aware of it.
/// Call `build_image_for_test()` before running.
pub fn create_container(
    config: &DockerConfig,
    image_id: &str,
    network_id: &str,
    host_name: &str,
    docker_host: &str,
) -> ToolsetResult<String> {
    let mut options = Options::new();
    options.image(image_id);
    options.hostname(host_name);
    options.domain_name(host_name);

    let mut host_config = HostConfig::new();
    match &config.network_mode {
        dockurl::network::NetworkMode::Bridge => {
            host_config.network_mode(dockurl::network::NetworkMode::Bridge)
        }
        dockurl::network::NetworkMode::Host => {
            host_config.extra_host("tfb-database", &config.database_host);
            host_config.network_mode(dockurl::network::NetworkMode::Host);
        }
    }
    host_config.publish_all_ports(true);

    options.host_config(host_config);

    let mut endpoint_settings = EndpointSettings::new();
    endpoint_settings.alias(host_name);
    endpoint_settings.network_id(network_id);

    options.networking_config(NetworkingConfig {
        endpoints_config: EndpointsConfig { endpoint_settings },
    });

    options.tty(true);

    let container_id = dockurl::container::create_container(
        options,
        config.use_unix_socket,
        docker_host,
        BuildContainer::new(),
    )?;

    Ok(container_id)
}

/// Creates the container for the `TFBVerifier`.
/// Note: this function makes the assumption that the image has already been
/// pulled from Dockerhub and the Docker daemon is aware of it.
/// todo - v does not exist yet.
/// Call `pull_verifier()` before running.
pub fn create_verifier_container(
    config: &DockerConfig,
    orchestration: &DockerOrchestration,
    test_type: &(&String, &String),
) -> ToolsetResult<String> {
    let mut options = Options::new();
    options.image("tfb.verifier");
    options.tty(true);
    options.add_env("PORT", &orchestration.host_internal_port);
    options.add_env("ENDPOINT", test_type.1);
    options.add_env("TEST_TYPE", test_type.0);
    options.add_env("CONCURRENCY_LEVELS", &config.concurrency_levels);
    if let Some(database_name) = &orchestration.database_name {
        options.add_env("DATABASE", database_name);
    }

    let mut host_config = HostConfig::new();
    match &config.network_mode {
        dockurl::network::NetworkMode::Bridge => {
            host_config.network_mode(dockurl::network::NetworkMode::Bridge)
        }
        dockurl::network::NetworkMode::Host => {
            host_config.extra_host("tfb-server", &config.server_host);
            host_config.extra_host("tfb-database", &config.database_host);
            host_config.network_mode(dockurl::network::NetworkMode::Host);
        }
    }
    host_config.publish_all_ports(true);

    options.host_config(host_config);

    let mut endpoint_settings = EndpointSettings::new();
    endpoint_settings.network_id(&orchestration.network_id);

    options.networking_config(NetworkingConfig {
        endpoints_config: EndpointsConfig { endpoint_settings },
    });

    let container_id = dockurl::container::create_container(
        options,
        config.use_unix_socket,
        &config.client_docker_host,
        BuildContainer::new(),
    )?;

    Ok(container_id)
}

/// Creates the container for the `TFBBenchmarker`.
/// Note: this function makes the assumption that the image has already been
/// pulled from Dockerhub and the Docker daemon is aware of it.
/// todo - TFBBenchmarker does not exist yet.
/// Call `pull_benchmarker()` before running.
pub fn create_benchmarker_container(
    _config: &DockerConfig,
    _orchestration: &DockerOrchestration,
    _test_type: &(&String, &String),
) -> ToolsetResult<String> {
    // let mut easy = Easy2::new(BuildContainer::new());
    // if config.use_unix_socket {
    //     easy.unix_socket("/var/run/docker.sock")?;
    // }
    //
    // let mut headers = List::new();
    // headers.append("Content-Type: application/json")?;
    //
    // let json = match &config.network_mode {
    //     NetworkMode::Bridge => bridge::Builder::new("tfb.benchmarker")
    //         .publish_all_ports(true)
    //         .network_id(&orchestration.network_id)
    //         .env(&format!("name={}", ""))
    //         .env(&format!("server_host={}", ""))
    //         .env(&format!("levels={}", ""))
    //         .env(&format!("duration={}", ""))
    //         .env(&format!("max_concurrency={}", ""))
    //         .env(&format!("max_threads={}", ""))
    //         .env(&format!("pipeline={}", ""))
    //         .env(&format!("accept={}", ""))
    //         .build()
    //         .to_json(),
    //     NetworkMode::Host => host::Builder::new("tfb.benchmarker")
    //         .with_extra_host(&format!("tfb-server:{}", config.server_host))
    //         .env(&format!("name={}", ""))
    //         .env(&format!("server_host={}", ""))
    //         .env(&format!("levels={}", ""))
    //         .env(&format!("duration={}", ""))
    //         .env(&format!("max_concurrency={}", ""))
    //         .env(&format!("max_threads={}", ""))
    //         .env(&format!("pipeline={}", ""))
    //         .env(&format!("accept={}", ""))
    //         .build()
    //         .to_json(),
    // };
    // let len = json.as_bytes().len();
    //
    // easy.post(true)?;
    // easy.url(&format!(
    //     "http://{}/containers/create",
    //     config.server_docker_host
    // ))?;
    // easy.http_headers(headers)?;
    // easy.in_filesize(len as u64)?;
    // easy.post_field_size(len as u64)?;
    // easy.post_fields_copy(json.as_bytes())?;
    // easy.perform()?;
    //
    // match easy.response_code() {
    //     Ok(code) => match code {
    //         201 => {
    //             if let Some(container_id) = &easy.get_mut().container_id {
    //                 return Ok(container_id.clone());
    //             } else if let Some(error) = &easy.get_ref().error_message {
    //                 return Err(FailedToCreateDockerVerifierContainerError(error.clone()));
    //             }
    //             Err(DockerVerifierContainerCreateError)
    //         }
    //         code => {
    //             if let Some(error) = &easy.get_ref().error_message {
    //                 return Err(FailedToCreateDockerVerifierContainerError(error.clone()));
    //             }
    //             Err(FailedToCreateDockerVerifierContainerError(format!(
    //                 "{}",
    //                 code
    //             )))
    //         }
    //     },
    //     Err(e) => Err(FailedToCreateDockerVerifierContainerError(e.to_string())),
    // }
    Ok("".to_string())
}

/// Gets both the internet and host port binding for the container given by
/// `container_id`.
pub fn get_database_port_bindings(
    docker_config: &DockerConfig,
    database_container_id: &Option<String>,
) -> ToolsetResult<(Option<String>, Option<String>)> {
    let mut database_ports = (None, None);
    if let Some(container_id) = database_container_id {
        match get_port_bindings_for_container_unsafe(
            docker_config,
            &docker_config.database_docker_host,
            container_id,
        ) {
            Ok(ports) => database_ports = (Some(ports.0), Some(ports.1)),
            Err(e) => {
                stop_container(
                    container_id,
                    &docker_config.database_docker_host,
                    docker_config.use_unix_socket,
                    Simple::new(),
                )?;
                return Err(e);
            }
        }
    }
    Ok(database_ports)
}

/// Gets both the internet and host port binding for the container given by
/// `container_id`.
pub fn get_port_bindings_for_container(
    docker_config: &DockerConfig,
    docker_host: &str,
    container_ids: &(String, Option<String>),
) -> ToolsetResult<(String, String)> {
    match get_port_bindings_for_container_unsafe(docker_config, docker_host, &container_ids.0) {
        Ok(ports) => Ok(ports),
        Err(e) => Err(stop_containers_because_of_error(
            docker_config,
            container_ids,
            e,
        )),
    }
}

/// Starts the container for the given `Test`.
/// Note: this function makes the assumption that the container is already
/// built and that the docker daemon is aware of it.
/// Call `create_container()` before running.
pub fn start_container(
    docker_config: &DockerConfig,
    container_ids: &(String, Option<String>),
    docker_host: &str,
    logger: &Logger,
) -> ToolsetResult<()> {
    match dockurl::container::start_container(
        &container_ids.0,
        docker_host,
        docker_config.use_unix_socket,
        Simple::new(),
    ) {
        Err(e) => Err(stop_containers_because_of_error(
            docker_config,
            container_ids,
            ToolsetError::DockerError(e),
        )),
        _ => {
            let container_id = container_ids.0.clone();
            let docker_host = docker_config.client_docker_host.clone();
            let use_unix_socket = docker_config.use_unix_socket;
            let logger = logger.clone();
            thread::spawn(move || {
                attach_to_container(
                    &container_id,
                    &docker_host,
                    use_unix_socket,
                    Application::new(&logger),
                )
                .unwrap();
            });
            Ok(())
        }
    }
}

/// Starts the benchmarker container and logs its stdout/stderr.
pub fn start_benchmarker_container(
    docker_config: &DockerConfig,
    test_type: &(&String, &String),
    container_ids: &(String, Option<String>),
    logger: &Logger,
) -> ToolsetResult<()> {
    match start_benchmarker_container_unsafe(docker_config, test_type, &container_ids.0, logger) {
        Err(e) => Err(stop_containers_because_of_error(
            docker_config,
            container_ids,
            e,
        )),
        Ok(_) => Ok(()),
    }
}

/// Starts the verification container, captures its stdout/stderr, parses any
/// messages sent from the verifier, and logs the rest.
pub fn start_verification_container(
    docker_config: &DockerConfig,
    project: &Project,
    test: &Test,
    test_type: &(&String, &String),
    container_ids: &(String, Option<String>),
    logger: &Logger,
) -> ToolsetResult<Verification> {
    match dockurl::container::start_container(
        &container_ids.0,
        &docker_config.client_docker_host,
        docker_config.use_unix_socket,
        Simple::new(),
    ) {
        Err(e) => Err(stop_containers_because_of_error(
            docker_config,
            container_ids,
            ToolsetError::DockerError(e),
        )),
        Ok(()) => {
            match attach_to_container(
                &container_ids.0,
                &docker_config.client_docker_host,
                docker_config.use_unix_socket,
                Verifier::new(project, test, test_type, logger),
            ) {
                Ok(verifier) => Ok(verifier.verification),
                Err(e) => Err(stop_containers_because_of_error(
                    docker_config,
                    container_ids,
                    ToolsetError::DockerError(e),
                )),
            }
        }
    }
}

/// Helper function to ensure that running containers started by the toolset
/// are stopped on error.
pub fn stop_containers_because_of_error(
    config: &DockerConfig,
    container_ids: &(String, Option<String>),
    error: ToolsetError,
) -> ToolsetError {
    match stop_container(
        &container_ids.0,
        &config.server_docker_host,
        config.use_unix_socket,
        Simple::new(),
    ) {
        Err(e) => ToolsetError::DockerError(e),
        _ => {
            if let Some(container_id) = &container_ids.1 {
                match stop_container(
                    container_id,
                    &config.database_docker_host,
                    config.use_unix_socket,
                    Simple::new(),
                ) {
                    Err(e) => ToolsetError::DockerError(e),
                    _ => error,
                }
            } else {
                error
            }
        }
    }
}

/// Polls until `container` is ready with either some `container_id` or `None`,
/// then kills that `container_id`, and sets the internal `container_id` to
/// `None`.
///
/// Note: this function blocks until the given `container` is in a ready state.
pub fn stop_docker_container_future(
    docker_config: &DockerConfig,
    container: &Arc<Mutex<DockerContainerIdFuture>>,
) {
    let mut poll = Poll::Pending;
    while poll == Poll::Pending {
        if let Ok(container) = container.try_lock() {
            poll = container.poll();
            if poll == Poll::Pending {
                thread::sleep(Duration::from_secs(1));
            }
        }
    }
    if let Ok(mut container) = container.try_lock() {
        if let Some(container_id) = &container.container_id {
            kill_container(
                container_id,
                &container.docker_host,
                docker_config.use_unix_socket,
                Simple::new(),
            )
            .unwrap();
            container.container_id = None;
        }
    }
}

//
// PRIVATES
//

/// Starts the Benchmarker container for the given `Test`.
/// Note: this function makes the assumption that the container is already
/// built and that the docker daemon is aware of it.
/// Call `create_container()` before running.
fn start_benchmarker_container_unsafe(
    _config: &DockerConfig,
    _test_type: &(&String, &String),
    _container_id: &str,
    _logger: &Logger,
) -> ToolsetResult<()> {
    // let mut easy = Easy2::new(Simple::new());
    // if config.use_unix_socket {
    //     easy.unix_socket("/var/run/docker.sock")?;
    // }
    //
    // easy.post(true)?;
    // easy.url(&format!(
    //     "http://{}/containers/{}/start",
    //     config.client_docker_host, container_id
    // ))?;
    // easy.post_fields_copy(&[])?;
    // easy.perform()?;
    //
    // match easy.response_code() {
    //     Ok(204) => attach_to_benchmarker_and_log(config, test_type, container_id, logger),
    //     Ok(code) => {
    //         if let Some(error) = &easy.get_ref().error_message {
    //             return Err(FailedToStartDockerContainerError(error.clone(), code));
    //         }
    //         Err(DockerContainerStartError(code))
    //     }
    //     Err(e) => Err(ToolsetError::CurlError(e)),
    // }
    Ok(())
}

/// Gets both the internet and host port binding for the container given by
/// `container_id`.
fn get_port_bindings_for_container_unsafe(
    config: &DockerConfig,
    docker_host: &str,
    container_id: &str,
) -> ToolsetResult<(String, String)> {
    let inspection = inspect_container(
        container_id,
        docker_host,
        config.use_unix_socket,
        Simple::new(),
    )?;

    for key in inspection.network_settings.ports.keys() {
        let inner_port: Vec<&str> = key.split('/').collect();
        if let Some(key) = inspection.network_settings.ports.get(key) {
            if let Some(port_mapping) = key.get(0) {
                if let Some(inner_port) = inner_port.get(0) {
                    return Ok((port_mapping.host_port.clone(), inner_port.to_string()));
                }
            }
        }
    }

    Err(ContainerPortMappingInspectionError)
}
