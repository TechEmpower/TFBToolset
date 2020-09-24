use crate::error::ToolsetError::BenchmarkDataParseError;
use crate::error::ToolsetResult;
use crate::io::Logger;
use curl::easy::{Handler, WriteError};
use regex::Regex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct Benchmarker {
    logger: Logger,
    data: Vec<u8>,
    start_time: u128,
    pub error_message: Option<String>,
}
impl Benchmarker {
    pub fn new(logger: &Logger) -> Self {
        Self {
            start_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            logger: logger.clone(),
            error_message: None,
            data: vec![],
        }
    }
    pub fn parse_wrk_output(&self) -> ToolsetResult<BenchmarkResults> {
        lazy_static! {
            static ref THREADS_CONNECTIONS: Regex = Regex::new(r"([0-9]+) threads and ([0-9]+) connections").unwrap();
            static ref LATENCY: Regex = Regex::new(r"Latency(\s)*([0-9]+\.*[0-9]*[us|ms|s|m|%]+)(\s)*([0-9]+\.*[0-9]*[us|ms|s|m|%]+)(\s)*([0-9]+\.*[0-9]*[us|ms|s|m|%]+)(\s)*([0-9]+\.*[0-9]*[us|ms|s|m|%]+)").unwrap();
            static ref REQ_SEC: Regex = Regex::new(r"Req/Sec(\s)*([0-9]+\.*[0-9]*[k|m|%]*)(\s)*([0-9]+\.*[0-9]*[k|m|%]*)(\s)*([0-9]+\.*[0-9]*[k|m|%]*)(\s)*([0-9]+\.*[0-9]*[k|m|%]*)").unwrap();
            static ref TOTAL_REQUESTS: Regex = Regex::new(r"([0-9]+) requests in ([0-9]+\.*[0-9]*)s, ([0-9]+\.*[0-9]*[B|KB|MB|GB]+) read").unwrap();
            static ref NON_2XX_3XX: Regex = Regex::new(r"Non-2xx or 3xx responses: ([0-9]+)").unwrap();
            static ref REQUESTS_PER_SECOND: Regex = Regex::new(r"Requests/sec:(\s)*([0-9]+\.*[0-9]*)").unwrap();
            static ref TRANSFER_PER_SECOND: Regex = Regex::new(r"Transfer/sec:(\s)*([0-9]+\.*[0-9]*[B|KB|MB]+)").unwrap();
            static ref LATENCY_DIST_50: Regex = Regex::new(r"50%(\s)*([0-9]+\.*[0-9]*[us|ms|s|m|%]+)").unwrap();
            static ref LATENCY_DIST_75: Regex = Regex::new(r"75%(\s)*([0-9]+\.*[0-9]*[us|ms|s|m|%]+)").unwrap();
            static ref LATENCY_DIST_90: Regex = Regex::new(r"90%(\s)*([0-9]+\.*[0-9]*[us|ms|s|m|%]+)").unwrap();
            static ref LATENCY_DIST_99: Regex = Regex::new(r"99%(\s)*([0-9]+\.*[0-9]*[us|ms|s|m|%]+)").unwrap();
            static ref SOCKET_ERRORS: Regex = Regex::new(r"Socket errors: connect ([0-9]+), read ([0-9]+), write ([0-9]+), timeout ([0-9]+)").unwrap();
            // Socket Errors
            static ref CONNECT: Regex = Regex::new(r"connect ([0-9]+)").unwrap();
            static ref READ: Regex = Regex::new(r"read ([0-9]+)").unwrap();
            static ref WRITE: Regex = Regex::new(r"write ([0-9]+)").unwrap();
            static ref TIMEOUT: Regex = Regex::new(r"timeout ([0-9]+)").unwrap();
        }
        if let Ok(data) = std::str::from_utf8(&self.data) {
            let mut threads = 0;
            let mut connections = 0;
            let mut latency_average = String::default();
            let mut latency_stddev = String::default();
            let mut latency_max = String::default();
            let mut latency_plus_minus = String::default();
            let mut req_sec_average = String::default();
            let mut req_sec_stddev = String::default();
            let mut req_sec_max = String::default();
            let mut req_sec_plus_minus = String::default();
            let mut total_requests = 0;
            let mut duration = 0f32;
            let mut data_read = String::default();
            let mut socket_errors = None;
            let mut non_2xx_3xx = None;
            let mut requests_per_second = 0f32;
            let mut transfer_per_second = String::default();
            let mut percentile_50 = String::default();
            let mut percentile_75 = String::default();
            let mut percentile_90 = String::default();
            let mut percentile_99 = String::default();
            for line in data.lines() {
                if let Some(captures) = THREADS_CONNECTIONS.captures(line) {
                    threads = str::parse::<u32>(captures.get(1).unwrap().as_str()).unwrap();
                    connections = str::parse::<u32>(captures.get(2).unwrap().as_str()).unwrap();
                }
                if let Some(captures) = &LATENCY.captures(line) {
                    latency_average = captures.get(2).unwrap().as_str().to_string();
                    latency_stddev = captures.get(4).unwrap().as_str().to_string();
                    latency_max = captures.get(6).unwrap().as_str().to_string();
                    latency_plus_minus = captures.get(8).unwrap().as_str().to_string();
                }
                if let Some(captures) = &REQ_SEC.captures(line) {
                    req_sec_average = captures.get(2).unwrap().as_str().to_string();
                    req_sec_stddev = captures.get(4).unwrap().as_str().to_string();
                    req_sec_max = captures.get(6).unwrap().as_str().to_string();
                    req_sec_plus_minus = captures.get(8).unwrap().as_str().to_string();
                }
                if let Some(captures) = TOTAL_REQUESTS.captures(line) {
                    total_requests = str::parse::<u32>(captures.get(1).unwrap().as_str()).unwrap();
                    duration = str::parse::<f32>(captures.get(2).unwrap().as_str()).unwrap();
                    data_read = captures.get(3).unwrap().as_str().to_string();
                }
                if let Some(captures) = SOCKET_ERRORS.captures(line) {
                    // todo - test this; Gemini exercise these.
                    socket_errors = Some(SocketErrors {
                        connect: str::parse::<u32>(captures.get(1).unwrap().as_str()).unwrap(),
                        read: str::parse::<u32>(captures.get(2).unwrap().as_str()).unwrap(),
                        write: str::parse::<u32>(captures.get(3).unwrap().as_str()).unwrap(),
                        timeout: str::parse::<u32>(captures.get(4).unwrap().as_str()).unwrap(),
                    });
                }
                if let Some(captures) = NON_2XX_3XX.captures(line) {
                    non_2xx_3xx =
                        Some(str::parse::<u32>(captures.get(1).unwrap().as_str()).unwrap());
                }
                if let Some(captures) = REQUESTS_PER_SECOND.captures(line) {
                    requests_per_second =
                        str::parse::<f32>(captures.get(2).unwrap().as_str()).unwrap();
                }
                if let Some(captures) = TRANSFER_PER_SECOND.captures(line) {
                    transfer_per_second = captures.get(2).unwrap().as_str().to_string();
                }
                if let Some(captures) = LATENCY_DIST_50.captures(line) {
                    percentile_50 = captures.get(2).unwrap().as_str().to_string();
                }
                if let Some(captures) = LATENCY_DIST_75.captures(line) {
                    percentile_75 = captures.get(2).unwrap().as_str().to_string();
                }
                if let Some(captures) = LATENCY_DIST_90.captures(line) {
                    percentile_90 = captures.get(2).unwrap().as_str().to_string();
                }
                if let Some(captures) = LATENCY_DIST_99.captures(line) {
                    percentile_99 = captures.get(2).unwrap().as_str().to_string();
                }
            }
            Ok(BenchmarkResults {
                start_time: self.start_time,
                end_time: self.start_time + (duration * 1_000f32) as u128,
                threads,
                connections,
                thread_stats: ThreadStats {
                    latency: Latency {
                        average: latency_average,
                        standard_deviation: latency_stddev,
                        max: latency_max,
                        plus_minus_std_dev: latency_plus_minus,
                    },
                    requests_per_second: RequestsPerSecond {
                        average: req_sec_average,
                        standard_deviation: req_sec_stddev,
                        max: req_sec_max,
                        plus_minus_std_dev: req_sec_plus_minus,
                    },
                },
                latency_distribution: LatencyDistribution {
                    percentile_50,
                    percentile_75,
                    percentile_90,
                    percentile_99,
                },
                total_requests,
                duration,
                data_read,
                socket_errors,
                requests_per_second,
                transfer_per_second,
                non_2xx_3xx,
            })
        } else {
            Err(BenchmarkDataParseError)
        }
    }
}
impl Handler for Benchmarker {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        self.data.extend_from_slice(data);

