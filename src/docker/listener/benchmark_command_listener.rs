use crate::docker::BenchmarkCommands;
use crate::io::Logger;
use curl::easy::{Handler, WriteError};

#[derive(Clone)]
pub struct BenchmarkCommandListener {
    logger: Logger,
    pub error_message: Option<String>,
    pub benchmark_commands: Option<BenchmarkCommands>,
}
impl BenchmarkCommandListener {
    pub fn new(test_type: &(&String, &String), logger: &Logger) -> Self {
        let mut logger = logger.clone();
        logger.set_log_file(&format!("{}.txt", test_type.0));
        logger.quiet = true;

        Self {
            logger,
            error_message: None,
            benchmark_commands: None,
        }
    }
}
impl Handler for BenchmarkCommandListener {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        if let Ok(logs) = std::str::from_utf8(&data) {
            for line in logs.lines() {
                if !line.trim().is_empty() {
                    if let Ok(commands) = serde_json::from_str::<BenchmarkCommands>(line) {
                        self.benchmark_commands = Some(commands);
                    } else {
                        self.logger.log(line.trim_end()).unwrap();
                    }
                }
            }
        }

        Ok(data.len())
    }
}
