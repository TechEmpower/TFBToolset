use crate::benchmarker::modes::CICD;
use crate::config::{Framework, Named, Project, Test};
use crate::docker::container::{
    block_until_database_is_ready, create_benchmarker_container, create_container,
    create_database_verifier_container, create_verifier_container, get_port_bindings_for_container,
    start_benchmark_command_retrieval_container, start_benchmarker_container, start_container,
    start_verification_container, stop_docker_container_future,
};
use crate::docker::docker_config::DockerConfig;
use crate::docker::image::{build_image, pull_image};
use crate::docker::listener::benchmarker::BenchmarkResults;
use crate::docker::listener::simple::Simple;
use crate::docker::listener::verifier::Error;
use crate::docker::network::connect_container_to_network;
use crate::docker::{
    BenchmarkCommands, DockerContainerIdFuture, DockerOrchestration, Verification,
};
use crate::error::ToolsetError::{
    AppServerContainerShutDownError, DebugFailedException, NoResponseFromDockerContainerError,
    VerificationFailedException,
};
use crate::error::{ToolsetError, ToolsetResult};
use crate::io::{report_verifications, Logger};
use crate::results::{BenchmarkData, Results};
use colored::Colorize;
use curl::easy::Easy2;
use dockurl::container::inspect_container;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{thread, time};

pub mod modes {
    pub const BENCHMARK: &str = "benchmark";
    pub const VERIFY: &str = "verify";
    pub const CICD: &str = "cicd";
    pub const DEBUG: &str = "debug";
}

pub enum Mode {
    Verify,
    Benchmark,
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
#[derive(Debug)]
pub struct Benchmarker<'a> {
    docker_config: DockerConfig<'a>,
    projects: Vec<Project>,
    application_container_id: Arc<Mutex<DockerContainerIdFuture>>,
    database_container_id: Arc<Mutex<DockerContainerIdFuture>>,
    verifier_container_id: Arc<Mutex<DockerContainerIdFuture>>,
    benchmarker_container_id: Arc<Mutex<DockerContainerIdFuture>>,
    ctrlc_received: Arc<AtomicBool>,
}

impl<'a> Benchmarker<'a> {
    pub fn new(docker_config: DockerConfig<'a>, projects: Vec<Project>, mode: &str) -> Self {
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
            projects,
            application_container_id,
            database_container_id,
            verifier_container_id,
            benchmarker_container_id,
            ctrlc_received: Arc::new(AtomicBool::new(false)),
        };

