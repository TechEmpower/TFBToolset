use clap::ArgMatches;

use crate::config::{Named, Project, Test};
use crate::docker::container::{
    create_benchmarker_container, create_container, create_verifier_container,
    get_database_port_bindings, get_port_bindings_for_container, start_benchmarker_container,
    start_container, start_verification_container, stop_containers_because_of_error,
    stop_docker_container_future,
};
use crate::docker::docker_config::DockerConfig;
use crate::docker::image::build_image;
use crate::docker::listener::simple::Simple;
use crate::docker::network::{connect_container_to_network, create_network};
use crate::docker::{DockerContainerIdFuture, DockerOrchestration, Verification};
use crate::error::ToolsetError::{NoResponseFromDockerContainerError, VerificationFailedException};
use crate::error::ToolsetResult;
use crate::io::{report_verifications, Logger};
use crate::metadata;
use colored::Colorize;
use curl::easy::Easy2;
use dockurl::container::stop_container;
use dockurl::network::NetworkMode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{thread, time};

pub mod modes {
    pub const BENCHMARK: &str = "benchmark";
    pub const VERIFY: &str = "verify";
    pub const DEBUG: &str = "debug";
}

/// Benchmarker supports three different functions which all perform the
/// underlying Docker orchestration of getting a `Test` implementation running
/// in a Container and accepting requests on their exposed port. The three
/// different way to run the benchmarker and how they differ are as follows:  
///
/// 1. `debug` - starts the `Test` container and reports the exposed host port
///              for the purpose of making requests from the host.
/// 2. `verify` - starts the `Test` container and runs the `TFBVerifier`
///              container against the URLs configured for said `Test` in its
///              config. Logs information about the verification of each URL.
/// 3. `benchmark` - starts the `Test` container, runs the `TFBVerifier`, and
///              if the verification of the `URL` passes, runs the
///              `TFBBenchmarker` against it, captures the results, parses
///              them, and writes them to the results file.
pub struct Benchmarker {
    docker_config: DockerConfig,
    projects: Vec<Project>,
    application_container_id: Arc<Mutex<DockerContainerIdFuture>>,
    database_container_id: Arc<Mutex<DockerContainerIdFuture>>,
    verifier_container_id: Arc<Mutex<DockerContainerIdFuture>>,
    benchmarker_container_id: Arc<Mutex<DockerContainerIdFuture>>,
    ctrlc_received: Arc<AtomicBool>,
}
impl Benchmarker {
    pub fn new(matches: ArgMatches) -> Self {
        let docker_config = DockerConfig::new(&matches);
        let application_container_id = Arc::new(Mutex::new(DockerContainerIdFuture::new(
            &docker_config.server_docker_host,
        )));
        let database_container_id = Arc::new(Mutex::new(DockerContainerIdFuture::new(
            &docker_config.database_docker_host,
        )));
        let verifier_container_id = Arc::new(Mutex::new(DockerContainerIdFuture::new(
            &docker_config.client_docker_host,
        )));
        let benchmarker_container_id = Arc::new(Mutex::new(DockerContainerIdFuture::new(
            &docker_config.client_docker_host,
        )));

        let benchmarker = Self {
            docker_config,
            projects: metadata::list_projects_to_run(&matches),
            application_container_id,
            database_container_id,
            verifier_container_id,
            benchmarker_container_id,
            ctrlc_received: Arc::new(AtomicBool::new(false)),
        };

        let docker_config = benchmarker.docker_config.clone();
        let application_container_id = Arc::clone(&benchmarker.application_container_id);
        let database_container_id = Arc::clone(&benchmarker.database_container_id);
        let verifier_container_id = Arc::clone(&benchmarker.verifier_container_id);
        let benchmarker_container_id = Arc::clone(&benchmarker.benchmarker_container_id);
        let ctrlc_received = Arc::clone(&benchmarker.ctrlc_received);
        ctrlc::set_handler(move || {
            let logger = Logger::default();
            logger.log("Shutting down (may take a moment)").unwrap();
            if ctrlc_received.load(Ordering::Acquire) {
                logger
                    .log("Exiting immediately (there may still be running containers to stop)")
                    .unwrap();
                std::process::exit(0);
            } else {
                let docker_config = docker_config.clone();
                let application_container_id = Arc::clone(&application_container_id);
                let database_container_id = Arc::clone(&database_container_id);
                let verifier_container_id = Arc::clone(&verifier_container_id);
                let benchmarker_container_id = Arc::clone(&benchmarker_container_id);
                let ctrlc_received = Arc::clone(&ctrlc_received);
                thread::spawn(move || {
                    ctrlc_received.store(true, Ordering::Release);
                    stop_docker_container_future(&docker_config, &verifier_container_id);
                    stop_docker_container_future(&docker_config, &benchmarker_container_id);
                    stop_docker_container_future(&docker_config, &application_container_id);
                    stop_docker_container_future(&docker_config, &database_container_id);
                    std::process::exit(0);
                });
            }
        })
        .unwrap();

        benchmarker
    }

