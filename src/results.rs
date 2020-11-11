use crate::config::Named;
use crate::docker::docker_config::DockerConfig;
use crate::error::ToolsetResult;
use crate::io::get_tfb_dir;
use crate::metadata::list_all_projects;
use rand::Rng;
use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Results {
    pub uuid: String,
    pub name: String,
    pub start_time: u128,
    pub completion_time: u128,
    pub duration: u32,
    pub test_metadata: Vec<MetaData>,
    pub environment_description: String,
    pub git: Git,
    pub query_intervals: Vec<u32>,
    pub cached_query_intervals: Vec<u32>,
    pub concurrency_levels: Vec<u32>,
    pub pipeline_concurrency_levels: Vec<u32>,
    pub frameworks: Vec<String>,
    // Holdover from legacy, this should be improved in the future but the idea
    // is to support a structure like:
    // `{ "json": { "gemini": { ... } } }`
    pub raw_data: HashMap<String, HashMap<String, Vec<BenchmarkData>>>,
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
    pub fn new(docker_config: &DockerConfig) -> ToolsetResult<Self> {
        let mut results = Results::default();

        results.test_metadata = Vec::default();
        for project in list_all_projects()? {
            for test in &project.tests {
                results.test_metadata.push(MetaData {
                    versus: test.versus.clone(),
                    project_name: project.name.to_lowercase(),
                    // Legacy - we no longer support display_name
                    display_name: test.get_name(),
                    name: test.get_name(),
                    classification: test.classification.clone(),
                    database: if let Some(database) = &test.database {
                        database.clone()
                    } else {
                        // ↓ is a holdover from legacy metadata
                        "none".to_string()
                    },
                    language: project.language.clone(),
                    os: test.os.clone(),
                    // ↓ is a holdover from legacy metadata
                    notes: "".to_string(),
                    tags: if let Some(tags) = &test.tags {
                        tags.clone()
                    } else {
                        // ↓ is a holdover from legacy metadata
                        vec![]
                    },
                    framework: project.framework.get_name(),
                    webserver: test.webserver.clone(),
                    orm: if let Some(orm) = &test.orm {
                        orm.clone()
                    } else {
                        // ↓ is a holdover from legacy metadata
                        "none".to_string()
                    },
                    platform: test.platform.clone(),
                    database_os: if let Some(database_os) = &test.database_os {
                        database_os.clone()
                    } else {
                        // ↓ is a holdover from legacy metadata
                        "linux".to_string()
                    },
                    approach: test.approach.clone(),
                });
            }
        }
        let mut rng = rand::thread_rng();
        results.uuid = Uuid::from_u128(rng.gen::<u128>())
            .to_hyphenated()
            .to_string();
        results.name = docker_config.results_name.to_string();
        results.start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        results.duration = docker_config.duration;
        results.concurrency_levels = docker_config
            .concurrency_levels
            .split(',')
            .map(|l| str::parse::<u32>(l).unwrap())
            .collect();
        results.pipeline_concurrency_levels = docker_config
            .pipeline_concurrency_levels
            .split(',')
            .map(|l| str::parse::<u32>(l).unwrap())
            .collect();
        results.cached_query_intervals = docker_config
            .cached_query_levels
            .split(',')
            .map(|l| str::parse::<u32>(l).unwrap())
            .collect();
        results.query_intervals = docker_config
            .query_levels
            .split(',')
            .map(|l| str::parse::<u32>(l).unwrap())
            .collect();
        results.environment_description = docker_config.results_environment.to_string();
        results.git = Git::default();

        Ok(results)
    }
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkData {
    pub latency_avg: String,
    pub latency_max: String,
    pub latency_stdev: String,
    pub total_requests: u32,
    pub start_time: u128,
    pub end_time: u128,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Git {
    pub commit_id: String,
    pub repository_url: String,
    pub branch_name: String,
}

impl Default for Git {
    fn default() -> Self {
        let tfb_dir = get_tfb_dir().unwrap();
        let mut command = Command::new("git");
        command.args(&["rev-parse", "HEAD"]);
        command.current_dir(&tfb_dir);
        let commit_id = String::from_utf8(
            command
                .output()
                .unwrap_or_else(|_| {
                    panic!("Failed to execute `git rev-parse HEAD` in {:?}", &tfb_dir)
                })
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();

        command = Command::new("git");
        command.args(&["config", "--get", "remote.origin.url"]);
        command.current_dir(&tfb_dir);
        let repository_url = String::from_utf8(
            command
                .output()
                .unwrap_or_else(|_| {
                    panic!(
                        "Failed to execute `git config --get remote.origin.url`, in {:?}",
                        &tfb_dir
                    )
                })
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();

        command = Command::new("git");
        command.args(&["rev-parse", "--abbrev-ref", "HEAD"]);
        command.current_dir(&tfb_dir);
        let branch_name = String::from_utf8(
            command
                .output()
                .unwrap_or_else(|_| {
                    panic!(
                        "Failed to execute `git rev-parse --abbrev-ref HEAD`, in {:?}",
                        &tfb_dir
                    )
                })
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();

        Git {
            commit_id,
            repository_url,
            branch_name,
        }
    }
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
