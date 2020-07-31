use crate::config::{Project, Test};
use crate::docker::container::create_options::{bridge, host};
use crate::docker::docker_config::DockerConfig;
use crate::docker::listener::application::Application;
use crate::docker::listener::benchmarker::Benchmarker;
use crate::docker::listener::build_container::BuildContainer;
use crate::docker::listener::inspect_container::InspectContainer;
use crate::docker::listener::simple::Simple;
use crate::docker::listener::verifier::Verifier;
use crate::docker::network::NetworkMode;
use crate::docker::{DockerContainerIdFuture, DockerOrchestration, Verification};
use crate::error::ToolsetError::{
    DockerContainerCreateError, DockerContainerStartError, DockerVerifierContainerCreateError,
    FailedToCreateDockerContainerError, FailedToCreateDockerVerifierContainerError,
    FailedToKillDockerContainerError, FailedToStartDockerContainerError,
};
use crate::error::{ToolsetError, ToolsetResult};
use crate::io::Logger;
use curl::easy::{Easy2, List};
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::thread;
use std::time::Duration;

pub mod create_options;

/// Creates the container for the given `Test`.
/// Note: this function makes the assumption that the image is already
/// built and that the Docker daemon is aware of it.
/// Call `build_image_for_test()` before running.
pub fn create_container(
    docker_config: &DockerConfig,
    image_id: &str,
    network_id: &str,
    host_name: &str,
    database_container_id: &Option<String>,
) -> ToolsetResult<String> {
    match create_container_unsafe(docker_config, image_id, network_id, host_name) {
        Ok(id) => Ok(id),
        Err(e) => {
            if let Some(container_id) = database_container_id {
                stop_container(docker_config, container_id)?;
            }
            Err(e)
        }
    }
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
    let mut easy = Easy2::new(BuildContainer::new());
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    let mut headers = List::new();
    headers.append("Content-Type: application/json")?;

    let json = match &config.network_mode {
        NetworkMode::Bridge => {
            let mut builder = bridge::Builder::new("tfb.verifier")
                .publish_all_ports(true)
                .network_id(&orchestration.network_id)
                .env(&format!("PORT={}", orchestration.host_internal_port))
                .env(&format!("ENDPOINT={}", test_type.1.clone()))
                .env(&format!("TEST_TYPE={}", test_type.0.clone()))
                .env(&format!(
                    "CONCURRENCY_LEVELS={}",
                    &config.concurrency_levels
                ));
            if orchestration.database_name.is_some() {
                builder = builder.env(&format!(
                    "DATABASE={}",
                    orchestration
                        .database_name
                        .as_ref()
                        .unwrap_or(&String::new())
                ));
            }
            builder.build().to_json()
        }
        NetworkMode::Host => {
            let mut builder = host::Builder::new("tfb.verifier")
                .with_extra_host(&format!("tfb-database:{}", config.database_host))
                .env(&format!("PORT={}", orchestration.host_internal_port))
                .env(&format!("ENDPOINT={}", test_type.1.clone()))
                .env(&format!("TEST_TYPE={}", test_type.0.clone()))
                .env(&format!(
                    "CONCURRENCY_LEVELS={}",
                    &config.concurrency_levels
                ));
            if orchestration.database_name.is_some() {
                builder = builder.env(&format!(
                    "DATABASE={}",
                    orchestration
                        .database_name
                        .as_ref()
                        .unwrap_or(&String::new())
                ));
            }
            builder.build().to_json()
        }
    };
    let len = json.as_bytes().len();

    easy.post(true)?;
    easy.url(&format!(
        "http://{}/containers/create",
        config.server_docker_host
    ))?;
    easy.http_headers(headers)?;
    easy.in_filesize(len as u64)?;
    easy.post_field_size(len as u64)?;
    easy.post_fields_copy(json.as_bytes())?;
    easy.perform()?;

    match easy.response_code() {
        Ok(code) => match code {
            201 => {
                if let Some(container_id) = &easy.get_mut().container_id {
                    return Ok(container_id.clone());
                } else if let Some(error) = &easy.get_ref().error_message {
                    return Err(FailedToCreateDockerVerifierContainerError(error.clone()));
                }
                Err(DockerVerifierContainerCreateError)
            }
            code => {
                if let Some(error) = &easy.get_ref().error_message {
                    return Err(FailedToCreateDockerVerifierContainerError(error.clone()));
                }
                Err(FailedToCreateDockerVerifierContainerError(format!(
                    "{}",
                    code
                )))
            }
        },
        Err(e) => Err(FailedToCreateDockerVerifierContainerError(e.to_string())),
    }
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
    Ok(String::from("TODO"))
}

