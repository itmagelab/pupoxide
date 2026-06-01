use crate::domain::error::Result;
use crate::domain::resource::{Ensure, Resource, ResourceProvider, ResourceState};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

pub mod brew;
pub mod apt;
pub mod yum;

#[async_trait]
pub trait PackageProvider: Send + Sync {
    /// Returns the provider name (e.g., "brew", "apt")
    fn name(&self) -> &str;

    /// Returns true if the package is currently installed
    async fn is_installed(&self, package_name: &str) -> Result<bool>;

    /// Installs the package
    async fn install(&self, package_name: &str) -> Result<()>;

    /// Uninstalls the package
    async fn uninstall(&self, package_name: &str) -> Result<()>;
}

pub struct PackageAdapter {
    providers: HashMap<String, Arc<dyn PackageProvider>>,
}

impl Default for PackageAdapter {
    fn default() -> Self {
        Self::new()
            .with_provider(Arc::new(brew::BrewProvider))
            .with_provider(Arc::new(apt::AptProvider))
            .with_provider(Arc::new(yum::YumProvider))
    }
}

impl PackageAdapter {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn with_provider(mut self, provider: Arc<dyn PackageProvider>) -> Self {
        self.providers.insert(provider.name().to_string(), provider);
        self
    }

    async fn get_provider(&self, name: &str) -> Result<&Arc<dyn PackageProvider>> {
        self.providers
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Package provider '{}' not found", name))
    }
}

#[async_trait]
impl ResourceProvider for PackageAdapter {
    fn can_handle(&self, resource: &Resource) -> bool {
        matches!(resource, Resource::Package(_))
    }

    async fn get_state(&self, resource: &Resource, _full: bool) -> Result<ResourceState> {
        match resource {
            Resource::Package(pkg) => {
                let provider = self.get_provider(&pkg.provider).await?;
                let is_installed = provider.is_installed(&pkg.name).await?;
                let ensure = if is_installed {
                    Ensure::Present
                } else {
                    Ensure::Absent
                };
                Ok(ResourceState::Ensure(ensure))
            }
            _ => Ok(ResourceState::Ensure(Ensure::Present)),
        }
    }

    async fn apply(&self, resource: &Resource) -> Result<()> {
        match resource {
            Resource::Package(pkg) => {
                let provider = self.get_provider(&pkg.provider).await?;
                let is_installed = provider.is_installed(&pkg.name).await?;

                match (&pkg.ensure, is_installed) {
                    (Ensure::Present, false) => {
                        tracing::info!(package = %pkg.name, provider = %pkg.provider, "Installing package");
                        provider.install(&pkg.name).await
                    }
                    (Ensure::Absent, true) => {
                        tracing::info!(package = %pkg.name, provider = %pkg.provider, "Uninstalling package");
                        provider.uninstall(&pkg.name).await
                    }
                    _ => {
                        tracing::debug!(package = %pkg.name, ensure = ?pkg.ensure, "Package already in desired state");
                        Ok(())
                    }
                }
            }
            _ => Ok(()),
        }
    }
}