        if let Ok(logs) = std::str::from_utf8(&data) {
            for line in logs.lines() {
                if !line.trim().is_empty() {
                    self.logger.log(line.trim_end()).unwrap();
                }
            }
        }

        Ok(data.len())
    }
}

#[derive(Debug)]
pub struct BenchmarkResults {
    pub start_time: u128,
    pub end_time: u128,
    pub threads: u32,
    pub connections: u32,
    pub thread_stats: ThreadStats,
    pub latency_distribution: LatencyDistribution,
    pub total_requests: u32,
    pub duration: f32,
    pub data_read: String,
    pub socket_errors: Option<SocketErrors>,
    pub requests_per_second: f32,
    pub transfer_per_second: String,
    pub non_2xx_3xx: Option<u32>,
}

#[derive(Debug)]
pub struct ThreadStats {
    pub latency: Latency,
    pub requests_per_second: RequestsPerSecond,
}

#[derive(Debug)]
pub struct Latency {
    pub average: String,
    pub standard_deviation: String,
    pub max: String,
    pub plus_minus_std_dev: String,
}

#[derive(Debug)]
pub struct RequestsPerSecond {
    pub average: String,
    pub standard_deviation: String,
    pub max: String,
    pub plus_minus_std_dev: String,
}

#[derive(Debug)]
pub struct LatencyDistribution {
    pub percentile_50: String,
    pub percentile_75: String,
    pub percentile_90: String,
    pub percentile_99: String,
}

#[derive(Debug)]
pub struct SocketErrors {
    pub connect: u32,
    pub read: u32,
    pub write: u32,
    pub timeout: u32,
}
