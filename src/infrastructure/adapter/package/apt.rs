use crate::domain::error::Result;
use crate::infrastructure::adapter::package::PackageProvider;
use async_trait::async_trait;
use tokio::process::Command;

pub struct AptProvider;

#[async_trait]
impl PackageProvider for AptProvider {
    fn name(&self) -> &str {
        "apt"
    }

    async fn is_installed(&self, package_name: &str) -> Result<bool> {
        tracing::debug!(package = %package_name, "Checking if apt package is installed");
        
        let status = Command::new("dpkg")
            .arg("-s")
            .arg(package_name)
            .stderr(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .status()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    anyhow::anyhow!("dpkg command not found on this system. Fail fast.")
                } else {
                    anyhow::anyhow!("Failed to check apt package '{}': {}", package_name, e)
                }
            })?;

        Ok(status.success())
    }

    async fn install(&self, package_name: &str) -> Result<()> {
        tracing::info!(package = %package_name, "Executing apt-get install");

        let status = Command::new("apt-get")
            .env("DEBIAN_FRONTEND", "noninteractive")
            .arg("install")
            .arg("-y")
            .arg(package_name)
            .status()
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to execute 'apt-get install {}': {}", package_name, e)
            })?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "'apt-get install {}' failed with status: {:?}",
                package_name,
                status.code()
            ));
        }
        Ok(())
    }

    async fn uninstall(&self, package_name: &str) -> Result<()> {
        tracing::info!(package = %package_name, "Executing apt-get remove");

        let status = Command::new("apt-get")
            .arg("remove")
            .arg("-y")
            .arg(package_name)
            .status()
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to execute 'apt-get remove {}': {}", package_name, e)
            })?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "'apt-get remove {}' failed with status: {:?}",
                package_name,
                status.code()
            ));
        }
        Ok(())
    }
}
