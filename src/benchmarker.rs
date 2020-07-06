use clap::ArgMatches;

use crate::config::{Named, Project, Test};
use crate::docker::container::{
    create_container, get_database_port_bindings, get_port_bindings_for_container, start_container,
    stop_container, stop_containers_because_of_error,
};
use crate::docker::docker_config::DockerConfig;
use crate::docker::image::{build_image, pull_image};
use crate::docker::listener::simple::Simple;
use crate::docker::network::{connect_container_to_network, create_network, NetworkMode};
use crate::docker::verification::verify;
use crate::docker::DockerOrchestration;
use crate::error::ToolsetError::NoResponseFromDockerContainerError;
use crate::error::ToolsetResult;
use crate::io::{report_verifications, Logger};
use crate::metadata;
use colored::Colorize;
use curl::easy::Easy2;
use std::sync::{Arc, Mutex};
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
    application_container_id: Arc<Mutex<String>>,
    dependency_container_ids: Arc<Mutex<Vec<String>>>,
}
impl Benchmarker {
    pub fn new(matches: ArgMatches) -> Self {
        let benchmarker = Self {
            docker_config: DockerConfig::new(&matches),
            projects: metadata::list_projects_to_run(&matches),
            application_container_id: Arc::new(Mutex::new(String::new())),
            dependency_container_ids: Arc::new(Mutex::new(Vec::new())),
        };

        let docker_config = benchmarker.docker_config.clone();
        let application_container_id = Arc::clone(&benchmarker.application_container_id);
        let dependency_container_ids = Arc::clone(&benchmarker.dependency_container_ids);
        ctrlc::set_handler(move || {
            let logger = Logger::default();
            logger.log("Shutting down (may take a moment)").unwrap();
            // We `unwrap_or` instead of matching because logging occurs in the
            // `stop_container` function, and we need to continue trying to stop
            // every `container_id` we started.
            stop_container(&docker_config, &**application_container_id.lock().unwrap())
                .unwrap_or(());
            let guard = dependency_container_ids.lock().unwrap();
            for container_id in guard.iter() {
                stop_container(&docker_config, container_id).unwrap_or(());
            }

            std::process::exit(0);
        })
        .unwrap();

        benchmarker
    }

    /// Starts all the underlying docker orchestration required for the given
    /// `Test` to be able to respond to requests and, optionally, communicate
    /// with a database container.
    /// Note: This function blocks the current thread until the test implementation
    ///  is able to successfully respond to a request.
    pub fn start_test_orchestration(
        &mut self,
        project: &Project,
        test: &Test,
        logger: &Logger,
    ) -> ToolsetResult<DockerOrchestration> {
        let network_id = match &self.docker_config.network_mode {
            NetworkMode::Bridge => create_network(&self.docker_config, test)?,
            NetworkMode::Host => "host".to_string(),
        };

        let database_container_id = self.start_database_if_necessary(test, &network_id)?;
        let database_ports =
            get_database_port_bindings(&self.docker_config, &database_container_id)?;

        let image_id = build_image(
            &self.docker_config,
            project,
            test,
            &database_container_id,
            &logger,
        )?;

        let container_id = create_container(
            &self.docker_config,
            &image_id,
            &network_id,
            &self.docker_config.server_host,
            &database_container_id,
        )?;

        let container_ids = (container_id, database_container_id);

        connect_container_to_network(&self.docker_config, &network_id, &container_ids)?;

        start_container(&self.docker_config, &container_ids, &logger)?;

        let host_ports = get_port_bindings_for_container(&self.docker_config, &container_ids)?;

        if let Err(e) = self.wait_until_accepting_requests(&container_ids, &host_ports.0, test) {
            return Err(stop_containers_because_of_error(
                &self.docker_config,
                &container_ids,
                e,
            ));
        };

        // We have started a container and need to push its id.
        // Note: If a database container was started, its id should already be
        // pushed.
        let mut guard = self.application_container_id.lock().unwrap();
        guard.push_str(&container_ids.0);

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

    /// Iterates over the specified test implementation(s), starts configured
    /// required services (like a database), starts the test implementation,
    /// verifies the configured end-points for each test type, and, if
    /// successful, will benchmark the running test implementation. When
    /// benchmarking completes, the results are parsed and stored in the
    /// results directory for this benchmark.
    pub fn benchmark(&mut self) -> ToolsetResult<()> {
        // todo - listener needs real logs
        let logger = Logger::default();
        pull_image(&self.docker_config, "hello-world", &logger)?;

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
                    thread::sleep(time::Duration::from_secs(1));
                }
            }
        }

        Ok(())
    }

    /// Attempts to run the suite of verifications against the specified
    /// test implementation(s).
    pub fn verify(&mut self) -> ToolsetResult<()> {
        let mut verifications = Vec::new();
        let logger = self.docker_config.logger.clone();
        let projects = &self.projects.clone();
        for project in projects {
            for test in &project.tests {
                let mut logger = logger.clone();
                logger.set_test(test);
                let orchestration = self.start_test_orchestration(project, test, &logger)?;
                for test_type in &test.urls {
                    let verification = verify(
                        &self.docker_config,
                        project,
                        test,
                        &test_type,
                        &orchestration,
                        &logger,
                    )?;
                    verifications.push(verification);
                }
                self.stop_containers()?;
            }
        }

        report_verifications(verifications, logger)?;

        Ok(())
    }

    //
    // PRIVATES
    //

    /// Convenience method for stopping all running containers and popping them
    /// off the running containers vec.
    fn stop_containers(&mut self) -> ToolsetResult<()> {
        let mut application_guard = self.application_container_id.lock().unwrap();
        // We `unwrap_or` instead of matching because logging occurs in the
        // `stop_container` function, and we need to continue trying to stop
        // every `container_id` we started.
        stop_container(&self.docker_config, &application_guard).unwrap_or(());
        // Little hack to "empty" the underlying string.
        while let Some(_0) = application_guard.pop() {}

        let mut guard = self.dependency_container_ids.lock().unwrap();
        while let Some(container_id) = guard.pop() {
            stop_container(&self.docker_config, &container_id).unwrap_or(());
        }

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
                &None,
            )?;

            let container_ids = (container_id, None);

            connect_container_to_network(&self.docker_config, network_id, &container_ids)?;

            let mut logger = Logger::with_prefix(&database);
            logger.quiet = true;

            start_container(&self.docker_config, &container_ids, &logger)?;

            // We have started a container with a known id; need to push it.
            let mut guard = self.dependency_container_ids.lock().unwrap();
            guard.push(container_ids.0.clone());

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
            if slept_for > 60 {
                stop_container(&self.docker_config, &container_ids.0)?;
                if let Some(database_container_id) = &container_ids.1 {
                    stop_container(&self.docker_config, &database_container_id)?;
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
                    thread::sleep(time::Duration::from_secs(1));
                }
            }
        }
    }
}
