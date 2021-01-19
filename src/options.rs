use crate::benchmarker::modes;
use clap::{App, Arg};

/// All the arguments that the CLI accepts.
pub mod args {
    pub const AUDIT: &str = "Audit";
    pub const CLEAN: &str = "Clean";
    pub const QUIET: &str = "Quiet";
    pub const RESULTS_NAME: &str = "Results Name";
    pub const RESULTS_ENVIRONMENT: &str = "Results Environment";
    pub const RESULTS_UPLOAD_URI: &str = "Results Upload URI";
    pub const PARSE_RESULTS: &str = "Parse Results";
    pub const TEST_NAMES: &str = "Test Name(s)";
    pub const TEST_DIRS: &str = "Test Dir(s)";
    pub const TEST_LANGUAGES: &str = "Test Language(s)";
    pub const TAGS: &str = "Tag(s)";
    pub const EXCLUDE: &str = "Exclude";
    pub const TYPES: &str = "Type(s)";
    pub const MODE: &str = "Mode";
    pub const LIST_FRAMEWORKS: &str = "List Frameworks";
    pub const LIST_TESTS: &str = "List Tests";
    pub const LIST_TESTS_WITH_TAG: &str = "List Tests with Tag";
    pub const LIST_TESTS_FOR_FRAMEWORK: &str = "List Tests for Framework";
    pub const DURATION: &str = "Duration";
    pub const SERVER_DOCKER_HOST: &str = "Server Docker Host";
    pub const DOCKER_HOST_DEFAULT: &str = "localhost";
    pub const SERVER_HOST: &str = "Server Host";
    pub const SERVER_HOST_DEFAULT: &str = "tfb-server";
    pub const DATABASE_DOCKER_HOST: &str = "Database Docker Host";
    pub const DATABASE_HOST: &str = "Database Host";
    pub const DATABASE_HOST_DEFAULT: &str = "tfb-database";
    pub const CLIENT_DOCKER_HOST: &str = "Client Docker Host";
    pub const CLIENT_HOST: &str = "Client Host";
    pub const CLIENT_HOST_DEFAULT: &str = "tfb-client";
    pub const CONCURRENCY_LEVELS: &str = "Concurrency Levels";
    pub const PIPELINE_CONCURRENCY_LEVELS: &str = "Pipeline Concurrency Levels";
    pub const QUERY_LEVELS: &str = "Query Levels";
    pub const CACHED_QUERY_LEVELS: &str = "Cached Query Levels";
    pub const NETWORK_MODE: &str = "Network Mode";
    pub const REMOVE_CONTAINERS: &str = "Remove Containers";
}

