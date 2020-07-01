use curl::easy::{Handler, WriteError};
use serde_json::Value;

pub struct BuildContainer {
    pub container_id: Option<String>,
    pub error_message: Option<String>,
}
impl BuildContainer {
    pub fn new() -> Self {
        Self {
            container_id: None,
            error_message: None,
        }
    }
}
impl Handler for BuildContainer {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        if let Ok(logs) = std::str::from_utf8(&data) {
            for line in logs.lines() {
                if !line.trim().is_empty() {
                    if let Ok(json) = serde_json::from_str::<Value>(line) {
                        if !json["Id"].is_null() {
                            let mut container_id = json["Id"].as_str().unwrap();
                            container_id = &container_id[0..12];
                            self.container_id = Some(container_id.to_string());
                        } else if !json["message"].is_null() {
                            // fixme - this APPEARS to be how docker communicates error messages.
                            let error = json["message"].as_str().unwrap().to_string();
                            self.error_message = Some(error);
                        }
                    }
                }
            }
        }

        Ok(data.len())
    }
}