/// Gets both the internet and host port binding for the container given by
/// `container_id`.
pub fn get_database_port_bindings(
    docker_config: &DockerConfig,
    database_container_id: &Option<String>,
) -> ToolsetResult<(Option<String>, Option<String>)> {
    let mut database_ports = (None, None);
    if let Some(container_id) = database_container_id {
        match get_port_bindings_for_container_unsafe(docker_config, container_id) {
            Ok(ports) => database_ports = (Some(ports.0), Some(ports.1)),
            Err(e) => {
                stop_container(docker_config, container_id)?;
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
    container_ids: &(String, Option<String>),
) -> ToolsetResult<(String, String)> {
    match get_port_bindings_for_container_unsafe(docker_config, &container_ids.0) {
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
    match start_container_unsafe(docker_config, &container_ids.0, docker_host, logger) {
        Err(e) => Err(stop_containers_because_of_error(
            docker_config,
            container_ids,
            e,
        )),
        _ => Ok(()),
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
    match start_verification_container_unsafe(
        docker_config,
        project,
        test,
        test_type,
        &container_ids.0,
        logger,
    ) {
        Err(e) => Err(stop_containers_because_of_error(
            docker_config,
            container_ids,
            e,
        )),
        Ok(verification) => Ok(verification),
    }
}

/// Helper function to ensure that running containers started by the toolset
/// are stopped on error.
pub fn stop_containers_because_of_error(
    config: &DockerConfig,
    container_ids: &(String, Option<String>),
    error: ToolsetError,
) -> ToolsetError {
    match stop_container(config, &container_ids.0) {
        Err(e) => e,
        _ => {
            if let Some(container_id) = &container_ids.1 {
                match stop_container(config, container_id) {
                    Err(e) => e,
                    _ => error,
                }
            } else {
                error
            }
        }
    }
}

/// Stops the running container given by `container_id`.
/// This *will not* exit the running application. Callers must do so manually.
pub fn stop_container(config: &DockerConfig, container_id: &str) -> ToolsetResult<()> {
    let mut easy = Easy2::new(Simple::new());
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    easy.post(true)?;
    easy.url(&format!(
        "http://{}/containers/{}/stop",
        config.server_docker_host, container_id
    ))?;
    easy.perform()?;

    match easy.response_code()? {
        204 => Ok(()),
        _ => kill_container(config, container_id),
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
            kill_container(&docker_config, container_id).unwrap();
            container.container_id = None;
        }
    }
}

//
// PRIVATES
//

/// Creates the container for the given `Test`.
/// Note: this function makes the assumption that the image is already
/// built and that the docker daemon is aware of it.
/// Call `build_image_for_test()` before running.
fn create_container_unsafe(
    config: &DockerConfig,
    image_id: &str,
    network_id: &str,
    host_name: &str,
) -> ToolsetResult<String> {
    let mut easy = Easy2::new(BuildContainer::new());
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    let mut headers = List::new();
    headers.append("Content-Type: application/json")?;

    let json = match &config.network_mode {
        NetworkMode::Bridge => bridge::Builder::new(image_id)
            .publish_all_ports(true)
            .domainname(host_name)
            .hostname(host_name)
            .network_id(network_id)
            .alias(host_name)
            .build()
            .to_json(),
        NetworkMode::Host => host::Builder::new(image_id)
            .with_extra_host(&format!("tfb-database:{}", config.database_host))
            .domainname(host_name)
            .hostname(host_name)
            .build()
            .to_json(),
    };
    let len = json.as_bytes().len();

    easy.post(true)?;
    easy.url(&format!(
        "http://{}/containers/create",
        config.server_docker_host
    ))?;
    easy.http_headers(headers)?;
    easy.in_filesize(len as u64)?;
    easy.post_field_size(len as u64)?;
    easy.post_fields_copy(json.as_bytes())?;
    easy.perform()?;

    match easy.response_code() {
        Ok(code) => match code {
            201 => {
                if let Some(container_id) = &easy.get_mut().container_id {
                    return Ok(container_id.clone());
                } else if let Some(error) = &easy.get_ref().error_message {
                    return Err(FailedToCreateDockerContainerError(error.clone()));
                }
                Err(DockerContainerCreateError)
            }
            code => {
                if let Some(error) = &easy.get_ref().error_message {
                    return Err(FailedToCreateDockerContainerError(error.clone()));
                }
                Err(FailedToCreateDockerContainerError(format!("{}", code)))
            }
        },
        Err(e) => Err(FailedToCreateDockerContainerError(e.to_string())),
    }
}

/// Starts the container for the given `Test`.
/// Note: this function makes the assumption that the container is already
/// built and that the docker daemon is aware of it.
/// Call `create_container()` before running.
fn start_container_unsafe(
    config: &DockerConfig,
    container_id: &str,
    docker_host: &str,
    logger: &Logger,
) -> ToolsetResult<()> {
    let mut easy = Easy2::new(Simple::new());
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    easy.post(true)?;
    easy.url(&format!(
        "http://{}/containers/{}/start",
        docker_host, container_id
    ))?;
    easy.post_fields_copy(&[])?;
    easy.perform()?;

    match easy.response_code() {
        Ok(204) => attach_to_container_and_log(config, container_id, logger),
        Ok(code) => {
            if let Some(error) = &easy.get_ref().error_message {
                return Err(FailedToStartDockerContainerError(error.clone(), code));
            }
            Err(DockerContainerStartError(code))
        }
        Err(e) => Err(ToolsetError::CurlError(e)),
    }
}

/// Starts the Benchmarker container for the given `Test`.
/// Note: this function makes the assumption that the container is already
/// built and that the docker daemon is aware of it.
/// Call `create_container()` before running.
fn start_benchmarker_container_unsafe(
    config: &DockerConfig,
    test_type: &(&String, &String),
    container_id: &str,
    logger: &Logger,
) -> ToolsetResult<()> {
    let mut easy = Easy2::new(Simple::new());
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    easy.post(true)?;
    easy.url(&format!(
        "http://{}/containers/{}/start",
        config.client_docker_host, container_id
    ))?;
    easy.post_fields_copy(&[])?;
    easy.perform()?;

    match easy.response_code() {
        Ok(204) => attach_to_benchmarker_and_log(config, test_type, container_id, logger),
        Ok(code) => {
            if let Some(error) = &easy.get_ref().error_message {
                return Err(FailedToStartDockerContainerError(error.clone(), code));
            }
            Err(DockerContainerStartError(code))
        }
        Err(e) => Err(ToolsetError::CurlError(e)),
    }
}

/// Starts the verification container for the given `Test`.
/// Note: this function makes the assumption that the container is already
/// built and that the docker daemon is aware of it.
/// Call `create_container()` before running.
fn start_verification_container_unsafe(
    config: &DockerConfig,
    project: &Project,
    test: &Test,
    test_type: &(&String, &String),
    container_id: &str,
    logger: &Logger,
) -> ToolsetResult<Verification> {
    let mut easy = Easy2::new(Simple::new());
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    easy.post(true)?;
    easy.url(&format!(
        "http://{}/containers/{}/start",
        config.server_docker_host, container_id
    ))?;
    easy.post_fields_copy(&[])?;
    easy.perform()?;

    match easy.response_code() {
        Ok(204) => {
            attach_to_verifier_and_log(config, project, test, test_type, container_id, logger)
        }
        Ok(code) => {
            if let Some(error) = &easy.get_ref().error_message {
                return Err(FailedToStartDockerContainerError(error.clone(), code));
            }
            Err(DockerContainerStartError(code))
        }
        Err(e) => Err(ToolsetError::CurlError(e)),
    }
}

/// Gets both the internet and host port binding for the container given by
/// `container_id`.
fn get_port_bindings_for_container_unsafe(
    config: &DockerConfig,
    container_id: &str,
) -> ToolsetResult<(String, String)> {
    let mut easy = Easy2::new(InspectContainer::new(config));
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    easy.url(&format!(
        "http://{}/containers/{}/json",
        config.server_docker_host, container_id
    ))?;
    easy.perform()?;

    easy.get_ref().get_host_ports()
}

/// Kills the running container given by `container_id`.
/// This *will not* exit the running application. Callers must do so manually.
fn kill_container(config: &DockerConfig, container_id: &str) -> ToolsetResult<()> {
    let mut easy = Easy2::new(Simple::new());
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    easy.post(true)?;
    easy.url(&format!(
        "http://{}/containers/{}/kill",
        config.server_docker_host, container_id
    ))?;
    easy.perform()?;

    match easy.response_code()? {
        204 => Ok(()),
        _ => Err(FailedToKillDockerContainerError(format!(
            "Could not kill container: {}",
            container_id
        ))),
    }
}

/// Attaches to a running container given by `container_id` in a non-blocking
/// way. This spawns a new thread to proxy stdout/stderr from the running
/// container to stdout/stderr.
/// Note: this function makes the assumption that the container is already
/// built and running.
/// Call `start_container()` before running.
fn attach_to_container_and_log(
    config: &DockerConfig,
    container_id: &str,
    logger: &Logger,
) -> ToolsetResult<()> {
    let mut easy = Easy2::new(Application::new(logger));
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    let query_string = "?logs=1&stream=1&stdout=1&stderr=1";
    easy.post(true)?;
    easy.url(&format!(
        "http://{}/containers/{}/attach{}",
        config.server_docker_host, container_id, query_string
    ))?;

    thread::spawn(move || easy.perform().unwrap());

    Ok(())
}

/// Attaches to a running container given by `container_id` in a blocking way.
/// While it is expected that the `Verifier` handler will log to stdout/stderr,
/// it will be blocking the current thread while doing so, and eventually exit.
/// Note: this function makes the assumption that the container is already
/// built and running.
/// Call `start_container()` before running.
fn attach_to_verifier_and_log(
    config: &DockerConfig,
    project: &Project,
    test: &Test,
    test_type: &(&String, &String),
    container_id: &str,
    logger: &Logger,
) -> ToolsetResult<Verification> {
    let mut easy = Easy2::new(Verifier::new(project, test, test_type, logger));
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    let query_string = "?logs=1&stream=1&stdout=1&stderr=1";
    easy.post(true)?;
    easy.url(&format!(
        "http://{}/containers/{}/attach{}",
        config.server_docker_host, container_id, query_string
    ))?;
    easy.perform()?;

    Ok(easy.get_ref().verification.clone())
}

/// Attaches to a running container given by `container_id` in a blocking way.
/// While it is expected that the `Benchmarker` handler will log to
/// stdout/stderr, it will be blocking the current thread while doing so, and
/// eventually exit.
/// Note: this function makes the assumption that the container is already
/// built and running.
/// Call `start_container()` before running.
fn attach_to_benchmarker_and_log(
    config: &DockerConfig,
    test_type: &(&String, &String),
    container_id: &str,
    logger: &Logger,
) -> ToolsetResult<()> {
    let mut easy = Easy2::new(Benchmarker::new(test_type, logger));
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    let query_string = "?logs=1&stream=1&stdout=1&stderr=1";
    easy.post(true)?;
    easy.url(&format!(
        "http://{}/containers/{}/attach{}",
        config.server_docker_host, container_id, query_string
    ))?;
    easy.perform()?;

    Ok(())
}
