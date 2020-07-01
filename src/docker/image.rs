use crate::config::{Named, Project, Test};
use crate::docker::container::stop_container;
use crate::docker::docker_config::DockerConfig;
use crate::docker::listener::build_image::BuildImage;
use crate::error::ToolsetError::{
    DockerImageCreateError, DockerImagePullError, FailedToCreateDockerImageError,
    FailedToPullDockerImageError,
};
use crate::error::ToolsetResult;
use crate::io::Logger;
use curl::easy::{Easy2, List};
use std::io::{Error, Write};

/// Simple helper for housing a tarball in a buffer. We just want the bytes
/// and this keeps us from writing to disk.
struct Tarchive(Vec<u8>);
impl Tarchive {
    fn buffer(&mut self) -> &[u8] {
        self.0.as_slice()
    }
}
impl Write for Tarchive {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), Error> {
        // ¯\_(ツ)_/¯
        Ok(())
    }
}

/// Pulls an image given by `image_name` into the daemon's registry.
pub fn pull_image(config: &DockerConfig, image_name: &str, logger: &Logger) -> ToolsetResult<()> {
    let query_string = format!("?fromImage={}&tag=latest", image_name);

    let mut easy = Easy2::new(BuildImage::new(logger));
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    easy.post(true)?;
    easy.url(&format!(
        "http://{}/images/create{}",
        config.docker_host, query_string
    ))?;
    easy.perform()?;

    match easy.response_code() {
        Ok(code) => match code {
            200 => Ok(()),
            _ => {
                let error_message = &easy.get_ref().error_message;
                if error_message.is_some() {
                    return Err(FailedToPullDockerImageError(error_message.clone().unwrap()));
                }
                Err(DockerImagePullError)
            }
        },
        Err(e) => Err(FailedToPullDockerImageError(e.to_string())),
    }
}

/// Takes a `framework_dir` and the `Test` to run and instructs docker to
/// build the image.
pub fn build_image(
    docker_config: &DockerConfig,
    project: &Project,
    test: &Test,
    database_container_id: &Option<String>,
    logger: &Logger,
) -> ToolsetResult<String> {
    match build_image_unsafe(docker_config, project, test, logger) {
        Ok(id) => Ok(id),
        Err(e) => {
            if let Some(container_id) = &database_container_id {
                stop_container(&docker_config, &container_id)?;
            }
            Err(e)
        }
    }
}

//
// PRIVATES
//

/// Takes a `framework_dir` and the `Test` to run and instructs docker to
/// build the image.
fn build_image_unsafe(
    config: &DockerConfig,
    project: &Project,
    test: &Test,
    logger: &Logger,
) -> ToolsetResult<String> {
    let mut tarchive = Tarchive(Vec::new());
    let mut tar = tar::Builder::new(&mut tarchive);
    tar.append_dir_all("", project.get_path()?.to_str().unwrap())?;
    tar.finish()?;

    let mut dockerfile;
    if test.dockerfile.is_some() {
        dockerfile = test.clone().dockerfile.unwrap();
    } else {
        dockerfile = test.get_name();
        dockerfile.push_str(".dockerfile");
    }

    let query_string = format!("?dockerfile={}&t={}", dockerfile, test.get_tag());
    let mut headers = List::new();
    headers.append("Content-Type: application/x-tar")?;
    let bytes = tar.get_mut().buffer();
    let len = bytes.len();

    let mut easy = Easy2::new(BuildImage::new(logger));
    if config.use_unix_socket {
        easy.unix_socket("/var/run/docker.sock")?;
    }

    easy.post(true)?;
    easy.http_headers(headers)?;
    easy.in_filesize(len as u64)?;
    easy.post_field_size(len as u64)?;
    easy.url(&format!(
        "http://{}/build{}",
        config.docker_host, query_string
    ))?;
    easy.post_fields_copy(bytes)?;
    easy.perform()?;

    match easy.response_code() {
        Ok(code) => match code {
            200 => {
                let image_id = &easy.get_ref().image_id;
                if image_id.is_some() {
                    return Ok(image_id.clone().unwrap());
                }
                Err(DockerImageCreateError)
            }
            _ => {
                let error_message = &easy.get_ref().error_message;
                if error_message.is_some() {
                    return Err(FailedToCreateDockerImageError(
                        error_message.clone().unwrap(),
                    ));
                }
                Err(DockerImageCreateError)
            }
        },
        Err(e) => Err(FailedToCreateDockerImageError(e.to_string())),
    }
}
