use crate::docker::network::NetworkMode;
use crate::io::{create_results_dir, Logger};
use crate::options;
use clap::ArgMatches;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct DockerConfig {
    pub use_unix_socket: bool,
    pub docker_host: String,
    pub server_host: String,
    pub database_host: String,
    pub client_host: String,
    pub network_mode: NetworkMode,
    pub concurrency_levels: String,
    pub logger: Logger,
}
impl DockerConfig {
    pub fn new(matches: &ArgMatches) -> Self {
        let docker_host = format!(
            "{}:2375",
            matches
                .value_of(options::args::DOCKER_HOST)
                .unwrap()
                .to_string()
        );
        let server_host = matches
            .value_of(options::args::SERVER_HOST)
            .unwrap()
            .to_string();
        let database_host = matches
            .value_of(options::args::DATABASE_HOST)
            .unwrap()
            .to_string();
        let client_host = matches
            .value_of(options::args::CLIENT_HOST)
            .unwrap()
            .to_string();
        let network_mode =
            NetworkMode::from_str(matches.value_of(options::args::NETWORK_MODE).unwrap()).unwrap();
        let concurrency_levels = matches
            .values_of(options::args::CONCURRENCY_LEVELS)
            .unwrap()
            .map(|item| item.to_string())
            .collect::<Vec<String>>()
            .join(",");

        // By default, we communicate with docker over a unix socket.
        let use_unix_socket = if cfg!(windows) {
            // Even if we want to run locally, Windows cannot communicate over a
            // Unix socket, so don't bother or cURL will panic.
            false
        } else {
            // However, in benchmarking with a multi-machine setup, we want to
            // communicate over TCP.
            server_host == options::args::SERVER_HOST_DEFAULT
        };

        let logger = Logger::in_dir(&create_results_dir().unwrap());

        Self {
            use_unix_socket,
            docker_host,
            server_host,
            database_host,
            client_host,
            network_mode,
            concurrency_levels,
            logger,
        }
    }
}
