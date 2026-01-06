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
    pub id: String,
    pub path: PathBuf,
    pub ensure: Ensure,
    pub content: Option<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MetaKind {
    ModuleStart,
    ModuleEnd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaResource {
    pub id: String,
    pub kind: MetaKind,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Resource {
    File(FileResource),
    Meta(MetaResource),
}

impl Resource {
    pub fn id(&self) -> &str {
        match self {
            Resource::File(f) => &f.id,
            Resource::Meta(m) => &m.id,
        }
    }

    pub fn dependencies(&self) -> &[String] {
        match self {
            Resource::File(f) => &f.dependencies,
            Resource::Meta(m) => &m.dependencies,
        }
    }

    pub fn add_dependency(&mut self, dep_id: String) {
        match self {
            Resource::File(f) => {
                if !f.dependencies.contains(&dep_id) {
                    f.dependencies.push(dep_id);
                }
            }
            Resource::Meta(m) => {
                if !m.dependencies.contains(&dep_id) {
                    m.dependencies.push(dep_id);
                }
            }
        }
    }
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
