use crate::domain::bootstrap::{BootstrapRequest, BootstrapResponse};
use crate::domain::catalog::Catalog;
use crate::infrastructure::certificate::AgentCertificateRequest;
use crate::infrastructure::facter::Facter;
use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::time::Duration;
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
        let cert_dir = cert_dir
            .unwrap_or_else(|| PathBuf::from(format!("/etc/pupoxide/agents/{}", node_name)));

        Self {
            server_url,
            node_name,
            environment,
            cert_dir,
        }
    }

    /// Phase 1: Bootstrap - Submit CSR request to master (no token needed)
    pub async fn bootstrap(&self) -> Result<()> {
        info!(
            node_name = %self.node_name,
            "Starting bootstrap process - submitting CSR request"
        );

        // Create cert directory if not exists
        tokio::fs::create_dir_all(&self.cert_dir)
            .await
            .context("Failed to create certificate directory")?;

        // 1. Generate CSR and private key
        let (csr_req, private_key_pem, self_signed_cert) =
            AgentCertificateRequest::generate(&self.node_name).context("Failed to generate CSR")?;

        debug!(node_name = %self.node_name, "CSR generated");

        // Save the self-signed cert and private key (these are guaranteed to match)
        let key_path = self.cert_dir.join("agent.key");
        let self_signed_path = self.cert_dir.join("agent-self-signed.pem");

        tokio::fs::write(&key_path, &private_key_pem)
            .await
            .context("Failed to write agent private key")?;

        tokio::fs::write(&self_signed_path, &self_signed_cert)
            .await
            .context("Failed to write agent self-signed certificate")?;

        // Set restrictive permissions on private key
        #[cfg(unix)]
        {
            use std::fs::Permissions;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&key_path, Permissions::from_mode(0o600))
                .context("Failed to set key permissions")?;
        }

        debug!(node_name = %self.node_name, "Self-signed cert and key saved");

        // 2. Send CSR to Master (no token required)
        let bootstrap_request = BootstrapRequest {
            node_id: self.node_name.clone(),
            csr: csr_req.csr_pem,
            requested_at: chrono::Utc::now().timestamp(),
            status: "pending".to_string(),
            certificate: Some(self_signed_cert),
        };

        let client = reqwest::Client::new();
        let bootstrap_url = format!("{}/bootstrap", self.server_url);

        let response = client
            .post(&bootstrap_url)
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
                "Bootstrap request failed with status {}: {}",
                status,
                error_text
            ));
        }

        let bootstrap_response: BootstrapResponse = response
            .json()
            .await
            .context("Failed to parse bootstrap response")?;

        info!(
            node_name = %self.node_name,
            "Bootstrap request submitted. Status: {}. Message: {}",
            bootstrap_response.status,
            bootstrap_response.message
        );

        println!("\n✓ Bootstrap request submitted!");
        println!("  Node ID: {}", self.node_name);
        println!("  Status: {}", bootstrap_response.status);
        println!("  Message: {}", bootstrap_response.message);
        println!("\n→ Admin must approve request before agent can run.");
        println!(
            "  Check status with: pupoxide agent --server {} --node {} --environment {} --bootstrap --check",
            self.server_url, self.node_name, self.environment
        );

        Ok(())
    }

    /// Check bootstrap status - polls until approved
    pub async fn check_bootstrap_status(&self, timeout_secs: u64) -> Result<()> {
        info!(
            node_name = %self.node_name,
            "Checking bootstrap approval status"
        );

        let client = reqwest::Client::new();
        let check_url = format!("{}/bootstrap/check", self.server_url);
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);

        loop {
            let response = client
                .post(&check_url)
                .json(&serde_json::json!({ "node_id": self.node_name }))
                .send()
                .await
                .context("Failed to check bootstrap status")?;

            let bootstrap_response: BootstrapResponse = response
                .json()
                .await
                .context("Failed to parse bootstrap check response")?;

            match bootstrap_response.status.as_str() {
                "pending" => {
                    if start.elapsed() > timeout {
                        return Err(anyhow!("Bootstrap approval timeout ({}s)", timeout_secs));
                    }
                    info!("Still pending approval... waiting 5 seconds");
                    println!("⏳ Request still pending... waiting");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                "approved" => {
                    // Save certificate
                    let cert_pem = bootstrap_response
                        .certificate
                        .ok_or_else(|| anyhow!("No certificate in approval response"))?;
                    let ca_pem = bootstrap_response
                        .ca_certificate
                        .ok_or_else(|| anyhow!("No CA certificate in approval response"))?;

                    let cert_path = self.cert_dir.join("agent.pem");
                    let ca_path = self.cert_dir.join("ca.pem");

                    tokio::fs::write(&cert_path, &cert_pem)
                        .await
                        .context("Failed to write agent certificate")?;

                    tokio::fs::write(&ca_path, &ca_pem)
                        .await
                        .context("Failed to write CA certificate")?;

                    info!(
                        node_name = %self.node_name,
                        cert_path = ?cert_path,
                        "Bootstrap approved! Certificate saved."
                    );

                    println!("\n✓ Bootstrap approved!");
                    println!("  Certificate saved to: {:?}", cert_path);
                    println!("\n→ You can now run the agent:");
                    println!(
                        "  pupoxide agent --server {} --node {} --environment {}",
                        self.server_url, self.node_name, self.environment
                    );

                    return Ok(());
                }
                "rejected" => {
                    return Err(anyhow!("Bootstrap request was rejected by admin"));
                }
                _ => {
                    return Err(anyhow!(
                        "Unknown bootstrap status: {}",
                        bootstrap_response.status
                    ));
                }
            }
        }
    }

    /// Phase 2: Regular operation - Fetch catalog using mTLS
    pub async fn run(&self, dry_run: bool, show_unchanged: bool) -> Result<()> {
        // Acquire exclusive lock - only one instance per agent can run at a time
        let _lock = self.acquire_lock(300).await?; // 5 minute timeout

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
                "Agent certificates not found. Run bootstrap first and wait for approval: \
                 pupoxide agent --server {} --node {} --environment {} --bootstrap --check",
                self.server_url,
                self.node_name,
                self.environment
            ));
        }

        // 1. Collect facts
        let facts = Facter::collect();
        info!(fact_count = facts.values.len(), "Collected facts");

        // 2. Fetch catalog using mTLS
        let catalog = self
            .fetch_catalog(&cert_path, &key_path, &ca_path, facts)
            .await
            .context("Failed to fetch catalog")?;

        info!(resource_count = catalog.resources.len(), "Received catalog");

        // 3. Apply changes with rollback support
        let state_dir = std::path::PathBuf::from("/tmp/pupoxide");
        let state_store = crate::infrastructure::StateStore::new(state_dir.join("state"));

        // Initialize provider registry with default adapters
        let mut provider_registry = crate::application::ProviderRegistry::new();
        provider_registry.register(std::sync::Arc::new(crate::infrastructure::FsAdapter));
        provider_registry.register(std::sync::Arc::new(crate::infrastructure::ExecAdapter));
        let provider = std::sync::Arc::new(provider_registry);

        crate::interface::formatter::PrettyFormatter::print_header();
        let _reports = crate::application::execute_transaction(
            catalog,
            &state_store,
            provider,
            dry_run,
            |report| {
                if show_unchanged
                    || report.status != crate::domain::report::ResourceStatus::Unchanged
                {
                    println!(
                        "{}",
                        crate::interface::formatter::PrettyFormatter::format_line(report)
                    );
                }
            },
        )
        .await?;

        Ok(())
    }

    /// Fetch catalog from master using mTLS
    async fn fetch_catalog(
        &self,
        cert_path: &Path,
        key_path: &Path,
        _ca_path: &Path,
        facts: crate::domain::facts::Facts,
    ) -> Result<Catalog> {
        // For mTLS, we use the self-signed certificate that matches the private key
        // The server-signed certificate is stored separately for audit/verification
        let self_signed_cert_path = self.cert_dir.join("agent-self-signed.pem");

        // Load self-signed certificate and private key (these are guaranteed to match)
        let cert_pem = match tokio::fs::read_to_string(&self_signed_cert_path).await {
            Ok(content) => content,
            Err(_) => {
                // Fallback to agent.pem if self-signed doesn't exist
                tokio::fs::read_to_string(cert_path)
                    .await
                    .context("Failed to read agent certificate")?
            }
        };

        let key_pem = tokio::fs::read_to_string(key_path)
            .await
            .context("Failed to read agent private key")?;

        // Create client identity from cert and key
        // Combine certificate and key with proper newline separation for PEM format
        let combined_pem = format!("{}\n{}", cert_pem.trim_end(), key_pem.trim_end());
        let identity = reqwest::Identity::from_pem(combined_pem.as_bytes())
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

    /// Acquire an exclusive lock for this agent
    /// Waits up to timeout_secs for the lock to become available
    pub async fn acquire_lock(&self, timeout_secs: u64) -> Result<AgentLock> {
        // Create lock file in the agent's cert directory to ensure it exists
        let lock_file = self.cert_dir.join(format!("{}.lock", self.node_name));

        // Ensure the cert directory exists
        tokio::fs::create_dir_all(&self.cert_dir)
            .await
            .context("Failed to create agent cert directory")?;

        let timeout = Duration::from_secs(timeout_secs);
        let start = std::time::Instant::now();

        loop {
            // Try to create the lock file exclusively (fail if it already exists)
            match tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_file)
                .await
            {
                Ok(_) => {
                    info!(node_name = %self.node_name, "Acquired agent lock");
                    return Ok(AgentLock {
                        lock_path: lock_file,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if start.elapsed() > timeout {
                        return Err(anyhow!(
                            "Failed to acquire lock for agent {} (timeout after {} seconds)",
                            self.node_name,
                            timeout_secs
                        ));
                    }
                    // Wait a bit before trying again
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    return Err(anyhow!("Failed to create lock file: {}", e));
                }
            }
        }
    }
}

/// Guard for exclusive agent lock - automatically releases lock when dropped
pub struct AgentLock {
    lock_path: PathBuf,
}

impl AgentLock {
    /// Manually release the lock before the guard is dropped
    pub async fn release(&self) -> Result<()> {
        tokio::fs::remove_file(&self.lock_path)
            .await
            .context("Failed to remove lock file")?;
        info!(path = ?self.lock_path, "Released agent lock");
        Ok(())
    }
}

impl Drop for AgentLock {
    fn drop(&mut self) {
        // Try to remove lock file on drop, but don't panic if it fails
        if self.lock_path.exists() {
            let _ = std::fs::remove_file(&self.lock_path);
        }
    }
}