    /// Iterates over the specified test implementation(s), starts configured
    /// required services (like a database), starts the test implementation,
    /// verifies the configured end-points for each test type, and, if
    /// successful, will benchmark the running test implementation. When
    /// benchmarking completes, the results are parsed and stored in the
    /// results directory for this benchmark.
    pub fn benchmark(&mut self) -> ToolsetResult<()> {
        let logger = self.docker_config.logger.clone();
        let projects = &self.projects.clone();
        for project in projects {
            for test in &project.tests {
                let mut logger = logger.clone();
                logger.set_test(test);
                self.trip();
                let orchestration = self.start_test_orchestration(project, test, &logger)?;
                for test_type in &test.urls {
                    if self
                        .run_benchmark(&orchestration, &test_type, &logger)
                        .is_err()
                    {
                        // At present, we purposefully do not bubble this error
                        // up because there may be more tests to benchmark.
                    }
                }

                self.trip();
                self.stop_containers()?;
            }
        }

        Ok(())
    }

    /// Starts the given test implementation as a running server and waits
    /// indefinitely. This is useful for locally debugging why your service may
    /// not be responding correctly and failing verification, for example.
    pub fn debug(&mut self) -> ToolsetResult<()> {
        // Because it makes no sense to loop over all the specified tests when
        // the first test found will cause the main thread to sleep forever, we
        // just check *that* there is a test to run and start it.
        let projects = self.projects.clone();
        if let Some(project) = projects.get(0) {
            if let Some(test) = project.tests.get(0) {
                let logger = Logger::with_prefix(&test.get_name());
                let orchestration = self.start_test_orchestration(&project, &test, &logger)?;
                logger.log(
                    &format!(
                        "Entering debug mode. Server http://localhost:{} has started. CTRL-c to stop.",
                        orchestration.host_port
                    )
                    .yellow(),
                )?;
                loop {
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }

        Ok(())
    }

    /// Attempts to run the suite of verifications against the specified
    /// test implementation(s).
    pub fn verify(&mut self) -> ToolsetResult<()> {
        let mut succeeded = true;
        let mut verifications = Vec::new();
        let logger = self.docker_config.logger.clone();
        let projects = &self.projects.clone();
        for project in projects {
            for test in &project.tests {
                let mut logger = logger.clone();
                logger.set_test(test);
                self.trip();
                let orchestration = self.start_test_orchestration(project, test, &logger)?;
                for test_type in &test.urls {
                    let verification = self.run_verification(
                        &project,
                        &test,
                        &orchestration,
                        &test_type,
                        &logger,
                    )?;
                    succeeded &= verification.errors.is_empty();
                    verifications.push(verification);
                }

                self.trip();
                self.stop_containers()?;
            }
        }

        self.trip();
        report_verifications(verifications, logger)?;

        if succeeded {
            Ok(())
        } else {
            Err(VerificationFailedException)
        }
    }

    //
    // PRIVATES
    //

    /// Runs the benchmarker container against the given test orchestration.
    fn run_benchmark(
        &mut self,
        orchestration: &DockerOrchestration,
        test_type: &(&String, &String),
        logger: &Logger,
    ) -> ToolsetResult<()> {
        let container_id =
            create_benchmarker_container(&self.docker_config, orchestration, test_type)?;
        let container_ids = (container_id, None);

        connect_container_to_network(
            &self.docker_config,
            &self.docker_config.server_docker_host,
            &orchestration.network_id,
            &container_ids,
        )?;
        if let Ok(mut benchmarker) = self.benchmarker_container_id.try_lock() {
            benchmarker.requires_wait_to_stop = true;
            benchmarker.container_id = Some(container_ids.0.clone());
        }
        self.trip();
        start_benchmarker_container(&self.docker_config, test_type, &container_ids, logger)?;
        // This signals that the benchmarker exited naturally on
        // its own, so we don't need to stop its container.
        if let Ok(mut benchmarker) = self.benchmarker_container_id.try_lock() {
            benchmarker.requires_wait_to_stop = false;
            benchmarker.container_id = None;
        }

        Ok(())
    }

    /// Runs the verifier against the given test orchestration and returns the
    /// `Verification` result.
    fn run_verification(
        &mut self,
        project: &Project,
        test: &Test,
        orchestration: &DockerOrchestration,
        test_type: &(&String, &String),
        logger: &Logger,
    ) -> ToolsetResult<Verification> {
        self.trip();
        let container_id =
            create_verifier_container(&self.docker_config, orchestration, test_type)?;

        let container_ids = (container_id, None);

        connect_container_to_network(
            &self.docker_config,
            &self.docker_config.server_docker_host,
            &orchestration.network_id,
            &container_ids,
        )?;

        // This DockerContainerIdFuture is different than the others
        // because it blocks until the verifier exits.
        if let Ok(mut verifier) = self.verifier_container_id.try_lock() {
            verifier.requires_wait_to_stop = true;
            verifier.container_id = Some(container_ids.0.clone());
        }
        self.trip();
        let verification = start_verification_container(
            &self.docker_config,
            project,
            test,
            test_type,
            &container_ids,
            logger,
        )?;
        // This signals that the verifier exited naturally on
        // its own, so we don't need to stop its container.
        if let Ok(mut verifier) = self.verifier_container_id.try_lock() {
            verifier.requires_wait_to_stop = false;
            verifier.container_id = None;
        }

        Ok(verification)
    }

    /// Starts all the underlying docker orchestration required for the given
    /// `Test` to be able to respond to requests and, optionally, communicate
    /// with a database container.
    /// Note: This function blocks the current thread until the test implementation
    /// is able to successfully respond to a request.
    fn start_test_orchestration(
        &mut self,
        project: &Project,
        test: &Test,
        logger: &Logger,
    ) -> ToolsetResult<DockerOrchestration> {
        let network_id = match &self.docker_config.network_mode {
            NetworkMode::Bridge => create_network(&self.docker_config)?,
            NetworkMode::Host => "host".to_string(),
        };

        let database_container_id = self.start_database_if_necessary(test, &network_id)?;
        let database_ports =
            get_database_port_bindings(&self.docker_config, &database_container_id)?;

        let image_id = build_image(&self.docker_config, project, test, logger)?;

        let container_id = create_container(
            &self.docker_config,
            &image_id,
            &network_id,
            &self.docker_config.server_host,
            &self.docker_config.server_docker_host,
        )?;

        let container_ids = (container_id, database_container_id);

        connect_container_to_network(
            &self.docker_config,
            &self.docker_config.server_docker_host,
            &network_id,
            &container_ids,
        )?;

        if let Ok(mut application_container_id) = self.application_container_id.try_lock() {
            application_container_id.requires_wait_to_stop = true;
        }

        self.trip();
        start_container(
            &self.docker_config,
            &container_ids,
            &self.docker_config.server_docker_host,
            logger,
        )?;

        if let Ok(mut application_container_id) = self.application_container_id.try_lock() {
            application_container_id.container_id = Some(container_ids.0.clone());
        }

        let host_ports = get_port_bindings_for_container(
            &self.docker_config,
            &self.docker_config.server_docker_host,
            &container_ids,
        )?;

        if let Err(e) = self.wait_until_accepting_requests(&container_ids, &host_ports.0, test) {
            self.trip();
            return Err(stop_containers_because_of_error(
                &self.docker_config,
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

    /// Sentinel helper for tripping when ctrlc has been pressed. Because the
    /// handler itself is in a separate thread, the main thread can continue
    /// longer than needed starting and stopping containers while the ctrlc
    /// thread is trying to take everything down.
    ///
    /// If, and only if, ctrlc has occurred this function will block forever.
    ///
    /// Note: the expectation is that the ctrlc thread will always exit the
    /// program.
    fn trip(&mut self) {
        if self.ctrlc_received.load(Ordering::Acquire) {
            loop {
                // We may be cleaning up containers on the ctrl-c thread,
                // so sleep forever (the ctrlc handler will exit the program
                // for us eventually.
                thread::sleep(Duration::from_secs(1));
            }
        }
    }

    /// Convenience method for stopping all running containers and popping them
    /// off the running containers vec.
    fn stop_containers(&mut self) -> ToolsetResult<()> {
        stop_docker_container_future(&self.docker_config, &self.verifier_container_id);
        stop_docker_container_future(&self.docker_config, &self.benchmarker_container_id);
        stop_docker_container_future(&self.docker_config, &self.application_container_id);
        stop_docker_container_future(&self.docker_config, &self.database_container_id);

        Ok(())
    }

    /// Starts the database for the given `Test` if one is specified as being
    /// required by the underlying configuration file.
    fn start_database_if_necessary(
        &mut self,
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
                &self.docker_config,
                &format!("tfb.database.{}", database.to_lowercase()),
                network_id,
                &self.docker_config.database_host,
                &self.docker_config.database_docker_host,
            )?;

            let container_ids = (container_id, None);

            connect_container_to_network(
                &self.docker_config,
                &self.docker_config.database_docker_host,
                network_id,
                &container_ids,
            )?;

            let mut logger = Logger::with_prefix(&database);
            logger.quiet = true;

            if let Ok(mut database_container_id) = self.database_container_id.try_lock() {
                database_container_id.requires_wait_to_stop = true;
            }

            self.trip();
            start_container(
                &self.docker_config,
                &container_ids,
                &self.docker_config.database_docker_host,
                &logger,
            )?;

            if let Ok(mut database_container_id) = self.database_container_id.try_lock() {
                database_container_id.container_id = Some(container_ids.0.clone());
            }

            return Ok(Some(container_ids.0));
        }

        Ok(None)
    }

    /// Blocks the current thread until either the operation times out or `Test`
    /// responds successfully (200).
    fn wait_until_accepting_requests(
        &mut self,
        container_ids: &(String, Option<String>),
        host_port: &str,
        test: &Test,
    ) -> ToolsetResult<()> {
        let mut slept_for = 0;
        loop {
            self.trip();
            if slept_for > 60 {
                self.trip();
                stop_container(
                    &container_ids.0,
                    &self.docker_config.server_docker_host,
                    self.docker_config.use_unix_socket,
                    Simple::new(),
                )?;
                if let Some(database_container_id) = &container_ids.1 {
                    self.trip();
                    stop_container(
                        database_container_id,
                        &self.docker_config.database_docker_host,
                        self.docker_config.use_unix_socket,
                        Simple::new(),
                    )?;
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

            match self.docker_config.server_host.as_str() {
                "tfb-server" => easy.url(&format!("http://localhost:{}{}", host_port, endpoint))?,
                _ => easy.url(&format!(
                    "http://{}:{}{}",
                    &self.docker_config.server_host, host_port, endpoint
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
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
    }
}
