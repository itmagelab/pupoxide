use crate::domain::error::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum Ensure {
    #[default]
    Present,
    Absent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResourceState {
    Ensure(Ensure),
    /// Current state including content (used for backups)
    Full {
        ensure: Ensure,
        content: Option<Vec<u8>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RollbackStatus {
    Success,
    Failed(String),
    SkippedNoBackup,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate, PartialEq)]
pub struct FileResource {
    pub id: String,
    pub path: PathBuf,
    pub ensure: Ensure,
    pub content: Option<String>,
    pub dependencies: Vec<String>,
    pub backup: bool,
    pub max_backup_size: Option<u64>,
    pub owner: Option<String>,
    pub group: Option<String>,
    pub mode: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct ExecResource {
    pub id: String,
    pub command: String,
    pub creates: Option<PathBuf>,
    pub unless: Option<String>,
    pub cwd: Option<PathBuf>,
    pub environment: Option<std::collections::HashMap<String, String>>,
    pub dependencies: Vec<String>,
    pub backup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct DirectoryResource {
    pub id: String,
    pub path: PathBuf,
    pub ensure: Ensure,
    pub dependencies: Vec<String>,
    pub backup: bool,
    pub owner: Option<String>,
    pub group: Option<String>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Resource {
    File(FileResource),
    Directory(DirectoryResource),
    Exec(ExecResource),
    Meta(MetaResource),
}

impl Resource {
    pub fn id(&self) -> &str {
        match self {
            Resource::File(f) => &f.id,
            Resource::Directory(d) => &d.id,
            Resource::Exec(e) => &e.id,
            Resource::Meta(m) => &m.id,
        }
    }

    pub fn dependencies(&self) -> &[String] {
        match self {
            Resource::File(f) => &f.dependencies,
            Resource::Directory(d) => &d.dependencies,
            Resource::Exec(e) => &e.dependencies,
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
            Resource::Directory(d) => {
                if !d.dependencies.contains(&dep_id) {
                    d.dependencies.push(dep_id);
                }
            }
            Resource::Exec(e) => {
                if !e.dependencies.contains(&dep_id) {
                    e.dependencies.push(dep_id);
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
    /// Returns true if this provider can handle the given resource
    fn can_handle(&self, resource: &Resource) -> bool;

    /// Returns the current state of the resource on the system.
    /// If full is true, the provider SHOULD attempt to return the full content for backup.
    async fn get_state(&self, resource: &Resource, full: bool) -> Result<ResourceState>;

    /// Applies the desired state to the system
    async fn apply(&self, resource: &Resource) -> Result<()>;
}
