use std::fs;
use std::io::Write;
use async_trait::async_trait;
use crate::domain::resource::{Resource, ResourceProvider, Ensure, FileResource, ResourceState};
use crate::domain::error::{DomainError, Result};

pub struct FsAdapter;

#[async_trait]
impl ResourceProvider for FsAdapter {
    async fn get_state(&self, resource: &Resource, full: bool) -> Result<ResourceState> {
        match resource {
            Resource::File(file) => {
                let physical_exists = file.path.exists() && !file.path.is_dir();
                
                if full && physical_exists {
                    let content = fs::read(&file.path).ok();
                    Ok(ResourceState::Full { 
                        ensure: Ensure::Present, 
                        content 
                    })
                } else {
                    let ensure = self.get_file_ensure(file).await?;
                    Ok(ResourceState::Ensure(ensure))
                }
            }
            Resource::Directory(dir) => {
                let ensure = if dir.path.exists() && dir.path.is_dir() {
                    Ensure::Present
                } else {
                    Ensure::Absent
                };
                Ok(ResourceState::Ensure(ensure))
            }
            Resource::Meta(_) => Ok(ResourceState::Ensure(Ensure::Present)),
        }
    }

    async fn apply(&self, resource: &Resource) -> Result<()> {
        match resource {
            Resource::File(file) => self.apply_file(file).await,
            Resource::Directory(dir) => self.apply_directory(dir).await,
            Resource::Meta(_) => Ok(()),
        }
    }
}

impl FsAdapter {
    async fn get_file_ensure(&self, file: &FileResource) -> Result<Ensure> {
        if !file.path.exists() {
            return Ok(Ensure::Absent);
        }

        if file.path.is_dir() {
             return Ok(Ensure::Absent);
        }

        if let Some(expected_content) = &file.content {
            let actual_content = fs::read_to_string(&file.path)
                .map_err(|e| DomainError::Internal(format!("Failed to read file: {}", e)))?;
            
            if &actual_content != expected_content {
                return Ok(Ensure::Absent); 
            }
        }

        Ok(Ensure::Present)
    }

    async fn apply_file(&self, file: &FileResource) -> Result<()> {
        match file.ensure {
            Ensure::Present => {
                // Auto-create parent directories
                if let Some(parent) = file.path.parent() {
                    if !parent.exists() {
                        fs::create_dir_all(parent)
                            .map_err(|e| DomainError::Internal(format!("Failed to create parent directory: {}", e)))?;
                    }
                }

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

    async fn apply_directory(&self, dir: &crate::domain::resource::DirectoryResource) -> Result<()> {
        match dir.ensure {
            Ensure::Present => {
                if !dir.path.exists() {
                    fs::create_dir_all(&dir.path)
                        .map_err(|e| DomainError::Internal(format!("Failed to create directory: {}", e)))?;
                    tracing::info!(path = %dir.path.display(), "Directory ensured present");
                } else if !dir.path.is_dir() {
                    return Err(DomainError::Internal(format!("Path exists but is not a directory: {}", dir.path.display())));
                }
            }
            Ensure::Absent => {
                if dir.path.exists() {
                    if dir.path.is_dir() {
                        fs::remove_dir_all(&dir.path)
                            .map_err(|e| DomainError::Internal(format!("Failed to remove directory: {}", e)))?;
                        tracing::info!(path = %dir.path.display(), "Directory ensured absent");
                    } else {
                        fs::remove_file(&dir.path)
                            .map_err(|_e| DomainError::Internal(format!("Failed to remove file blocking directory removal: {}", dir.path.display())))?;
                    }
                }
            }
        }
        Ok(())
    }
}
