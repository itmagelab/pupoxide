use crate::domain::bootstrap::{BootstrapRequest, BootstrapRequestMetadata, RegisteredAgent};
use anyhow::{Result, anyhow};
use chrono::Utc;
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, info};

/// File-based bootstrap request manager
/// Stores CSR requests in /etc/pupoxide/bootstrap_requests/
pub struct BootstrapRequestManager {
    requests_dir: PathBuf,
}

impl BootstrapRequestManager {
    pub fn new(requests_dir: PathBuf) -> Self {
        Self { requests_dir }
    }

    /// Create a new bootstrap request from CSR
    pub async fn create_request(
        &self,
        node_id: &str,
        csr: String,
        certificate: Option<String>,
    ) -> Result<BootstrapRequest> {
        // Create directory if not exists
        fs::create_dir_all(&self.requests_dir)
            .await
            .map_err(|e| anyhow!("Failed to create requests directory: {}", e))?;

        let request = BootstrapRequest {
            node_id: node_id.to_string(),
            csr,
            requested_at: Utc::now().timestamp(),
            status: "pending".to_string(),
            certificate,
        };

        // Save request to file
        let request_path = self.requests_dir.join(format!("{}.json", node_id));
        let request_json = serde_json::to_string_pretty(&request)?;
        fs::write(&request_path, request_json)
            .await
            .map_err(|e| anyhow!("Failed to write request file: {}", e))?;

        info!(node_id = node_id, "Bootstrap request created");
        Ok(request)
    }

    /// Get a bootstrap request by node_id
    pub async fn get_request(&self, node_id: &str) -> Result<BootstrapRequest> {
        let request_path = self.requests_dir.join(format!("{}.json", node_id));

        let content = fs::read_to_string(&request_path)
            .await
            .map_err(|_| anyhow!("Request {} not found", node_id))?;

        let request: BootstrapRequest = serde_json::from_str(&content)?;
        Ok(request)
    }

    /// List all pending requests
    pub async fn list_pending_requests(&self) -> Result<Vec<BootstrapRequestMetadata>> {
        if !self.requests_dir.exists() {
            return Ok(Vec::new());
        }

        let mut requests = Vec::new();
        let mut entries = fs::read_dir(&self.requests_dir)
            .await
            .map_err(|e| anyhow!("Failed to read requests directory: {}", e))?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| anyhow!("{}", e))? {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json")
                && let Ok(content) = fs::read_to_string(&path).await
                && let Ok(req) = serde_json::from_str::<BootstrapRequest>(&content)
                && req.is_pending()
            {
                requests.push(BootstrapRequestMetadata {
                    node_id: req.node_id,
                    status: req.status,
                    requested_at: req.requested_at,
                });
            }
        }

        Ok(requests)
    }

    /// Approve a bootstrap request
    pub async fn approve_request(&self, node_id: &str) -> Result<BootstrapRequest> {
        let request_path = self.requests_dir.join(format!("{}.json", node_id));

        let content = fs::read_to_string(&request_path)
            .await
            .map_err(|_| anyhow!("Request {} not found", node_id))?;

        let mut request: BootstrapRequest = serde_json::from_str(&content)?;

        if !request.is_pending() {
            return Err(anyhow!(
                "Request {} is not pending (status: {})",
                node_id,
                request.status
            ));
        }

        request.approve();
        let request_json = serde_json::to_string_pretty(&request)?;
        fs::write(&request_path, request_json)
            .await
            .map_err(|e| anyhow!("Failed to update request file: {}", e))?;

        info!(node_id = node_id, "Bootstrap request approved");
        Ok(request)
    }

    /// Reject a bootstrap request
    pub async fn reject_request(&self, node_id: &str) -> Result<()> {
        let request_path = self.requests_dir.join(format!("{}.json", node_id));

        let content = fs::read_to_string(&request_path)
            .await
            .map_err(|_| anyhow!("Request {} not found", node_id))?;

        let mut request: BootstrapRequest = serde_json::from_str(&content)?;
        request.reject();

        let request_json = serde_json::to_string_pretty(&request)?;
        fs::write(&request_path, request_json)
            .await
            .map_err(|e| anyhow!("Failed to update request file: {}", e))?;

        info!(node_id = node_id, "Bootstrap request rejected");
        Ok(())
    }
}

