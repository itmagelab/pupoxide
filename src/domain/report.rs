use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ResourceStatus {
    Applied,
    Unchanged,
    Failed,
    Skipped,
    WouldApply, // For dry-run
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceReport {
    pub resource_id: String,
    pub status: ResourceStatus,
    pub message: Option<String>,
    pub changed: bool,
    pub duration: std::time::Duration,
}

impl ResourceReport {
    pub fn new(resource_id: String, status: ResourceStatus, changed: bool) -> Self {
        Self {
            resource_id,
            status,
            message: None,
            changed,
            duration: std::time::Duration::from_secs(0),
        }
    }

    pub fn with_duration(mut self, duration: std::time::Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn with_message(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }
}
