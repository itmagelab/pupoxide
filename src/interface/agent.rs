use crate::domain::bootstrap::{BootstrapRequest, BootstrapResponse};
use crate::domain::catalog::Catalog;
use crate::infrastructure::facter::Facter;
use crate::infrastructure::certificate::AgentCertificateRequest;
use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

pub struct PupoxideAgent {
    pub server_url: String,
    pub node_name: String,
    pub environment: String,
    pub cert_dir: PathBuf,
}

impl PupoxideAgent {
    pub fn new(
        server_url: String,
        node_name: String,
        environment: String,
        cert_dir: Option<PathBuf>,
    ) -> Self {
        let cert_dir = cert_dir.unwrap_or_else(|| {
            PathBuf::from(format!("/etc/pupoxide/agents/{}", node_name))
        });

        Self {
            server_url,
            node_name,
            environment,
            cert_dir,
        }
    }

    /// Phase 1: Bootstrap - Register agent with master using bootstrap token
    pub async fn bootstrap(&self, bootstrap_token: String) -> Result<()> {
        info!(
            node_name = %self.node_name,
            "Starting bootstrap process"
        );

        // Create cert directory if not exists
        tokio::fs::create_dir_all(&self.cert_dir)
            .await
            .context("Failed to create certificate directory")?;

        // 1. Generate CSR and private key
        let (csr_req, private_key_pem) = AgentCertificateRequest::generate(&self.node_name)
            .context("Failed to generate CSR")?;

        debug!(node_name = %self.node_name, "CSR generated");

        // 2. Send CSR to Master with bootstrap token
        let bootstrap_request = BootstrapRequest {
            node_id: self.node_name.clone(),
            csr: csr_req.csr_pem,
        };

        let client = reqwest::Client::new();
        let bootstrap_url = format!("{}/bootstrap", self.server_url);

        let response = client
            .post(&bootstrap_url)
            .header("Authorization", format!("Bearer {}", bootstrap_token))
            .json(&bootstrap_request)
            .send()
            .await
            .context("Failed to send bootstrap request to master")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!(
                "Bootstrap failed with status {}: {}",
                status,
                error_text
            ));
        }

        let bootstrap_response: BootstrapResponse = response
            .json()
            .await
            .context("Failed to parse bootstrap response")?;

        // 3. Save signed certificate and private key
        let cert_path = self.cert_dir.join("agent.pem");
        let key_path = self.cert_dir.join("agent.key");
        let ca_path = self.cert_dir.join("ca.pem");

        tokio::fs::write(&cert_path, &bootstrap_response.certificate)
            .await
            .context("Failed to write agent certificate")?;

        tokio::fs::write(&key_path, &private_key_pem)
            .await
            .context("Failed to write agent private key")?;

        tokio::fs::write(&ca_path, &bootstrap_response.ca_certificate)
            .await
            .context("Failed to write CA certificate")?;

        // Set restrictive permissions on private key
        #[cfg(unix)]
        {
            use std::fs::Permissions;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&key_path, Permissions::from_mode(0o600))
                .context("Failed to set key permissions")?;
        }

        info!(
            node_name = %self.node_name,
            cert_path = ?cert_path,
            "Agent successfully registered. Certificate saved."
        );

        Ok(())
    }

    /// Phase 2: Regular operation - Fetch catalog using mTLS
    pub async fn run(&self, dry_run: bool) -> Result<()> {
        info!(
            node_name = %self.node_name,
            environment = %self.environment,
            "Agent starting for node in environment"
        );

        // Check if certificates exist
        let cert_path = self.cert_dir.join("agent.pem");
        let key_path = self.cert_dir.join("agent.key");
        let ca_path = self.cert_dir.join("ca.pem");

        if !cert_path.exists() || !key_path.exists() {
            return Err(anyhow!(
                "Agent certificates not found. Run bootstrap first: \
                 pupoxide agent bootstrap --server {} --node {} --token <token>",
                self.server_url,
                self.node_name
            ));
        }

        // 1. Collect facts
        let facts = Facter::collect();
        info!(
            fact_count = facts.values.len(),
            "Collected facts"
        );

        // 2. Fetch catalog using mTLS
        let catalog = self.fetch_catalog(&cert_path, &key_path, &ca_path, facts)
            .await
            .context("Failed to fetch catalog")?;

        info!(
            resource_count = catalog.resources.len(),
            "Received catalog"
        );

        // 3. Apply changes with rollback support
        let state_dir = std::path::PathBuf::from("/tmp/pupoxide");
        let state_store = crate::infrastructure::StateStore::new(state_dir.join("state"));

        // Initialize provider registry with default adapters
        let mut provider_registry = crate::application::ProviderRegistry::new();
        provider_registry.register(std::sync::Arc::new(crate::infrastructure::FsAdapter));
        provider_registry.register(std::sync::Arc::new(crate::infrastructure::ExecAdapter));
        let provider = std::sync::Arc::new(provider_registry);

        crate::application::execute_transaction(
            catalog,
            &state_store,
            provider,
            dry_run,
        )
        .await?;

        info!("Catalog application finished successfully");
        Ok(())
    }

    /// Fetch catalog from master using mTLS
    async fn fetch_catalog(
        &self,
        cert_path: &Path,
        key_path: &Path,
        ca_path: &Path,
        facts: crate::domain::facts::Facts,
    ) -> Result<Catalog> {
        // Load certificates and private key
        let cert_pem = tokio::fs::read(cert_path)
            .await
            .context("Failed to read agent certificate")?;

        let key_pem = tokio::fs::read(key_path)
            .await
            .context("Failed to read agent private key")?;

        let _ca_pem = tokio::fs::read(ca_path)
            .await
            .context("Failed to read CA certificate")?;

        // Create client identity from cert and key
        let identity = reqwest::Identity::from_pem(
            &format!("{}{}", String::from_utf8(cert_pem)?, String::from_utf8(key_pem)?).into_bytes(),
        )
        .context("Failed to create client identity from certificate and key")?;

        // Create HTTP client with mTLS
        let client = reqwest::Client::builder()
            .identity(identity)
            .build()
            .context("Failed to build HTTP client with mTLS")?;

        let url = format!(
            "{}/catalog/{}/{}",
            self.server_url, self.environment, self.node_name
        );

        debug!(url = %url, "Fetching catalog");

        let response = client
            .post(&url)
            .json(&facts)
            .send()
            .await
            .context("Failed to send catalog request to master")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!(
                "Catalog request failed with status {}: {}",
                status,
                error_text
            ));
        }

        response
            .json()
            .await
            .context("Failed to parse catalog from server")
    }
}
