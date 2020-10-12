use crate::config::{Named, Project, Test};
use crate::docker::docker_config::DockerConfig;
use crate::docker::listener::build_image::BuildImage;
use crate::docker::listener::simple::Simple;
use crate::error::ToolsetError::DockerError;
use crate::error::ToolsetResult;
use crate::io::Logger;
use std::path::PathBuf;

/// Takes a `framework_dir` and the `Test` to run and instructs docker to
/// build the image.
pub fn build_image(
    config: &DockerConfig,
    project: &Project,
    test: &Test,
    logger: &Logger,
) -> ToolsetResult<String> {
    let mut dockerfile;
    if test.dockerfile.is_some() {
        dockerfile = test.dockerfile.clone().unwrap();
    } else {
        dockerfile = test.get_name();
        dockerfile.push_str(".dockerfile");
    }

    let image_id = dockurl::image::build_image(
        &test.get_tag(),
        &PathBuf::from(dockerfile),
        &project.get_path()?,
        &config.server_docker_host,
        config.use_unix_socket,
        BuildImage::new(logger),
    )?;

    Ok(image_id)
}

/// Pulls the given `image_name`.
pub fn pull_image(config: &DockerConfig, docker_host: &str, image_name: &str) -> ToolsetResult<()> {
    match dockurl::image::create_image(
        image_name,
        "latest",
        docker_host,
        config.use_unix_socket,
        Simple::new(),
    ) {
        Ok(()) => Ok(()),
        Err(e) => Err(DockerError(e)),
    }
}
