use std::path::PathBuf;
use crate::domain::error::{Result, DomainError};

pub struct EnvironmentLoader {
    base_path: PathBuf,
}

impl EnvironmentLoader {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Finds the entry point manifest for a given environment (site.rhai)
    pub fn get_site_manifest(&self, env_name: &str) -> Result<PathBuf> {
        let path = self.base_path
            .join("environments")
            .join(env_name)
            .join("manifests")
            .join("site.rhai");

        if path.exists() {
            Ok(path)
        } else {
            Err(DomainError::Internal(format!("Site manifest not found at: {}", path.display())))
        }
    }

    /// Returns the modules path for a given environment
    pub fn get_modules_path(&self, env_name: &str) -> PathBuf {
        self.base_path
            .join("environments")
            .join(env_name)
            .join("modules")
    }
}
