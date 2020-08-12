use crate::io::{create_results_dir, Logger};
use crate::options;
use clap::ArgMatches;
use dockurl::network::NetworkMode::{Bridge, Host};

#[derive(Debug, Clone)]
pub struct DockerConfig {
    pub use_unix_socket: bool,
    pub server_docker_host: String,
    pub server_host: String,
    pub database_docker_host: String,
    pub database_host: String,
    pub client_docker_host: String,
    pub client_host: String,
    pub network_mode: dockurl::network::NetworkMode,
    pub concurrency_levels: String,
    pub pipeline_concurrency_levels: String,
    pub query_levels: String,
    pub cached_query_levels: String,
    pub duration: usize,
    pub logger: Logger,
}
impl DockerConfig {
    pub fn new(matches: &ArgMatches) -> Self {
        let server_docker_host = format!(
            "{}:2375",
            matches
                .value_of(options::args::SERVER_DOCKER_HOST)
                .unwrap()
                .to_string()
        );
        let database_docker_host = format!(
            "{}:2375",
            matches
                .value_of(options::args::DATABASE_DOCKER_HOST)
                .unwrap()
                .to_string()
        );
        let client_docker_host = format!(
            "{}:2375",
            matches
                .value_of(options::args::CLIENT_DOCKER_HOST)
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
        let network_mode = match matches.value_of(options::args::NETWORK_MODE).unwrap() {
            options::network_modes::HOST => Host,
            _ => Bridge,
        };
        let duration =
            str::parse::<usize>(matches.value_of(options::args::DURATION).unwrap()).unwrap();
        let concurrency_levels = matches
            .values_of(options::args::CONCURRENCY_LEVELS)
            .unwrap()
            .map(|item| item.to_string())
            .collect::<Vec<String>>()
            .join(",");
        let pipeline_concurrency_levels = matches
            .values_of(options::args::PIPELINE_CONCURRENCY_LEVELS)
            .unwrap()
            .map(|item| item.to_string())
            .collect::<Vec<String>>()
            .join(",");

        let query_levels = matches
            .values_of(options::args::QUERY_LEVELS)
            .unwrap()
            .map(|item| item.to_string())
            .collect::<Vec<String>>()
            .join(",");
        let cached_query_levels = matches
            .values_of(options::args::CACHED_QUERY_LEVELS)
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
            // communicate over TCP (also, Windows can only communicate over
            // TCP as of this writing).
            server_host == options::args::SERVER_HOST_DEFAULT
        };

        let logger = Logger::in_dir(&create_results_dir().unwrap());

        Self {
            use_unix_socket,
            server_docker_host,
            server_host,
            database_docker_host,
            database_host,
            client_docker_host,
            client_host,
            network_mode,
            concurrency_levels,
            pipeline_concurrency_levels,
            logger,
            query_levels, // todo - we don't use these correctly
            cached_query_levels,
            duration,
        }
    }
}
