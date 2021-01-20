use crate::benchmarker::modes;
use crate::docker::network::{get_network_id, get_tfb_network_id};
use crate::io::{create_results_dir, Logger};
use crate::options;
use dockurl::network::NetworkMode::{Bridge, Host};

#[derive(Debug, Clone)]
pub struct DockerConfig<'a> {
    pub use_unix_socket: bool,
    pub server_docker_host: String,
    pub server_host: &'a str,
    pub server_network_id: String,
    pub database_docker_host: String,
    pub database_host: &'a str,
    pub database_network_id: String,
    pub client_docker_host: String,
    pub client_host: &'a str,
    pub client_network_id: String,
    pub network_mode: dockurl::network::NetworkMode,
    pub concurrency_levels: String,
    pub pipeline_concurrency_levels: String,
    pub query_levels: String,
    pub cached_query_levels: String,
    pub duration: u32,
    pub results_name: &'a str,
    pub results_environment: &'a str,
    pub results_upload_uri: Option<&'a str>,
    pub logger: Logger,
    pub clean_up: bool,
}
impl<'a> DockerConfig<'a> {
    pub fn new(matches: &'a clap::ArgMatches) -> Self {
        let server_docker_host = format!(
            "{}:2375",
            matches.value_of(options::args::SERVER_DOCKER_HOST).unwrap()
        );
        let database_docker_host = format!(
            "{}:2375",
            matches
                .value_of(options::args::DATABASE_DOCKER_HOST)
                .unwrap()
        );
        let client_docker_host = format!(
            "{}:2375",
            matches.value_of(options::args::CLIENT_DOCKER_HOST).unwrap()
        );
        let server_host = matches.value_of(options::args::SERVER_HOST).unwrap();
        let database_host = matches.value_of(options::args::DATABASE_HOST).unwrap();
        let client_host = matches.value_of(options::args::CLIENT_HOST).unwrap();
        let network_mode = match matches.value_of(options::args::NETWORK_MODE).unwrap() {
            options::network_modes::HOST => Host,
            _ => Bridge,
        };
        let duration =
            str::parse::<u32>(matches.value_of(options::args::DURATION).unwrap()).unwrap();
        let concurrency_levels = matches
            .values_of(options::args::CONCURRENCY_LEVELS)
            .unwrap()
            .collect::<Vec<&str>>()
            .join(",");
        let pipeline_concurrency_levels = matches
            .values_of(options::args::PIPELINE_CONCURRENCY_LEVELS)
            .unwrap()
            .collect::<Vec<&str>>()
            .join(",");

        let query_levels = matches
            .values_of(options::args::QUERY_LEVELS)
            .unwrap()
            .collect::<Vec<&str>>()
            .join(",");
        let cached_query_levels = matches
            .values_of(options::args::CACHED_QUERY_LEVELS)
            .unwrap()
            .collect::<Vec<&str>>()
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

        let logger = match matches.value_of(options::args::MODE).unwrap() {
            // We don't want to log to disk in CICD.
            modes::CICD => Logger::default(),
            &_ => Logger::in_dir(&create_results_dir().unwrap()),
        };

        // There is a chance this is a hack, but it seems that these two
        // networks are always available out of the box for Docker.
        let server_network_id = match &network_mode {
            Bridge => get_tfb_network_id(use_unix_socket, &database_docker_host),
            Host => get_network_id(use_unix_socket, &server_docker_host, "host"),
        }
        .unwrap();
        let database_network_id = match &network_mode {
            Bridge => get_tfb_network_id(use_unix_socket, &database_docker_host),
            Host => get_network_id(use_unix_socket, &database_docker_host, "host"),
        }
        .unwrap();
        let client_network_id = match &network_mode {
            Bridge => get_tfb_network_id(use_unix_socket, &database_docker_host),
            Host => get_network_id(use_unix_socket, &client_docker_host, "host"),
        }
        .unwrap();

        let results_name = matches.value_of(options::args::RESULTS_NAME).unwrap();
        let results_environment = matches
            .value_of(options::args::RESULTS_ENVIRONMENT)
            .unwrap();
        let results_upload_uri = match matches.value_of(options::args::RESULTS_UPLOAD_URI) {
            None => None,
            Some(str) => Some(str),
        };
        let clean_up = matches.is_present(options::args::DOCKER_CLEANUP);

        Self {
            use_unix_socket,
            server_docker_host,
            server_host,
            server_network_id,
            database_docker_host,
            database_host,
            database_network_id,
            client_docker_host,
            client_host,
            client_network_id,
            network_mode,
            concurrency_levels,
            pipeline_concurrency_levels,
            logger,
            query_levels,
            cached_query_levels,
            duration,
            results_name,
            results_environment,
            results_upload_uri,
            clean_up,
        }
    }
}
