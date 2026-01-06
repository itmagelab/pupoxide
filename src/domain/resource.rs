use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::domain::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Ensure {
    Present,
    Absent,
}

impl Default for Ensure {
    fn default() -> Self {
        Self::Present
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct FileResource {
    pub path: PathBuf,
    pub ensure: Ensure,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Resource {
    File(FileResource),
}

/// The Port for resource providers.
/// Every adapter (Infrastructure) must implement this to handle specific resource types.
#[async_trait::async_trait]
pub trait ResourceProvider: Send + Sync {
    /// Returns the current state of the resource on the system
    async fn get_state(&self, resource: &Resource) -> Result<Ensure>;
    
    /// Applies the desired state to the system
    async fn apply(&self, resource: &Resource) -> Result<()>;
}
