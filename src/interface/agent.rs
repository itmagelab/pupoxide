use crate::domain::catalog::Catalog;
use crate::infrastructure::facter::Facter;
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

    pub async fn run(&self, dry_run: bool) -> anyhow::Result<()> {
        tracing::info!("Agent starting for node {} in environment {}", self.node_name, self.environment);

        // 1. Collect facts
        let facts = Facter::collect();
        tracing::info!("Collected {} facts", facts.values.len());

        // 2. Fetch catalog
        let url = format!("{}/catalog/{}/{}", self.server_url, self.environment, self.node_name);
        let client = reqwest::Client::new();
        let catalog: Catalog = client.post(url)
            .json(&facts)
            .send()
            .await?
            .json()
            .await
            .context("Failed to parse catalog from server")?;

        tracing::info!("Received catalog with {} resources", catalog.resources.len());

        // 3. Apply changes with rollback support
        let state_dir = std::path::PathBuf::from("/tmp/pupoxide");
        let backup_store = crate::infrastructure::BackupStore::new(state_dir.join("backups"));
        let state_store = crate::infrastructure::StateStore::new(state_dir.join("state"));

        crate::application::execute_transaction(catalog, &backup_store, &state_store, dry_run).await?;

        tracing::info!("Catalog application finished successfully");
        Ok(())
    }
}
