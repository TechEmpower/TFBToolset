use std::io;

use thiserror::Error;

pub type ToolsetResult<T> = Result<T, ToolsetError>;

#[derive(Error, Debug)]
pub enum ToolsetError {
    #[error("Dockurl Error: {0}")]
    DockerError(#[from] dockurl::error::DockerError),

    #[error("Curl error occurred")]
    CurlError(#[from] curl::Error),

    #[error("IO error occurred")]
    IoError(#[from] io::Error),

    #[error("Toml deserialize error occurred")]
    TomlDeserializeError(#[from] toml::de::Error),

    #[error("Toml serialize error occurred")]
    TomlSerializeError(#[from] toml::ser::Error),

    #[error("Invalid config.toml: {0}, {1}")]
    InvalidConfigError(String, toml::de::Error),

    #[error("Serde json error")]
    SerdeJsonError(#[from] serde_json::error::Error),

    #[error("Language not found for config file: {0}; {1}")]
    LanguageNotFoundError(String, String),

    #[error("CtrlC Error occurred")]
    CtrlCError(#[from] ctrlc::Error),

    #[error("Invalid FrameworkBenchmarks directory: {0}")]
    InvalidFrameworkBenchmarksDirError(String),

    #[error("Docker Container did not respond")]
    NoResponseFromDockerContainerError,

    #[error("Unknown benchmarker mode: {0}")]
    UnknownBenchmarkerModeError(String),

    #[error("Debug failed")]
    DebugFailedException,

    #[error("Verification failed")]
    VerificationFailedException,

    #[error("Application server container shut down after start")]
    AppServerContainerShutDownError,

    #[error("Failed to inspect container for port mappings")]
    ContainerPortMappingInspectionError,

    #[error("Dockerfile must expose port")]
    ExposePortError,

    #[error("Failed to retrieve benchmark commands")]
    FailedBenchmarkCommandRetrievalError,

    #[error("Failed to parse benchmark results")]
    BenchmarkDataParseError,
}
