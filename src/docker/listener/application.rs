//! The DockerContainerLogger module is used as the mechanism for IO with
//! containers running in Docker. The module should not be called except by the
//! `docker` module in practice.

use crate::io::Logger;
use curl::easy::{Handler, WriteError};

#[derive(Clone)]
pub struct Application {
    pub error_message: Option<String>,
    pub logger: Logger,
}
impl Application {
    pub fn new(logger: &Logger) -> Self {
        let mut logger = logger.clone();
        logger.set_log_file("log.txt");

        Self {
            error_message: None,
            logger,
        }
    }
}
impl Handler for Application {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        if let Ok(logs) = std::str::from_utf8(data) {
            self.logger.log(logs).unwrap();
        }

        Ok(data.len())
    }
}
