use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionDetails(pub Value);

impl ExecutionDetails {
    pub fn new(value: Value) -> Self {
        Self(value)
    }

    pub fn stdout(&self) -> Option<&str> {
        self.0.get("stdout").and_then(|v| v.as_str())
    }

    pub fn stderr(&self) -> Option<&str> {
        self.0.get("stderr").and_then(|v| v.as_str())
    }

    pub fn msg(&self) -> Option<&str> {
        self.0.get("msg").and_then(|v| v.as_str())
    }

    pub fn cmd(&self) -> Option<String> {
        // cmd can be a string or list of strings
        let cmd_val = self.0.get("cmd").or_else(|| {
            self.0
                .get("invocation")
                .and_then(|i| i.get("module_args"))
                .and_then(|m| m.get("cmd"))
        });

        if let Some(val) = cmd_val {
            if let Some(s) = val.as_str() {
                return Some(s.to_string());
            }
            if let Some(arr) = val.as_array() {
                return Some(
                    arr.iter()
                        .map(|v| v.as_str().unwrap_or(""))
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            }
        }
        None
    }

    pub fn invocation(&self) -> Option<&Value> {
        self.0.get("invocation")
    }

    pub fn inner(&self) -> &Value {
        &self.0
    }
}