        if mode != CICD {
            let use_unix_socket = benchmarker.docker_config.use_unix_socket;
            let docker_cleanup = benchmarker.docker_config.clean_up;
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
                    let application_container_id = Arc::clone(&application_container_id);
                    let database_container_id = Arc::clone(&database_container_id);
                    let verifier_container_id = Arc::clone(&verifier_container_id);
                    let benchmarker_container_id = Arc::clone(&benchmarker_container_id);
                    let ctrlc_received = Arc::clone(&ctrlc_received);
                    thread::spawn(move || {
                        ctrlc_received.store(true, Ordering::Release);
                        stop_docker_container_future(
                            use_unix_socket,
                            docker_cleanup,
                            &verifier_container_id,
                        );
                        stop_docker_container_future(
                            use_unix_socket,
                            docker_cleanup,
                            &benchmarker_container_id,
                        );
                        stop_docker_container_future(
                            use_unix_socket,
                            docker_cleanup,
                            &application_container_id,
                        );
                        stop_docker_container_future(
                            use_unix_socket,
                            docker_cleanup,
                            &database_container_id,
                        );
                        std::process::exit(0);
                    });
                }
            })
                .unwrap();
        }

        benchmarker
    }

    /// Iterates over the specified test implementation(s), starts configured
    /// required services (like a database), starts the test implementation,
    /// verifies the configured end-points for each test type, and, if
    /// successful, will benchmark the running test implementation. When
    /// benchmarking completes, the results are parsed and stored in the
    /// results directory for this benchmark.
    pub fn benchmark(&mut self) -> ToolsetResult<()> {
        let mut benchmark_results = Results::new(&self.docker_config)?;
        let logger = self.docker_config.logger.clone();
        logger.log("Pulling verifier; this may take some time.")?;
        // todo - how should we version this?
        pull_image(
            &self.docker_config,
            &self.docker_config.client_docker_host,
            "techempower/tfb.verifier",
        )?;
        let projects = &self.projects.clone();
        for project in projects {
            for test in &project.tests {
                let mut logger = logger.clone();
                logger.set_test(test);
                self.trip();
                match self.start_test_orchestration(project, test, &logger) {
                    Ok(orchestration) => {
                        for test_type in &test.urls {
                            logger.log(format!("Benchmarking: {}", test_type.0))?;
                            match self.run_benchmarks(&orchestration, &test_type, &logger) {
                                Ok(results) => self.report_benchmark_success(
                                    &mut benchmark_results,
                                    results,
                                    &project.framework,
                                    test_type.0,
                                    &logger,
                                ),
                                Err(e) => self.report_benchmark_error(
                                    &mut benchmark_results,
                                    &test,
                                    test_type.0,
                                    &e,
                                    &logger,
                                ),
                            }

                            logger.write_results(&benchmark_results)?;
                            logger.log(format!("Completed benchmarking: {}", test_type.0))?;
                        }
                    }
                    Err(e) => {
                        logger.error(&e)?;
                        // We could not start this implementation's docker
                        // container(s); all of its test implementations must
                        // fail.
                        for test_type in &test.urls {
                            self.report_benchmark_error(
                                &mut benchmark_results,
                                &test,
                                test_type.0,
                                &e,
                                &logger,
                            );
                        }
                    }
                }

                self.trip();
                self.stop_containers();
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
                match self.start_test_orchestration(&project, &test, &logger) {
                    Ok(orchestration) => {
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
                    Err(e) => {
                        logger.error(&e)?;
                        self.stop_containers();
                        return Err(DebugFailedException);
                    }
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
        let projects = &self.projects.clone();
        if projects.is_empty() {
            succeeded = false;
        } else {
            let logger = self.docker_config.logger.clone();
            logger.log("Pulling verifier; this may take some time.")?;
            // todo - how should we version this?
            pull_image(
                &self.docker_config,
                &self.docker_config.client_docker_host,
                "techempower/tfb.verifier",
            )?;
            for project in projects {
                for test in &project.tests {
                    let mut logger = logger.clone();
                    logger.set_test(test);
                    self.trip();
                    match self.start_test_orchestration(project, test, &logger) {
                        Ok(orchestration) => {
                            for test_type in &test.urls {
                                self.trip();
                                match self.run_verification(
                                    &project,
                                    &test,
                                    &orchestration,
                                    &test_type,
                                    &logger,
                                ) {
                                    Ok(verification) => {
                                        succeeded &= verification.errors.is_empty();
                                        verifications.push(verification);
                                    }
                                    Err(e) => {
                                        verifications.push(Verification {
                                            framework_name: project.framework.get_name(),
                                            test_name: test.get_name(),
                                            type_name: String::default(),
                                            warnings: Vec::default(),
                                            errors: vec![Error {
                                                message: format!("{:?}", e),
                                                short_message: "Failed to Verify".to_string(),
                                            }],
                                        });
                                        succeeded = false;
                                        self.trip();
                                        self.stop_containers();
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            logger.error(&e)?;
                            verifications.push(Verification {
                                framework_name: project.framework.get_name(),
                                test_name: test.get_name(),
                                type_name: String::default(),
                                warnings: Vec::default(),
                                errors: vec![Error {
                                    message: format!("{:?}", e),
                                    short_message: "Failed to Start".to_string(),
                                }],
                            });
                            succeeded = false;
                            self.trip();
                            self.stop_containers();
                        }
                    };

                    self.trip();
                    self.stop_containers();
                }
            }

            self.trip();
            self.stop_containers();
            report_verifications(verifications, logger)?;
        }

        if succeeded {
            Ok(())
        } else {
            Err(VerificationFailedException)
        }
    }
}

//
// PRIVATES
//
impl<'a> Benchmarker<'a> {
    /// Runs the benchmarks for a given `DockerOrchestration` and `test_type`.
    fn run_benchmarks(
        &mut self,
        orchestration: &DockerOrchestration,
        test_type: &(&String, &String),
        logger: &Logger,
    ) -> ToolsetResult<Vec<BenchmarkResults>> {
        let mut results = Vec::default();
        let mut logger = logger.clone();
        logger.set_log_file(&format!("{}.txt", test_type.0));
        logger.quiet = true;
        let benchmark_commands = self.run_command_retrieval(&orchestration, &test_type, &logger)?;

        logger.log("---------------------------------------------------------")?;
        logger.log(" Running Primer")?;
        logger.log(format!(
            "   {}",
            &benchmark_commands.primer_command.join(" ")
        ))?;
        logger.log("---------------------------------------------------------")?;
        self.run_benchmark(&benchmark_commands.primer_command, &logger)?;

        logger.log("---------------------------------------------------------")?;
        logger.log(" Running Warmup")?;
        logger.log(format!(
            "   {}",
            &benchmark_commands.warmup_command.join(" ")
        ))?;
        logger.log("---------------------------------------------------------")?;
        self.run_benchmark(&benchmark_commands.warmup_command, &logger)?;

        for command in &benchmark_commands.benchmark_commands {
            logger.log("---------------------------------------------------------")?;
            logger.log(format!(" {}", command.join(" ")))?;
            logger.log("---------------------------------------------------------")?;

            results.push(self.run_benchmark(command, &logger)?);
        }

        Ok(results)
    }

    /// Runs the benchmarker container against the given `DockerOrchestration`.
    fn run_benchmark(
        &mut self,
        command: &[String],
        logger: &Logger,
    ) -> ToolsetResult<BenchmarkResults> {
        let container_id = create_benchmarker_container(&self.docker_config, command)?;

        connect_container_to_network(
            &self.docker_config,
            &self.docker_config.client_docker_host,
            &self.docker_config.client_network_id,
            &container_id,
        )?;

        if let Ok(mut benchmarker) = self.benchmarker_container_id.lock() {
            benchmarker.register(&container_id);
        }

        self.trip();
        let benchmark_results =
            start_benchmarker_container(&self.docker_config, &container_id, logger)?;

        // This signals that the benchmarker exited naturally on
        // its own, so we don't need to stop its container.
        if let Ok(mut benchmarker) = self.benchmarker_container_id.lock() {
            benchmarker.unregister();
        }

        Ok(benchmark_results)
    }

    /// Reports the successful benchmark of a given `framework` / `test_type`
    /// via `results.json` output.
    fn report_benchmark_success(
        &self,
        benchmark_results: &mut Results,
        results: Vec<BenchmarkResults>,
        framework: &Framework,
        test_type: &str,
        _logger: &Logger,
    ) {
        for result in results {
            if benchmark_results.raw_data.get(test_type).is_none() {
                benchmark_results
                    .raw_data
                    .insert(test_type.to_string(), HashMap::default());
            }
            if let Some(test_type) = benchmark_results.raw_data.get_mut(test_type) {
                if test_type
                    .get(&framework.get_name().to_lowercase())
                    .is_none()
                {
                    test_type.insert(framework.get_name().to_lowercase(), Vec::default());
                }

                if let Some(results) = test_type.get_mut(&framework.get_name().to_lowercase()) {
                    results.push(BenchmarkData {
                        latency_avg: result.thread_stats.latency.average,
                        latency_max: result.thread_stats.latency.max,
                        latency_stdev: result.thread_stats.latency.standard_deviation,
                        total_requests: result.total_requests,
                        start_time: result.start_time,
                        end_time: result.end_time,
                    });
                }
            }
        }
        if benchmark_results.succeeded.get(test_type).is_none() {
            benchmark_results
                .succeeded
                .insert(test_type.to_string(), Vec::default());
        }
        if let Some(test_type) = benchmark_results.succeeded.get_mut(test_type) {
            test_type.push(framework.get_name().to_lowercase());
        }
        benchmark_results.completed.insert(
            framework.get_name().to_lowercase(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()
                .to_string(),
        );
    }

    /// Reports the unsuccessful benchmark of a given `test` / `test_type` via
    /// `results.json` output.
    fn report_benchmark_error(
        &self,
        benchmark_results: &mut Results,
        test: &Test,
        test_type: &str,
        _error: &ToolsetError,
        _logger: &Logger,
    ) {
        if benchmark_results.failed.get(test_type).is_none() {
            benchmark_results
                .failed
                .insert(test_type.to_string(), Vec::default());
        }
        if let Some(test_type) = benchmark_results.failed.get_mut(test_type) {
            test_type.push(test.get_name());
        }
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
            create_verifier_container(&self.docker_config, orchestration, Mode::Verify, test_type)?;

        connect_container_to_network(
            &self.docker_config,
            &self.docker_config.client_docker_host,
            &self.docker_config.client_network_id,
            &container_id,
        )?;

        // This DockerContainerIdFuture is different than the others
        // because it blocks until the verifier exits.
        if let Ok(mut verifier) = self.verifier_container_id.lock() {
            verifier.register(&container_id);
        }
        self.trip();
        let verification = start_verification_container(
            &self.docker_config,
            project,
            test,
            test_type,
            &container_id,
            logger,
        )?;

        // This signals that the verifier exited naturally on
        // its own, so we don't need to stop its container.
        if let Ok(mut verifier) = self.verifier_container_id.lock() {
            verifier.unregister();
        }

        Ok(verification)
    }

    /// Requests the verifier to start for the purposes of retrieving the run
    /// commands for the purposes of benchmarking.
    /// In practice, this will retrieve, for some test type, a `wrk` command to
    /// run on the client.
    fn run_command_retrieval(
        &mut self,
        orchestration: &DockerOrchestration,
        test_type: &(&String, &String),
        logger: &Logger,
    ) -> ToolsetResult<BenchmarkCommands> {
        self.trip();
        let container_id = create_verifier_container(
            &self.docker_config,
            orchestration,
            Mode::Benchmark,
            test_type,
        )?;

        connect_container_to_network(
            &self.docker_config,
            &self.docker_config.client_docker_host,
            &self.docker_config.client_network_id,
            &container_id,
        )?;

        // This DockerContainerIdFuture is different than the others
        // because it blocks until the verifier exits.
        if let Ok(mut verifier) = self.verifier_container_id.lock() {
            verifier.register(&container_id);
        }
        self.trip();
        let commands = start_benchmark_command_retrieval_container(
            &self.docker_config,
            &test_type,
            &container_id,
            logger,
        )?;
        // This signals that the verifier exited naturally on
        // its own, so we don't need to stop its container.
        if let Ok(mut verifier) = self.verifier_container_id.lock() {
            verifier.unregister();
        }

        Ok(commands)
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
        let database_container_id = self.start_database_if_necessary(test)?;
        let mut database_ports = (None, None);
        if let Some(container_id) = &database_container_id {
            let ports = get_port_bindings_for_container(
                &self.docker_config,
                &self.docker_config.database_docker_host,
                container_id,
            )?;
            database_ports = (Some(ports.0), Some(ports.1));
        }

        let image_id = build_image(&self.docker_config, project, test, logger)?;

        if let Ok(mut application_container_id) = self.application_container_id.lock() {
            application_container_id.image_id(&image_id);
        }

        let container_id = create_container(
            &self.docker_config,
            &image_id,
            &self.docker_config.server_network_id,
            &self.docker_config.server_host,
            &self.docker_config.server_docker_host,
        )?;

        let container_ids = (container_id.clone(), database_container_id);

        connect_container_to_network(
            &self.docker_config,
            &self.docker_config.server_docker_host,
            &self.docker_config.server_network_id,
            &container_id,
        )?;

        if let Ok(mut application_container_id) = self.application_container_id.lock() {
            application_container_id.register(&container_id);
        }

        self.trip();
        start_container(
            &self.docker_config,
            &container_id,
            &self.docker_config.server_docker_host,
            logger,
        )?;

        let host_ports = get_port_bindings_for_container(
            &self.docker_config,
            &self.docker_config.server_docker_host,
            &container_id,
        )?;

        self.wait_until_accepting_requests(&container_ids, &host_ports.0, test)?;

        Ok(DockerOrchestration {
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
    fn stop_containers(&mut self) {
        stop_docker_container_future(
            self.docker_config.use_unix_socket,
            self.docker_config.clean_up,
            &self.verifier_container_id,
        );
        stop_docker_container_future(
            self.docker_config.use_unix_socket,
            self.docker_config.clean_up,
            &self.benchmarker_container_id,
        );
        stop_docker_container_future(
            self.docker_config.use_unix_socket,
            self.docker_config.clean_up,
            &self.application_container_id,
        );
        stop_docker_container_future(
            self.docker_config.use_unix_socket,
            self.docker_config.clean_up,
            &self.database_container_id,
        );
    }

    /// Starts the database for the given `Test` if one is specified as being
    /// required by the underlying configuration file.
    fn start_database_if_necessary(&mut self, test: &Test) -> ToolsetResult<Option<String>> {
        if let Some(database) = &test.database {
            let mut logger = Logger::with_prefix(&database);
            let image_name = format!("techempower/tfb.database.{}", database.to_lowercase());
            logger.log(format!("Pulling {}; this may take some time.", &image_name))?;
            pull_image(
                &self.docker_config,
                &self.docker_config.database_docker_host,
                &image_name,
            )?;

            let container_id = create_container(
                &self.docker_config,
                &image_name,
                &self.docker_config.database_network_id,
                &self.docker_config.database_host,
                &self.docker_config.database_docker_host,
            )?;

            connect_container_to_network(
                &self.docker_config,
                &self.docker_config.database_docker_host,
                &self.docker_config.database_network_id,
                &container_id,
            )?;

            logger.quiet = true;

            if let Ok(mut database_container_id) = self.database_container_id.lock() {
                database_container_id.register(&container_id);
            }

            self.trip();
            start_container(
                &self.docker_config,
                &container_id,
                &self.docker_config.database_docker_host,
                &logger,
            )?;

            // Block until the database is accepting requests.
            self.trip();
            let verifier_container_id =
                create_database_verifier_container(&self.docker_config, &database.to_lowercase())?;

            connect_container_to_network(
                &self.docker_config,
                &self.docker_config.client_docker_host,
                &self.docker_config.client_network_id,
                &verifier_container_id,
            )?;

            // This DockerContainerIdFuture is different than the others
            // because it blocks until the verifier exits.
            if let Ok(mut verifier) = self.verifier_container_id.lock() {
                verifier.register(&verifier_container_id);
            }
            self.trip();

            block_until_database_is_ready(&self.docker_config, &verifier_container_id)?;

            // This signals that the verifier exited naturally on
            // its own, so we don't need to stop its container.
            if let Ok(mut verifier) = self.verifier_container_id.lock() {
                verifier.unregister();
            }

            return Ok(Some(container_id));
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
            let inspect = inspect_container(
                &container_ids.0,
                &self.docker_config.server_docker_host,
                self.docker_config.use_unix_socket,
                Simple::new(),
            )?;
            if !inspect.state.running {
                return Err(AppServerContainerShutDownError);
            }
            self.trip();
            if slept_for > 60 {
                self.trip();
                self.stop_containers();

                return Err(NoResponseFromDockerContainerError);
            }
            let mut easy = Easy2::new(Simple::new());

            let mut endpoint = String::new();
            if let Some(key) = test.urls.keys().next() {
                if let Some(_endpoint) = test.urls.get(key) {
                    endpoint = _endpoint.clone();
                }
            }

            let url = match self.docker_config.server_host {
                "tfb-server" => format!("http://localhost:{}{}", host_port, endpoint),
                _ => format!(
                    "http://{}:{}{}",
                    &self.docker_config.server_host, host_port, endpoint
                ),
            };
            easy.url(&url)?;
            easy.timeout(time::Duration::from_secs(1))?;
            let _ = easy.perform();

            if let Ok(code) = easy.response_code() {
                if code > 0 {
                    return Ok(());
                }
            }
            slept_for += 1;
            thread::sleep(Duration::from_secs(1));
        }
    }
}