/// File-based agent registry
pub struct AgentRegistryFs {
    agents_dir: PathBuf,
}

impl AgentRegistryFs {
    pub fn new(agents_dir: PathBuf) -> Self {
        Self { agents_dir }
    }

    /// Register (save) an agent certificate
    pub async fn register(
        &self,
        node_id: &str,
        cert_cn: &str,
        certificate_pem: String,
    ) -> Result<()> {
        fs::create_dir_all(&self.agents_dir)
            .await
            .map_err(|e| anyhow!("Failed to create agents directory: {}", e))?;

        let agent = RegisteredAgent {
            node_id: node_id.to_string(),
            cert_cn: cert_cn.to_string(),
            certificate_pem,
            approved_at: Utc::now().timestamp(),
            last_seen: None,
            is_active: true,
        };

        // Save certificate
        let cert_path = self.agents_dir.join(format!("{}.pem", node_id));
        fs::write(&cert_path, &agent.certificate_pem)
            .await
            .map_err(|e| anyhow!("Failed to write certificate: {}", e))?;

        // Save metadata
        let metadata_path = self.agents_dir.join(format!("{}.json", node_id));
        let metadata_json = serde_json::to_string_pretty(&agent)?;
        fs::write(&metadata_path, metadata_json)
            .await
            .map_err(|e| anyhow!("Failed to write metadata: {}", e))?;

        info!(node_id = node_id, "Agent registered and certificate saved");
        Ok(())
    }

    /// Check if agent is registered and active
    pub async fn is_registered(&self, node_id: &str) -> Result<bool> {
        let metadata_path = self.agents_dir.join(format!("{}.json", node_id));

        match fs::read_to_string(&metadata_path).await {
            Ok(content) => {
                if let Ok(agent) = serde_json::from_str::<RegisteredAgent>(&content) {
                    Ok(agent.is_active)
                } else {
                    Ok(false)
                }
            }
            Err(_) => Ok(false),
        }
    }

    /// Get agent info
    pub async fn get_agent(&self, node_id: &str) -> Result<RegisteredAgent> {
        let metadata_path = self.agents_dir.join(format!("{}.json", node_id));

        let content = fs::read_to_string(&metadata_path)
            .await
            .map_err(|_| anyhow!("Agent {} not found", node_id))?;

        serde_json::from_str::<RegisteredAgent>(&content)
            .map_err(|e| anyhow!("Failed to parse agent metadata: {}", e))
    }

    /// Update last_seen timestamp
    pub async fn update_last_seen(&self, node_id: &str) -> Result<()> {
        let metadata_path = self.agents_dir.join(format!("{}.json", node_id));

        let content = fs::read_to_string(&metadata_path)
            .await
            .map_err(|_| anyhow!("Agent {} not found", node_id))?;

        let mut agent = serde_json::from_str::<RegisteredAgent>(&content)?;
        agent.last_seen = Some(Utc::now().timestamp());

        let metadata_json = serde_json::to_string_pretty(&agent)?;
        fs::write(&metadata_path, metadata_json)
            .await
            .map_err(|e| anyhow!("Failed to update metadata: {}", e))?;

        debug!(node_id = node_id, "Updated last_seen");
        Ok(())
    }