pub mod network_modes {
    pub const BRIDGE: &str = "bridge";
    pub const HOST: &str = "host";
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Parses all the arguments from the CLI and returns the configured matches.
pub fn parse<'app>() -> App<'app> {
    App::new("tfb_toolset")
        .version(VERSION)
        .author("Mike Smith <msmith@techempower.com>")
        .about("The toolset for the TechEmpower Framework Benchmarks.")
        // Suite options
        .arg(
            Arg::new(args::AUDIT)
                .about("Audits framework tests for inconsistencies")
                .takes_value(false)
                .short('a')
                .long("audit")
        )
        .arg(
            Arg::new(args::CLEAN)
                .about("Removes the results directory")
                .takes_value(false)
                .short('c')
                .long("clean")
        )
        .arg(
            Arg::new(args::QUIET)
                .about(
                    "Only print a limited set of messages to stdout, keep the bulk of messages in log files only",
                )
                .takes_value(false)
                .short('q')
                .long("quiet")
        )
        .arg(
            Arg::new(args::RESULTS_NAME)
                .about(
                    "Gives a name to this set of results, formatted as a date",
                )
                .takes_value(true)
                .long("results-name")
                .default_value("(unspecified, datetime = %Y-%m-%d %H:%M:%S)")
        )
        .arg(
            Arg::new(args::RESULTS_ENVIRONMENT)
                .about("Describes the environment in which these results were gathered")
                .long("results-environment")
                .takes_value(true)
                .default_value("(unspecified, hostname = todo")
        )
        .arg(
            Arg::new(args::RESULTS_UPLOAD_URI)
                .about("A URI where the in-progress results.json file will be POSTed periodically")
                .long("results-upload-uri")
        )
        .arg(
            Arg::new(args::PARSE_RESULTS)
                .about("Parses the results of the given timestamp and merges that with the latest results")
                .long("parse")
        )
        .arg(
            Arg::new(args::REMOVE_CONTAINERS)
                .about("Automatically remove containers after they have exited")
                .long("rm")
        )
        // Test options
        .arg(
            Arg::new(args::TEST_NAMES)
                .about("Name(s) of the test(s) to run")
                .long("test")
                .short('t')
                .takes_value(true)
                .multiple(true)
        )
        .arg(
            Arg::new(args::TEST_DIRS)
                .about("Name(s) of framework director(y|ies) containing all tests to run")
                .long("test-dir")
                .short('d')
                .takes_value(true)
                .multiple(true)
        )
        .arg(
            Arg::new(args::TEST_LANGUAGES)
                .about("Name(s) of language director(y|ies) containing all tests to run")
                .long("test-lang")
                .short('l')
                .takes_value(true)
                .multiple(true)
        )
        .arg(
            Arg::new(args::TAGS)
                .about("Tests to be run with the associated tag(s) name(s)")
                .long("tag")
                .takes_value(true)
                .multiple(true)
        )
        .arg(
            Arg::new(args::EXCLUDE)
                .about("Name(s) of test(s) to to exclude")
                .long("exclude")
                .takes_value(true)
                .multiple(true)
        )
        .arg(
            Arg::new(args::TYPES)
                .about("Which type(s) of tests to run")
                .long("type")
                .takes_value(true)
                .multiple(true)
        )
        .arg(
            Arg::new(args::MODE)
                .about("Verify mode will only start up the tests, curl the urls and shutdown. \
                    Debug mode will skip verification and leave the server running.")
                .long("mode")
                .short('m')
                .takes_value(true)
                .possible_values(&[modes::BENCHMARK, modes::VERIFY, modes::CICD, modes::DEBUG])
        )
        .arg(
            Arg::new(args::LIST_FRAMEWORKS)
                .about("Lists all the known frameworks found in the current dir that can be run")
                .long("list-frameworks")
        )
        .arg(
            Arg::new(args::LIST_TESTS)
                .about("Lists all the known tests found in the current dir that can be run")
                .long("list-tests")
        )
        .arg(
            Arg::new(args::LIST_TESTS_FOR_FRAMEWORK)
                .about("Lists all the tests for the given framework")
                .long("framework-tests")
                .takes_value(true)
        )
        .arg(
            Arg::new(args::LIST_TESTS_WITH_TAG)
                .about("Lists all the tests with the associated tag")
                .long("list-tag")
                .takes_value(true)
        )
        // Benchmark Options
        .arg(
            Arg::new(args::DURATION)
                .about("The duration in seconds for which each benchmark should be measured")
                .long("duration")
                .default_value("15")
        )
        .arg(
            Arg::new(args::SERVER_DOCKER_HOST)
                .about("Hostname/IP for the Server Docker daemon")
                .long("server-docker-host")
                .default_value(args::DOCKER_HOST_DEFAULT)
        )
        .arg(
            Arg::new(args::DATABASE_DOCKER_HOST)
                .about("Hostname/IP for the Database Docker daemon")
                .long("database-docker-host")
                .default_value(args::DOCKER_HOST_DEFAULT)
        )
        .arg(
            Arg::new(args::CLIENT_DOCKER_HOST)
                .about("Hostname/IP for the Client Docker daemon")
                .long("client-docker-host")
                .default_value(args::DOCKER_HOST_DEFAULT)
        )
        .arg(
            Arg::new(args::SERVER_HOST)
                .about("Hostname/IP for the application server")
                .long("server-host")
                .default_value(args::SERVER_HOST_DEFAULT)
        )
        .arg(
            Arg::new(args::DATABASE_HOST)
                .about("Hostname/IP for the database server")
                .long("database-host")
                .default_value(args::DATABASE_HOST_DEFAULT)
        )
        .arg(
            Arg::new(args::CLIENT_HOST)
                .about("Hostname/IP for the client server")
                .long("client-host")
                .default_value(args::CLIENT_HOST_DEFAULT)
        )
        .arg(
            Arg::new(args::CONCURRENCY_LEVELS)
                .about("List of concurrencies to benchmark")
                .long("concurrency-levels")
                .takes_value(true)
                .multiple(true)
                .default_values(&["16", "32", "64", "128", "256", "512"])
        )
        .arg(
            Arg::new(args::PIPELINE_CONCURRENCY_LEVELS)
                .about("List of pipeline concurrencies to benchmark")
                .long("pipeline-concurrency-levels")
                .takes_value(true)
                .multiple(true)
                .default_values(&["256", "1024", "4096", "16384"])
        )
        .arg(
            Arg::new(args::QUERY_LEVELS)
                .about("List of query levels to benchmark")
                .long("query-levels")
                .takes_value(true)
                .multiple(true)
                .default_values(&["1", "5", "10", "15", "20"])
        )
        .arg(
            Arg::new(args::CACHED_QUERY_LEVELS)
                .about("List of cached query levels to benchmark")
                .long("cached-query-levels")
                .takes_value(true)
                .multiple(true)
                .default_values(&["1", "10", "20", "50", "100"])
        )
        // Network options
        .arg(
            Arg::new(args::NETWORK_MODE)
                .about("The network mode with which Docker should be run")
                .long("network-mode")
                .takes_value(true)
                .default_value(network_modes::BRIDGE)
                .possible_values(&[network_modes::BRIDGE, network_modes::HOST])
        )
}

//
// TESTS
//

#[cfg(test)]
mod tests {
    use crate::options::parse;

    #[test]
    fn it_can_parse_with_no_program_arguments() {
        parse();
    }
}
