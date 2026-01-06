use std::fs;
use std::io::Write;
use async_trait::async_trait;
use crate::domain::resource::{Resource, ResourceProvider, Ensure, FileResource};
use crate::domain::error::{DomainError, Result};

pub struct FsAdapter;

#[async_trait]
impl ResourceProvider for FsAdapter {
    async fn get_state(&self, resource: &Resource) -> Result<Ensure> {
        match resource {
            Resource::File(file) => self.get_file_state(file).await,
        }
    }

    async fn apply(&self, resource: &Resource) -> Result<()> {
        match resource {
            Resource::File(file) => self.apply_file(file).await,
        }
    }
}

impl FsAdapter {
    async fn get_file_state(&self, file: &FileResource) -> Result<Ensure> {
        if !file.path.exists() {
            return Ok(Ensure::Absent);
        }

        if let Some(expected_content) = &file.content {
            let actual_content = fs::read_to_string(&file.path)
                .map_err(|e| DomainError::Internal(format!("Failed to read file: {}", e)))?;
            
            if &actual_content != expected_content {
                // If content differs, we consider it "absent" from the desired state perspective, 
                // or more accurately, we'll need to re-apply. 
                // For simplicity in this iteration, if it exists but content is wrong, 
                // we'll say it's not in the desired "Present" state.
                return Ok(Ensure::Absent); 
            }
        }

        Ok(Ensure::Present)
    }

    async fn apply_file(&self, file: &FileResource) -> Result<()> {
        match file.ensure {
            Ensure::Present => {
                let mut f = fs::File::create(&file.path)
                    .map_err(|e| DomainError::Internal(format!("Failed to create file: {}", e)))?;
                
                if let Some(content) = &file.content {
                    f.write_all(content.as_bytes())
                        .map_err(|e| DomainError::Internal(format!("Failed to write file: {}", e)))?;
                }
                
                tracing::info!(path = %file.path.display(), "File ensured present");
            }
            Ensure::Absent => {
                if file.path.exists() {
                    fs::remove_file(&file.path)
                        .map_err(|e| DomainError::Internal(format!("Failed to remove file: {}", e)))?;
                    tracing::info!(path = %file.path.display(), "File ensured absent");
                }
            }
        }
        Ok(())
    }
}