    /// Revoke an agent
    pub async fn revoke(&self, node_id: &str) -> Result<()> {
        let metadata_path = self.agents_dir.join(format!("{}.json", node_id));

        let content = fs::read_to_string(&metadata_path)
            .await
            .map_err(|_| anyhow!("Agent {} not found", node_id))?;

        let mut agent = serde_json::from_str::<RegisteredAgent>(&content)?;
        agent.is_active = false;

        let metadata_json = serde_json::to_string_pretty(&agent)?;
        fs::write(&metadata_path, metadata_json)
            .await
            .map_err(|e| anyhow!("Failed to update metadata: {}", e))?;

        info!(node_id = node_id, "Agent revoked");
        Ok(())
    }

    /// List all registered agents
    pub async fn list_agents(&self) -> Result<Vec<RegisteredAgent>> {
        if !self.agents_dir.exists() {
            return Ok(Vec::new());
        }

        let mut agents = Vec::new();
        let mut entries = fs::read_dir(&self.agents_dir)
            .await
            .map_err(|e| anyhow!("Failed to read agents directory: {}", e))?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| anyhow!("{}", e))? {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                let content = fs::read_to_string(&path)
                    .await
                    .map_err(|e| anyhow!("Failed to read agent metadata file: {}", e))?;
                let agent: RegisteredAgent = serde_json::from_str(&content)
                    .map_err(|e| anyhow!("Failed to parse agent metadata: {}", e))?;
                agents.push(agent);
            }
        }

        Ok(agents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_request() -> Result<()> {
        let temp_dir = tempfile::TempDir::new()?;
        let manager = BootstrapRequestManager::new(temp_dir.path().to_path_buf());

        let req = manager
            .create_request(
                "agent-01",
                "test_csr".to_string(),
                Some("test_cert".to_string()),
            )
            .await
            .expect("Failed to create request");

        assert_eq!(req.node_id, "agent-01");
        assert!(req.is_pending());
        Ok(())
    }

    #[tokio::test]
    async fn test_approve_request() -> Result<()> {
        let temp_dir = tempfile::TempDir::new()?;
        let manager = BootstrapRequestManager::new(temp_dir.path().to_path_buf());

        manager
            .create_request(
                "agent-01",
                "test_csr".to_string(),
                Some("test_cert".to_string()),
            )
            .await
            .expect("Failed to create request");

        let approved = manager
            .approve_request("agent-01")
            .await
            .expect("Failed to approve");

        assert!(approved.is_approved());
        Ok(())
    }

    #[tokio::test]
    async fn test_register_agent() -> Result<()> {
        let temp_dir = tempfile::TempDir::new()?;
        let registry = AgentRegistryFs::new(temp_dir.path().to_path_buf());

        registry
            .register("agent-01", "agent-01", "cert_pem".to_string())
            .await
            .expect("Failed to register");

        let is_registered = registry
            .is_registered("agent-01")
            .await
            .expect("Failed to check");

        assert!(is_registered);
        Ok(())
    }

    #[tokio::test]
    async fn test_list_agents() -> Result<()> {
        let temp_dir = tempfile::TempDir::new()?;
        let registry = AgentRegistryFs::new(temp_dir.path().to_path_buf());

        // Should be empty initially
        let agents = registry.list_agents().await?;
        assert_eq!(agents.len(), 0);

        // Register two agents
        registry
            .register("agent-01", "agent-01", "cert_pem_1".to_string())
            .await?;
        registry
            .register("agent-02", "agent-02", "cert_pem_2".to_string())
            .await?;

        // List and verify
        let agents = registry.list_agents().await?;
        assert_eq!(agents.len(), 2);

        let node_ids: Vec<String> = agents.iter().map(|a| a.node_id.clone()).collect();
        assert!(node_ids.contains(&"agent-01".to_string()));
        assert!(node_ids.contains(&"agent-02".to_string()));

        // Revoke one and check
        registry.revoke("agent-01").await?;
        let agents = registry.list_agents().await?;
        let a1 = agents
            .iter()
            .find(|a| a.node_id == "agent-01")
            .ok_or_else(|| anyhow!("Agent agent-01 not found"))?;
        assert!(!a1.is_active);

        Ok(())
    }
}
