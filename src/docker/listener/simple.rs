use curl::easy::{Handler, WriteError};
use serde_json::Value;

pub struct Simple {
    pub error_message: Option<String>,
}
impl Simple {
    pub fn new() -> Self {
        Self {
            error_message: None,
        }
    }
}
impl Handler for Simple {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        if let Ok(logs) = std::str::from_utf8(&data) {
            for line in logs.lines() {
                if !line.trim().is_empty() {
                    if let Ok(json) = serde_json::from_str::<Value>(line) {
                        if !json["message"].is_null() {
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
