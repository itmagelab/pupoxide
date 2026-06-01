use crate::domain::error::Result;
use crate::domain::resource::PackageResource;
use crate::infrastructure::adapter::package::PackageProvider;
use async_trait::async_trait;
use tokio::process::Command;

#[derive(serde::Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct BrewParams {
    pub version: Option<String>,
}

pub struct BrewProvider;

#[async_trait]
impl PackageProvider for BrewProvider {
    fn name(&self) -> &str {
        "brew"
    }

    async fn is_installed(&self, resource: &PackageResource) -> Result<bool> {
        let package_name = &resource.name;

        let brew_params: BrewParams = match &resource.params {
            Some(val) => serde_json::from_value(val.clone())
                .map_err(|e| anyhow::anyhow!("Failed to parse BrewParams: {}", e))?,
            None => BrewParams::default(),
        };

        if let Some(req_ver) = &brew_params.version {
            let output = Command::new("brew")
                .arg("list")
                .arg("--versions")
                .arg(package_name)
                .output()
                .await
                .map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to check brew package '{}' versions: {}",
                        package_name,
                        e
                    )
                })?;

            if !output.status.success() {
                return Ok(false);
            }

            let output_str = String::from_utf8_lossy(&output.stdout);
            let required = req_ver.trim();
            let is_match = output_str
                .split_whitespace()
                .skip(1)
                .any(|v| v.trim() == required);

            Ok(is_match)
        } else {
            // brew list formula_name returns 0 if installed, non-zero otherwise
            let status = Command::new("brew")
                .arg("list")
                .arg("--formula")
                .arg(package_name)
                .stderr(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .status()
                .await
                .map_err(|e| {
                    anyhow::anyhow!("Failed to check brew package '{}': {}", package_name, e)
                })?;

            Ok(status.success())
        }
    }

    async fn install(&self, resource: &PackageResource) -> Result<()> {
        let package_name = &resource.name;
        let status = Command::new("brew")
            .arg("install")
            .arg(package_name)
            .status()
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to execute 'brew install {}': {}", package_name, e)
            })?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "'brew install {}' failed with status: {:?}",
                package_name,
                status.code()
            ));
        }
        Ok(())
    }

    async fn uninstall(&self, resource: &PackageResource) -> Result<()> {
        let package_name = &resource.name;
        let status = Command::new("brew")
            .arg("uninstall")
            .arg(package_name)
            .status()
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to execute 'brew uninstall {}': {}", package_name, e)
            })?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "'brew uninstall {}' failed with status: {:?}",
                package_name,
                status.code()
            ));
        }
        Ok(())
    }
}
