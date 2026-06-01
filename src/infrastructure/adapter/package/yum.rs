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
    pub version: Option<String>,
}

impl Default for YumParams {
    fn default() -> Self {
        Self {
            update_cache: true,
            version: None,
        }
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

        let yum_params: YumParams = match &resource.params {
            Some(val) => serde_json::from_value(val.clone())
                .map_err(|e| anyhow::anyhow!("Failed to parse YumParams: {}", e))?,
            None => YumParams::default(),
        };

        if let Some(req_ver) = &yum_params.version {
            tracing::debug!(package = %package_name, version = %req_ver, "Checking if specific yum package version is installed");
            let output = Command::new("rpm")
                .arg("-q")
                .arg("--queryformat")
                .arg("%{VERSION}\n%{VERSION}-%{RELEASE}")
                .arg(package_name)
                .output()
                .await
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        anyhow::anyhow!("rpm command not found on this system. Fail fast.")
                    } else {
                        anyhow::anyhow!(
                            "Failed to check yum package '{}' version: {}",
                            package_name,
                            e
                        )
                    }
                })?;

            if !output.status.success() {
                return Ok(false);
            }

            let stdout_str = String::from_utf8_lossy(&output.stdout);
            let required = req_ver.trim();
            let is_match = stdout_str.lines().any(|line| line.trim() == required);
            Ok(is_match)
        } else {
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

        let package_arg = if let Some(ver) = &yum_params.version {
            tracing::info!(package = %package_name, version = %ver, "Executing yum install for specific version");
            format!("{}-{}", package_name, ver)
        } else {
            tracing::info!(package = %package_name, "Executing yum install");
            package_name.clone()
        };

        let status = Command::new("yum")
            .arg("install")
            .arg("-y")
            .arg(&package_arg)
            .status()
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to execute 'yum install {}': {}", package_arg, e)
            })?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "'yum install {}' failed with status: {:?}",
                package_arg,
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
