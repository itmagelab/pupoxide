use std::path::PathBuf;
use std::fs;
use sha2::{Sha256, Digest};
use crate::domain::error::{DomainError, Result};

pub struct BackupStore {
    root: PathBuf,
}

impl BackupStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Stores content in the CAS and returns its hash
    pub fn store(&self, content: &[u8]) -> Result<String> {
        let hash = format!("{:x}", Sha256::digest(content));
        let path = self.get_path(&hash);

        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| DomainError::Internal(format!("Failed to create backup dir: {}", e)))?;
            }
            fs::write(&path, content)
                .map_err(|e| DomainError::Internal(format!("Failed to write backup: {}", e)))?;
        }

        Ok(hash)
    }

    /// Retrieves content from the CAS by hash
    pub fn retrieve(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.get_path(hash);
        fs::read(path)
            .map_err(|e| DomainError::Internal(format!("Failed to read backup {}: {}", hash, e)))
    }

    fn get_path(&self, hash: &str) -> PathBuf {
        // Use first two chars as subdirectory to avoid flat directory issues
        let prefix = &hash[..2];
        let rest = &hash[2..];
        self.root.join(prefix).join(rest)
    }
}
