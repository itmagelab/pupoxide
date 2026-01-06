use crate::domain::catalog::Catalog;
use crate::domain::resource::ResourceProvider;
use crate::infrastructure::fs_adapter::FsAdapter;
use anyhow::Context;

pub struct PupoxideAgent {
    pub server_url: String,
    pub node_name: String,
    pub environment: String,
}

impl PupoxideAgent {
    pub fn new(server_url: String, node_name: String, environment: String) -> Self {
        Self {
            server_url,
            node_name,
            environment,
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        tracing::info!("Agent starting for node {} in environment {}", self.node_name, self.environment);

        // 1. Fetch catalog
        let url = format!("{}/catalog/{}/{}", self.server_url, self.environment, self.node_name);
        let catalog: Catalog = reqwest::get(url)
            .await?
            .json()
            .await
            .context("Failed to parse catalog from server")?;

        tracing::info!("Received catalog with {} resources", catalog.resources.len());

        // 2. Apply resources
        let adapter = FsAdapter;
        for resource in catalog.resources {
            tracing::info!("Applying resource: {}", resource.id());
            adapter.apply(&resource).await.context(format!("Failed to apply resource {}", resource.id()))?;
        }

        tracing::info!("Catalog application finished successfully");
        Ok(())
    }
}
