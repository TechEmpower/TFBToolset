// use crate::config::{Named, Project, Test};
use crate::docker::Verification;
use crate::io::Logger;
use curl::easy::{Handler, WriteError};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct Verifier {
    pub verification: Arc<Mutex<Verification>>,
    logger: Logger,
}
impl Verifier {
    pub fn new(verification: Arc<Mutex<Verification>>, logger: &Logger) -> Self {
        let mut logger = logger.clone();
        logger.set_log_file("verifications.txt");

        Self {
            logger,
            verification,
        }
    }
}
impl Handler for Verifier {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        if let Ok(logs) = std::str::from_utf8(&data) {
            for line in logs.lines() {
                if !line.trim().is_empty() {
                    if let Ok(warning) = serde_json::from_str::<WarningMessage>(line) {
                        if let Ok(mut verification) = self.verification.lock() {
                            verification.warnings.push(warning.warning);
                        }
                    } else if let Ok(error) = serde_json::from_str::<ErrorMessage>(line) {
                        if let Ok(mut verification) = self.verification.lock() {
                            verification.errors.push(error.error);
                        }
                    } else {
                        self.logger.log(line.trim_end()).unwrap();
                    }
                }
            }
        }

        Ok(data.len())
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Warning {
    pub message: String,
    pub short_message: String,
}
#[derive(Deserialize, Clone, Debug)]
pub struct Error {
    pub message: String,
    pub short_message: String,
}

#[derive(Deserialize)]
struct WarningMessage {
    warning: Warning,
}
#[derive(Deserialize)]
struct ErrorMessage {
    error: Error,
}
