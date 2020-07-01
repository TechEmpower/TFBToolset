use clap::ArgMatches;

use crate::config::{Named, Project};
use crate::docker::container::stop_containers;
use crate::docker::docker_config::DockerConfig;
use crate::docker::image::pull_image;
use crate::docker::start_test_orchestration;
use crate::docker::verification::verify;
use crate::error::ToolsetResult;
use crate::io::{report_verifications, Logger};
use crate::metadata;
use colored::Colorize;
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
}
impl Benchmarker {
    pub fn new(matches: ArgMatches) -> Self {
        Self {
            docker_config: DockerConfig::new(&matches),
            projects: metadata::list_projects_to_run(&matches),
        }
    }

    /// Iterates over the specified test implementation(s), starts configured
    /// required services (like a database), starts the test implementation,
    /// verifies the configured end-points for each test type, and, if
    /// successful, will benchmark the running test implementation. When
    /// benchmarking completes, the results are parsed and stored in the
    /// results directory for this benchmark.
    pub fn benchmark(&self) -> ToolsetResult<()> {
        // todo - listener needs real logs
        let logger = Logger::default();
        pull_image(&self.docker_config, "hello-world", &logger)?;

        Ok(())
    }

    /// Starts the given test implementation as a running server and waits
    /// indefinitely. This is useful for locally debugging why your service may
    /// not be responding correctly and failing verification, for example.
    pub fn debug(&self) -> ToolsetResult<()> {
        // Because it makes no sense to loop over all the specified tests when
        // the first test found will cause the main thread to sleep forever, we
        // just check *that* there is a test to run and start it.
        if let Some(project) = self.projects.get(0) {
            if let Some(test) = project.tests.get(0) {
                let logger = Logger::with_prefix(&test.get_name());
                let orchestration =
                    start_test_orchestration(&self.docker_config, &project, &test, &logger)?;
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
    pub fn verify(&self) -> ToolsetResult<()> {
        let mut verifications = Vec::new();
        let logger = self.docker_config.logger.clone();
        for project in &self.projects {
            for test in &project.tests {
                let mut logger = logger.clone();
                logger.set_test(test);
                let orchestration =
                    start_test_orchestration(&self.docker_config, project, test, &logger)?;
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
                stop_containers(&self.docker_config, &orchestration)?;
            }
        }

        report_verifications(verifications, logger)?;

        Ok(())
    }
}
