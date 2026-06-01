use crate::domain::error::Result;
use crate::domain::resource::PackageResource;
use crate::infrastructure::adapter::package::PackageProvider;
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::process::Command;

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(default)]
pub struct AptParams {
    pub update_cache: bool,
    pub version: Option<String>,
}

impl Default for AptParams {
    fn default() -> Self {
        Self {
            update_cache: true,
            version: None,
        }
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

        let apt_params: AptParams = match &resource.params {
            Some(val) => serde_json::from_value(val.clone())
                .map_err(|e| anyhow::anyhow!("Failed to parse AptParams: {}", e))?,
            None => AptParams::default(),
        };

        if let Some(req_ver) = &apt_params.version {
            tracing::debug!(package = %package_name, version = %req_ver, "Checking if specific apt package version is installed");
            let output = Command::new("dpkg-query")
                .arg("-W")
                .arg("-f=${Version}")
                .arg(package_name)
                .output()
                .await
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        anyhow::anyhow!("dpkg-query command not found on this system. Fail fast.")
                    } else {
                        anyhow::anyhow!(
                            "Failed to check apt package '{}' version: {}",
                            package_name,
                            e
                        )
                    }
                })?;

            if !output.status.success() {
                return Ok(false);
            }

            let installed_ver = String::from_utf8_lossy(&output.stdout);
            let installed = installed_ver.trim();
            let required = req_ver.trim();
            let is_match = if installed == required {
                true
            } else if let Some((_epoch, rest)) = installed.split_once(':') {
                rest == required
            } else {
                false
            };

            Ok(is_match)
        } else {
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

        let package_arg = if let Some(ver) = &apt_params.version {
            tracing::info!(package = %package_name, version = %ver, "Executing apt-get install for specific version");
            format!("{}={}", package_name, ver)
        } else {
            tracing::info!(package = %package_name, "Executing apt-get install");
            package_name.clone()
        };

        let status = Command::new("apt-get")
            .env("DEBIAN_FRONTEND", "noninteractive")
            .arg("install")
            .arg("-y")
            .arg(&package_arg)
            .status()
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to execute 'apt-get install {}': {}", package_arg, e)
            })?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "'apt-get install {}' failed with status: {:?}",
                package_arg,
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
