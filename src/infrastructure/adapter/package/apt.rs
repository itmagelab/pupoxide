use crate::domain::error::Result;
use crate::domain::resource::PackageResource;
use crate::infrastructure::adapter::package::PackageProvider;
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::process::Command;

#[derive(serde::Deserialize, Debug, Clone)]
pub struct AptParams {
    #[serde(default = "default_true")]
    pub update_cache: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AptParams {
    fn default() -> Self {
        Self { update_cache: true }
    }
}

pub struct AptProvider {
    update_performed: AtomicBool,
}

impl Default for AptProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AptProvider {
    pub fn new() -> Self {
        Self {
            update_performed: AtomicBool::new(false),
        }
    }

    async fn perform_update(&self) -> Result<()> {
        if !self.update_performed.load(Ordering::SeqCst) {
            tracing::info!("Executing apt-get update before package installation");
            let status = Command::new("apt-get")
                .arg("update")
                .status()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to execute 'apt-get update': {}", e))?;

            if !status.success() {
                return Err(anyhow::anyhow!(
                    "'apt-get update' failed with status: {:?}",
                    status.code()
                ));
            }
            self.update_performed.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

#[async_trait]
impl PackageProvider for AptProvider {
    fn name(&self) -> &str {
        "apt"
    }

    async fn is_installed(&self, resource: &PackageResource) -> Result<bool> {
        let package_name = &resource.name;
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

    async fn install(&self, resource: &PackageResource) -> Result<()> {
        let package_name = &resource.name;

        let apt_params: AptParams = match &resource.params {
            Some(val) => serde_json::from_value(val.clone())
                .map_err(|e| anyhow::anyhow!("Failed to parse AptParams: {}", e))?,
            None => AptParams::default(),
        };

        if apt_params.update_cache {
            self.perform_update().await?;
        }

        tracing::info!(package = %package_name, "Executing apt-get install");

        let status = Command::new("apt-get")
            .env("DEBIAN_FRONTEND", "noninteractive")
            .arg("install")
            .arg("-y")
            .arg(package_name)
            .status()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to execute 'apt-get install {}': {}",
                    package_name,
                    e
                )
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

    async fn uninstall(&self, resource: &PackageResource) -> Result<()> {
        let package_name = &resource.name;
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
