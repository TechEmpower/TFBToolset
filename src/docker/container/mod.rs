use crate::benchmarker::Mode;
use crate::config::{Project, Test};
use crate::docker::docker_config::DockerConfig;
use crate::docker::listener::application::Application;
use crate::docker::listener::benchmark_command_listener::BenchmarkCommandListener;
use crate::docker::listener::benchmarker::{BenchmarkResults, Benchmarker};
use crate::docker::listener::build_container::BuildContainer;
use crate::docker::listener::simple::Simple;
use crate::docker::listener::verifier::Verifier;
use crate::docker::{
    BenchmarkCommands, DockerContainerIdFuture, DockerOrchestration, Verification,
};
use crate::error::ToolsetError::{
    ContainerPortMappingInspectionError, FailedBenchmarkCommandRetrievalError,
};
use crate::error::ToolsetResult;
use crate::io::Logger;
use dockurl::container::create::host_config::{HostConfig, Ulimit};
use dockurl::container::create::networking_config::{
    EndpointSettings, EndpointsConfig, NetworkingConfig,
};
use dockurl::container::create::options::Options;
use dockurl::container::{
    attach_to_container, get_container_logs, inspect_container, kill_container,
    wait_for_container_to_exit,
};
use dockurl::network::NetworkMode;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::thread;
use std::time::Duration;

/// Note: this function makes the assumption that the image is already
/// built and that the Docker daemon is aware of it.
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
    let mut endpoint_settings = EndpointSettings::new();
    endpoint_settings.network_id(network_id);
    match &config.network_mode {
        dockurl::network::NetworkMode::Bridge => {
            host_config.network_mode(dockurl::network::NetworkMode::Bridge);
            endpoint_settings.alias(host_name);
        }
        dockurl::network::NetworkMode::Host => {
            host_config.extra_host("tfb-database", &config.database_host);
            host_config.network_mode(dockurl::network::NetworkMode::Host);
        }
    }
    host_config.publish_all_ports(true);

    options.networking_config(NetworkingConfig {
        endpoints_config: EndpointsConfig { endpoint_settings },
    });

    options.host_config(host_config);

    options.tty(true);

    let container_id = dockurl::container::create_container(
        options,
        config.use_unix_socket,
        docker_host,
        BuildContainer::new(),
    )?;

    Ok(container_id)
}

