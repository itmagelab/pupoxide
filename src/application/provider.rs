use crate::domain::error::Result;
use crate::domain::resource::{Resource, ResourceProvider, ResourceState};
use async_trait::async_trait;
use std::sync::Arc;

pub struct ProviderRegistry {
    providers: Vec<Arc<dyn ResourceProvider>>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Arc<dyn ResourceProvider>) {
        self.providers.push(provider);
    }

    fn find_provider(&self, resource: &Resource) -> Result<&Arc<dyn ResourceProvider>> {
        self.providers
            .iter()
            .find(|p| p.can_handle(resource))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No provider found for resource: {}",
                    resource.id()
                )
            })
    }
}

#[async_trait]
impl ResourceProvider for ProviderRegistry {
    fn can_handle(&self, resource: &Resource) -> bool {
        self.providers.iter().any(|p| p.can_handle(resource))
    }

    async fn get_state(&self, resource: &Resource, full: bool) -> Result<ResourceState> {
        let provider = self.find_provider(resource)?;
        provider.get_state(resource, full).await
    }

    async fn apply(&self, resource: &Resource) -> Result<()> {
        let provider = self.find_provider(resource)?;
        provider.apply(resource).await
    }
}
