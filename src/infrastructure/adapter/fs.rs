use crate::domain::error::Result;
use crate::domain::resource::{Ensure, FileResource, Resource, ResourceProvider, ResourceState};
use async_trait::async_trait;
use nix::unistd::{Group, User, chown};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;

pub struct FsAdapter;

#[async_trait]
impl ResourceProvider for FsAdapter {
    fn can_handle(&self, resource: &Resource) -> bool {
        matches!(resource, Resource::File(_) | Resource::Directory(_))
    }

    async fn get_state(&self, resource: &Resource, full: bool) -> Result<ResourceState> {
        match resource {
            Resource::File(file) => {
                let physical_exists = file.path.exists() && !file.path.is_dir();

                if full && physical_exists {
                    let content = fs::read(&file.path).ok();
                    Ok(ResourceState::Full {
                        ensure: Ensure::Present,
                        content,
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
            Resource::Exec(_) => Ok(ResourceState::Ensure(Ensure::Present)),
            Resource::Meta(_) => Ok(ResourceState::Ensure(Ensure::Present)),
        }
    }

    async fn apply(&self, resource: &Resource) -> Result<()> {
        match resource {
            Resource::File(file) => self.apply_file(file).await,
            Resource::Directory(dir) => self.apply_directory(dir).await,
            Resource::Exec(_) => Ok(()),
            Resource::Meta(_) => Ok(()),
        }
    }
}

impl FsAdapter {
    async fn apply_directory(
        &self,
        dir: &crate::domain::resource::DirectoryResource,
    ) -> Result<()> {
        if dir.ensure == Ensure::Present {
            if !dir.path.exists() {
                fs::create_dir_all(&dir.path).map_err(|e| {
                    anyhow::anyhow!("Failed to create directory: {}", e)
                })?;
            }
            self.apply_metadata(&dir.path, &dir.owner, &dir.group, &dir.mode)
                .await?;
        } else if dir.path.exists() {
            fs::remove_dir_all(&dir.path)
                .map_err(|e| anyhow::anyhow!("Failed to remove directory: {}", e))?;
        }
        Ok(())
    }

    async fn apply_metadata(
        &self,
        path: &std::path::Path,
        owner: &Option<String>,
        group: &Option<String>,
        mode: &Option<String>,
    ) -> Result<()> {
        // Mode
        if let Some(mode_str) = mode {
            let mode_val = u32::from_str_radix(mode_str, 8).map_err(|e| {
                anyhow::anyhow!("Invalid mode octal string '{}': {}", mode_str, e)
            })?;
            let mut perms = fs::metadata(path)
                .map_err(|e| anyhow::anyhow!("Failed to get metadata: {}", e))?
                .permissions();
            perms.set_mode(mode_val);
            fs::set_permissions(path, perms)
                .map_err(|e| anyhow::anyhow!("Failed to set permissions: {}", e))?;
        }

        // Owner / Group
        let uid = if let Some(owner_name) = owner {
            let user = User::from_name(owner_name)
                .map_err(|e| {
                    anyhow::anyhow!("Failed to resolve user '{}': {}", owner_name, e)
                })?
                .ok_or_else(|| anyhow::anyhow!("User '{}' not found", owner_name))?;
            Some(user.uid)
        } else {
            None
        };

        let gid = if let Some(group_name) = group {
            let grp = Group::from_name(group_name)
                .map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to resolve group '{}': {}",
                        group_name, e
                    )
                })?
                .ok_or_else(|| {
                    anyhow::anyhow!("Group '{}' not found", group_name)
                })?;
            Some(grp.gid)
        } else {
            None
        };

        if uid.is_some() || gid.is_some() {
            chown(path, uid, gid)
                .map_err(|e| anyhow::anyhow!("Failed to chown: {}", e))?;
        }

        Ok(())
    }

    async fn get_file_ensure(&self, file: &FileResource) -> Result<Ensure> {
        if !file.path.exists() {
            return Ok(Ensure::Absent);
        }

        if file.path.is_dir() {
            return Ok(Ensure::Absent);
        }

        if let Some(expected_content) = &file.content {
            let actual_content = fs::read_to_string(&file.path)
                .map_err(|e| anyhow::anyhow!("Failed to read file: {}", e))?;

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
                if let Some(parent) = file.path.parent()
                    && !parent.exists()
                {
                    fs::create_dir_all(parent).map_err(|e| {
                        anyhow::anyhow!("Failed to create parent directory: {}", e)
                    })?;
                }

                let mut f = fs::File::create(&file.path)
                    .map_err(|e| anyhow::anyhow!("Failed to create file: {}", e))?;

                if let Some(content) = &file.content {
                    f.write_all(content.as_bytes()).map_err(|e| {
                        anyhow::anyhow!("Failed to write file: {}", e)
                    })?;
                }

                // Set permissions and ownership
                self.apply_metadata(&file.path, &file.owner, &file.group, &file.mode)
                    .await?;

                tracing::info!(path = %file.path.display(), "File ensured present");
            }
            Ensure::Absent => {
                if file.path.exists() {
                    fs::remove_file(&file.path).map_err(|e| {
                        anyhow::anyhow!("Failed to remove file: {}", e)
                    })?;
                    tracing::info!(path = %file.path.display(), "File ensured absent");
                }
            }
        }
        Ok(())
    }
}
