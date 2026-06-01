use crate::domain::error::Result;
use crate::domain::resource::PackageResource;
use crate::infrastructure::adapter::package::PackageProvider;
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::process::Command;

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(default)]
pub struct YumParams {
    pub update_cache: bool,
}

impl Default for YumParams {
    fn default() -> Self {
        Self { update_cache: true }
    }
}

pub struct YumProvider {
    update_performed: AtomicBool,
}

impl Default for YumProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl YumProvider {
    pub fn new() -> Self {
        Self {
            update_performed: AtomicBool::new(false),
        }
    }

    async fn perform_update(&self) -> Result<()> {
        if !self.update_performed.load(Ordering::SeqCst) {
            tracing::info!("Executing yum makecache before package installation");
            let status = Command::new("yum")
                .arg("makecache")
                .status()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to execute 'yum makecache': {}", e))?;

            if !status.success() {
                return Err(anyhow::anyhow!(
                    "'yum makecache' failed with status: {:?}",
                    status.code()
                ));
            }
            self.update_performed.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

#[async_trait]
impl PackageProvider for YumProvider {
    fn name(&self) -> &str {
        "yum"
    }

    async fn is_installed(&self, resource: &PackageResource) -> Result<bool> {
        let package_name = &resource.name;
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

    async fn install(&self, resource: &PackageResource) -> Result<()> {
        let package_name = &resource.name;

        let yum_params: YumParams = match &resource.params {
            Some(val) => serde_json::from_value(val.clone())
                .map_err(|e| anyhow::anyhow!("Failed to parse YumParams: {}", e))?,
            None => YumParams::default(),
        };

        if yum_params.update_cache {
            self.perform_update().await?;
        }

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

    async fn uninstall(&self, resource: &PackageResource) -> Result<()> {
        let package_name = &resource.name;
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
