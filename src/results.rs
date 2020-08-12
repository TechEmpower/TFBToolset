use crate::docker::docker_config::DockerConfig;
use rand::Rng;
use serde::Serialize;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Serialize, Clone, Debug, Default)]
pub struct Results {
    pub uuid: String,
    pub name: String,
    pub start_time: u128,
    pub completion_time: u128,
    pub duration: usize,
    pub test_metadata: Vec<MetaData>,
    pub environment_description: String,
    pub git: Git,
    pub cached_query_intervals: Vec<usize>,
    pub concurrency_levels: Vec<usize>,
    pub pipeline_concurrency_levels: Vec<usize>,
    pub frameworks: Vec<String>,
    // Holdover from legacy, this should be improved in the future but the idea
    // is to support a structure like:
    // `{ "json": { "gemini": { ... } } }`
    pub raw_data: HashMap<String, Vec<BenchmarkData>>,
    // Holdover from legacy, this should be improved in the future but the idea
    // is to support a structure like:
    // `{ "gemini": { "json": "passed" } }`
    pub verify: HashMap<String, HashMap<String, String>>,
    // Holdover from legacy; this should be improved in the future but the idea
    // is to support a structure like:
    // `{ "json": [ "gemini" ] }`
    pub succeeded: HashMap<String, Vec<String>>,
    // Holdover from legacy; this should be improved in the future but the idea
    // is to support a structure like:
    // `{ "json": [ "gemini" ] }`
    pub failed: HashMap<String, Vec<String>>,
    // Holdover from legacy; should be updated to better represent intent:
    // `{ "gemini": "20200810202733" }` - change to `u128` instead of string.
    pub completed: HashMap<String, String>,
}
impl Results {
    pub fn new(docker_config: &DockerConfig) -> Self {
        let mut results = Results::default();

        let mut rng = rand::thread_rng();
        results.uuid = Uuid::from_u128(rng.gen::<u128>())
            .to_hyphenated()
            .to_string();
        results.name = String::default(); // todo
        results.start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        results.duration = docker_config.duration;
        results.pipeline_concurrency_levels = docker_config
            .pipeline_concurrency_levels
            .split(',')
            .map(|l| str::parse::<usize>(l).unwrap())
            .collect();
        results.cached_query_intervals = docker_config
            .cached_query_levels
            .split(',')
            .map(|l| str::parse::<usize>(l).unwrap())
            .collect();
        results.environment_description = String::default(); // todo
        results.git = Git::default(); // todo

        results
    }
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct BenchmarkData {
    pub latency_avg: String,
    pub latency_max: String,
    pub latency_stdev: String,
    pub total_requests: String,
    pub start_time: u128,
    pub end_time: u128,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Git {
    pub commit_id: String,
    pub repository_url: String,
    pub branch_name: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct MetaData {
    pub versus: String,
    pub project_name: String,
    pub display_name: String,
    pub name: String,
    pub classification: String,
    pub database: String,
    pub language: String,
    pub os: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub framework: String,
    pub webserver: String,
    pub orm: String,
    pub platform: String,
    pub database_os: String,
    pub approach: String,
}
