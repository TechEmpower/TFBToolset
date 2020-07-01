use crate::config::{Project, Test};
use crate::docker::container::{create_verifier_container, start_verification_container};
use crate::docker::docker_config::DockerConfig;
use crate::docker::listener::verifier::{Error, Warning};
use crate::docker::network::connect_container_to_network;
use crate::docker::DockerOrchestration;
use crate::error::ToolsetResult;
use crate::io::Logger;

#[derive(Clone)]
pub struct Verification {
    pub framework_name: String,
    pub test_name: String,
    pub type_name: String,
    pub warnings: Vec<Warning>,
    pub errors: Vec<Error>,
}

/// Runs the given `Test` and `test_type` against a published verifier.
/// Note: this should only be called after `start_test_orchestration`.
pub fn verify(
    config: &DockerConfig,
    project: &Project,
    test: &Test,
    test_type: &(&String, &String),
    orchestration: &DockerOrchestration,
    logger: &Logger,
) -> ToolsetResult<Verification> {
    let container_id = create_verifier_container(config, orchestration, test_type)?;

    let container_ids = (container_id, None);

    connect_container_to_network(config, &orchestration.network_id, &container_ids)?;

    start_verification_container(config, project, test, test_type, &container_ids, logger)
}
