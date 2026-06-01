use crate::domain::error::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents the desired state of a resource.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Ensure {
    /// The resource must exist on the system.
    #[default]
    Present,
    /// The resource must NOT exist on the system.
    Absent,
}

/// Metadata indicating where a resource was defined in the hierarchy.
///
/// Helps trace resources back to specific Rhai modules, roles, or profiles.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SourceContext {
    /// The name of the role where the resource was declared.
    pub role: Option<String>,
    /// The name of the profile where the resource was declared.
    pub profile: Option<String>,
    /// The nested module path (e.g., `std::pkg`).
    pub module: Option<String>,
}

/// Represents the actual, current state of a resource on the target system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResourceState {
    /// Represents existence-only state (present/absent).
    Ensure(Ensure),
    /// Represents full state, including metadata and/or actual binary content.
    Full {
        /// Whether the resource is present or absent.
        ensure: Ensure,
        /// Optional binary content of the resource.
        content: Option<Vec<u8>>,
    },
}

/// A resource configuration for managing a file on the file system.
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate, PartialEq)]
pub struct FileResource {
    /// Unique identifier of the resource.
    pub id: String,
    /// The target absolute path on the file system.
    pub path: PathBuf,
    /// Desired state (present or absent).
    pub ensure: Ensure,
    /// Desired text content of the file.
    pub content: Option<String>,
    /// List of resource IDs that this resource depends on.
    pub dependencies: Vec<String>,
    /// Target file owner.
    pub owner: Option<String>,
    /// Target file group.
    pub group: Option<String>,
    /// Target file mode (e.g., `"0644"`).
    pub mode: Option<String>,
    /// Location metadata.
    pub source_context: Option<SourceContext>,
    /// Optional mutex group to serialize execution.
    pub mutex: Option<String>,
}

/// The category of a metadata resource.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MetaKind {
    /// Marks the start of a Rhai module execution block.
    ModuleStart,
    /// Marks the end of a Rhai module execution block.
    ModuleEnd,
}

/// An internal metadata resource used for dependency grouping and order constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaResource {
    /// Unique identifier of the resource.
    pub id: String,
    /// The category of metadata block boundary.
    pub kind: MetaKind,
    /// List of resource IDs that this metadata block depends on.
    pub dependencies: Vec<String>,
}

/// A resource configuration for executing a system command.
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct ExecResource {
    /// Unique identifier of the resource.
    pub id: String,
    /// The command line string to run.
    pub command: String,
    /// Optional path to check; if it exists, the command will NOT run.
    pub creates: Option<PathBuf>,
    /// Optional shell command to evaluate; if it returns status code 0, the command will NOT run.
    pub unless: Option<String>,
    /// Optional working directory to run the command in.
    pub cwd: Option<PathBuf>,
    /// Environment variables for the child process.
    pub environment: Option<std::collections::HashMap<String, String>>,
    /// List of resource IDs that this resource depends on.
    pub dependencies: Vec<String>,
    /// Location metadata.
    pub source_context: Option<SourceContext>,
    /// Optional mutex group to serialize execution.
    pub mutex: Option<String>,
}

/// A resource configuration for managing a directory on the file system.
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct DirectoryResource {
    /// Unique identifier of the resource.
    pub id: String,
    /// Target path on the file system.
    pub path: PathBuf,
    /// Desired state (present or absent).
    pub ensure: Ensure,
    /// List of resource IDs that this resource depends on.
    pub dependencies: Vec<String>,
    /// Target directory owner.
    pub owner: Option<String>,
    /// Target directory group.
    pub group: Option<String>,
    /// Target directory mode (e.g., `"0755"`).
    pub mode: Option<String>,
    /// Location metadata.
    pub source_context: Option<SourceContext>,
    /// Optional mutex group to serialize execution.
    pub mutex: Option<String>,
}

/// A resource configuration for managing a package.
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate, PartialEq)]
pub struct PackageResource {
    /// Unique identifier of the resource (often includes provider name).
    pub id: String,
    /// The package name to install or remove.
    pub name: String,
    /// Desired state (present/installed or absent/removed).
    pub ensure: Ensure,
    /// The package manager provider (e.g., `"brew"`, `"apt"`).
    pub provider: String,
    /// List of resource IDs that this resource depends on.
    pub dependencies: Vec<String>,
    /// Location metadata.
    pub source_context: Option<SourceContext>,
    /// Optional mutex group to serialize execution.
    pub mutex: Option<String>,
    /// Optional provider-specific parameters as a JSON object.
    pub params: Option<serde_json::Value>,
}

/// Represents any configurable system component managed by Pupoxide.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Resource {
    /// A file resource.
    File(FileResource),
    /// A directory resource.
    Directory(DirectoryResource),
    /// An executable command resource.
    Exec(ExecResource),
    /// A software package resource.
    Package(PackageResource),
    /// An internal metadata block marker resource.
    Meta(MetaResource),
}

impl Resource {
    /// Returns the unique identifier of the resource.
    pub fn id(&self) -> &str {
        match self {
            Resource::File(f) => &f.id,
            Resource::Directory(d) => &d.id,
            Resource::Exec(e) => &e.id,
            Resource::Package(p) => &p.id,
            Resource::Meta(m) => &m.id,
        }
    }

    /// Returns the list of resource identifiers this resource depends on.
    pub fn dependencies(&self) -> &[String] {
        match self {
            Resource::File(f) => &f.dependencies,
            Resource::Directory(d) => &d.dependencies,
            Resource::Exec(e) => &e.dependencies,
            Resource::Package(p) => &p.dependencies,
            Resource::Meta(m) => &m.dependencies,
        }
    }

    /// Returns the execution mutex name if defined for this resource.
    pub fn mutex(&self) -> Option<&str> {
        match self {
            Resource::File(f) => f.mutex.as_deref(),
            Resource::Directory(d) => d.mutex.as_deref(),
            Resource::Exec(e) => e.mutex.as_deref(),
            Resource::Package(p) => p.mutex.as_deref(),
            Resource::Meta(_) => None,
        }
    }

    /// Adds a dependency resource ID to this resource.
    pub fn add_dependency(&mut self, dep_id: String) {
        let deps = match self {
            Resource::File(f) => &mut f.dependencies,
            Resource::Directory(d) => &mut d.dependencies,
            Resource::Exec(e) => &mut e.dependencies,
            Resource::Package(p) => &mut p.dependencies,
            Resource::Meta(m) => &mut m.dependencies,
        };

        if !deps.contains(&dep_id) {
            deps.push(dep_id);
        }
    }
}

/// The Port trait for resource providers.
///
/// Every adapter (Infrastructure) must implement this to handle specific resource types.
#[async_trait::async_trait]
pub trait ResourceProvider: Send + Sync {
    /// Returns `true` if this provider is capable of handling the given resource type.
    fn can_handle(&self, resource: &Resource) -> bool;

    /// Retrieves the current system state of the resource.
    ///
    /// If `full` is true, the provider should attempt to read and return file contents or detailed metadata.
    async fn get_state(&self, resource: &Resource, full: bool) -> Result<ResourceState>;

    /// Synchronizes the system to match the resource's desired configuration.
    async fn apply(&self, resource: &Resource) -> Result<()>;
}
