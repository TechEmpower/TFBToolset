use crate::io::Logger;
use curl::easy::{Handler, WriteError};

pub struct Benchmarker {
    logger: Logger,
    pub error_message: Option<String>,
}
impl Benchmarker {
    pub fn new(test_type: &(&String, &String), logger: &Logger) -> Self {
        let mut logger = logger.clone();
        logger.set_log_file(&format!("{}.txt", test_type.0));
        logger.quiet = true;

        Self {
            logger,
            error_message: None,
        }
    }
}
impl Handler for Benchmarker {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
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
