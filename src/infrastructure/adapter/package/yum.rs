use crate::domain::error::Result;
use crate::infrastructure::adapter::package::PackageProvider;
use async_trait::async_trait;
use tokio::process::Command;

pub struct YumProvider;

#[async_trait]
impl PackageProvider for YumProvider {
    fn name(&self) -> &str {
        "yum"
    }

    async fn is_installed(&self, package_name: &str) -> Result<bool> {
        tracing::debug!(package = %package_name, "Checking if yum package is installed");

        let status = Command::new("rpm")
            .arg("-q")
            .arg(package_name)
            .stderr(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .status()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    anyhow::anyhow!("rpm command not found on this system. Fail fast.")
                } else {
                    anyhow::anyhow!("Failed to check yum package '{}': {}", package_name, e)
                }
            })?;

        Ok(status.success())
    }

    async fn install(&self, package_name: &str) -> Result<()> {
        tracing::info!(package = %package_name, "Executing yum install");

        let status = Command::new("yum")
            .arg("install")
            .arg("-y")
            .arg(package_name)
            .status()
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to execute 'yum install {}': {}", package_name, e)
            })?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "'yum install {}' failed with status: {:?}",
                package_name,
                status.code()
            ));
        }
        Ok(())
    }

    async fn uninstall(&self, package_name: &str) -> Result<()> {
        tracing::info!(package = %package_name, "Executing yum remove");

        let status = Command::new("yum")
            .arg("remove")
            .arg("-y")
            .arg(package_name)
            .status()
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to execute 'yum remove {}': {}", package_name, e)
            })?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "'yum remove {}' failed with status: {:?}",
                package_name,
                status.code()
            ));
        }
        Ok(())
    }
}
