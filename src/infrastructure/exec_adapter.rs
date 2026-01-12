use crate::domain::error::{DomainError, Result};
use crate::domain::resource::{Ensure, ExecResource, Resource, ResourceProvider, ResourceState};
use async_trait::async_trait;
use tokio::process::Command;

pub struct ExecAdapter;

#[async_trait]
impl ResourceProvider for ExecAdapter {
    fn can_handle(&self, resource: &Resource) -> bool {
        matches!(resource, Resource::Exec(_))
    }

    async fn get_state(&self, resource: &Resource, _full: bool) -> Result<ResourceState> {
        match resource {
            Resource::Exec(exec) => {
                let should_run = self.should_execute(exec).await?;
                let ensure = if should_run {
                    Ensure::Absent // Needs to run
                } else {
                    Ensure::Present // Already satisfied
                };
                Ok(ResourceState::Ensure(ensure))
            }
            _ => Ok(ResourceState::Ensure(Ensure::Present)),
        }
    }

    async fn apply(&self, resource: &Resource) -> Result<()> {
        match resource {
            Resource::Exec(exec) => self.apply_exec(exec).await,
            _ => Ok(()),
        }
    }
}

impl ExecAdapter {
    /// Check if the command should be executed based on creates/unless conditions
    async fn should_execute(&self, exec: &ExecResource) -> Result<bool> {
        // Check 'creates' - skip if file exists
        if let Some(creates_path) = &exec.creates
            && creates_path.exists()
        {
            tracing::debug!(
                command = %exec.command,
                creates = %creates_path.display(),
                "Skipping exec: file already exists"
            );
            return Ok(false);
        }

        // Check 'unless' - skip if command returns 0
        if let Some(unless_cmd) = &exec.unless {
            let status = Command::new("sh")
                .arg("-c")
                .arg(unless_cmd)
                .status()
                .await
                .map_err(|e| {
                    DomainError::Internal(format!("Failed to execute unless command: {}", e))
                })?;

            if status.success() {
                tracing::debug!(
                    command = %exec.command,
                    unless = %unless_cmd,
                    "Skipping exec: unless condition satisfied"
                );
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn apply_exec(&self, exec: &ExecResource) -> Result<()> {
        // Check if we should execute
        if !self.should_execute(exec).await? {
            return Ok(());
        }

        // Build command
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&exec.command);

        // Set working directory
        if let Some(cwd) = &exec.cwd {
            cmd.current_dir(cwd);
        }

        // Set environment variables
        if let Some(env) = &exec.environment {
            for (key, value) in env {
                cmd.env(key, value);
            }
        }

        // Execute command
        let output = cmd.output().await.map_err(|e| {
            DomainError::Internal(format!("Failed to execute command '{}': {}", exec.command, e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DomainError::Internal(format!(
                "Command '{}' failed with exit code {:?}: {}",
                exec.command,
                output.status.code(),
                stderr
            )));
        }

        tracing::info!(
            command = %exec.command,
            exit_code = ?output.status.code(),
            "Command executed successfully"
        );

        Ok(())
    }
}
