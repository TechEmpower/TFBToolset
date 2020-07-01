use curl::easy::{Handler, WriteError};
use serde_json::Value;

pub struct BuildNetwork {
    pub network_id: Option<String>,
    pub error_message: Option<String>,
}
impl BuildNetwork {
    pub fn new() -> Self {
        Self {
            network_id: None,
            error_message: None,
        }
    }
}
impl Handler for BuildNetwork {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        if let Ok(logs) = std::str::from_utf8(&data) {
            for line in logs.lines() {
                if !line.trim().is_empty() {
                    if let Ok(json) = serde_json::from_str::<Value>(line) {
                        if !json["Id"].is_null() {
                            let mut network_id = json["Id"].as_str().unwrap();
                            network_id = &network_id[0..12];
                            self.network_id = Some(network_id.to_string());
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