///
///
pub fn create_benchmarker_container(
    config: &DockerConfig,
    command: &[String],
) -> ToolsetResult<String> {
    let mut options = Options::new();
    options.image("techempower/tfb.verifier");
    options.tty(true);
    options.attach_stderr(true);
    options.cmds(command);

    let mut host_config = HostConfig::new();
    match &config.network_mode {
        dockurl::network::NetworkMode::Bridge => {
            host_config.network_mode(dockurl::network::NetworkMode::Bridge);
        }
        dockurl::network::NetworkMode::Host => {
            host_config.extra_host("tfb-server", &config.server_host);
            host_config.network_mode(dockurl::network::NetworkMode::Host);
        }
    }
    let mut sysctls = HashMap::new();
    sysctls.insert("net.core.somaxconn", "65535");
    host_config.sysctls(sysctls);
    let ulimit = Ulimit {
        name: "nofile",
        soft: 65535,
        hard: 65535,
    };
    host_config.ulimits(vec![ulimit]);

    options.host_config(host_config);

    let mut endpoint_settings = EndpointSettings::new();
    endpoint_settings.network_id(config.client_network_id.as_str());

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

/// Creates the container for the `TFBVerifier`.
/// Note: this function makes the assumption that the image has already been
/// pulled from Dockerhub and the Docker daemon is aware of it.
pub fn create_verifier_container(
    config: &DockerConfig,
    orchestration: &DockerOrchestration,
    mode: Mode,
    test_type: &(&String, &String),
) -> ToolsetResult<String> {
    let mut options = Options::new();
    options.image("techempower/tfb.verifier");
    options.tty(true);
    options.add_env(
        "MODE",
        match mode {
            Mode::Verify => "verify",
            Mode::Benchmark => "benchmark",
        },
    );
    options.add_env("PORT", &orchestration.host_internal_port);
    options.add_env("ENDPOINT", test_type.1);
    options.add_env("TEST_TYPE", test_type.0);
    options.add_env("CONCURRENCY_LEVELS", &config.concurrency_levels);
    options.add_env(
        "PIPELINE_CONCURRENCY_LEVELS",
        &config.pipeline_concurrency_levels,
    );
    if let Some(database_name) = &orchestration.database_name {
        options.add_env("DATABASE", database_name);
    }

    let mut host_config = HostConfig::new();
    match &config.network_mode {
        dockurl::network::NetworkMode::Bridge => {
            host_config.network_mode(dockurl::network::NetworkMode::Bridge);
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
    endpoint_settings.network_id(config.client_network_id.as_str());

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

/// Gets both the internal and host port binding for the container given by
/// `container_id`.
pub fn get_port_bindings_for_container(
    docker_config: &DockerConfig,
    docker_host: &str,
    container_id: &str,
) -> ToolsetResult<(String, String)> {
    let inspection = inspect_container(
        container_id,
        docker_host,
        docker_config.use_unix_socket,
        Simple::new(),
    )?;

    if let Some(exposed_ports) = inspection.config.exposed_ports {
        for key in exposed_ports.keys() {
            let inner_port: Vec<&str> = key.split('/').collect();

            match docker_config.network_mode {
                NetworkMode::Bridge => {
                    if let Some(key) = inspection.network_settings.ports.get(key) {
                        if let Some(port_mapping) = key.get(0) {
                            if let Some(inner_port) = inner_port.get(0) {
                                return Ok((
                                    port_mapping.host_port.clone(),
                                    inner_port.to_string(),
                                ));
                            }
                        }
                    }
                }
                NetworkMode::Host => {
                    return Ok((
                        inner_port.get(0).unwrap().to_string(),
                        inner_port.get(0).unwrap().to_string(),
                    ));
                }
            };
        }
    }

    Err(ContainerPortMappingInspectionError)
}

/// Starts the container for the given `Test`.
/// Note: this function makes the assumption that the container is already
/// built and that the docker daemon is aware of it.
/// Call `create_container()` before running.
pub fn start_container(
    docker_config: &DockerConfig,
    container_id: &str,
    docker_host: &str,
    logger: &Logger,
) -> ToolsetResult<()> {
    dockurl::container::start_container(
        container_id,
        docker_host,
        docker_config.use_unix_socket,
        Simple::new(),
    )?;
    let container_id = container_id.to_string();
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

///
///
pub fn start_benchmark_command_retrieval_container(
    docker_config: &DockerConfig,
    test_type: &(&String, &String),
    container_id: &str,
    logger: &Logger,
) -> ToolsetResult<BenchmarkCommands> {
    dockurl::container::start_container(
        container_id,
        &docker_config.client_docker_host,
        docker_config.use_unix_socket,
        Simple::new(),
    )?;
    wait_for_container_to_exit(
        container_id,
        &docker_config.client_docker_host,
        docker_config.use_unix_socket,
        Simple::new(),
    )?;
    let listener = get_container_logs(
        container_id,
        &docker_config.client_docker_host,
        docker_config.use_unix_socket,
        BenchmarkCommandListener::new(test_type, logger),
    )?;
    if let Some(commands) = listener.benchmark_commands {
        Ok(commands)
    } else {
        Err(FailedBenchmarkCommandRetrievalError)
    }
}

/// Starts the benchmarker container and logs its stdout/stderr.
pub fn start_benchmarker_container(
    docker_config: &DockerConfig,
    container_id: &str,
    logger: &Logger,
) -> ToolsetResult<BenchmarkResults> {
    dockurl::container::start_container(
        container_id,
        &docker_config.client_docker_host,
        docker_config.use_unix_socket,
        Simple::new(),
    )?;
    wait_for_container_to_exit(
        container_id,
        &docker_config.client_docker_host,
        docker_config.use_unix_socket,
        Simple::new(),
    )?;
    let benchmarker = get_container_logs(
        container_id,
        &docker_config.client_docker_host,
        docker_config.use_unix_socket,
        Benchmarker::new(logger),
    )?;

    benchmarker.parse_wrk_output()
}

/// Starts the verification container, captures its stdout/stderr, parses any
/// messages sent from the verifier, and logs the rest.
pub fn start_verification_container(
    docker_config: &DockerConfig,
    project: &Project,
    test: &Test,
    test_type: &(&String, &String),
    container_id: &str,
    logger: &Logger,
) -> ToolsetResult<Verification> {
    dockurl::container::start_container(
        &container_id,
        &docker_config.client_docker_host,
        docker_config.use_unix_socket,
        Simple::new(),
    )?;
    let verifier = attach_to_container(
        &container_id,
        &docker_config.client_docker_host,
        docker_config.use_unix_socket,
        Verifier::new(project, test, test_type, logger),
    )?;

    Ok(verifier.verification)
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
    let mut requires_wait_to_stop = false;
    if let Ok(container) = container.lock() {
        requires_wait_to_stop = container.requires_wait_to_stop;
    }
    if requires_wait_to_stop {
        let mut poll = Poll::Pending;
        while poll == Poll::Pending {
            if let Ok(container) = container.lock() {
                poll = container.poll();
                if poll == Poll::Pending {
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
        if let Ok(mut container) = container.lock() {
            if let Some(container_id) = &container.container_id {
                kill_container(
                    container_id,
                    &container.docker_host,
                    docker_config.use_unix_socket,
                    Simple::new(),
                )
                .unwrap_or(());
                container.unregister();
            }
        }
    }
}
