use crate::config::{Named, Project, Test};
use crate::docker::verification::Verification;
use crate::io::Logger;
use curl::easy::{Handler, WriteError};
use serde::Deserialize;

pub struct Verifier {
    pub verification: Verification,
    logger: Logger,
}
impl Verifier {
    pub fn new(
        project: &Project,
        test: &Test,
        test_type: &(&String, &String),
        logger: &Logger,
    ) -> Self {
        let mut logger = logger.clone();
        logger.set_log_file("verifications.txt");

        Self {
            logger,
            verification: Verification {
                framework_name: project.framework.get_name(),
                test_name: test.get_name(),
                type_name: test_type.0.clone(),
                warnings: vec![],
                errors: vec![],
            },
        }
    }
}
impl Handler for Verifier {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        if let Ok(logs) = std::str::from_utf8(&data) {
            for line in logs.lines() {
                if !line.trim().is_empty() {
                    if let Ok(warning) = serde_json::from_str::<WarningMessage>(line) {
                        self.verification.warnings.push(warning.warning);
                    } else if let Ok(error) = serde_json::from_str::<ErrorMessage>(line) {
                        self.verification.errors.push(error.error);
                    } else {
                        self.logger.log(line.trim_end()).unwrap();
                    }
                }
            }
        }

        Ok(data.len())
    }
}

#[derive(Deserialize, Clone)]
pub struct Warning {
    pub message: String,
    pub short_message: String,
}
#[derive(Deserialize, Clone)]
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
