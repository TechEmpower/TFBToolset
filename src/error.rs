use std::io;

use thiserror::Error;

pub type ToolsetResult<T> = Result<T, ToolsetError>;

#[derive(Error, Debug)]
pub enum ToolsetError {
    #[error("Dockurl Error")]
    DockerError(#[from] dockurl::error::DockerError),

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

    #[error("Docker Container did not respond")]
    NoResponseFromDockerContainerError,

    #[error("Unknown benchmarker mode: {0}")]
    UnknownBenchmarkerModeError(String),

    #[error("Verification failed")]
    VerificationFailedException,

    #[error("Failed to inspect container for port mappings")]
    ContainerPortMappingInspectionError,

    #[error("Failed to retrieve benchmark commands")]
    FailedBenchmarkCommandRetrievalError,
}
