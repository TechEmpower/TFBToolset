use std::io;

use thiserror::Error;

pub type ToolsetResult<T> = Result<T, ToolsetError>;

#[derive(Error, Debug)]
pub enum ToolsetError {
    #[error("Curl error occurred")]
    CurlError(#[from] curl::Error),

    #[error("IO error occurred")]
    IoError(#[from] io::Error),

    #[error("Toml deserialize error occurred")]
    TomlDeserializeError(#[from] toml::de::Error),

    #[error("Toml serialize error occurred")]
    TomlSerializeError(#[from] toml::ser::Error),

    #[error("Serde json error")]
    SerdeJsonError(#[from] serde_json::error::Error),

    #[error("Language not found for config file")]
    LanguageNotFoundError,

    #[error("CtrlC Error occurred")]
    CtrlCError(#[from] ctrlc::Error),

    #[error("Invalid FrameworkBenchmarks directory")]
    InvalidFrameworkBenchmarksDirError,

    #[error("Error creating Docker Image: {0}")]
    FailedToCreateDockerImageError(String),

    #[error("Error creating Docker Image")]
    DockerImageCreateError,

    #[error("Error pulling Docker Image: {0}")]
    _FailedToPullDockerImageError(String),

    #[error("Error pulling Docker Image")]
    _DockerImagePullError,

    #[error("Error creating Docker Container: {0}")]
    FailedToCreateDockerContainerError(String),

    #[error("Error creating Docker Container")]
    DockerContainerCreateError,

    #[error("Error creating Docker Container: {0}")]
    FailedToCreateDockerVerifierContainerError(String),

    #[error("Error creating Docker Container")]
    DockerVerifierContainerCreateError,

    #[error("Error creating Docker Network: {0}")]
    FailedToCreateDockerNetworkError(String),

    #[error("Error creating Docker Network")]
    DockerNetworkCreateError,

    #[error("Error deleting Docker Network: {0}")]
    FailedToDeleteNetworkError(String),

    #[error("Error deleting Docker Network")]
    DockerNetworkDeleteError,

    #[error("Error attaching Docker Container to Network: {0}")]
    FailedToAttachDockerContainerToNetworkError(String),

    #[error("Error attaching Docker Container to Network")]
    DockerAttachContainerToNetworkError,

    #[error("Docker Container did not respond")]
    NoResponseFromDockerContainerError,

    #[error("Error starting Docker Container: {0}; response code {1}")]
    FailedToStartDockerContainerError(String, u32),

    #[error("Error starting Docker Container; response code {0}")]
    DockerContainerStartError(u32),

    #[error("Error killing Docker Container: {0}")]
    FailedToKillDockerContainerError(String),

    #[error("Error inspecting Docker Container: {0}")]
    FailedToInspectDockerContainerError(String),

    #[error("Unknown benchmarker mode: {0}")]
    UnknownBenchmarkerModeError(String),

    #[error("Verification failed")]
    VerificationFailedException,
}
